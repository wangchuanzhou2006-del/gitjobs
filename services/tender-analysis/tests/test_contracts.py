"""Contract validation tests for tender-analysis.v1."""

from __future__ import annotations

import json
from pathlib import Path

import pytest
from pydantic import ValidationError

FIXTURE_PATH = (
    Path(__file__).resolve().parent.parent.parent.parent
    / "contracts" / "fixtures" / "tender-analysis.v1.json"
)


def _load_fixture():
    return json.loads(FIXTURE_PATH.read_text(encoding="utf-8"))


# ── Fixture structural tests ──────────────────────────────────


def test_fixture_is_valid_json():
    """The fixture must parse as valid JSON."""
    data = _load_fixture()
    assert isinstance(data, dict)
    assert "schema_version" in data


def test_fixture_matches_v1_contract():
    """The fixture must validate against TenderAnalysisV1."""
    from app.contracts import TenderAnalysisV1

    payload = _load_fixture()
    report = TenderAnalysisV1.model_validate(payload)
    assert report.schema_version == "tender-analysis.v1"
    assert report.project_info.project_name.value == "某市第三中学食堂承包服务项目"
    assert report.project_info.project_name.citations[0].page_number == 1


def test_fixture_schema_version_exact():
    """schema_version must be exactly 'tender-analysis.v1'."""
    from app.contracts import TenderAnalysisV1

    payload = _load_fixture()
    payload["schema_version"] = "tender-analysis.v2"
    with pytest.raises(ValidationError):
        TenderAnalysisV1.model_validate(payload)


# ── Extra fields (fail-closed) ────────────────────────────────


def test_unknown_top_level_fields_fail_closed():
    """Unknown top-level keys must be rejected."""
    from app.contracts import TenderAnalysisV1

    payload = _load_fixture()
    payload["surprise"] = True
    with pytest.raises(ValidationError):
        TenderAnalysisV1.model_validate(payload)


def test_unknown_nested_fields_fail_closed():
    """Unknown keys inside nested models must be rejected."""
    from app.contracts import Citation

    with pytest.raises(ValidationError):
        Citation.model_validate({
            "page_number": 1,
            "chunk_id": "page-1-chunk-0",
            "quote": "test",
            "extra_field": "nope",
        })


# ── Confidence bounds ─────────────────────────────────────────


def test_confidence_rejects_below_zero():
    """Confidence < 0 must be rejected."""
    from app.contracts import EvidenceValue

    with pytest.raises(ValidationError):
        EvidenceValue.model_validate({
            "value": "test",
            "confidence": -0.1,
            "requires_review": False,
            "citations": [],
        })


def test_confidence_rejects_above_one():
    """Confidence > 1 must be rejected."""
    from app.contracts import EvidenceValue

    with pytest.raises(ValidationError):
        EvidenceValue.model_validate({
            "value": "test",
            "confidence": 1.5,
            "requires_review": False,
            "citations": [],
        })


def test_confidence_accepts_boundary_values():
    """Confidence at 0.0 and 1.0 must be accepted."""
    from app.contracts import EvidenceValue

    low = EvidenceValue.model_validate({
        "value": None,
        "confidence": 0.0,
        "requires_review": True,
        "citations": [],
    })
    assert low.confidence == 0.0

    high = EvidenceValue.model_validate({
        "value": None,
        "confidence": 1.0,
        "requires_review": True,
        "citations": [],
    })
    assert high.confidence == 1.0


# ── Page number bounds ────────────────────────────────────────


def test_page_number_rejects_zero():
    """Page number 0 must be rejected."""
    from app.contracts import Citation

    with pytest.raises(ValidationError):
        Citation.model_validate({
            "page_number": 0,
            "chunk_id": "page-0-chunk-0",
            "quote": "test",
        })


def test_page_number_rejects_above_1000():
    """Page number > 1000 must be rejected."""
    from app.contracts import Citation

    with pytest.raises(ValidationError):
        Citation.model_validate({
            "page_number": 1001,
            "chunk_id": "page-1001-chunk-0",
            "quote": "test",
        })


def test_page_number_accepts_valid():
    """Valid page numbers must be accepted."""
    from app.contracts import Citation

    c = Citation.model_validate({
        "page_number": 1,
        "chunk_id": "page-1-chunk-0",
        "quote": "valid",
    })
    assert c.page_number == 1

    c = Citation.model_validate({
        "page_number": 1000,
        "chunk_id": "page-1000-chunk-0",
        "quote": "valid",
    })
    assert c.page_number == 1000


# ── chunk_id pattern ──────────────────────────────────────────


def test_chunk_id_pattern_valid():
    """chunk_id must match 'page-{page}-chunk-{seq}'."""
    from app.contracts import Citation

    valid_ids = [
        "page-1-chunk-0",
        "page-42-chunk-7",
        "page-1000-chunk-999",
    ]
    for cid in valid_ids:
        c = Citation.model_validate({
            "page_number": 1,
            "chunk_id": cid,
            "quote": "test",
        })
        assert c.chunk_id == cid


def test_chunk_id_pattern_invalid():
    """Invalid chunk_id formats must be rejected."""
    from app.contracts import Citation

    invalid_ids = [
        "page-0-chunk-0",
        "chunk-1",
        "page-1-chunk",
        "page-one-chunk-0",
        "",
    ]
    for cid in invalid_ids:
        with pytest.raises(ValidationError):
            Citation.model_validate({
                "page_number": 1,
                "chunk_id": cid,
                "quote": "test",
            })


# ── Enum validation ───────────────────────────────────────────


def test_qualification_kind_enum():
    """Qualification.kind must be one of the five allowed values."""
    from app.contracts import Qualification

    valid = [
        "hard_qualification",
        "scoring_item",
        "submission_material",
        "contract_performance",
        "general_note",
    ]
    base = {
        "requirement": "test requirement",
        "original_text": "original text",
        "confidence": 0.9,
        "requires_review": False,
        "citations": [],
    }
    for kind in valid:
        q = Qualification.model_validate({**base, "kind": kind})
        assert q.kind == kind

    with pytest.raises(ValidationError):
        Qualification.model_validate({**base, "kind": "invalid_kind"})


def test_risk_severity_enum():
    """Risk.severity must be low/medium/high."""
    from app.contracts import Risk

    base = {
        "fact": "a fact",
        "risk_judgment": "a judgment",
        "impact": "an impact",
        "suggestion": "a suggestion",
        "confidence": 0.8,
        "requires_review": False,
        "citations": [],
    }
    for sev in ["low", "medium", "high"]:
        r = Risk.model_validate({**base, "severity": sev})
        assert r.severity == sev

    with pytest.raises(ValidationError):
        Risk.model_validate({**base, "severity": "critical"})


def test_unknown_reason_enum():
    """UnknownItem.reason must be missing/conflict/low_confidence/unsupported."""
    from app.contracts import UnknownItem

    base = {
        "field_name": "field",
        "detail": "some detail",
        "citations": [],
    }
    for reason in ["missing", "conflict", "low_confidence", "unsupported"]:
        u = UnknownItem.model_validate({**base, "reason": reason})
        assert u.reason == reason

    with pytest.raises(ValidationError):
        UnknownItem.model_validate({**base, "reason": "unknown"})


# ── Schema version literal ────────────────────────────────────


def test_schema_version_must_be_exact_literal():
    """schema_version must be exactly 'tender-analysis.v1'."""
    from app.contracts import TenderAnalysisV1

    payload = _load_fixture()
    payload["schema_version"] = "tender-analysis-v1"
    with pytest.raises(ValidationError):
        TenderAnalysisV1.model_validate(payload)


# ── Citation quote length ─────────────────────────────────────


def test_citation_quote_max_length():
    """Citation.quote is capped at 2000 characters."""
    from app.contracts import Citation

    long_quote = "x" * 2001
    with pytest.raises(ValidationError):
        Citation.model_validate({
            "page_number": 1,
            "chunk_id": "page-1-chunk-0",
            "quote": long_quote,
        })


def test_citation_quote_min_length():
    """Citation.quote must be at least 1 character."""
    from app.contracts import Citation

    with pytest.raises(ValidationError):
        Citation.model_validate({
            "page_number": 1,
            "chunk_id": "page-1-chunk-0",
            "quote": "",
        })


# ── AnalysisMeta contract_version ─────────────────────────────


def test_analysis_meta_contract_version_exact():
    """AnalysisMeta.contract_version must be 'tender-analysis.v1'."""
    from app.contracts import AnalysisMeta

    with pytest.raises(ValidationError):
        AnalysisMeta.model_validate({
            "model_provider": "openai",
            "model_name": "gpt-4o",
            "prompt_versions": {
                "project_info": "tender-project-info-v1",
                "qualifications": "tender-qualifications-v1",
                "risks": "tender-risks-v1",
                "materials": "tender-materials-v1",
                "review": "tender-review-v1",
            },
            "contract_version": "tender-analysis.v2",
            "completeness": "complete",
        })


# ── Missing required fields ───────────────────────────────────


def test_citation_missing_required():
    """Citation must reject missing required fields."""
    from app.contracts import Citation

    with pytest.raises(ValidationError):
        Citation.model_validate({"page_number": 1})


def test_tender_analysis_missing_required():
    """TenderAnalysisV1 must reject missing top-level fields."""
    from app.contracts import TenderAnalysisV1

    with pytest.raises(ValidationError):
        TenderAnalysisV1.model_validate({"schema_version": "tender-analysis.v1"})
