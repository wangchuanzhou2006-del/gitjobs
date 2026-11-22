"""Exercise the real internal HTTP boundary with a deterministic model double."""

from __future__ import annotations

import fitz
import pytest
from httpx import ASGITransport, AsyncClient


@pytest.fixture
def anyio_backend():
    return "asyncio"


@pytest.fixture
def fixture_app(monkeypatch):
    monkeypatch.setenv("INTERNAL_SERVICE_TOKEN", "e2e-only-fictional-tender-token-00000000")
    monkeypatch.setenv("LLM_PROVIDER", "e2e")
    monkeypatch.setenv("LLM_MODEL", "fixture-model")
    from app import main

    original_provider_factory = main._build_provider
    from e2e_fixture_server import _build_fixture_provider, app

    main._build_provider = _build_fixture_provider
    try:
        yield app
    finally:
        main._build_provider = original_provider_factory


def _pdf(second_line: str = "Bid deadline: 2026-09-01") -> bytes:
    document = fitz.open()
    for line in ("Budget amount: CNY 2800000", second_line):
        page = document.new_page()
        page.insert_text((50, 72), line)
    output = document.tobytes()
    document.close()
    return output


@pytest.mark.anyio
async def test_real_parse_and_analyze_preserve_two_page_citations(fixture_app):
    headers = {"Authorization": "Bearer e2e-only-fictional-tender-token-00000000"}
    async with AsyncClient(
        transport=ASGITransport(app=fixture_app), base_url="http://test"
    ) as client:
        parsed = await client.post(
            "/internal/v1/parse",
            headers=headers,
            files={"file": ("fictional.pdf", _pdf(), "application/pdf")},
        )
        assert parsed.status_code == 200
        document = parsed.json()["parsed_document"]
        assert document["page_count"] == 2

        analyzed = await client.post(
            "/internal/v1/analyze",
            headers=headers,
            json={"job_id": "synthetic-e2e-job", "document": document},
        )

    assert analyzed.status_code == 200
    report = analyzed.json()
    assert report["project_info"]["budget_amount"]["value"] == "CNY 2800000"
    assert report["project_info"]["budget_amount"]["citations"][0]["page_number"] == 1
    assert report["project_info"]["bid_deadline"]["value"] == "2026-09-01"
    assert report["project_info"]["bid_deadline"]["citations"][0]["page_number"] == 2


@pytest.mark.anyio
async def test_forged_model_quote_is_not_a_confirmed_fact(fixture_app):
    headers = {"Authorization": "Bearer e2e-only-fictional-tender-token-00000000"}
    async with AsyncClient(
        transport=ASGITransport(app=fixture_app), base_url="http://test"
    ) as client:
        parsed = await client.post(
            "/internal/v1/parse",
            headers=headers,
            files={
                "file": (
                    "fictional.pdf",
                    _pdf("Bid deadline: 2026-09-01 E2E_FORGED_BID"),
                    "application/pdf",
                )
            },
        )
        assert parsed.status_code == 200
        analyzed = await client.post(
            "/internal/v1/analyze",
            headers=headers,
            json={"job_id": "synthetic-e2e-job", "document": parsed.json()["parsed_document"]},
        )

    assert analyzed.status_code == 200
    report = analyzed.json()
    assert report["project_info"]["budget_amount"]["value"] is None
    assert any(
        item["field_name"] == "budget_amount" and item["reason"] == "unsupported"
        for item in report["unknowns"]
    )
    assert "Invented budget" not in str(report)
