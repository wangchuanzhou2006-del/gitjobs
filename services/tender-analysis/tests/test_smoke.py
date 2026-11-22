"""Two-page synthetic PDF smoke test with a deterministic, no-cost model."""

import fitz
import pytest

from app.analysis import MaterialList, QualificationList, ReviewResult, RiskList, analyze_document
from app.contracts import Citation, EvidenceValue, ProjectInfo
from app.pdf import parse_pdf


@pytest.fixture
def anyio_backend():
    return "asyncio"


def _pdf() -> bytes:
    document = fitz.open()
    for text in ("Budget amount: CNY 2800000", "Bid deadline: 2026-09-01"):
        page = document.new_page()
        page.insert_text((50, 80), text)
    data = document.tobytes()
    document.close()
    return data


class _Provider:
    def __init__(self, forged: bool):
        self.forged = forged

    async def complete_structured(self, system_prompt, user_payload, response_model):
        version = system_prompt.splitlines()[0].strip()
        if version == "tender-project-info-v1":
            missing = EvidenceValue(value=None, confidence=0, requires_review=True, citations=[])
            return ProjectInfo(
                project_name=missing,
                procurement_entity=missing,
                budget_amount=EvidenceValue(
                    value="CNY 2800000",
                    confidence=0.95,
                    requires_review=False,
                    citations=[
                        Citation(
                            page_number=1,
                            chunk_id="page-1-chunk-0",
                            quote="Invented budget: CNY 9999999"
                            if self.forged
                            else "Budget amount: CNY 2800000",
                        )
                    ],
                ),
                budget_currency=missing,
                service_period=missing,
                bid_deadline=EvidenceValue(
                    value="2026-09-01",
                    confidence=0.95,
                    requires_review=False,
                    citations=[
                        Citation(
                            page_number=2,
                            chunk_id="page-2-chunk-0",
                            quote="Bid deadline: 2026-09-01",
                        )
                    ],
                ),
            )
        if version == "tender-qualifications-v1":
            return QualificationList(items=[])
        if version == "tender-risks-v1":
            return RiskList(items=[])
        if version == "tender-materials-v1":
            return MaterialList(items=[])
        if version == "tender-review-v1":
            return ReviewResult()
        raise AssertionError("unexpected prompt version")


@pytest.mark.anyio
async def test_real_pdf_parse_and_fake_model_keep_budget_and_deadline_on_source_pages():
    parsed = parse_pdf(_pdf())
    assert parsed.page_count == 2
    assert "Budget amount: CNY 2800000" in parsed.pages[0].text
    assert "Bid deadline: 2026-09-01" in parsed.pages[1].text

    report = await analyze_document(parsed, _Provider(False), "fixture-model", model_provider="e2e")
    budget = report.project_info.budget_amount
    deadline = report.project_info.bid_deadline
    assert budget.value == "CNY 2800000"
    assert budget.citations[0].page_number == 1
    assert budget.citations[0].quote in parsed.pages[0].text
    assert deadline.value == "2026-09-01"
    assert deadline.citations[0].page_number == 2
    assert deadline.citations[0].quote in parsed.pages[1].text


@pytest.mark.anyio
async def test_real_pdf_parse_demotes_invented_quote_before_report_output():
    parsed = parse_pdf(_pdf())
    report = await analyze_document(parsed, _Provider(True), "fixture-model", model_provider="e2e")
    assert report.project_info.budget_amount.value is None
    assert any(
        item.field_name == "budget_amount" and item.reason == "unsupported"
        for item in report.unknowns
    )
    assert all(
        "Invented budget" not in citation.quote
        for item in report.unknowns
        for citation in item.citations
    )
