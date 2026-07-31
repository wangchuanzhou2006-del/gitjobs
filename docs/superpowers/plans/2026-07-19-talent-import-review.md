# Talent Import, AI Review, and Controlled Publishing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a Moderator-only pipeline that imports pasted restaurant recruitment and job-seeking text, structures it with rules plus a configurable OpenAI-compatible model, risk-checks it, conditionally auto-publishes low-risk entries, and serves only published masked records on `/talent`.

**Architecture:** Keep the feature inside the existing Rust/Axum/PostgreSQL monolith. A focused `talent` domain module owns splitting, encryption, AI extraction, duplicate analysis, risk decisions, processing, and publication orchestration; PostgreSQL functions own atomic claiming, optimistic review updates, publication, and cleanup. Existing Moderator middleware protects import and review routes, while the public query returns a deliberately restricted DTO that cannot contain ciphertext or complete contacts.

**Tech Stack:** Rust 1.96.1, Axum 0.8, Askama 0.16, PostgreSQL with pgcrypto/pg_trgm, reqwest 0.13, OpenSSL AES-256-GCM and HMAC-SHA256, Tokio workers, pgTAP, Mockall, and Playwright.

## Global Constraints

- First release accepts pasted text only and handles `hiring`, `seeking`, `irrelevant`, and `uncertain`; it does not log in to WeChat/QQ and does not crawl the web.
- Reuse the existing `user_is_moderator` middleware for every `/dashboard/moderator/talent/*` route.
- Ordinary users can access only published masked public records; only Moderator users can decrypt imports or change workflow state; AI workers have no interactive account and can act only through the constrained processing service.
- Maximum import size is 200,000 UTF-8 bytes, maximum 500 fragments per batch, and maximum 8,000 UTF-8 bytes per fragment.
- AI timeout is 20 seconds, one retry is allowed, and default worker concurrency is 4.
- AI provider is configured with OpenAI-compatible `base_url`, `model`, and API key; secrets come from environment-backed server configuration and render as `[REDACTED]` in `Debug`.
- Raw batch text, raw fragments, and complete contacts use application-layer AES-256-GCM; contact dedupe uses HMAC-SHA256 with a separate pepper.
- Raw batch and fragment ciphertext is deleted after 90 days; published structured records, irreversible hashes, source summaries, and review events remain.
- Public DTOs, HTML, JavaScript JSON, logs, and errors never contain raw text, ciphertext, API keys, encryption keys, or complete phone/WeChat values.
- Risk score is 0–100; defaults are low 0–29, medium 30–69, and high 70–100. A hard block always produces high risk.
- Auto-publish requires the global switch, a confirmed trusted source with its source switch enabled, `confidence >= 0.90`, all type-specific required fields, low risk, no hard block, and no high-similarity duplicate.
- Hiring auto-publish requires role, city or district, salary minimum and maximum, at least one contact, source, and published time. Seeking requires expected role, expected city or district, at least one contact, source, and published time; expected salary may be absent.
- New, watchlist, and blocked sources cannot auto-publish. Medium/high risk and high-similarity duplicates always require Moderator action.
- Formal environments set `prototype_data: false`; prototype fixtures are enabled only by explicit local/test configuration and are always labeled.
- Tests and documentation use fictional data only; never copy a real group member name, phone number, WeChat ID, or business identity.
- Each task follows red-green-refactor, runs its focused checks, and commits only its own files.

---

### Task 1: Add the persistence schema and invariants

**Files:**
- Create: `database/migrations/schema/0017_talent_import_review.sql`
- Modify: `database/tests/schema/schema.sql`

**Interfaces:**
- Produces: tables `talent_sources`, `talent_import_batches`, `talent_import_items`, `talent_records`, `talent_contacts`, `talent_review_events`, and singleton `talent_runtime_settings`.
- Produces: database checks and unique indexes used by every later repository method.

- [ ] **Step 1: Write failing pgTAP schema assertions**

Add assertions for all seven tables, their exact column order, primary keys, foreign keys, the singleton settings check, the unique `(source_type, lower(name))` source index, the unique `talent_import_items(batch_id, position)` index, and the unique non-null `published_record_id` index. The central columns must be:

```sql
select columns_are('talent_import_items', array[
    'item_id', 'batch_id', 'position', 'raw_fragment_ciphertext',
    'encryption_key_version', 'contact_candidates_ciphertext',
    'contact_hashes', 'contact_masked', 'normalized_fingerprint',
    'kind', 'status', 'extraction', 'extraction_confidence',
    'validation_errors', 'risk_score', 'risk_level', 'risk_reasons',
    'duplicate_candidates', 'published_record_id', 'lock_version',
    'created_at', 'processed_at', 'reviewed_at', 'updated_at'
]);
```

Increase `select plan(...)` by the exact number of new assertions added.

- [ ] **Step 2: Run the schema test to verify it fails**

Run: `just db-tests-file database/tests/schema/schema.sql`

Expected: FAIL with `relation "talent_sources" does not exist`.

- [ ] **Step 3: Create the migration with reversible DDL**

Create all seven tables with text `check` constraints for the approved enum values. Use nullable ciphertext columns so cleanup can erase protected payloads without deleting workflow rows. The key invariants must include:

```sql
create unique index talent_source_type_name_idx
on talent_sources (source_type, lower(name));

create unique index talent_import_item_batch_position_idx
on talent_import_items (batch_id, position);

create unique index talent_import_item_published_record_idx
on talent_import_items (published_record_id)
where published_record_id is not null;

create table talent_runtime_settings (
    singleton boolean primary key default true check (singleton),
    auto_publish_enabled boolean not null default false,
    updated_by uuid references "user" (user_id) on delete set null,
    updated_at timestamptz not null default current_timestamp
);

insert into talent_runtime_settings (singleton) values (true);
```

`talent_review_events.before_snapshot` and `after_snapshot` are JSONB and must have comments stating that complete contacts are forbidden. The down migration drops the seven tables in reverse foreign-key order.

- [ ] **Step 4: Run schema tests and migration load verification**

Run: `just db-tests-file database/tests/schema/schema.sql`

Expected: PASS with the updated assertion count.

Run: `cd database/migrations && TERN_CONF="$HOME/.config/gitjobs/tern-tests.conf" ./migrate.sh`

Expected: exit 0.

- [ ] **Step 5: Commit**

```bash
git add database/migrations/schema/0017_talent_import_review.sql database/tests/schema/schema.sql
git commit -s -m "feat: add talent import persistence schema"
```

### Task 2: Define the talent domain and deterministic splitter

**Files:**
- Create: `gitjobs-server/src/talent/mod.rs`
- Create: `gitjobs-server/src/talent/model.rs`
- Create: `gitjobs-server/src/talent/splitter.rs`
- Modify: `gitjobs-server/src/main.rs`

**Interfaces:**
- Produces: `TalentKind`, `ImportItemStatus`, `RiskLevel`, `SourceType`, `SourceTrust`, `TalentExtraction`, `ContactCandidate`, `RiskReason`, `DuplicateCandidate`, and `TextFragment`.
- Produces: `split_messages(raw: &str, limits: SplitLimits) -> Result<Vec<TextFragment>, SplitError>`.
- Produces: `ImportItemStatus::can_transition_to(self, next) -> bool`.

- [ ] **Step 1: Write failing model and splitter tests**

Tests cover Windows/Unix line endings, timestamps plus nicknames, blank separators, continuation lines, repeated quoted messages, unrelated text, stable byte offsets, 500-fragment rejection, and 8,000-byte fragment rejection. Use fictional input such as:

```rust
const SAMPLE: &str = "测试用户甲 10:01:02\n招聘切配，测试市测试区，月薪6000至7000\n电话：13800000001\n\n测试用户乙 10:03:00\n求职炒锅，期望测试市\n微信：test_candidate_01";

let fragments = split_messages(SAMPLE, SplitLimits::default()).unwrap();
assert_eq!(fragments.len(), 2);
assert_eq!(fragments[0].position, 0);
assert!(fragments[0].text.contains("招聘切配"));
assert!(fragments[1].text.contains("求职炒锅"));
```

State tests must assert `Queued -> Extracting`, `Extracting -> NeedsReview`, `NeedsReview -> Published`, `NeedsReview -> Rejected`, `Rejected -> Extracting`, and reject `Published -> Extracting`.

- [ ] **Step 2: Run focused tests to verify they fail**

Run: `cargo test -p gitjobs-server talent::`

Expected: FAIL because module `talent` does not exist.

- [ ] **Step 3: Implement focused domain types and state transitions**

Use owned strings and serde camelCase only at browser boundaries. The extraction contract is:

```rust
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct TalentExtraction {
    pub kind: TalentKind,
    pub confidence: f32,
    pub title: Option<String>,
    pub scene: Option<String>,
    pub region: Option<String>,
    pub city: Option<String>,
    pub district: Option<String>,
    pub salary_min: Option<u32>,
    pub salary_max: Option<u32>,
    pub roles: Vec<String>,
    pub skills: Vec<String>,
    pub experience_years: Option<u8>,
    pub hours_per_day: Option<u8>,
    pub rest_days_per_month: Option<u8>,
    pub lodging: Option<String>,
    pub housing_allowance: Option<u32>,
    pub availability: Option<String>,
    pub urgency: Option<String>,
    pub published_at: Option<DateTime<Utc>>,
    pub contacts: Vec<ContactCandidate>,
    pub missing_fields: Vec<String>,
    pub risk_candidates: Vec<RiskReason>,
}
```

Implement the state transition table as one exhaustive `matches!` expression. `TextFragment` contains `position`, `start_byte`, `end_byte`, and normalized `text`; it never contains author identity as a separate public field.

- [ ] **Step 4: Implement and test deterministic splitting**

Strip timestamp/nickname prefixes only when a line matches an anchored group-message pattern. Join continuation lines until the next recognized header or a double blank boundary. Preserve the original byte offsets before normalization and reject any limit violation with a typed error.

Run: `cargo test -p gitjobs-server talent::model talent::splitter`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add gitjobs-server/src/talent gitjobs-server/src/main.rs
git commit -s -m "feat: add talent domain and message splitter"
```

### Task 3: Encrypt raw data and normalize contacts

**Files:**
- Create: `gitjobs-server/src/talent/crypto.rs`
- Modify: `gitjobs-server/src/talent/mod.rs`
- Modify: `gitjobs-server/src/config.rs`
- Modify: `gitjobs-server/src/handlers/tests.rs`

**Interfaces:**
- Consumes: `ContactCandidate` from Task 2.
- Produces: `TalentCrypto::new(active_version, keys, contact_pepper)`.
- Produces: `encrypt(&self, plaintext: &[u8], aad: &[u8]) -> Result<EncryptedValue, CryptoError>` and `decrypt(&self, value: &EncryptedValue, aad: &[u8]) -> Result<Vec<u8>, CryptoError>`.
- Produces: `normalize_contact`, `mask_contact`, and `contact_hash`.

- [ ] **Step 1: Write failing privacy tests**

Tests assert round-trip encryption, random ciphertext for the same plaintext, rejection after byte tampering, decryption with an older configured key version, failure for an unknown key version, stable HMAC hashes, and masking:

```rust
assert_eq!(mask_contact(ContactType::Phone, "13800000001"), "138****0001");
assert_eq!(mask_contact(ContactType::Wechat, "test_candidate_01"), "tes***_01");
assert_ne!(crypto.encrypt(b"same", b"batch").unwrap().bytes,
           crypto.encrypt(b"same", b"batch").unwrap().bytes);
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p gitjobs-server talent::crypto`

Expected: FAIL because `TalentCrypto` is undefined.

- [ ] **Step 3: Add redacted secret configuration**

Add `TalentConfig`, `TalentEncryptionConfig`, and `SecretValue`. `SecretValue` implements `Deserialize`, exposes `expose_secret()` only inside the crate, and implements `Debug` as `[REDACTED]`. Configuration uses a map of key version to 64-character hexadecimal AES key, a separate pepper, and an active version. `HttpServerConfig` receives:

```rust
#[serde(default)]
pub talent: TalentConfig,
```

The default is disabled with `prototype_data = false` and no secrets. `test_http_server_cfg()` explicitly supplies a deterministic test-only key and pepper.

- [ ] **Step 4: Implement AES-256-GCM and HMAC-SHA256**

Use `openssl::symm::{encrypt_aead, decrypt_aead, Cipher}` with a random 12-byte nonce and 16-byte tag. Store the envelope as `nonce || ciphertext || tag`; bind ciphertext to AAD values such as `batch:{uuid}`, `item:{uuid}`, or `contact:{uuid}`. Use OpenSSL `PKey::hmac` plus `Signer` for contact hashes. Never implement custom cryptographic primitives.

Run: `cargo test -p gitjobs-server talent::crypto config::`

Expected: PASS and snapshots/debug assertions contain `[REDACTED]`, not test secrets.

- [ ] **Step 5: Commit**

```bash
git add gitjobs-server/src/talent/crypto.rs gitjobs-server/src/talent/mod.rs gitjobs-server/src/config.rs gitjobs-server/src/handlers/tests.rs
git commit -s -m "feat: protect talent source data"
```

### Task 4: Add the OpenAI-compatible extraction provider and validator

**Files:**
- Create: `gitjobs-server/src/talent/ai.rs`
- Create: `gitjobs-server/src/talent/validation.rs`
- Modify: `gitjobs-server/src/talent/mod.rs`
- Modify: `gitjobs-server/src/config.rs`
- Modify: `config/examples/server.local.yml`

**Interfaces:**
- Consumes: `TalentExtraction` from Task 2 and `SecretValue` from Task 3.
- Produces: async trait `TalentAiProvider::extract(&self, request: ExtractionRequest) -> Result<TalentExtraction, AiError>`.
- Produces: `OpenAiCompatibleProvider::new(config, reqwest::Client)`.
- Produces: `validate_extraction(extraction: TalentExtraction) -> Result<ValidatedExtraction, Vec<ValidationIssue>>`.

- [ ] **Step 1: Write failing provider and validator tests**

Test successful parsing from `choices[0].message.content`, refusal of unknown JSON fields, missing choice, non-JSON content, out-of-range confidence, salary inversion, `hours_per_day > 24`, `rest_days_per_month > 31`, `experience_years > 70`, oversized fields, and HTML/script content remaining escaped plain text.

```rust
let invalid = TalentExtraction {
    confidence: 1.2,
    salary_min: Some(8_000),
    salary_max: Some(6_000),
    ..valid_extraction()
};
let issues = validate_extraction(invalid).unwrap_err();
assert!(issues.iter().any(|issue| issue.code == "confidence_out_of_range"));
assert!(issues.iter().any(|issue| issue.code == "salary_range_inverted"));
```

- [ ] **Step 2: Run focused tests to verify they fail**

Run: `cargo test -p gitjobs-server talent::ai talent::validation`

Expected: FAIL because the provider and validator modules do not exist.

- [ ] **Step 3: Implement the provider contract**

Send `POST {base_url}/chat/completions` with an authorization bearer token, deterministic system prompt version `talent-extract-v1`, temperature 0, and `response_format: {"type":"json_object"}`. The system prompt states that the model must return `null` when uncertain and may never claim verification or truth. Wrap timeout, HTTP status, missing choice, invalid JSON, and schema failures in distinct `AiError` variants; do not include the prompt fragment or response body in `Display`.

- [ ] **Step 4: Implement strict server validation**

Normalize whitespace and deduplicate roles/skills. Move `contacts` out before serializing the safe extraction JSON, returning:

```rust
pub(crate) struct ValidatedExtraction {
    pub safe: TalentExtraction,
    pub contacts: Vec<ContactCandidate>,
    pub issues: Vec<ValidationIssue>,
}
```

The `safe.contacts` vector must always be empty so complete contact data cannot enter `extraction JSONB`.

Run: `cargo test -p gitjobs-server talent::ai talent::validation`

Expected: PASS.

- [ ] **Step 5: Document local provider configuration and commit**

Add disabled example values with environment-variable comments; do not commit a working key:

```yaml
server:
  talent:
    enabled: false
    prototype_data: true
    ai:
      base_url: https://api.example.invalid/v1
      model: replace-with-compatible-model
```

Run: `cargo check -p gitjobs-server`

Expected: PASS.

```bash
git add gitjobs-server/src/talent gitjobs-server/src/config.rs config/examples/server.local.yml
git commit -s -m "feat: add configurable talent extraction provider"
```

### Task 5: Persist sources, batches, fragments, and worker claims

**Files:**
- Create: `database/migrations/functions/talent/upsert_talent_source.sql`
- Create: `database/migrations/functions/talent/create_talent_import_batch.sql`
- Create: `database/migrations/functions/talent/list_talent_import_batches.sql`
- Create: `database/migrations/functions/talent/claim_talent_import_item.sql`
- Create: `database/migrations/functions/talent/complete_talent_import_item.sql`
- Create: `database/migrations/functions/talent/fail_talent_import_item.sql`
- Create: `database/tests/functions/talent/upsert_talent_source.sql`
- Create: `database/tests/functions/talent/import_processing.sql`
- Modify: `database/migrations/functions/001_load_functions.sql`
- Create: `gitjobs-server/src/db/talent.rs`
- Modify: `gitjobs-server/src/db/mod.rs`
- Modify: `gitjobs-server/src/db/mock.rs`

**Interfaces:**
- Produces: trait `DBTalent` and makes it a supertrait of `DB`.
- Produces: `create_talent_import(&self, NewTalentImport) -> Result<Uuid>`.
- Produces: `list_talent_import_batches(&self, limit: u16) -> Result<Vec<TalentImportBatchSummary>>`.
- Produces: `claim_talent_import_item(&self) -> Result<Option<ClaimedTalentItem>>` using `FOR UPDATE SKIP LOCKED`.
- Produces: `complete_talent_import_item(&self, CompletedTalentItem) -> Result<()>` and `fail_talent_import_item(&self, FailedTalentItem) -> Result<()>`.

- [ ] **Step 1: Write failing pgTAP tests for import persistence**

Assert source upsert is case-insensitive, a new source remains `new` with auto-publish disabled, batch plus fragments are atomic, claims cannot return the same item twice, completion increments batch counters, one item failure produces `partially_failed`, and stored extraction contains no complete contact value.

- [ ] **Step 2: Run the database tests to verify they fail**

Run: `just db-tests-file database/tests/functions/talent/import_processing.sql`

Expected: FAIL because `create_talent_import_batch` does not exist.

- [ ] **Step 3: Implement SQL functions and load order**

The claim function performs one atomic update:

```sql
with next_item as (
    select item_id
    from talent_import_items
    where status = 'queued'
    order by created_at, batch_id, position
    for update skip locked
    limit 1
)
update talent_import_items i
set status = 'extracting', updated_at = current_timestamp
from next_item n
where i.item_id = n.item_id
returning row_to_json(i);
```

All write functions reject invalid state changes and update batch counters in the same transaction.

- [ ] **Step 4: Implement `DBTalent` with typed JSON boundaries**

The Rust trait receives encrypted bytes and safe JSON only. Define exact input structs in `db/talent.rs`; do not pass plaintext source text or contacts to tracing instrumentation. Add all methods to `MockDB` so later handler/service tests can use expectations.

Run: `cargo test -p gitjobs-server db::talent`

Expected: PASS.

Run: `just db-tests-file database/tests/functions/talent/import_processing.sql`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add database/migrations/functions/talent database/tests/functions/talent database/migrations/functions/001_load_functions.sql gitjobs-server/src/db
git commit -s -m "feat: persist talent imports"
```

### Task 6: Add review, duplicate lookup, publication, and cleanup persistence

**Files:**
- Create: `database/migrations/functions/talent/list_talent_review_items.sql`
- Create: `database/migrations/functions/talent/get_talent_review_item.sql`
- Create: `database/migrations/functions/talent/find_talent_duplicate_candidates.sql`
- Create: `database/migrations/functions/talent/update_talent_review_item.sql`
- Create: `database/migrations/functions/talent/publish_talent_item.sql`
- Create: `database/migrations/functions/talent/merge_talent_import_item.sql`
- Create: `database/migrations/functions/talent/transition_talent_import_item.sql`
- Create: `database/migrations/functions/talent/withdraw_talent_record.sql`
- Create: `database/migrations/functions/talent/list_published_talent_records.sql`
- Create: `database/migrations/functions/talent/set_talent_auto_publish.sql`
- Create: `database/migrations/functions/talent/get_talent_runtime_settings.sql`
- Create: `database/migrations/functions/talent/set_talent_source_status.sql`
- Create: `database/migrations/functions/talent/cleanup_talent_raw_data.sql`
- Create: `database/tests/functions/talent/review_publication.sql`
- Create: `database/tests/functions/talent/public_privacy.sql`
- Create: `database/tests/functions/talent/cleanup.sql`
- Modify: `database/migrations/functions/001_load_functions.sql`
- Modify: `gitjobs-server/src/db/talent.rs`
- Modify: `gitjobs-server/src/db/mock.rs`

**Interfaces:**
- Consumes: Task 5 `DBTalent`.
- Produces: review list/detail methods, duplicate candidates, optimistic `save_talent_review`, idempotent `publish_talent_item`, atomic `merge_talent_import_item`, `withdraw_talent_record`, `list_published_talent_records`, `get_talent_runtime_settings`, `set_talent_auto_publish`, `set_talent_source_status`, and `cleanup_talent_raw_data`.
- Produces: `PublicTalentRecord`, which has no ciphertext or complete-contact fields.

- [ ] **Step 1: Write failing database tests**

Tests must prove: filters return only requested states; stale `lock_version` updates affect zero rows; publishing the same item twice returns the same record ID; publish inserts contacts and an audit event atomically; merge records the target and event in one transaction; rollback leaves no record; high-risk items cannot use the automatic path; manual publish records the Moderator; source status changes record confirmer/time and disable auto-publish for `watchlist` or `blocked`; withdrawal hides public records; public function returns masked contact only; cleanup nulls raw ciphertext after 90 days but preserves the record and event.

- [ ] **Step 2: Run focused database tests to verify they fail**

Run: `just db-tests-file database/tests/functions/talent/review_publication.sql`

Expected: FAIL because `publish_talent_item` does not exist.

- [ ] **Step 3: Implement atomic review and publication functions**

`publish_talent_item` must lock the item, check `lock_version`, reuse `published_record_id` when present, insert/update the record and encrypted contacts, insert a contact-free review event, then change the item state. It receives an `automatic boolean`; when true it rejects any non-low risk, hard block, untrusted source, disabled global/source switch, unconfirmed source, incomplete required fields, confidence below threshold, or high duplicate.

- [ ] **Step 4: Implement restricted public and cleanup functions**

`list_published_talent_records` explicitly selects public fields and `contact_masked`; it never uses `row_to_json` on `talent_contacts` or import tables. `cleanup_talent_raw_data(now)` sets batch/fragment/contact-candidate ciphertext to null and timestamps the cleanup, without deleting hashes, safe extraction, audit events, or published rows.

- [ ] **Step 5: Extend the Rust repository and mocks**

Use typed filter/input/output structs. Return `SaveReviewOutcome::Conflict` for stale versions and `PublishOutcome { record_id, created }` for idempotent publishing.

Run: `just db-tests-file database/tests/functions/talent/review_publication.sql`

Expected: PASS.

Run: `just db-tests-file database/tests/functions/talent/public_privacy.sql`

Expected: PASS and no complete test contact in returned JSON.

Run: `just db-tests-file database/tests/functions/talent/cleanup.sql`

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add database/migrations/functions/talent database/tests/functions/talent database/migrations/functions/001_load_functions.sql gitjobs-server/src/db
git commit -s -m "feat: add talent review publication repository"
```

### Task 7: Implement duplicate analysis, risk scoring, and auto-publish decisions

**Files:**
- Create: `gitjobs-server/src/talent/duplicate.rs`
- Create: `gitjobs-server/src/talent/risk.rs`
- Create: `gitjobs-server/src/talent/rules.rs`
- Modify: `gitjobs-server/src/talent/mod.rs`

**Interfaces:**
- Consumes: validated safe extraction and repository duplicate rows.
- Produces: `DuplicateService::evaluate(probe, candidates) -> Vec<DuplicateCandidate>`.
- Produces: `RuleEngine::analyze(fragment: &str) -> RuleHints` for kind keywords, salary/contact candidates, fee/illegal keywords, and rule/model conflicts.
- Produces: `RiskEngine::evaluate(RiskInput) -> RiskAssessment`.
- Produces: `AutoPublishPolicy::decide(AutoPublishInput) -> AutoPublishDecision`.

- [ ] **Step 1: Write failing scoring and policy tests**

Cover deterministic recruitment/seeking keyword hints, salary and contact candidates, exact contact hash, exact fingerprint, structured role/city/salary similarity, high threshold `>= 0.85`, medium threshold `>= 0.60`, new/watchlist/blocked source behavior, fee and illegal-content hard blocks, identity anomaly, missing required fields, model/rule conflict, confidence below 0.90, global pause, source pause, and successful low-risk decisions for both kinds.

```rust
let decision = AutoPublishPolicy::default().decide(AutoPublishInput {
    global_enabled: true,
    source_trust: SourceTrust::Trusted,
    source_enabled: true,
    source_confirmed: true,
    assessment: low_risk(),
    extraction: complete_hiring(0.95),
    duplicates: vec![],
});
assert!(decision.eligible);
assert!(decision.reasons.is_empty());
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p gitjobs-server talent::rules talent::duplicate talent::risk`

Expected: FAIL because the rule, duplicate, and risk modules are missing.

- [ ] **Step 3: Implement deterministic local rule hints**

Anchor phone and WeChat patterns, parse salary ranges without treating timestamps as wages, and classify only when explicit hiring/seeking keywords are present. `RuleHints` carries evidence offsets and never marks content verified. Fee, illegal, and identity keywords become hard-risk candidates before any model call.

- [ ] **Step 4: Implement deterministic duplicate scoring**

Exact contact/fingerprint matches score 1.0. Otherwise calculate a weighted score: normalized role 0.35, city/district 0.25, overlapping salary band 0.20, same source 0.10, and text similarity 0.10. The AI may provide an explanation but cannot change the numeric score or threshold.

- [ ] **Step 5: Implement risk and the hard gate**

Use named version `talent-risk-v1`. Additive soft rules are capped at 100; fee solicitation, illegal content, identity anomaly, blocked source, and high duplicate set score 100 and `hard_block = true`. Return stable reason codes and Chinese Moderator-facing explanations. Auto-publish checks every approved gate independently and records every failure reason.

Run: `cargo test -p gitjobs-server talent::rules talent::duplicate talent::risk`

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add gitjobs-server/src/talent/duplicate.rs gitjobs-server/src/talent/risk.rs gitjobs-server/src/talent/rules.rs gitjobs-server/src/talent/mod.rs
git commit -s -m "feat: evaluate talent duplicate and publication risk"
```

### Task 8: Orchestrate import processing, publishing, and cleanup workers

**Files:**
- Create: `gitjobs-server/src/talent/service.rs`
- Create: `gitjobs-server/src/talent/processor.rs`
- Modify: `gitjobs-server/src/talent/mod.rs`
- Modify: `gitjobs-server/src/workers.rs`
- Modify: `gitjobs-server/src/main.rs`
- Modify: `gitjobs-server/src/router.rs`
- Modify: `gitjobs-server/src/handlers/tests.rs`

**Interfaces:**
- Consumes: DB repository, crypto, splitter, AI provider, duplicate service, risk engine, and policy.
- Produces: `TalentServices { import: Arc<ImportService>, review: Arc<ReviewService>, processor: Arc<TalentProcessor> }`.
- Produces: `ImportService::create_batch(actor_id, NewImport) -> Result<ImportReceipt, ImportError>`.
- Produces: `TalentProcessor::process_one() -> Result<ProcessOutcome, ProcessError>`.
- Produces: `ReviewService::{save,publish,reject,merge,reprocess,withdraw,pause_auto_publish}`.

- [ ] **Step 1: Write failing orchestration tests with mocks**

Assert import validates limits before DB writes, encrypts batch/fragments with distinct AAD, and never calls AI during the HTTP-facing import operation. Processor tests cover successful auto-publish, low-risk but untrusted source going to review, medium/high risk going to review, high duplicate going to review, one AI retry after timeout, second AI failure going to `needs_review`, and decrypt failure going to `failed` without exposing bytes in the error.

- [ ] **Step 2: Run focused service tests to verify they fail**

Run: `cargo test -p gitjobs-server talent::service talent::processor`

Expected: FAIL because orchestration types do not exist.

- [ ] **Step 3: Implement the import and review services**

`create_batch` performs split, encryption, source upsert, and atomic batch persistence; it returns only batch ID and fragment count. `ReviewService` re-runs validation, duplicate lookup, risk, and the server policy immediately before publication. Audit snapshots are built from `safe_snapshot()` that replaces contacts with `{ "present": true, "masked": [...] }`.

- [ ] **Step 4: Implement the processor and workers**

Processor sequence is claim → decrypt → `RuleEngine::analyze` → AI → strict validation → compare model/rule hints → encrypt/hash contacts → duplicate evaluation → risk → policy → persist → optionally publish. Add a cancellable polling worker with maximum four concurrent items and a daily cleanup worker. A panicking/failed item must release its concurrency permit; the database claim state makes restart safe.

- [ ] **Step 5: Inject `TalentServices` once**

Build services in `main.rs`, pass the same `Arc` values to `workers::run` and `router::setup`, and add them to router `State`. `TestRouterBuilder` supplies deterministic crypto plus mock AI and can disable the worker because handler tests call services directly.

Run: `cargo test -p gitjobs-server talent:: workers::`

Expected: PASS.

Run: `cargo clippy -p gitjobs-server --all-targets --all-features -- --deny warnings`

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add gitjobs-server/src/talent gitjobs-server/src/workers.rs gitjobs-server/src/main.rs gitjobs-server/src/router.rs gitjobs-server/src/handlers/tests.rs
git commit -s -m "feat: process talent imports safely"
```

### Task 9: Build the Moderator import page and batch status

**Files:**
- Create: `gitjobs-server/src/handlers/dashboard/moderator/talent/mod.rs`
- Create: `gitjobs-server/src/handlers/dashboard/moderator/talent/import.rs`
- Create: `gitjobs-server/src/templates/dashboard/moderator/talent/mod.rs`
- Create: `gitjobs-server/src/templates/dashboard/moderator/talent/import.rs`
- Create: `gitjobs-server/templates/dashboard/moderator/talent/import.html`
- Create: `gitjobs-server/templates/dashboard/moderator/talent/components/recent_batches.html`
- Modify: `gitjobs-server/src/handlers/dashboard/moderator/mod.rs`
- Modify: `gitjobs-server/src/templates/dashboard/moderator/mod.rs`
- Modify: `gitjobs-server/src/router.rs`
- Modify: `gitjobs-server/templates/dashboard/moderator/menu.html`

**Interfaces:**
- Consumes: `ImportService::create_batch` and batch list repository method.
- Produces: `GET /dashboard/moderator/talent/import` and `POST /dashboard/moderator/talent/import`.
- Produces: validated `ImportForm { source_type, source_name, raw_text }`.

- [ ] **Step 1: Write failing handler tests**

Use the existing authenticated session helper. Assert anonymous and non-Moderator users receive 403; Moderator GET renders source controls, character count, size limit, action button, and recent batches; valid POST returns 303 to the batch anchor; empty/oversized imports return 422; POST does not directly publish.

- [ ] **Step 2: Run handler tests to verify they fail**

Run: `cargo test -p gitjobs-server handlers::dashboard::moderator::talent::import`

Expected: FAIL because the route/module is absent.

- [ ] **Step 3: Add routes under existing Moderator middleware**

Add exactly:

```rust
.route(
    "/talent/import",
    get(dashboard::moderator::talent::import::page)
        .post(dashboard::moderator::talent::import::create),
)
```

Do not create a second authorization mechanism.

- [ ] **Step 4: Implement accessible import UI and errors**

The form uses a labeled select, name field, textarea, live character counter, server-rendered validation summary, and “开始 AI 整理”. Never echo the raw text into logs or flash messages. Recent batch rows show source, counts, status, time, and a review link, but no raw fragment.

Run: `cargo test -p gitjobs-server handlers::dashboard::moderator::talent::import`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add gitjobs-server/src/handlers/dashboard/moderator gitjobs-server/src/templates/dashboard/moderator gitjobs-server/templates/dashboard/moderator gitjobs-server/src/router.rs
git commit -s -m "feat: add moderator talent import page"
```

### Task 10: Build the review queue, workbench, and moderation actions

**Files:**
- Create: `gitjobs-server/src/handlers/dashboard/moderator/talent/review.rs`
- Create: `gitjobs-server/src/templates/dashboard/moderator/talent/review.rs`
- Create: `gitjobs-server/templates/dashboard/moderator/talent/review.html`
- Create: `gitjobs-server/templates/dashboard/moderator/talent/review_item.html`
- Create: `gitjobs-server/templates/dashboard/moderator/talent/components/review_table.html`
- Create: `gitjobs-server/templates/dashboard/moderator/talent/components/risk_panel.html`
- Modify: `gitjobs-server/src/handlers/dashboard/moderator/talent/mod.rs`
- Modify: `gitjobs-server/src/templates/dashboard/moderator/talent/mod.rs`
- Modify: `gitjobs-server/src/router.rs`

**Interfaces:**
- Consumes: Task 8 `ReviewService`.
- Produces: queue/detail GET routes and PUT routes for save, publish, reject, merge, reprocess, withdraw, global pause, and source status (`trusted`, `watchlist`, or `blocked`).
- Produces: `ReviewForm` with `lock_version` for optimistic concurrency.

- [ ] **Step 1: Write failing queue and action tests**

Test type/risk/source/time/status filters, masked contacts in queue, Moderator-only decrypted detail, escaped model HTML, save success, stale-version 409, manual publish, hard-block automatic refusal, reject with reason, merge target validation, reprocess, withdrawal, global pause, source trust confirmation, and watchlist/blocked transitions disabling source auto-publish. Assert no response available to a public user contains raw text or complete contact.

- [ ] **Step 2: Run focused tests to verify they fail**

Run: `cargo test -p gitjobs-server handlers::dashboard::moderator::talent::review`

Expected: FAIL because review routes are missing.

- [ ] **Step 3: Add the complete route contract**

Add GET `/talent/review` and `/talent/review/{item_id}` plus PUT endpoints ending in `/save`, `/publish`, `/reject`, `/merge`, `/reprocess`, and `/withdraw`; add PUT `/talent/auto-publish/pause` and `/talent/sources/{source_id}/status`. Keep the whole router beneath `user_is_moderator`.

- [ ] **Step 4: Implement queue and workbench templates**

Queue columns: kind, confidence, region/role/salary, completeness, contact-present flag, risk, duplicate count, source, time, and state. Detail renders decrypted raw fragment only on the left Moderator panel and editable safe fields on the right. Every action includes `lock_version`; 409 explains that another review changed the item and links to reload.

- [ ] **Step 5: Implement action responses and audit-safe errors**

Use 303 redirects for complete form submissions and 204 plus `HX-Trigger` only for HTMX fragments. Errors name the failed stage and next action without embedding source text. Withdrawal affects the formal record even when the item state remains `published` or `auto_published`.

Run: `cargo test -p gitjobs-server handlers::dashboard::moderator::talent::review`

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add gitjobs-server/src/handlers/dashboard/moderator/talent gitjobs-server/src/templates/dashboard/moderator/talent gitjobs-server/templates/dashboard/moderator/talent gitjobs-server/src/router.rs
git commit -s -m "feat: add talent review workbench"
```

### Task 11: Replace public prototype data with published database records

**Files:**
- Modify: `gitjobs-server/src/handlers/platform/talent.rs`
- Modify: `gitjobs-server/src/templates/platform/talent.rs`
- Modify: `gitjobs-server/templates/platform/components/talent_results.html`
- Modify: `gitjobs-server/templates/platform/components/talent_detail_drawer.html`
- Modify: `tests/e2e/public/talent-workbench.spec.ts`
- Modify: `tests/e2e/public/talent-workbench-visual.spec.ts`

**Interfaces:**
- Consumes: `DBTalent::list_published_talent_records()`.
- Produces: existing JavaScript-compatible `TalentRecord` with owned values and an optional masked contact.
- Preserves: `/talent`, matching commands, detail drawer, no-JavaScript fallback, theme, accessibility, and reduced-motion behavior.

- [ ] **Step 1: Change handler tests to require DB data and verify privacy**

Configure a mock public record and assert it renders. Configure no records with `prototype_data = false` and assert the empty state. Configure `prototype_data = true` and assert labeled fixtures appear only in that mode. Serialize the browser JSON and assert it contains the masked value but not ciphertext field names or the complete fictional contact.

- [ ] **Step 2: Run public handler tests to verify they fail**

Run: `cargo test -p gitjobs-server handlers::platform::talent`

Expected: FAIL because the handler still calls prototype functions directly.

- [ ] **Step 3: Convert the public model to owned production data**

Change `&'static str` and `Vec<&'static str>` to `String` and `Vec<String>`, add `contact_masked: Option<String>`, and map repository rows explicitly. Keep `prototype: bool`; it is false for database rows and true only for explicit fixture mode.

- [ ] **Step 4: Update templates without changing matching semantics**

Render masked contact as display-only text. Do not add a “reveal” request or contact-unlock flow in this release. Empty state explains that only reviewed published information appears. Keep all model strings under Askama escaping and continue using `serialize_json_for_script`.

- [ ] **Step 5: Update E2E fixtures and run browser tests**

Load fictional published records in the E2E database or run with explicit prototype mode. Replace fixed assumptions that production always has 8+8 records with seeded counts while preserving matching, drawer, keyboard, no-JS, fallback, theme, visual, and mobile assertions.

Run: `npx playwright test --config tests/e2e/playwright.config.ts tests/e2e/public/talent-workbench.spec.ts tests/e2e/public/talent-workbench-visual.spec.ts --project=chromium`

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add gitjobs-server/src/handlers/platform/talent.rs gitjobs-server/src/templates/platform/talent.rs gitjobs-server/templates/platform/components tests/e2e/public/talent-workbench.spec.ts tests/e2e/public/talent-workbench-visual.spec.ts
git commit -s -m "feat: publish reviewed talent records"
```

### Task 12: Prove security, recovery, and cross-browser acceptance

**Files:**
- Create: `tests/e2e/dashboard/moderator/talent-import-review.spec.ts`
- Create: `docs/operations/talent-import-review.md`
- Modify: `database/tests/data/e2e.sql`
- Modify: `config/examples/server.local.yml`

**Interfaces:**
- Consumes: the complete pipeline.
- Produces: an operator runbook and acceptance evidence for all 14 design criteria.

- [ ] **Step 1: Add E2E setup using fictional data only**

Seed a Moderator, one trusted source, one new source, safe hiring/seeking samples, a medium-risk fee sample, and a high-similarity duplicate. Use reserved fictional phone numbers and clearly synthetic WeChat IDs; never reuse user-provided group content.

- [ ] **Step 2: Write end-to-end acceptance tests**

Test one pasted batch becomes multiple items; safe trusted low-risk data auto-publishes when globally enabled; new and medium/high-risk entries remain in review; duplicate remains in review; Moderator edit/publish/reject/merge/reprocess/withdraw works; global pause prevents subsequent auto-publication; `/talent` shows only published records; complete contacts/raw/ciphertext/API keys do not appear in public HTML, JSON, browser console, or error responses.

- [ ] **Step 3: Add the operator runbook**

Document required environment variables, key rotation sequence, AI provider switch, global emergency pause, source trust workflow, one-click withdrawal, worker retry behavior, 90-day cleanup verification, database backup/restore, and incident response. Example secrets must be invalid and labeled.

- [ ] **Step 4: Run the full verification matrix**

Run: `cargo fmt --all -- --check`

Expected: PASS.

Run: `cargo test --workspace`

Expected: PASS with zero failed tests.

Run: `cargo clippy --workspace --all-targets --all-features -- --deny warnings`

Expected: PASS with zero warnings.

Run: `just db-tests`

Expected: PASS with all pgTAP files successful.

Run: `npx playwright test --config tests/e2e/playwright.config.ts tests/e2e/dashboard/moderator/talent-import-review.spec.ts tests/e2e/public/talent-workbench.spec.ts tests/e2e/public/talent-workbench-visual.spec.ts`

Expected: PASS in Chromium, Firefox, and WebKit with zero `console.error` and zero `pageerror` events.

- [ ] **Step 5: Perform a final secret/privacy scan**

Run:

```bash
rg -n "api[_-]?key|raw_text|raw_fragment|ciphertext|13800000001|test_candidate_01" \
  gitjobs-server/templates gitjobs-server/static tests/e2e docs/operations
```

Expected: only field names, explicitly synthetic E2E input, and deliberately masked assertions; no user-provided contact, real source text, ciphertext value, or usable secret in templates/static output.

- [ ] **Step 6: Commit**

```bash
git add tests/e2e/dashboard/moderator/talent-import-review.spec.ts docs/operations/talent-import-review.md database/tests/data/e2e.sql config/examples/server.local.yml
git commit -s -m "test: verify talent import review workflow"
```

### Task 13: Add optional public-source subscriptions

**Files:**
- Create: `database/migrations/schema/0019_talent_source_subscriptions.sql`
- Create: `database/migrations/functions/talent/claim_talent_source_subscription.sql`
- Create: `database/migrations/functions/talent/complete_talent_source_run.sql`
- Create: `database/migrations/functions/talent/upsert_talent_source_subscription.sql`
- Create: `database/tests/functions/talent/source_subscriptions.sql`
- Create: `gitjobs-server/src/talent/source_subscription.rs`
- Modify: `gitjobs-server/src/talent/mod.rs`
- Modify: `gitjobs-server/src/talent/service.rs`
- Modify: `gitjobs-server/src/db/talent.rs`
- Modify: `gitjobs-server/src/db/mock.rs`
- Modify: `gitjobs-server/src/workers.rs`
- Modify: `gitjobs-server/src/config.rs`
- Modify: `config/examples/server.local.yml`
- Modify: `gitjobs-server/src/handlers/dashboard/moderator/talent.rs`
- Modify: `gitjobs-server/src/templates/dashboard/moderator/talent.rs`
- Modify: `gitjobs-server/templates/dashboard/moderator/talent_import.html`
- Modify: `database/migrations/functions/001_load_functions.sql`
- Modify: `database/tests/schema/schema.sql`
- Modify: `tests/e2e/dashboard/moderator/talent-import-review.spec.ts`
- Create: `tests/e2e/dashboard/moderator/talent-source-subscriptions-visual.spec.ts`
- Modify: `docs/operations/talent-import-review.md`

**Interfaces:**
- Consumes: the existing `ImportService`; collected text has no separate publication path.
- Produces: Moderator-managed public source, keyword topic, update interval, enabled state, incremental cursor, and privacy-safe run summary.
- Preserves: manual paste import, source trust, AI extraction, duplicate detection, risk gates, review, publication, and cleanup behavior.

- [ ] **Step 1: Add subscription persistence and atomic claims**

Store only approved public HTTPS URLs. A subscription belongs to one `talent_sources` row and contains normalized keywords, interval, enabled state, next run time, incremental cursor, conditional-request metadata, consecutive failure count, and last success/error timestamps. Store each run's discovered, accepted, duplicate, and failed counters without response bodies. Claim due subscriptions with `FOR UPDATE SKIP LOCKED`; one broken source must not block another.

- [ ] **Step 2: Implement strict source safety and deterministic filtering**

Allow collection only when the URL host is present in a server-side allowlist. Reject credentials, fragments, non-HTTPS schemes, IP literals, localhost, and redirects to a different or disallowed host. Resolve and reject private, loopback, link-local, multicast, and reserved addresses before every request. Limit redirects, response bytes, elapsed time, and requests per run. Match normalized keywords against title and text using deterministic case-insensitive rules before invoking AI.

- [ ] **Step 3: Implement incremental public-feed collection**

Support RSS and Atom first because they provide stable item identifiers and publication times. Use `ETag` and `Last-Modified` when available, then deduplicate entries by source item ID or normalized content fingerprint. Convert accepted entries to ordinary `NewImport` batches with source type `web`; never auto-publish from the collector itself. A failed fetch records a safe error code, applies bounded exponential backoff, and leaves the previous cursor intact.

- [ ] **Step 4: Add the source worker and a mobile-first information radar**

Run one low-frequency source worker alongside the existing talent processor. Add a mobile-first “信息雷达” view that inherits the existing product shell and visual tokens. Its first viewport shows one restrained live-search status band, what the system is watching, newly found count, and whether anything needs attention. This is the single expressive visual moment; the rest of the view stays quiet and direct.

Adding a source is a one-column flow using the exact user-facing concepts “从哪里找”, “找什么”, and “多久找一次”, followed by an enabled switch and a bottom-fixed primary action on mobile. Use plain frequency choices such as “每天一次”, “每天两次”, and “每周一次”. Do not expose crawler, cursor, RSS, HTTP, concurrency, pagination waits, retries, request headers, or network tuning. Source rows show only name, watched topic, last update, and a plain-language state; editing opens a focused sheet or page rather than expanding parameters inline.

At 360 CSS pixels the layout remains single-column, all interactive targets are at least 44 by 44 CSS pixels, long Chinese labels wrap without collision, the software keyboard does not cover the primary action, and no horizontal scroll appears. Desktop adds breathing room but preserves the same hierarchy instead of becoming an admin parameter grid. Respect reduced motion, visible keyboard focus, semantic labels, and non-color status cues.

- [ ] **Step 5: Prove isolation and compatibility**

Unit tests cover RSS/Atom parsing, keyword normalization, unchanged feeds, cursor updates, response-size limits, timeout/backoff, redirect rejection, private-address rejection, and safe errors. Database tests cover atomic claims and stale-worker recovery. E2E tests prove manual paste import behaves unchanged, a fictional allowlisted feed creates a normal batch, repeated runs create no duplicate batch items, and collected entries cannot bypass review or source trust gates. Visual tests cover 360x800 mobile, a modern phone viewport, desktop, empty/loading/success/failure states, the software-keyboard-safe action area, reduced motion, and text overlap.

- [ ] **Step 6: Update operations and run the full verification matrix**

Document allowlist ownership, source permission checks, robots/terms review, emergency disable, backoff recovery, and removal of a source. Run the Task 12 verification matrix plus the focused source-subscription database, Rust, and browser tests.

- [ ] **Step 7: Commit**

```bash
git add database gitjobs-server config tests/e2e/dashboard/moderator/talent-import-review.spec.ts docs/operations/talent-import-review.md
git commit -s -m "feat: add optional public talent source subscriptions"
```

## Completion Gate

Before calling the feature complete, verify every design acceptance criterion maps to at least one passing test, inspect public HTTP bodies for protected values, confirm formal config disables prototypes, exercise global pause and withdrawal manually, and obtain an independent code review of migration safety, cryptography usage, authorization, auto-publish policy, and audit completeness. If public-source subscriptions are enabled, also verify the host allowlist, network-address restrictions, incremental cursor behavior, source isolation, backoff, and the invariant that collected content enters the same import and review pipeline as manual content.
