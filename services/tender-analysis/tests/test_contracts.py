"""Contract validation tests for tender-analysis.v1.

Covers: evidence citations, parsed-document consistency, fixed prompt versions,
JSON Schema generation parity, and jsonschema cross-validation.
"""

from __future__ import annotations

import json
from pathlib import Path

import pytest
from pydantic import ValidationError

FIXTURE_PATH = (
    Path(__file__).resolve().parent.parent.parent.parent
    / "contracts"
    / "fixtures"
    / "tender-analysis.v1.json"
)

SCHEMA_PATH = (
    Path(__file__).resolve().parent.parent.parent.parent
    / "contracts"
    / "tender-analysis.v1.schema.json"
)


def _load_fixture():
    return json.loads(FIXTURE_PATH.read_text(encoding="utf-8"))


# ═══════════════════════════════════════════════════════════════
# ── Fixture structural tests ──────────────────────────────────
# ═══════════════════════════════════════════════════════════════


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


# ═══════════════════════════════════════════════════════════════
# ── Extra fields (fail-closed) ────────────────────────────────
# ═══════════════════════════════════════════════════════════════


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
        Citation.model_validate(
            {
                "page_number": 1,
                "chunk_id": "page-1-chunk-0",
                "quote": "test",
                "extra_field": "nope",
            }
        )


# ═══════════════════════════════════════════════════════════════
# ── Confidence bounds ─────────────────────────────────────────
# ═══════════════════════════════════════════════════════════════


def test_confidence_rejects_below_zero():
    """Confidence < 0 must be rejected."""
    from app.contracts import EvidenceValue

    with pytest.raises(ValidationError):
        EvidenceValue.model_validate(
            {
                "value": "test",
                "confidence": -0.1,
                "requires_review": False,
                "citations": [{"page_number": 1, "chunk_id": "page-1-chunk-0", "quote": "x"}],
            }
        )


def test_confidence_rejects_above_one():
    """Confidence > 1 must be rejected."""
    from app.contracts import EvidenceValue

    with pytest.raises(ValidationError):
        EvidenceValue.model_validate(
            {
                "value": "test",
                "confidence": 1.5,
                "requires_review": False,
                "citations": [{"page_number": 1, "chunk_id": "page-1-chunk-0", "quote": "x"}],
            }
        )


def test_confidence_accepts_boundary_values():
    """Confidence at 0.0 and 1.0 must be accepted."""
    from app.contracts import EvidenceValue

    low = EvidenceValue.model_validate(
        {
            "value": None,
            "confidence": 0.0,
            "requires_review": True,
            "citations": [],
        }
    )
    assert low.confidence == 0.0

    high = EvidenceValue.model_validate(
        {
            "value": None,
            "confidence": 1.0,
            "requires_review": True,
            "citations": [],
        }
    )
    assert high.confidence == 1.0


# ═══════════════════════════════════════════════════════════════
# ── Page number bounds ────────────────────────────────────────
# ═══════════════════════════════════════════════════════════════


def test_page_number_rejects_zero():
    """Page number 0 must be rejected."""
    from app.contracts import Citation

    with pytest.raises(ValidationError):
        Citation.model_validate({"page_number": 0, "chunk_id": "page-0-chunk-0", "quote": "test"})


def test_page_number_rejects_above_1000():
    """Page number > 1000 must be rejected."""
    from app.contracts import Citation

    with pytest.raises(ValidationError):
        Citation.model_validate(
            {"page_number": 1001, "chunk_id": "page-1001-chunk-0", "quote": "test"}
        )


def test_page_number_accepts_valid():
    """Valid page numbers must be accepted."""
    from app.contracts import Citation

    c = Citation.model_validate({"page_number": 1, "chunk_id": "page-1-chunk-0", "quote": "valid"})
    assert c.page_number == 1

    c = Citation.model_validate(
        {"page_number": 1000, "chunk_id": "page-1000-chunk-0", "quote": "valid"}
    )
    assert c.page_number == 1000


# ═══════════════════════════════════════════════════════════════
# ── chunk_id pattern ──────────────────────────────────────────
# ═══════════════════════════════════════════════════════════════


def test_chunk_id_pattern_valid():
    """chunk_id must match 'page-{page}-chunk-{seq}'."""
    from app.contracts import Citation

    valid = [
        ("page-1-chunk-0", 1),
        ("page-42-chunk-7", 42),
        ("page-1000-chunk-999", 1000),
    ]
    for cid, page in valid:
        c = Citation.model_validate({"page_number": page, "chunk_id": cid, "quote": "test"})
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
            Citation.model_validate({"page_number": 1, "chunk_id": cid, "quote": "test"})


# ═══════════════════════════════════════════════════════════════
# ── Citation / DocumentChunk page ↔ chunk_id consistency ──────
# ═══════════════════════════════════════════════════════════════


def test_citation_page_must_match_chunk_id():
    """Citation.page_number must equal the page embedded in chunk_id."""
    from app.contracts import Citation

    with pytest.raises(ValidationError):
        Citation.model_validate(
            {"page_number": 5, "chunk_id": "page-3-chunk-0", "quote": "mismatch"}
        )


def test_document_chunk_page_must_match_chunk_id():
    """DocumentChunk.page_number must equal the page embedded in chunk_id."""
    from app.contracts import DocumentChunk

    with pytest.raises(ValidationError):
        DocumentChunk.model_validate(
            {"chunk_id": "page-3-chunk-0", "page_number": 7, "text": "mismatch"}
        )


# ═══════════════════════════════════════════════════════════════
# ── Enum validation ───────────────────────────────────────────
# ═══════════════════════════════════════════════════════════════


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
        "citations": [{"page_number": 1, "chunk_id": "page-1-chunk-0", "quote": "x"}],
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
        "citations": [{"page_number": 1, "chunk_id": "page-1-chunk-0", "quote": "x"}],
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


# ═══════════════════════════════════════════════════════════════
# ── Schema version literal ────────────────────────────────────
# ═══════════════════════════════════════════════════════════════


def test_schema_version_must_be_exact_literal():
    """schema_version must be exactly 'tender-analysis.v1'."""
    from app.contracts import TenderAnalysisV1

    payload = _load_fixture()
    payload["schema_version"] = "tender-analysis-v1"
    with pytest.raises(ValidationError):
        TenderAnalysisV1.model_validate(payload)


# ═══════════════════════════════════════════════════════════════
# ── Citation quote length ─────────────────────────────────────
# ═══════════════════════════════════════════════════════════════


def test_citation_quote_max_length():
    """Citation.quote is capped at 2000 characters."""
    from app.contracts import Citation

    long_quote = "x" * 2001
    with pytest.raises(ValidationError):
        Citation.model_validate(
            {"page_number": 1, "chunk_id": "page-1-chunk-0", "quote": long_quote}
        )


def test_citation_quote_min_length():
    """Citation.quote must be at least 1 character."""
    from app.contracts import Citation

    with pytest.raises(ValidationError):
        Citation.model_validate({"page_number": 1, "chunk_id": "page-1-chunk-0", "quote": ""})


# ═══════════════════════════════════════════════════════════════
# ── AnalysisMeta contract_version ─────────────────────────────
# ═══════════════════════════════════════════════════════════════


def test_analysis_meta_contract_version_exact():
    """AnalysisMeta.contract_version must be 'tender-analysis.v1'."""
    from app.contracts import AnalysisMeta

    with pytest.raises(ValidationError):
        AnalysisMeta.model_validate(
            {
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
            }
        )


# ═══════════════════════════════════════════════════════════════
# ── Missing required fields ───────────────────────────────────
# ═══════════════════════════════════════════════════════════════


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


# ═══════════════════════════════════════════════════════════════
# ── Fix 1: Evidence must carry citations ──────────────────────
# ═══════════════════════════════════════════════════════════════


def test_evidence_value_non_none_requires_citation():
    """A confirmed fact (value != None) must have at least one citation."""
    from app.contracts import EvidenceValue

    with pytest.raises(ValidationError):
        EvidenceValue.model_validate(
            {
                "value": "confirmed fact without citation",
                "confidence": 0.9,
                "requires_review": False,
                "citations": [],
            }
        )


def test_evidence_value_none_requires_review():
    """An unconfirmed fact (value is None) must have requires_review=True."""
    from app.contracts import EvidenceValue

    with pytest.raises(ValidationError):
        EvidenceValue.model_validate(
            {
                "value": None,
                "confidence": 0.5,
                "requires_review": False,
                "citations": [],
            }
        )


def test_evidence_value_none_may_have_empty_citations():
    """Unconfirmed fact with requires_review=True accepts empty citations."""
    from app.contracts import EvidenceValue

    ev = EvidenceValue.model_validate(
        {
            "value": None,
            "confidence": 0.5,
            "requires_review": True,
            "citations": [],
        }
    )
    assert ev.value is None
    assert ev.requires_review is True


def test_qualification_must_have_at_least_one_citation():
    """Qualification.citations must not be empty."""
    from app.contracts import Qualification

    with pytest.raises(ValidationError):
        Qualification.model_validate(
            {
                "kind": "hard_qualification",
                "requirement": "some requirement",
                "original_text": "original text",
                "confidence": 0.9,
                "requires_review": False,
                "citations": [],
            }
        )


def test_risk_must_have_at_least_one_citation():
    """Risk.citations must not be empty — the fact must be traceable."""
    from app.contracts import Risk

    with pytest.raises(ValidationError):
        Risk.model_validate(
            {
                "severity": "high",
                "fact": "a fact",
                "risk_judgment": "a judgment",
                "impact": "an impact",
                "suggestion": "a suggestion",
                "confidence": 0.8,
                "requires_review": False,
                "citations": [],
            }
        )


def test_material_must_have_at_least_one_citation():
    """RequiredMaterial.citations must not be empty."""
    from app.contracts import RequiredMaterial

    with pytest.raises(ValidationError):
        RequiredMaterial.model_validate(
            {
                "name": "some material",
                "description": "desc",
                "source_requirement": "source",
                "confidence": 0.9,
                "requires_review": False,
                "citations": [],
            }
        )


def test_unknown_missing_allows_empty_citations():
    """UnknownItem with reason='missing' may have empty citations."""
    from app.contracts import UnknownItem

    u = UnknownItem.model_validate(
        {
            "field_name": "field",
            "reason": "missing",
            "detail": "not found",
            "citations": [],
        }
    )
    assert u.citations == []


# ═══════════════════════════════════════════════════════════════
# ── Fix 2: ParsedDocument consistency ─────────────────────────
# ═══════════════════════════════════════════════════════════════


def _make_citation(page: int, chunk_idx: int = 0) -> dict:
    return {
        "page_number": page,
        "chunk_id": f"page-{page}-chunk-{chunk_idx}",
        "quote": f"text on page {page}",
    }


def _make_page(page_number: int, text: str = "") -> dict:
    return {
        "page_number": page_number,
        "text": text or f"Page {page_number} content",
        "chunks": [
            {
                "chunk_id": f"page-{page_number}-chunk-0",
                "page_number": page_number,
                "text": text or f"Page {page_number} content",
            }
        ],
    }


def test_parsed_document_page_count_at_least_one():
    """ParsedDocument.page_count must be >= 1."""
    from app.contracts import ParsedDocument

    with pytest.raises(ValidationError):
        ParsedDocument.model_validate({"page_count": 0, "pages": []})


def test_parsed_document_pages_length_must_equal_page_count():
    """pages array length must equal page_count."""
    from app.contracts import ParsedDocument

    with pytest.raises(ValidationError):
        ParsedDocument.model_validate({"page_count": 2, "pages": [_make_page(1)]})


def test_parsed_document_pages_must_be_sequential():
    """Pages must be sequential starting from 1, no gaps or duplicates."""
    from app.contracts import ParsedDocument

    # Gap: pages 1 and 3 but page_count=2
    with pytest.raises(ValidationError):
        ParsedDocument.model_validate(
            {
                "page_count": 2,
                "pages": [_make_page(1), _make_page(3)],
            }
        )

    # Wrong order
    with pytest.raises(ValidationError):
        ParsedDocument.model_validate(
            {
                "page_count": 2,
                "pages": [_make_page(2), _make_page(1)],
            }
        )

    # Duplicate
    with pytest.raises(ValidationError):
        ParsedDocument.model_validate(
            {
                "page_count": 2,
                "pages": [_make_page(1), _make_page(1)],
            }
        )

    # Not starting at 1
    with pytest.raises(ValidationError):
        ParsedDocument.model_validate(
            {
                "page_count": 2,
                "pages": [_make_page(2), _make_page(3)],
            }
        )


def test_parsed_document_valid_sequential():
    """A valid sequential document must be accepted."""
    from app.contracts import ParsedDocument

    doc = ParsedDocument.model_validate(
        {"page_count": 3, "pages": [_make_page(1), _make_page(2), _make_page(3)]}
    )
    assert doc.page_count == 3


def test_parsed_page_chunks_must_belong_to_page():
    """Every chunk in a ParsedPage must have that page's page_number."""
    from app.contracts import ParsedPage

    with pytest.raises(ValidationError):
        ParsedPage.model_validate(
            {
                "page_number": 1,
                "text": "page 1",
                "chunks": [
                    {
                        "chunk_id": "page-2-chunk-0",
                        "page_number": 2,
                        "text": "wrong page",
                    }
                ],
            }
        )


def test_parse_response_completed_with_document():
    """Completed parse must carry parsed_document and no error_code."""
    from app.contracts import ParseResponse

    doc = {
        "page_count": 1,
        "pages": [_make_page(1)],
    }
    r = ParseResponse.model_validate({"status": "completed", "parsed_document": doc})
    assert r.status == "completed"
    assert r.parsed_document is not None
    assert r.error_code is None


def test_parse_response_failed_with_error_code():
    """Failed parse must carry an approved error_code and no document."""
    from app.contracts import ParseResponse

    for code in [
        "invalid_pdf",
        "encrypted_pdf",
        "too_many_pages",
        "extracted_text_too_large",
        "ocr_required",
    ]:
        r = ParseResponse.model_validate({"status": "failed", "error_code": code})
        assert r.status == "failed"
        assert r.error_code == code
        assert r.parsed_document is None


def test_parse_response_completed_with_error_code_rejected():
    """Completed status must not carry error_code."""
    from app.contracts import ParseResponse

    doc = {"page_count": 1, "pages": [_make_page(1)]}
    with pytest.raises(ValidationError):
        ParseResponse.model_validate(
            {"status": "completed", "parsed_document": doc, "error_code": "invalid_pdf"}
        )


def test_parse_response_failed_without_error_code_rejected():
    """Failed status must carry an error_code."""
    from app.contracts import ParseResponse

    with pytest.raises(ValidationError):
        ParseResponse.model_validate({"status": "failed"})


def test_parse_response_failed_with_document_rejected():
    """Failed status must not carry parsed_document."""
    from app.contracts import ParseResponse

    doc = {"page_count": 1, "pages": [_make_page(1)]}
    with pytest.raises(ValidationError):
        ParseResponse.model_validate(
            {"status": "failed", "error_code": "invalid_pdf", "parsed_document": doc}
        )


def test_parse_response_unknown_error_code_rejected():
    """Only approved error codes are accepted for failed status."""
    from app.contracts import ParseResponse

    with pytest.raises(ValidationError):
        ParseResponse.model_validate({"status": "failed", "error_code": "unknown_code"})


def test_parse_response_invalid_status_rejected():
    """Status must be 'completed' or 'failed'."""
    from app.contracts import ParseResponse

    with pytest.raises(ValidationError):
        ParseResponse.model_validate({"status": "pending"})


# ═══════════════════════════════════════════════════════════════
# ── Fix 3: Fixed prompt versions ──────────────────────────────
# ═══════════════════════════════════════════════════════════════


def test_prompt_versions_must_be_exact_literals():
    """PromptVersions fields must be the exact five version strings."""
    from app.contracts import PromptVersions

    # Valid
    pv = PromptVersions.model_validate(
        {
            "project_info": "tender-project-info-v1",
            "qualifications": "tender-qualifications-v1",
            "risks": "tender-risks-v1",
            "materials": "tender-materials-v1",
            "review": "tender-review-v1",
        }
    )
    assert pv.project_info == "tender-project-info-v1"

    # Wrong project_info
    with pytest.raises(ValidationError):
        PromptVersions.model_validate(
            {
                "project_info": "tender-project-info-v2",
                "qualifications": "tender-qualifications-v1",
                "risks": "tender-risks-v1",
                "materials": "tender-materials-v1",
                "review": "tender-review-v1",
            }
        )

    # Arbitrary string
    with pytest.raises(ValidationError):
        PromptVersions.model_validate(
            {
                "project_info": "anything",
                "qualifications": "tender-qualifications-v1",
                "risks": "tender-risks-v1",
                "materials": "tender-materials-v1",
                "review": "tender-review-v1",
            }
        )


def test_completeness_must_be_complete_or_partial():
    """AnalysisMeta.completeness must be 'complete' or 'partial'."""
    from app.contracts import AnalysisMeta

    base = {
        "model_provider": "openai",
        "model_name": "gpt-4o",
        "prompt_versions": {
            "project_info": "tender-project-info-v1",
            "qualifications": "tender-qualifications-v1",
            "risks": "tender-risks-v1",
            "materials": "tender-materials-v1",
            "review": "tender-review-v1",
        },
        "contract_version": "tender-analysis.v1",
    }

    for c in ["complete", "partial"]:
        m = AnalysisMeta.model_validate({**base, "completeness": c})
        assert m.completeness == c

    with pytest.raises(ValidationError):
        AnalysisMeta.model_validate({**base, "completeness": "in_progress"})


# ═══════════════════════════════════════════════════════════════
# ── Fix 4: JSON Schema parity ─────────────────────────────────
# ═══════════════════════════════════════════════════════════════


def test_json_schema_equals_committed_file():
    """The committed schema file must equal the Pydantic-generated schema."""
    from app.contracts import generate_tender_analysis_schema

    generated = generate_tender_analysis_schema()
    committed = json.loads(SCHEMA_PATH.read_text(encoding="utf-8"))
    assert generated == committed, (
        "JSON Schema drift: regenerate with generate_tender_analysis_schema()"
    )


def test_fixture_validates_against_json_schema():
    """The fixture must pass jsonschema validation."""
    import jsonschema

    from app.contracts import generate_tender_analysis_schema

    schema = generate_tender_analysis_schema()
    payload = _load_fixture()
    jsonschema.validate(payload, schema)


def test_qualification_without_citation_rejected_by_jsonschema():
    """jsonschema must reject a qualification with empty citations."""
    import jsonschema

    from app.contracts import generate_tender_analysis_schema

    schema = generate_tender_analysis_schema()
    payload = _load_fixture()
    payload["qualifications"][0]["citations"] = []
    with pytest.raises(jsonschema.ValidationError):
        jsonschema.validate(payload, schema)


def test_risk_without_citation_rejected_by_jsonschema():
    """jsonschema must reject a risk with empty citations."""
    import jsonschema

    from app.contracts import generate_tender_analysis_schema

    schema = generate_tender_analysis_schema()
    payload = _load_fixture()
    payload["risks"][0]["citations"] = []
    with pytest.raises(jsonschema.ValidationError):
        jsonschema.validate(payload, schema)


def test_material_without_citation_rejected_by_jsonschema():
    """jsonschema must reject a material with empty citations."""
    import jsonschema

    from app.contracts import generate_tender_analysis_schema

    schema = generate_tender_analysis_schema()
    payload = _load_fixture()
    payload["required_materials"][0]["citations"] = []
    with pytest.raises(jsonschema.ValidationError):
        jsonschema.validate(payload, schema)


def test_evidence_non_null_empty_citations_rejected_by_both():
    """A confirmed EvidenceValue with empty citations must fail Pydantic and jsonschema."""
    import jsonschema

    from app.contracts import TenderAnalysisV1, generate_tender_analysis_schema

    payload = _load_fixture()
    payload["project_info"]["project_name"]["citations"] = []

    with pytest.raises(ValidationError):
        TenderAnalysisV1.model_validate(payload)

    with pytest.raises(jsonschema.ValidationError):
        jsonschema.validate(payload, generate_tender_analysis_schema())


def test_evidence_null_empty_citations_accepted_by_both():
    """An unconfirmed EvidenceValue with empty citations must pass both validators."""
    import jsonschema

    from app.contracts import TenderAnalysisV1, generate_tender_analysis_schema

    payload = _load_fixture()
    payload["project_info"]["project_name"]["value"] = None
    payload["project_info"]["project_name"]["requires_review"] = True
    payload["project_info"]["project_name"]["citations"] = []

    TenderAnalysisV1.model_validate(payload)
    jsonschema.validate(payload, generate_tender_analysis_schema())


def test_evidence_null_false_review_rejected_by_both():
    """A null EvidenceValue with requires_review=False must fail Pydantic and jsonschema."""
    import jsonschema

    from app.contracts import TenderAnalysisV1, generate_tender_analysis_schema

    payload = _load_fixture()
    payload["project_info"]["project_name"]["value"] = None
    payload["project_info"]["project_name"]["requires_review"] = False
    payload["project_info"]["project_name"]["citations"] = []

    with pytest.raises(ValidationError):
        TenderAnalysisV1.model_validate(payload)

    with pytest.raises(jsonschema.ValidationError):
        jsonschema.validate(payload, generate_tender_analysis_schema())


def test_wrong_prompt_version_rejected_by_jsonschema():
    """jsonschema must reject arbitrary prompt version strings."""
    import jsonschema

    from app.contracts import generate_tender_analysis_schema

    schema = generate_tender_analysis_schema()
    payload = _load_fixture()
    payload["analysis_meta"]["prompt_versions"]["project_info"] = "wrong-version"
    with pytest.raises(jsonschema.ValidationError):
        jsonschema.validate(payload, schema)


def test_json_schema_only_contains_report_types():
    """Public schema must not leak internal types (ParseResponse etc.)."""
    from app.contracts import generate_tender_analysis_schema

    schema = generate_tender_analysis_schema()
    defs = schema.get("$defs", {})
    internal_types = {
        "ParseResponse",
        "ParsedDocument",
        "ParsedPage",
        "DocumentChunk",
        "AnalyzeRequest",
    }
    for t in internal_types:
        assert t not in defs, f"Internal type {t} must not appear in public schema"
