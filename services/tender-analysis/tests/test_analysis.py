"""Analysis orchestration, review, and endpoint tests."""

from __future__ import annotations

import json
import secrets

import pytest
from httpx import ASGITransport, AsyncClient

from app.contracts import (
    Citation,
    DocumentChunk,
    EvidenceValue,
    ParsedDocument,
    ParsedPage,
    ProjectInfo,
    Qualification,
    RequiredMaterial,
    TenderAnalysisV1,
    UnknownItem,
)


@pytest.fixture
def anyio_backend():
    return "asyncio"


@pytest.fixture
def valid_token():
    return "test-token-" + secrets.token_hex(16)


@pytest.fixture
def auth_env(monkeypatch, valid_token):
    monkeypatch.setenv("INTERNAL_SERVICE_TOKEN", valid_token)
    from app.main import app

    return ASGITransport(app=app), valid_token


def _chunk(page: int, idx: int, text: str) -> DocumentChunk:
    return DocumentChunk(chunk_id=f"page-{page}-chunk-{idx}", page_number=page, text=text)


def _doc(pages: list[list[str]]) -> ParsedDocument:
    out = []
    for pno, texts in enumerate(pages, 1):
        chunks = [_chunk(pno, i, t) for i, t in enumerate(texts) if t]
        out.append(ParsedPage(page_number=pno, text="\n".join(texts), chunks=chunks))
    return ParsedDocument(page_count=len(pages), pages=out)


def _cite(page: int, idx: int, quote: str) -> Citation:
    return Citation(chunk_id=f"page-{page}-chunk-{idx}", page_number=page, quote=quote)


class FakeProvider:
    """Records prompt versions and returns canned responses per version."""

    def __init__(self, responses: dict[str, object]):
        self.responses = responses
        self.calls: list[str] = []
        self.payloads: list[tuple[str, str]] = []

    async def complete_structured(self, system_prompt, user_payload, response_model):
        version = system_prompt.splitlines()[0].strip()
        self.calls.append(version)
        self.payloads.append((version, user_payload))
        return self.responses[version]


# ── Standard happy-path fixture responses ──────────────────────

_DOC = _doc(
    [
        [
            "项目名称：某市第三中学食堂承包服务项目",
            "预算金额：人民币280万元",
            "投标人须具有有效的食品经营许可证",
            "具有ISO22000食品安全管理体系认证的得2分",
            "投标人须提供近三年无重大食品安全事故的承诺函",
        ]
    ]
)


def _missing(value: str | None) -> EvidenceValue:
    return EvidenceValue(value=value, confidence=0.0, requires_review=value is None, citations=[])


def _standard_responses():
    from app.analysis import MaterialList, QualificationList, ReviewResult, RiskList

    return {
        "tender-project-info-v1": ProjectInfo(
            project_name=EvidenceValue(
                value="某市第三中学食堂承包服务项目",
                confidence=0.95,
                requires_review=False,
                citations=[_cite(1, 0, "项目名称：某市第三中学食堂承包服务项目")],
            ),
            procurement_entity=_missing(None),
            budget_amount=EvidenceValue(
                value="人民币280万元",
                confidence=0.9,
                requires_review=False,
                citations=[_cite(1, 1, "预算金额：人民币280万元")],
            ),
            budget_currency=_missing(None),
            service_period=_missing(None),
            bid_deadline=_missing(None),
        ),
        "tender-qualifications-v1": QualificationList(
            items=[
                Qualification(
                    kind="hard_qualification",
                    requirement="投标人须具有有效的食品经营许可证",
                    original_text="投标人须具有有效的食品经营许可证",
                    confidence=0.95,
                    requires_review=False,
                    citations=[_cite(1, 2, "投标人须具有有效的食品经营许可证")],
                ),
                Qualification(
                    kind="scoring_item",
                    requirement="具有ISO22000食品安全管理体系认证的得2分",
                    original_text="具有ISO22000食品安全管理体系认证的得2分",
                    confidence=0.9,
                    requires_review=False,
                    citations=[_cite(1, 3, "具有ISO22000食品安全管理体系认证的得2分")],
                ),
            ]
        ),
        "tender-risks-v1": RiskList(items=[]),
        "tender-materials-v1": MaterialList(
            items=[
                RequiredMaterial(
                    name="近三年无重大食品安全事故的承诺函",
                    description="承诺函",
                    source_requirement="须提供",
                    confidence=0.9,
                    requires_review=False,
                    citations=[_cite(1, 4, "投标人须提供近三年无重大食品安全事故的承诺函")],
                )
            ]
        ),
        "tender-review-v1": ReviewResult(),
    }


# ── Orchestration tests ────────────────────────────────────────


@pytest.mark.anyio
async def test_five_prompt_versions_in_order():
    """The five stages must run with the exact versioned prompts in order."""
    from app.analysis import analyze_document

    provider = FakeProvider(_standard_responses())
    report = await analyze_document(_DOC, provider, "fixture-model", model_provider="openai")

    assert provider.calls == [
        "tender-project-info-v1",
        "tender-qualifications-v1",
        "tender-risks-v1",
        "tender-materials-v1",
        "tender-review-v1",
    ]
    assert report.schema_version == "tender-analysis.v1"


@pytest.mark.anyio
async def test_hard_and_scoring_qualifications_separated():
    """Hard qualifications and scoring items must remain distinct kinds."""
    from app.analysis import analyze_document

    report = await analyze_document(_DOC, FakeProvider(_standard_responses()), "m")
    kinds = {q.kind for q in report.qualifications}
    assert "hard_qualification" in kinds
    assert "scoring_item" in kinds


@pytest.mark.anyio
async def test_missing_fact_value_null_requires_review():
    """A missing project fact must surface as value=null and requires_review=True."""
    from app.analysis import analyze_document

    report = await analyze_document(_DOC, FakeProvider(_standard_responses()), "m")
    assert report.project_info.procurement_entity.value is None
    assert report.project_info.procurement_entity.requires_review is True


@pytest.mark.anyio
async def test_forged_citation_demoted_to_unknowns():
    """A claim whose citation is not in the document must be demoted to unknowns."""
    from app.analysis import analyze_document

    responses = _standard_responses()
    # Forge the budget citation to point at a nonexistent chunk.
    responses["tender-project-info-v1"] = ProjectInfo(
        project_name=EvidenceValue(
            value="某市第三中学食堂承包服务项目",
            confidence=0.95,
            requires_review=False,
            citations=[_cite(1, 0, "项目名称：某市第三中学食堂承包服务项目")],
        ),
        procurement_entity=_missing(None),
        budget_amount=EvidenceValue(
            value="人民币280万元",
            confidence=0.9,
            requires_review=False,
            citations=[_cite(99, 0, "预算金额：人民币280万元")],
        ),
        budget_currency=_missing(None),
        service_period=_missing(None),
        bid_deadline=_missing(None),
    )
    report = await analyze_document(_DOC, FakeProvider(responses), "m")
    assert report.project_info.budget_amount.value is None
    assert any(
        u.field_name == "budget_amount" and u.reason == "unsupported" for u in report.unknowns
    )


@pytest.mark.anyio
async def test_review_cannot_add_new_facts():
    """The review stage must not introduce new qualifications/risks/materials."""
    from app.analysis import ReviewResult, analyze_document

    responses = _standard_responses()
    responses["tender-review-v1"] = ReviewResult(
        unknowns=[
            UnknownItem(field_name="budget_amount", reason="missing", detail="未说明", citations=[])
        ]
    )
    report = await analyze_document(_DOC, FakeProvider(responses), "m")

    assert len(report.qualifications) == 2
    assert len(report.risks) == 0
    assert len(report.required_materials) == 1


# ── LLM failure safety ─────────────────────────────────────────


@pytest.mark.anyio
async def test_litellm_failure_maps_to_safe_code(monkeypatch):
    """Provider exceptions must become a fixed safe code without leaking text."""
    import litellm
    from pydantic import BaseModel

    from app.llm import AnalysisError, LiteLlmProvider

    class R(BaseModel):
        x: int

    sentinel = "TOP_SECRET_PROVIDER_DETAIL"

    async def boom(**kwargs):
        raise RuntimeError(sentinel)

    monkeypatch.setattr(litellm, "acompletion", boom)
    provider = LiteLlmProvider(model="openai/gpt-4o")
    with pytest.raises(AnalysisError) as exc:
        await provider.complete_structured("sys", "user", R)

    assert exc.value.code == "analysis_unavailable"
    assert sentinel not in str(exc.value)


@pytest.mark.anyio
async def test_litellm_invalid_json_maps_to_safe_code(monkeypatch):
    """Non-JSON model output must become a fixed safe code."""
    import litellm
    from pydantic import BaseModel

    from app.llm import AnalysisError, LiteLlmProvider

    class R(BaseModel):
        x: int

    class _Msg:
        def __init__(self, content):
            self.content = content

    class _Choice:
        def __init__(self, content):
            self.message = _Msg(content)

    class _Resp:
        def __init__(self, content):
            self.choices = [_Choice(content)]

    async def fake(**kwargs):
        return _Resp("definitely not json")

    monkeypatch.setattr(litellm, "acompletion", fake)
    provider = LiteLlmProvider(model="openai/gpt-4o")
    with pytest.raises(AnalysisError) as exc:
        await provider.complete_structured("sys", "user", R)

    assert exc.value.code == "analysis_unavailable"


# ── Analyze endpoint ───────────────────────────────────────────


@pytest.mark.anyio
async def test_analyze_unauthorized_body_not_read(monkeypatch, valid_token):
    """Unauthenticated analyze must return 401 without reading the body."""
    monkeypatch.setenv("INTERNAL_SERVICE_TOKEN", valid_token)
    from app.main import app

    class Counting:
        def __init__(self, inner):
            self.inner = inner
            self.bytes = 0

        async def __call__(self, scope, receive, send):
            async def counting_receive():
                message = await receive()
                if message["type"] == "http.request":
                    self.bytes += len(message.get("body", b""))
                return message

            await self.inner(scope, counting_receive, send)

    counting = Counting(app)
    transport = ASGITransport(app=counting)
    async with AsyncClient(transport=transport, base_url="http://test") as client:
        response = await client.post(
            "/internal/v1/analyze",
            content=b"{}",
            headers={"Content-Type": "application/json"},
        )
    assert response.status_code == 401
    assert counting.bytes == 0


@pytest.mark.anyio
async def test_analyze_valid_request_succeeds(auth_env, monkeypatch):
    """A valid authenticated analyze request returns a TenderAnalysisV1 report."""
    from app import main

    provider = FakeProvider(_standard_responses())
    monkeypatch.setattr(main, "_build_provider", lambda settings: provider)

    transport, token = auth_env
    body = json.dumps({"job_id": "job-1", "document": _DOC.model_dump(mode="json")})
    async with AsyncClient(transport=transport, base_url="http://test") as client:
        response = await client.post(
            "/internal/v1/analyze",
            headers={
                "Authorization": f"Bearer {token}",
                "Content-Type": "application/json",
            },
            content=body,
        )
    assert response.status_code == 200
    report = TenderAnalysisV1.model_validate(response.json())
    assert report.schema_version == "tender-analysis.v1"
    assert report.analysis_meta.contract_version == "tender-analysis.v1"


# ── Correction 01: evidence boundary ───────────────────────────


@pytest.mark.anyio
async def test_any_invalid_citation_demotes_whole_item():
    """One invalid citation must demote the whole item, not keep the valid one."""
    from app.analysis import QualificationList, analyze_document

    responses = _standard_responses()
    responses["tender-qualifications-v1"] = QualificationList(
        items=[
            Qualification(
                kind="hard_qualification",
                requirement="投标人须具有有效的食品经营许可证",
                original_text="投标人须具有有效的食品经营许可证",
                confidence=0.95,
                requires_review=False,
                citations=[
                    _cite(1, 2, "投标人须具有有效的食品经营许可证"),
                    _cite(99, 0, "投标人须具有有效的食品经营许可证"),
                ],
            )
        ]
    )
    report = await analyze_document(_DOC, FakeProvider(responses), "m")

    assert report.qualifications == []
    assert any(u.reason == "unsupported" for u in report.unknowns)
    assert all(u.citations == [] for u in report.unknowns)


@pytest.mark.anyio
async def test_review_unknown_invented_field_dropped():
    """A review unknown with an invented field_name must be dropped."""
    from app.analysis import ReviewResult, analyze_document

    responses = _standard_responses()
    responses["tender-review-v1"] = ReviewResult(
        unknowns=[
            UnknownItem(
                field_name="invented",
                reason="conflict",
                detail="虚构内容",
                citations=[_cite(99, 0, "不存在")],
            )
        ]
    )
    report = await analyze_document(_DOC, FakeProvider(responses), "m")
    assert not any(u.field_name == "invented" for u in report.unknowns)


@pytest.mark.anyio
async def test_conflict_unknown_with_invalid_citation_dropped():
    """A conflict unknown must carry a valid, verifiable citation."""
    from app.analysis import ReviewResult, analyze_document

    responses = _standard_responses()
    responses["tender-review-v1"] = ReviewResult(
        unknowns=[
            UnknownItem(
                field_name="budget_amount",
                reason="conflict",
                detail="两处金额不一致",
                citations=[_cite(99, 0, "不存在")],
            )
        ]
    )
    report = await analyze_document(_DOC, FakeProvider(responses), "m")
    assert not any(
        u.field_name == "budget_amount" and u.reason == "conflict" for u in report.unknowns
    )


@pytest.mark.anyio
async def test_analyze_illegal_utf8_returns_400(auth_env):
    """Illegal UTF-8 in the analyze body must return a fixed 400."""
    transport, token = auth_env
    async with AsyncClient(transport=transport, base_url="http://test") as client:
        response = await client.post(
            "/internal/v1/analyze",
            headers={"Authorization": f"Bearer {token}", "Content-Type": "application/json"},
            content=b"\xff",
        )
    assert response.status_code == 400
    assert response.json()["detail"] == "invalid_request"


@pytest.mark.anyio
async def test_assembly_overflow_returns_safe_code():
    """Report assembly exceeding contract limits must map to a safe code."""
    from app.analysis import QualificationList, ReviewResult, analyze_document
    from app.llm import AnalysisError

    responses = _standard_responses()
    responses["tender-qualifications-v1"] = QualificationList(
        items=[
            Qualification(
                kind="hard_qualification",
                requirement=f"要求{i}",
                original_text=f"要求{i}",
                confidence=0.9,
                requires_review=False,
                citations=[_cite(99, 0, "不存在")],
            )
            for i in range(200)
        ]
    )
    responses["tender-review-v1"] = ReviewResult(
        unknowns=[
            UnknownItem(field_name="budget_amount", reason="missing", detail="未说明", citations=[])
        ]
    )
    with pytest.raises(AnalysisError) as exc:
        await analyze_document(_DOC, FakeProvider(responses), "m")
    assert exc.value.code == "analysis_unavailable"


@pytest.mark.anyio
async def test_risk_stage_receives_facts_and_sources():
    """The risk stage must receive extracted facts and their source chunks."""
    from app.analysis import analyze_document

    provider = FakeProvider(_standard_responses())
    await analyze_document(_DOC, provider, "m")

    risk_payload = next(p for v, p in provider.payloads if v == "tender-risks-v1")
    assert '"project_info"' in risk_payload
    assert '"qualifications"' in risk_payload
    assert '"source_chunks"' in risk_payload
    assert "某市第三中学食堂承包服务项目" in risk_payload
    assert "page-1-chunk-0" in risk_payload


# ── Correction 02: consistency and coverage ────────────────────


@pytest.mark.anyio
async def test_confirmed_field_not_reported_missing():
    """A confirmed field must not also be reported missing."""
    from app.analysis import ReviewResult, analyze_document

    responses = _standard_responses()  # budget is confirmed
    responses["tender-review-v1"] = ReviewResult(
        unknowns=[
            UnknownItem(field_name="budget_amount", reason="missing", detail="缺失", citations=[])
        ]
    )
    report = await analyze_document(_DOC, FakeProvider(responses), "m")

    assert report.project_info.budget_amount.value is not None
    assert not any(
        u.field_name == "budget_amount" and u.reason == "missing" for u in report.unknowns
    )


@pytest.mark.anyio
async def test_conflict_with_single_citation_rejected():
    """A single citation cannot prove a two-place conflict."""
    from app.analysis import ReviewResult, analyze_document

    responses = _standard_responses()
    responses["tender-review-v1"] = ReviewResult(
        unknowns=[
            UnknownItem(
                field_name="budget_amount",
                reason="conflict",
                detail="两处冲突",
                citations=[_cite(1, 1, "预算金额：人民币280万元")],
            )
        ]
    )
    report = await analyze_document(_DOC, FakeProvider(responses), "m")

    assert report.project_info.budget_amount.value is not None
    assert not any(
        u.field_name == "budget_amount" and u.reason == "conflict" for u in report.unknowns
    )


@pytest.mark.anyio
async def test_conflict_with_two_distinct_citations_demotes_field():
    """A two-chunk conflict must demote the project field to unconfirmed."""
    from app.analysis import (
        MaterialList,
        QualificationList,
        ReviewResult,
        RiskList,
        analyze_document,
    )

    doc = _doc([["预算金额：100万元", "预算金额：200万元"]])
    responses = {
        "tender-project-info-v1": ProjectInfo(
            project_name=_missing(None),
            procurement_entity=_missing(None),
            budget_amount=EvidenceValue(
                value="100万元",
                confidence=0.9,
                requires_review=False,
                citations=[_cite(1, 0, "预算金额：100万元")],
            ),
            budget_currency=_missing(None),
            service_period=_missing(None),
            bid_deadline=_missing(None),
        ),
        "tender-qualifications-v1": QualificationList(items=[]),
        "tender-risks-v1": RiskList(items=[]),
        "tender-materials-v1": MaterialList(items=[]),
        "tender-review-v1": ReviewResult(
            unknowns=[
                UnknownItem(
                    field_name="budget_amount",
                    reason="conflict",
                    detail="两处预算金额不一致",
                    citations=[
                        _cite(1, 0, "预算金额：100万元"),
                        _cite(1, 1, "预算金额：200万元"),
                    ],
                )
            ]
        ),
    }
    report = await analyze_document(doc, FakeProvider(responses), "m")

    assert report.project_info.budget_amount.value is None
    assert any(u.field_name == "budget_amount" and u.reason == "conflict" for u in report.unknowns)


@pytest.mark.anyio
async def test_missing_facts_enter_unknowns_and_partial():
    """value=None project fields must become missing unknowns and set partial."""
    from app.analysis import analyze_document

    report = await analyze_document(_DOC, FakeProvider(_standard_responses()), "m")
    missing_fields = {u.field_name for u in report.unknowns if u.reason == "missing"}
    assert {
        "procurement_entity",
        "budget_currency",
        "service_period",
        "bid_deadline",
    } <= missing_fields
    assert report.analysis_meta.completeness == "partial"


@pytest.mark.anyio
async def test_review_action_conflict_demote_ignored():
    """A review demote action claiming conflict cannot bypass the citation rule."""
    from app.analysis import ReviewAction, ReviewResult, analyze_document

    responses = _standard_responses()
    responses["tender-review-v1"] = ReviewResult(
        actions=[
            ReviewAction(
                stage="project_info",
                field="budget_amount",
                action="demote",
                demote_reason="conflict",
                demote_detail="两处冲突",
            )
        ]
    )
    report = await analyze_document(_DOC, FakeProvider(responses), "m")

    assert report.project_info.budget_amount.value is not None
    assert not any(
        u.field_name == "budget_amount" and u.reason == "conflict" for u in report.unknowns
    )


@pytest.mark.anyio
async def test_review_source_chunks_capped():
    """Review source_chunks must be independently capped at 60."""
    from app.analysis import analyze_document

    pages = []
    for i in range(50):
        pages.append([f"项目名称：项目{i}"])
    for i in range(50):
        pages.append([f"资格要求{i}"])
    for i in range(50):
        pages.append([f"风险条款{i}"])
    for i in range(50):
        pages.append([f"提供材料{i}"])
    doc = _doc(pages)

    provider = FakeProvider(_standard_responses())
    await analyze_document(doc, provider, "m")
    review_payload = next(p for v, p in provider.payloads if v == "tender-review-v1")
    data = json.loads(review_payload)
    assert len(data["source_chunks"]) <= 60
