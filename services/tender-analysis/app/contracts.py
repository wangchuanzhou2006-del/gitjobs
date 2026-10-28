"""Pydantic request/result types for tender-analysis.v1.

All models use ConfigDict(extra="forbid").  Every key fact, qualification, risk,
and material must include at least one citation.
"""

from __future__ import annotations

import re
from typing import Literal

from pydantic import BaseModel, ConfigDict, Field, model_validator

# ── Helpers ────────────────────────────────────────────────────

_PAGE_CHUNK_RE = re.compile(r"^page-([1-9][0-9]*)-chunk-[0-9]+$")

_PARSE_ERROR_CODES = Literal[
    "invalid_pdf",
    "encrypted_pdf",
    "too_many_pages",
    "extracted_text_too_large",
    "ocr_required",
]


def _extract_page_from_chunk_id(chunk_id: str) -> int | None:
    m = _PAGE_CHUNK_RE.match(chunk_id)
    return int(m.group(1)) if m else None


# ── Leaf types ─────────────────────────────────────────────────


class Citation(BaseModel):
    model_config = ConfigDict(extra="forbid")

    page_number: int = Field(ge=1, le=1000)
    chunk_id: str = Field(pattern=r"^page-[1-9][0-9]*-chunk-[0-9]+$")
    quote: str = Field(min_length=1, max_length=2000)

    @model_validator(mode="after")
    def _check_page_consistency(self) -> Citation:
        extracted = _extract_page_from_chunk_id(self.chunk_id)
        if extracted is not None and extracted != self.page_number:
            raise ValueError(f"page_number {self.page_number} must match chunk_id page {extracted}")
        return self


class EvidenceValue(BaseModel):
    model_config = ConfigDict(extra="forbid")

    value: str | None
    confidence: float = Field(ge=0, le=1)
    requires_review: bool
    citations: list[Citation] = Field(max_length=20)

    @model_validator(mode="after")
    def _check_citation_evidence(self) -> EvidenceValue:
        if self.value is not None and len(self.citations) == 0:
            raise ValueError("confirmed facts require at least one citation")
        if self.value is None and not self.requires_review:
            raise ValueError("unconfirmed facts must have requires_review=True")
        return self


# ── Parsing types ─────────────────────────────────────────────


class DocumentChunk(BaseModel):
    model_config = ConfigDict(extra="forbid")

    chunk_id: str = Field(pattern=r"^page-[1-9][0-9]*-chunk-[0-9]+$")
    page_number: int = Field(ge=1, le=1000)
    text: str = Field(min_length=1)

    @model_validator(mode="after")
    def _check_page_consistency(self) -> DocumentChunk:
        extracted = _extract_page_from_chunk_id(self.chunk_id)
        if extracted is not None and extracted != self.page_number:
            raise ValueError(f"page_number {self.page_number} must match chunk_id page {extracted}")
        return self


class ParsedPage(BaseModel):
    model_config = ConfigDict(extra="forbid")

    page_number: int = Field(ge=1, le=1000)
    text: str
    chunks: list[DocumentChunk]

    @model_validator(mode="after")
    def _check_chunks_belong(self) -> ParsedPage:
        for i, chunk in enumerate(self.chunks):
            if chunk.page_number != self.page_number:
                raise ValueError(
                    f"chunk[{i}] page_number {chunk.page_number} "
                    f"must match parent page {self.page_number}"
                )
        return self


class ParsedDocument(BaseModel):
    model_config = ConfigDict(extra="forbid")

    page_count: int = Field(ge=1, le=1000)
    pages: list[ParsedPage]

    @model_validator(mode="after")
    def _check_page_sequence(self) -> ParsedDocument:
        if len(self.pages) != self.page_count:
            raise ValueError(
                f"pages length {len(self.pages)} must equal page_count {self.page_count}"
            )
        expected = list(range(1, self.page_count + 1))
        actual = [p.page_number for p in self.pages]
        if actual != expected:
            raise ValueError(f"pages must be sequential starting from 1, got {actual}")
        return self


class ParseResponse(BaseModel):
    model_config = ConfigDict(extra="forbid")

    status: Literal["completed", "failed"]
    parsed_document: ParsedDocument | None = None
    error_code: _PARSE_ERROR_CODES | None = None

    @model_validator(mode="after")
    def _check_status_consistency(self) -> ParseResponse:
        if self.status == "completed":
            if self.parsed_document is None:
                raise ValueError("completed status requires parsed_document")
            if self.error_code is not None:
                raise ValueError("completed status must not have error_code")
        elif self.status == "failed":
            if self.error_code is None:
                raise ValueError("failed status requires error_code")
            if self.parsed_document is not None:
                raise ValueError("failed status must not have parsed_document")
        return self


class AnalyzeRequest(BaseModel):
    model_config = ConfigDict(extra="forbid")

    job_id: str = Field(min_length=1)
    document: ParsedDocument


# ── Report types ──────────────────────────────────────────────


class ProjectInfo(BaseModel):
    model_config = ConfigDict(extra="forbid")

    project_name: EvidenceValue
    procurement_entity: EvidenceValue
    budget_amount: EvidenceValue
    budget_currency: EvidenceValue
    service_period: EvidenceValue
    bid_deadline: EvidenceValue


class Qualification(BaseModel):
    model_config = ConfigDict(extra="forbid")

    kind: Literal[
        "hard_qualification",
        "scoring_item",
        "submission_material",
        "contract_performance",
        "general_note",
    ]
    requirement: str = Field(min_length=1)
    original_text: str = Field(min_length=1)
    confidence: float = Field(ge=0, le=1)
    requires_review: bool
    citations: list[Citation] = Field(min_length=1, max_length=20)


class Risk(BaseModel):
    model_config = ConfigDict(extra="forbid")

    severity: Literal["low", "medium", "high"]
    fact: str = Field(min_length=1)
    risk_judgment: str = Field(min_length=1)
    impact: str
    suggestion: str
    confidence: float = Field(ge=0, le=1)
    requires_review: bool
    citations: list[Citation] = Field(min_length=1, max_length=20)


class RequiredMaterial(BaseModel):
    model_config = ConfigDict(extra="forbid")

    name: str = Field(min_length=1)
    description: str
    source_requirement: str
    confidence: float = Field(ge=0, le=1)
    requires_review: bool
    citations: list[Citation] = Field(min_length=1, max_length=20)


class UnknownItem(BaseModel):
    model_config = ConfigDict(extra="forbid")

    field_name: str = Field(min_length=1)
    reason: Literal["missing", "conflict", "low_confidence", "unsupported"]
    detail: str
    citations: list[Citation] = Field(max_length=20)


class PromptVersions(BaseModel):
    model_config = ConfigDict(extra="forbid")

    project_info: Literal["tender-project-info-v1"]
    qualifications: Literal["tender-qualifications-v1"]
    risks: Literal["tender-risks-v1"]
    materials: Literal["tender-materials-v1"]
    review: Literal["tender-review-v1"]


class AnalysisMeta(BaseModel):
    model_config = ConfigDict(extra="forbid")

    model_provider: str
    model_name: str
    prompt_versions: PromptVersions
    contract_version: Literal["tender-analysis.v1"]
    completeness: Literal["complete", "partial"]


class TenderAnalysisV1(BaseModel):
    model_config = ConfigDict(extra="forbid")

    schema_version: Literal["tender-analysis.v1"]
    project_info: ProjectInfo
    qualifications: list[Qualification] = Field(max_length=200)
    risks: list[Risk] = Field(max_length=200)
    required_materials: list[RequiredMaterial] = Field(max_length=300)
    unknowns: list[UnknownItem] = Field(max_length=200)
    analysis_meta: AnalysisMeta


# ── JSON Schema generation ────────────────────────────────────


def generate_tender_analysis_schema() -> dict:
    """Generate canonical JSON Schema from TenderAnalysisV1 Pydantic model.

    Uses model_json_schema(mode="validation") as the single source of truth.
    Only the public report root type and its $defs are included.
    """
    schema = TenderAnalysisV1.model_json_schema(mode="validation")
    schema["$schema"] = "https://json-schema.org/draft/2020-12/schema"
    schema["$id"] = "https://gitjobs.dev/contracts/tender-analysis.v1.schema.json"
    schema["title"] = "Tender Analysis v1"
    schema["description"] = "Canonical public result contract for AI tender document analysis v1."
    _require_evidence_citation_when_confirmed(schema)
    return schema


def _require_evidence_citation_when_confirmed(schema: dict) -> None:
    """Encode the EvidenceValue cross-field rule in the public schema.

    ``EvidenceValue._check_citation_evidence`` rejects a non-null ``value`` with
    no citations, but ``model_json_schema`` cannot express a model validator.
    Add the equivalent ``if``/``then`` condition so Pydantic and jsonschema
    reject the same payloads.
    """
    schema["$defs"]["EvidenceValue"]["allOf"] = [
        {
            "if": {
                "properties": {"value": {"not": {"type": "null"}}},
                "required": ["value"],
            },
            "then": {"properties": {"citations": {"minItems": 1}}},
        }
    ]
