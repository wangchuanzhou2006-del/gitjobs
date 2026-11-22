# AI Tender Document Analysis Integration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a production-shaped first-stage tender workflow to the existing GitJobs application: an authenticated user uploads a PDF, the Rust application owns the file and durable task, a stateless FastAPI service parses and analyzes it, and the user views a versioned report whose key claims cite the original page text.

**Architecture:** Keep `foundation-validation` as the only product, repository, account system, PostgreSQL database, UI, and audit source. Migrate only reviewed Python capabilities from `C:\Users\25351\ai-bid-assistant` into `services/tender-analysis`; this internal service has no frontend, login, user table, Redis, Celery, or business database. The Rust worker calls separate internal parse and analyze endpoints so durable status can advance from `queued` to `parsing` to `analyzing`, then validates and stores `tender-analysis.v1` results.

**Tech Stack:** Rust 1.96+, Axum 0.8, Askama 0.16, PostgreSQL 16, Tokio workers, reqwest 0.13, FastAPI, Pydantic 2, PyMuPDF, LiteLLM, pytest, pgTAP, Mockall, and Playwright.

## Global Constraints

- Work only inside `C:\Users\25351\gitjobs-candidate\.worktrees\foundation-validation`; `C:\Users\25351\ai-bid-assistant` is read-only reference material and must not be edited or deleted.
- Do not create a second product, account system, user database, React frontend, admin area, Redis queue, Celery worker, or persistent Chroma database.
- The first release implements PDF upload, parsing, analysis, status, retry, report display, citations, and unknown/conflict reporting only.
- Enterprise matching, S/A/B/C scoring, proposal generation, reminders, multi-Agent orchestration, and public ecosystem APIs are out of scope.
- The Rust application is the only durable data owner; the Python service must not persist files, users, projects, jobs, or final reports after a request ends.
- Maximum PDF size is 50 MiB, maximum page count is 1,000, maximum extracted UTF-8 payload is 10 MiB, and only files with a valid PDF signature and parseable PDF structure are accepted.
- Internal service requests require a redacted bearer secret, use a 300-second request timeout, and accept at most 64 MiB request/response bodies.
- Contract version is exactly `tender-analysis.v1`; prompt versions are `tender-project-info-v1`, `tender-qualifications-v1`, `tender-risks-v1`, `tender-materials-v1`, and `tender-review-v1`.
- Every key project fact, qualification, risk fact, and required material must include at least one citation with 1-based page number, exact source text, and stable chunk ID; otherwise it belongs in `unknowns`.
- AI output may never invent missing values, silently choose between conflicting clauses, or classify a scoring bonus as a hard qualification.
- Tests and fixtures use synthetic or licensed sample data only; never commit a user's tender document, API key, enterprise certificate, or complete sensitive business record.
- Every task follows red-green-refactor, runs focused tests, commits only its own files, and stops for Codex review before the next Claude task starts.

## Locked File Structure

```text
contracts/
  tender-analysis.v1.schema.json       # canonical public result contract
  fixtures/tender-analysis.v1.json     # synthetic cross-language fixture
services/tender-analysis/
  pyproject.toml                        # minimal internal-service dependencies
  Dockerfile
  app/
    __init__.py
    main.py                             # FastAPI composition only
    auth.py                             # internal bearer authentication
    config.py                           # fail-closed environment settings
    contracts.py                        # Pydantic request/result types
    pdf.py                              # page-preserving PDF parser
    retrieval.py                        # deterministic hybrid chunk selection
    prompts.py                          # five versioned prompt constants
    llm.py                              # provider protocol + LiteLLM adapter
    analysis.py                         # four extraction stages + final review
  tests/
    test_auth.py
    test_contracts.py
    test_pdf.py
    test_retrieval.py
    test_analysis.py
database/migrations/schema/0021_tender_analysis.sql
database/migrations/functions/tender/*.sql
database/tests/functions/tender/*.sql
gitjobs-server/src/tender/
  mod.rs                                # public module surface and service wiring
  model.rs                              # contract and durable workflow types
  storage.rs                            # private local PDF storage abstraction
  client.rs                             # internal FastAPI client abstraction
  processor.rs                          # one-job orchestration
gitjobs-server/src/db/tender.rs         # typed PostgreSQL repository boundary
gitjobs-server/src/handlers/platform/tenders.rs
gitjobs-server/src/templates/platform/tenders.rs
gitjobs-server/templates/platform/tenders/
  list.html
  report.html
  components/status.html
tests/e2e/public/tender-analysis.spec.ts
docs/operations/tender-analysis.md
```

---

### Task 1: Establish the versioned contract and stateless internal service shell

**Files:**
- Create: `contracts/tender-analysis.v1.schema.json`
- Create: `contracts/fixtures/tender-analysis.v1.json`
- Create: `services/tender-analysis/pyproject.toml`
- Create: `services/tender-analysis/app/__init__.py`
- Create: `services/tender-analysis/app/config.py`
- Create: `services/tender-analysis/app/auth.py`
- Create: `services/tender-analysis/app/contracts.py`
- Create: `services/tender-analysis/app/main.py`
- Create: `services/tender-analysis/tests/test_auth.py`
- Create: `services/tender-analysis/tests/test_contracts.py`

**Interfaces:**
- Produces: `TenderAnalysisV1`, `ParsedDocument`, `ParseResponse`, `AnalyzeRequest`, `Citation`, `UnknownItem`, and exact enum values used by both services.
- Produces: `GET /internal/health`, authenticated `POST /internal/v1/parse`, and authenticated `POST /internal/v1/analyze`; Task 1 endpoints return `501` after authentication until Tasks 2 and 3 supply implementations.

- [ ] **Step 1: Write failing authentication and contract tests**

Create tests that reject a missing/wrong bearer token, redact the token from settings `repr`, validate the synthetic fixture, reject an unknown top-level field, reject confidence outside `0..1`, and reject page number `0`:

```python
def test_fixture_matches_v1_contract() -> None:
    payload = json.loads(FIXTURE.read_text(encoding="utf-8"))
    report = TenderAnalysisV1.model_validate(payload)
    assert report.schema_version == "tender-analysis.v1"
    assert report.project_info.project_name.citations[0].page_number == 1


def test_unknown_fields_fail_closed() -> None:
    payload = json.loads(FIXTURE.read_text(encoding="utf-8"))
    payload["surprise"] = True
    with pytest.raises(ValidationError):
        TenderAnalysisV1.model_validate(payload)
```

- [ ] **Step 2: Run tests and confirm the red state**

Run: `cd services/tender-analysis && python -m pytest tests/test_auth.py tests/test_contracts.py -q`

Expected: FAIL because the service package and contract types do not exist.

- [ ] **Step 3: Define strict Pydantic models and the canonical fixture**

Every model uses `ConfigDict(extra="forbid")`. The central types are:

```python
class Citation(BaseModel):
    model_config = ConfigDict(extra="forbid")
    page_number: int = Field(ge=1, le=1000)
    chunk_id: str = Field(pattern=r"^page-[1-9][0-9]*-chunk-[0-9]+$")
    quote: str = Field(min_length=1, max_length=2000)


class EvidenceValue(BaseModel):
    model_config = ConfigDict(extra="forbid")
    value: str | None
    confidence: float = Field(ge=0, le=1)
    requires_review: bool
    citations: list[Citation] = Field(max_length=20)


class TenderAnalysisV1(BaseModel):
    model_config = ConfigDict(extra="forbid")
    schema_version: Literal["tender-analysis.v1"]
    project_info: ProjectInfo
    qualifications: list[Qualification] = Field(max_length=200)
    risks: list[Risk] = Field(max_length=200)
    required_materials: list[RequiredMaterial] = Field(max_length=300)
    unknowns: list[UnknownItem] = Field(max_length=200)
    analysis_meta: AnalysisMeta
```

`Qualification.kind` is one of `hard_qualification`, `scoring_item`, `submission_material`, `contract_performance`, `general_note`. `Risk.severity` is `low`, `medium`, or `high`. `UnknownItem.reason` is `missing`, `conflict`, `low_confidence`, or `unsupported`. The JSON Schema and fixture must encode the same names and enum values.

- [ ] **Step 4: Add fail-closed settings, bearer authentication, and shell routes**

`Settings` requires `internal_service_token` with minimum length 32 and renders it as `**********`. Authentication uses constant-time comparison:

```python
async def require_internal_token(
    credentials: Annotated[HTTPAuthorizationCredentials | None, Depends(HTTPBearer(auto_error=False))],
    settings: Annotated[Settings, Depends(get_settings)],
) -> None:
    supplied = credentials.credentials if credentials and credentials.scheme.lower() == "bearer" else ""
    if not secrets.compare_digest(supplied, settings.internal_service_token.get_secret_value()):
        raise HTTPException(status_code=401, detail="invalid internal credentials")
```

Do not add CORS. Do not import SQLAlchemy, Redis, Celery, authentication users, or any module from the old project's frontend/business layer.

- [ ] **Step 5: Run focused verification and commit**

Run: `cd services/tender-analysis && python -m pytest tests/test_auth.py tests/test_contracts.py -q`

Expected: PASS.

Run: `cd services/tender-analysis && python -m ruff check app tests`

Expected: PASS.

```bash
git add contracts services/tender-analysis
git commit -s -m "feat: add tender analysis service contract"
```

### Task 2: Parse PDFs without losing page evidence

**Files:**
- Create: `services/tender-analysis/app/pdf.py`
- Create: `services/tender-analysis/tests/test_pdf.py`
- Modify: `services/tender-analysis/app/main.py`
- Modify: `services/tender-analysis/app/contracts.py`

**Interfaces:**
- Consumes: authenticated `/internal/v1/parse` shell from Task 1.
- Produces: `parse_pdf(data: bytes) -> ParsedDocument` where each `ParsedPage` has 1-based `page_number`, normalized `text`, and page-local chunks with stable IDs.
- Produces typed error codes: `invalid_pdf`, `encrypted_pdf`, `too_many_pages`, `extracted_text_too_large`, and `ocr_required`.

- [ ] **Step 1: Write failing parser tests with generated synthetic PDFs**

Generate PDFs inside tests with PyMuPDF; do not commit real tender files. Assert page boundaries and citations survive:

```python
parsed = parse_pdf(make_pdf(["第一页：预算100万元", "第二页：截止日期2026-09-01"]))
assert parsed.page_count == 2
assert parsed.pages[0].page_number == 1
assert parsed.pages[0].chunks[0].chunk_id == "page-1-chunk-0"
assert "预算100万元" in parsed.pages[0].text
assert "截止日期" in parsed.pages[1].text
```

Also test `%PDF-` spoofing, encrypted input, 1,001 pages, blank scanned-style pages, NUL/control removal, and the 10 MiB extracted-text limit.

- [ ] **Step 2: Run parser tests and confirm failure**

Run: `cd services/tender-analysis && python -m pytest tests/test_pdf.py -q`

Expected: FAIL because `parse_pdf` does not exist.

- [ ] **Step 3: Implement bounded page-preserving parsing**

Open bytes through `fitz.open(stream=data, filetype="pdf")`, reject encrypted documents, iterate no more than 1,000 pages, normalize Unicode to NFC, remove unsafe controls while preserving newlines, and split within each page:

```python
def chunk_page(page_number: int, text: str, size: int = 1600, overlap: int = 200) -> list[DocumentChunk]:
    chunks: list[DocumentChunk] = []
    start = 0
    while start < len(text):
        end = min(len(text), start + size)
        chunks.append(DocumentChunk(
            chunk_id=f"page-{page_number}-chunk-{len(chunks)}",
            page_number=page_number,
            text=text[start:end],
        ))
        if end == len(text):
            break
        start = end - overlap
    return chunks
```

Never return the old sentinel string `[OCR_FAILED]`. If fewer than 10% of pages contain usable text, return `ocr_required`; OCR itself is not silently attempted in this task.

- [ ] **Step 4: Connect and test the parse endpoint**

The endpoint reads at most 50 MiB plus one byte, verifies filename and MIME only as hints, calls `parse_pdf`, and maps typed parser failures to status `422` with a safe code. It must not write the file to disk.

Run: `cd services/tender-analysis && python -m pytest tests/test_pdf.py tests/test_auth.py -q`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add services/tender-analysis/app services/tender-analysis/tests
git commit -s -m "feat: preserve tender PDF page evidence"
```

### Task 3: Implement retrieval, versioned prompts, and structured analysis

**Files:**
- Create: `services/tender-analysis/app/retrieval.py`
- Create: `services/tender-analysis/app/prompts.py`
- Create: `services/tender-analysis/app/llm.py`
- Create: `services/tender-analysis/app/analysis.py`
- Create: `services/tender-analysis/tests/test_retrieval.py`
- Create: `services/tender-analysis/tests/test_analysis.py`
- Modify: `services/tender-analysis/app/main.py`
- Modify: `services/tender-analysis/app/contracts.py`
- Modify: `services/tender-analysis/app/config.py`

**Interfaces:**
- Consumes: `ParsedDocument` and `TenderAnalysisV1` from Tasks 1–2.
- Produces: protocol `LlmProvider.complete_structured(system_prompt, user_payload, response_model)`.
- Produces: `analyze_document(document, provider, model_name) -> TenderAnalysisV1`.

- [ ] **Step 1: Write failing deterministic retrieval and orchestration tests**

Use a fake provider that records prompt versions and returns strict models. Tests must prove all pages can be selected, qualifications and scoring items stay distinct, conflicts enter `unknowns`, citation quotes must occur in their cited chunk, a missing citation demotes the claim to `unknowns`, and model text never becomes an exception/log payload.

```python
report = analyze_document(parsed_document(), FakeProvider(), "fixture-model")
assert report.schema_version == "tender-analysis.v1"
assert [call.prompt_version for call in provider.calls] == [
    "tender-project-info-v1",
    "tender-qualifications-v1",
    "tender-risks-v1",
    "tender-materials-v1",
    "tender-review-v1",
]
```

- [ ] **Step 2: Run tests and confirm failure**

Run: `cd services/tender-analysis && python -m pytest tests/test_retrieval.py tests/test_analysis.py -q`

Expected: FAIL because retrieval and analysis modules do not exist.

- [ ] **Step 3: Implement deterministic hybrid retrieval**

Rank chunks by normalized keyword hits, heading/number patterns, negation terms, and adjacency; use separate query packs for project facts, qualifications, risks, and materials. Always include first two pages, table-of-contents hits, and one neighboring chunk around each high-scoring hit. Deduplicate by `chunk_id`, cap each stage at 60 chunks, and preserve document order in the final context. Do not truncate the document to its first 18,000 characters and do not require Chroma or external embeddings in v1.

- [ ] **Step 4: Implement five immutable prompt constants**

Every system prompt starts with its exact version and includes these non-negotiable instructions:

```text
只陈述输入片段明确支持的事实。禁止使用常识补全。
每个关键结论必须引用 chunk_id、1-based 页码和逐字原文。
资格准入、评分加分、提交材料、合同履约和普通说明必须分开。
信息缺失、互相冲突或置信度不足时写入 unknowns，禁止猜测。
事实依据、风险判断、影响和建议必须分别输出。
```

The final review prompt receives the four typed intermediate results and only removes/demotes unsupported claims; it may not invent new facts.

- [ ] **Step 5: Implement the provider and evidence validator**

LiteLLM is behind a protocol so tests never call a network. Temperature is `0`, timeout is 90 seconds per stage, response models are strict Pydantic classes, and provider errors expose only safe codes. Validate each citation against the exact parsed chunk and demote unsupported claims to `unknowns` before returning.

- [ ] **Step 6: Connect the analyze endpoint and verify**

`POST /internal/v1/analyze` accepts `AnalyzeRequest { job_id, document }`, runs the five stages, and returns only `TenderAnalysisV1`. It does not accept or return a user ID and does not write a database.

Run: `cd services/tender-analysis && python -m pytest -q`

Expected: PASS.

Run: `cd services/tender-analysis && python -m ruff check app tests`

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add services/tender-analysis/app services/tender-analysis/tests
git commit -s -m "feat: analyze tenders with cited evidence"
```

### Task 4: Add durable tender projects, documents, jobs, and reports

**Files:**
- Create: `database/migrations/schema/0021_tender_analysis.sql`
- Create: `database/migrations/functions/tender/create_tender_project.sql`
- Create: `database/migrations/functions/tender/create_tender_document_job.sql`
- Create: `database/migrations/functions/tender/claim_tender_analysis_job.sql`
- Create: `database/migrations/functions/tender/transition_tender_analysis_job.sql`
- Create: `database/migrations/functions/tender/complete_tender_analysis_job.sql`
- Create: `database/migrations/functions/tender/fail_tender_analysis_job.sql`
- Create: `database/migrations/functions/tender/get_tender_analysis_job.sql`
- Create: `database/migrations/functions/tender/list_tender_projects.sql`
- Create: `database/migrations/functions/tender/get_tender_report.sql`
- Create: `database/tests/functions/tender/tender_analysis.sql`
- Modify: `database/migrations/functions/001_load_functions.sql`
- Modify: `database/tests/schema/schema.sql`
- Create: `gitjobs-server/src/db/tender.rs`
- Modify: `gitjobs-server/src/db/mod.rs`
- Modify: `gitjobs-server/src/db/mock.rs`

**Interfaces:**
- Produces tables `tender_projects`, `tender_documents`, `tender_analysis_jobs`, and `tender_analysis_reports`.
- Produces trait `DBTender` with typed create, claim, transition, complete, fail, list, status, and report methods.

- [ ] **Step 1: Write failing pgTAP schema and workflow tests**

Tests assert user ownership, `sha256` length, 50 MiB limit, unique idempotency key, exact job states, atomic `FOR UPDATE SKIP LOCKED` claiming, stale claim rejection, valid transition order, report version `tender-analysis.v1`, and report JSON object type. A second identical create returns the existing active job instead of creating a chargeable duplicate.

- [ ] **Step 2: Run the database tests and confirm failure**

Run: `just db-tests-file database/tests/functions/tender/tender_analysis.sql`

Expected: FAIL because `tender_projects` does not exist.

- [ ] **Step 3: Add reversible DDL with explicit invariants**

Use UUID primary keys and these states:

```sql
check (status in ('queued', 'parsing', 'analyzing', 'partial', 'completed', 'failed'))
```

`tender_documents` stores `storage_key`, never an absolute path. `tender_analysis_jobs` stores `claim_token`, `claim_version`, `claim_expires_at`, `attempt_count`, `error_code`, safe `error_message`, `model_provider`, `model_name`, `prompt_version`, and `analysis_config_hash`. `tender_analysis_reports.result` is JSONB; add a unique index on `(document_id, analysis_config_hash)` for completed reports.

- [ ] **Step 4: Implement atomic SQL functions**

Claim one queued or retryable job with `FOR UPDATE SKIP LOCKED`, assign a fresh claim token, increment `claim_version`, and expire the claim after ten minutes. Transition and completion functions require matching unexpired token/version. Completion validates top-level JSON fields and inserts the report plus completed status in one transaction. Failure stores only an approved error code/message and clears the claim.

- [ ] **Step 5: Add the typed Rust repository boundary**

Expose:

```rust
#[async_trait]
pub(crate) trait DBTender: Send + Sync {
    async fn create_tender_project(&self, input: NewTenderProject) -> Result<Uuid>;
    async fn create_tender_document_job(&self, input: NewTenderDocumentJob) -> Result<CreateJobOutcome>;
    async fn claim_tender_analysis_job(&self) -> Result<Option<ClaimedTenderJob>>;
    async fn transition_tender_analysis_job(&self, input: TenderJobTransition) -> Result<()>;
    async fn complete_tender_analysis_job(&self, input: CompletedTenderJob) -> Result<Uuid>;
    async fn fail_tender_analysis_job(&self, input: FailedTenderJob) -> Result<()>;
    async fn get_tender_analysis_job(&self, user_id: Uuid, job_id: Uuid) -> Result<Option<TenderJobView>>;
    async fn get_tender_report(&self, user_id: Uuid, report_id: Uuid) -> Result<Option<TenderReportRow>>;
}
```

Add `DBTender` to the existing `DB` supertrait and Mockall implementation.

- [ ] **Step 6: Verify and commit**

Run: `just db-tests-file database/tests/functions/tender/tender_analysis.sql`

Expected: PASS.

Run: `cargo test -p gitjobs-server db::tender`

Expected: PASS.

```bash
git add database gitjobs-server/src/db
git commit -s -m "feat: persist tender analysis workflow"
```

### Task 5: Add Rust contract types, private storage, and the internal client

**Files:**
- Create: `gitjobs-server/src/tender/mod.rs`
- Create: `gitjobs-server/src/tender/model.rs`
- Create: `gitjobs-server/src/tender/storage.rs`
- Create: `gitjobs-server/src/tender/client.rs`
- Modify: `gitjobs-server/src/main.rs`
- Modify: `gitjobs-server/src/config.rs`
- Modify: `gitjobs-server/src/handlers/tests.rs`
- Modify: `config/examples/server.local.yml`

**Interfaces:**
- Produces Rust `TenderAnalysisV1` with `#[serde(deny_unknown_fields)]` matching the shared fixture.
- Produces `TenderDocumentStore::save/open/delete` and `FileSystemTenderDocumentStore`.
- Produces `TenderAnalysisClient::parse/analyze` and `HttpTenderAnalysisClient`.

- [ ] **Step 1: Write failing contract, storage, and client tests**

Deserialize `contracts/fixtures/tender-analysis.v1.json`, reject unknown fields, validate citation quotes, reject path traversal filenames, prove same filename creates random storage keys, enforce 50 MiB streaming limit, and use a local Axum fake service to assert bearer auth, timeouts, request paths, and 64 MiB response cap.

- [ ] **Step 2: Run focused tests and confirm failure**

Run: `cargo test -p gitjobs-server tender::`

Expected: FAIL because module `tender` does not exist.

- [ ] **Step 3: Implement strict Rust contract types and validation**

Use owned strings, `NonZeroU16` or checked page numbers, and exhaustive enums. Provide:

```rust
impl TenderAnalysisV1 {
    pub(crate) fn validate_against(&self, parsed: &ParsedDocument) -> Result<(), ContractError>;
}
```

Validation confirms version, confidence ranges, collection limits, known `chunk_id`, matching page number, and exact quote containment. Invalid service results are never persisted as completed reports.

- [ ] **Step 4: Implement private local file storage**

Configure a canonical `document_dir`; create it with owner-only intent, generate UUID storage keys, stream to a temporary file while hashing SHA-256, verify `%PDF-` and size, fsync, then atomically rename. Store only the key in PostgreSQL. Reject separators, `..`, absolute paths, symlinks, and reads that resolve outside the root. The trait allows a future object-store implementation without changing handlers or workers.

- [ ] **Step 5: Implement the internal HTTP client**

The parse call sends multipart PDF bytes to `/internal/v1/parse`; the analyze call sends `AnalyzeRequest` JSON to `/internal/v1/analyze`. Use a redacted service token, 300-second timeout, bounded response reading, typed retryable/non-retryable errors, and no response-body text in `Display` or tracing.

- [ ] **Step 6: Wire fail-closed configuration**

Add `TenderConfig { enabled, document_dir, analysis_base_url, internal_service_token, max_upload_bytes, request_timeout_seconds }`. When enabled, all required settings must be valid; local plain HTTP is allowed only for loopback with explicit `allow_local_http: true`, matching the existing talent provider safety pattern.

Run: `cargo test -p gitjobs-server tender:: config::`

Expected: PASS.

Run: `cargo clippy -p gitjobs-server --all-targets --all-features -- --deny warnings`

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add gitjobs-server/src/tender gitjobs-server/src/main.rs gitjobs-server/src/config.rs gitjobs-server/src/handlers/tests.rs config/examples/server.local.yml
git commit -s -m "feat: add tender analysis client and storage"
```

### Task 6: Orchestrate durable parsing, analysis, retry, and cleanup

**Files:**
- Create: `gitjobs-server/src/tender/processor.rs`
- Modify: `gitjobs-server/src/tender/mod.rs`
- Modify: `gitjobs-server/src/workers.rs`
- Modify: `gitjobs-server/src/main.rs`
- Modify: `gitjobs-server/src/router.rs`
- Modify: `gitjobs-server/src/handlers/tests.rs`

**Interfaces:**
- Consumes: `DBTender`, `TenderDocumentStore`, and `TenderAnalysisClient`.
- Produces: `TenderServices { processor, storage }` shared by handlers and workers.
- Produces: `TenderProcessor::process_one() -> Result<TenderProcessOutcome, TenderProcessError>`.

- [ ] **Step 1: Write failing processor state-machine tests**

Mock DB, storage, and client. Assert `queued -> parsing -> analyzing -> completed`, parse failure, analysis failure, invalid citation rejection, one retry after retryable transport failure, no retry for `invalid_pdf`/`ocr_required`, stale claim handling, idempotent existing report, and cancellation without leaking a claim or file bytes into logs.

- [ ] **Step 2: Run focused tests and confirm failure**

Run: `cargo test -p gitjobs-server tender::processor workers::`

Expected: FAIL because `TenderProcessor` does not exist.

- [ ] **Step 3: Implement one-job orchestration**

The exact order is claim, open private document, transition to parsing, call parse, transition to analyzing, call analyze, validate result against parsed chunks, atomically complete report. Retryable service errors schedule at most three attempts with bounded backoff; permanent document/contract errors fail immediately. Error persistence uses safe codes such as `analysis_unavailable`, `invalid_pdf`, `ocr_required`, `invalid_analysis_contract`, and `storage_unavailable`.

- [ ] **Step 4: Add bounded cancellable worker**

Follow the existing Talent worker pattern with maximum concurrency two and one-second idle polling. A panic must release its semaphore permit, cancellation aborts active local tasks, and the database claim expiry makes restart safe. Start the worker only when `server.tender.enabled` is true.

- [ ] **Step 5: Build services once and share them**

Construct `TenderServices` in `main.rs`, pass the same `Arc` to `workers::run` and `router::setup`, and add it to router `State`. `TestRouterBuilder` supplies in-memory/mock storage and client; handler tests never start the background poller.

Run: `cargo test -p gitjobs-server tender::processor workers::`

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add gitjobs-server/src/tender gitjobs-server/src/workers.rs gitjobs-server/src/main.rs gitjobs-server/src/router.rs gitjobs-server/src/handlers/tests.rs
git commit -s -m "feat: process tender analysis jobs"
```

### Task 7: Add authenticated upload, status, retry, and cited report pages

**Files:**
- Create: `gitjobs-server/src/handlers/platform/tenders.rs`
- Modify: `gitjobs-server/src/handlers/platform/mod.rs`
- Create: `gitjobs-server/src/templates/platform/tenders.rs`
- Modify: `gitjobs-server/src/templates/platform/mod.rs`
- Create: `gitjobs-server/templates/platform/tenders/list.html`
- Create: `gitjobs-server/templates/platform/tenders/report.html`
- Create: `gitjobs-server/templates/platform/tenders/components/status.html`
- Modify: `gitjobs-server/src/router.rs`
- Modify: `gitjobs-server/templates/navigation_links.html`
- Create: `tests/e2e/public/tender-analysis.spec.ts`

**Interfaces:**
- Produces protected routes `GET /tenders`, `POST /tenders`, `POST /tenders/{project_id}/documents`, `GET /tenders/jobs/{job_id}/status`, `POST /tenders/jobs/{job_id}/retry`, and `GET /tenders/reports/{report_id}`.
- Preserves the existing account session and global page shell; no second login or SPA.

- [ ] **Step 1: Write failing handler tests**

Assert anonymous requests redirect to login; authenticated users can create/list only their own projects; upload accepts a valid synthetic PDF; non-PDF, empty, spoofed, and oversized files fail safely; duplicate upload returns the existing active job; status returns only owned jobs; retry permits failed jobs only; report HTML escapes model strings and renders page citations, unknowns, model/prompt version, and partial status.

- [ ] **Step 2: Run handler tests and confirm failure**

Run: `cargo test -p gitjobs-server handlers::platform::tenders`

Expected: FAIL because tender routes do not exist.

- [ ] **Step 3: Add routes under the existing login middleware**

All write, status, and report routes must be in the protected router segment. Do not trust `user_id` form fields; derive ownership only from `AuthSession`. Keep a simple project title and optional public source URL; connection to future collected opportunity records is nullable and out of scope here.

- [ ] **Step 4: Implement streaming upload and durable job creation**

Validate referer using the existing project helper, sanitize display filename, stream through `TenderDocumentStore`, then call `create_tender_document_job`. If DB creation fails after storage succeeds, delete the orphaned storage key. Return `303` to the job status anchor. Never send the browser directly to the Python service.

- [ ] **Step 5: Implement accessible status and report UI**

The status component uses `aria-live="polite"` and maps states to Chinese labels: 等待处理、正在解析、AI 分析中、部分完成、分析完成、分析失败. Poll only queued/parsing/analyzing jobs and stop on terminal states. Report sections are 项目概况、资格要求、风险提示、材料清单、待人工确认、原文依据. Every cited item exposes a “查看第 N 页原文” disclosure with escaped quote; no claim without evidence is presented as confirmed.

- [ ] **Step 6: Add browser acceptance for the mocked pipeline**

Use synthetic fixtures and a mocked internal service. Test upload, state progression, completed report, failed retry, keyboard operation, no-JavaScript fallback, 360px layout, and absence of `console.error`/`pageerror`.

Run: `cargo test -p gitjobs-server handlers::platform::tenders`

Expected: PASS.

Run: `npx playwright test --config tests/e2e/playwright.config.ts tests/e2e/public/tender-analysis.spec.ts --project=chromium`

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add gitjobs-server/src/handlers/platform gitjobs-server/src/templates/platform gitjobs-server/templates/platform/tenders gitjobs-server/src/router.rs gitjobs-server/templates/navigation_links.html tests/e2e/public/tender-analysis.spec.ts
git commit -s -m "feat: add tender analysis workflow UI"
```

### Task 8: Package, document, and prove the complete boundary

**Files:**
- Create: `services/tender-analysis/Dockerfile`
- Create: `docs/operations/tender-analysis.md`
- Modify: `Dockerfile.local`
- Modify: `config/examples/server.local.yml`
- Modify: `database/tests/data/e2e.sql`
- Modify: `README.md`

**Interfaces:**
- Consumes the complete Rust/Python pipeline.
- Produces local operation, model-provider switching, retry, security, cleanup, and troubleshooting instructions.

- [ ] **Step 1: Add a non-root internal-service container**

Install only the locked Python service dependencies, copy only `services/tender-analysis`, listen on an internal port, run as a non-root user, and expose `/internal/health`. Do not include `C:\Users\25351\ai-bid-assistant`, its `dev.db`, uploads, Chroma data, frontend, or `node_modules`.

- [ ] **Step 2: Write the operator runbook**

Document invalid example secrets, internal token rotation, GPT/Claude/DeepSeek configuration through LiteLLM, private network rules, 50 MiB/1,000-page limits, `ocr_required`, safe retry, prompt/contract versioning, temporary-data behavior, model data-sharing review, and how to disable tender analysis without disabling the main site.

- [ ] **Step 3: Add a synthetic end-to-end smoke path**

Seed a fictional authenticated user and project. Generate a two-page PDF during the test, use a deterministic fake LLM provider, run through upload/parse/analyze/report, and assert budget/deadline citations refer to the correct page. Assert a claim with an invented quote is rejected and never stored as completed.

- [ ] **Step 4: Run the complete verification matrix**

Run: `cd services/tender-analysis && python -m pytest -q`

Expected: PASS.

Run: `cd services/tender-analysis && python -m ruff check app tests`

Expected: PASS.

Run: `cargo fmt --all -- --check`

Expected: PASS.

Run: `cargo test --workspace`

Expected: PASS with zero failed tests.

Run: `cargo clippy --workspace --all-targets --all-features -- --deny warnings`

Expected: PASS with zero warnings.

Run: `just db-tests`

Expected: PASS.

Run: `npx playwright test --config tests/e2e/playwright.config.ts tests/e2e/public/tender-analysis.spec.ts --project=chromium --project=firefox --project=webkit`

Expected: PASS with no console or page errors.

- [ ] **Step 5: Scan for forbidden duplicate-product artifacts and secrets**

Run:

```bash
rg -n "CORSMiddleware|/api/v1/auth|sqlalchemy|celery|redis|dev\.db|change-me-in-production|internal_service_token\s*=\s*['\"][^$]" services/tender-analysis gitjobs-server config docs/operations
```

Expected: no second authentication/frontend/database/queue implementation and no usable secret; dependency names may appear only in an explicitly explanatory migration note if needed.

- [ ] **Step 6: Commit**

```bash
git add services/tender-analysis/Dockerfile docs/operations/tender-analysis.md Dockerfile.local config/examples/server.local.yml database/tests/data/e2e.sql README.md
git commit -s -m "docs: operate tender analysis safely"
```

## Claude Execution Protocol

Claude receives one task at a time. Before editing, it must read this plan, the approved design at `docs/superpowers/specs/2026-08-10-ai-tender-document-analysis-integration-design.md`, `CLAUDE.md`, and every existing file listed under that task. Claude must show the intended file list before implementation, follow the red-green steps, report exact commands and results, and stop after the task commit. Codex reviews the actual diff, contract adherence, and test evidence before authorizing the next task.

## Completion Gate

Do not call phase one complete until a synthetic end-to-end PDF produces a stored report with valid page citations, unsupported claims are rejected or demoted to unknowns, failure/retry behavior is visible in the same website, the Python service has no durable business data or public entry point, all verification commands pass, and Codex has reviewed every task diff.
