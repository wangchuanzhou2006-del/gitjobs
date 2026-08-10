"""Pydantic request/result types for tender-analysis.v1.

All models use ConfigDict(extra="forbid").  Every key fact, qualification, risk,
and material must include at least one citation.
"""

from __future__ import annotations

from typing import Literal

from pydantic import BaseModel, ConfigDict, Field

# ── Leaf types ─────────────────────────────────────────────────


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


# ── Parsing types ─────────────────────────────────────────────


class DocumentChunk(BaseModel):
    model_config = ConfigDict(extra="forbid")

    chunk_id: str = Field(pattern=r"^page-[1-9][0-9]*-chunk-[0-9]+$")
    page_number: int = Field(ge=1, le=1000)
    text: str = Field(min_length=1)


class ParsedPage(BaseModel):
    model_config = ConfigDict(extra="forbid")

    page_number: int = Field(ge=1, le=1000)
    text: str
    chunks: list[DocumentChunk]


class ParsedDocument(BaseModel):
    model_config = ConfigDict(extra="forbid")

    page_count: int = Field(ge=0, le=1000)
    pages: list[ParsedPage]


class ParseResponse(BaseModel):
    model_config = ConfigDict(extra="forbid")

    status: str
    parsed_document: ParsedDocument | None = None
    error_code: str | None = None


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
    citations: list[Citation] = Field(max_length=20)


class Risk(BaseModel):
    model_config = ConfigDict(extra="forbid")

    severity: Literal["low", "medium", "high"]
    fact: str = Field(min_length=1)
    risk_judgment: str = Field(min_length=1)
    impact: str
    suggestion: str
    confidence: float = Field(ge=0, le=1)
    requires_review: bool
    citations: list[Citation] = Field(max_length=20)


class RequiredMaterial(BaseModel):
    model_config = ConfigDict(extra="forbid")

    name: str = Field(min_length=1)
    description: str
    source_requirement: str
    confidence: float = Field(ge=0, le=1)
    requires_review: bool
    citations: list[Citation] = Field(max_length=20)


class UnknownItem(BaseModel):
    model_config = ConfigDict(extra="forbid")

    field_name: str = Field(min_length=1)
    reason: Literal["missing", "conflict", "low_confidence", "unsupported"]
    detail: str
    citations: list[Citation] = Field(max_length=20)


class AnalysisMeta(BaseModel):
    model_config = ConfigDict(extra="forbid")

    model_provider: str
    model_name: str
    prompt_versions: PromptVersions
    contract_version: Literal["tender-analysis.v1"]
    completeness: str


class PromptVersions(BaseModel):
    model_config = ConfigDict(extra="forbid")

    project_info: str
    qualifications: str
    risks: str
    materials: str
    review: str


class TenderAnalysisV1(BaseModel):
    model_config = ConfigDict(extra="forbid")

    schema_version: Literal["tender-analysis.v1"]
    project_info: ProjectInfo
    qualifications: list[Qualification] = Field(max_length=200)
    risks: list[Risk] = Field(max_length=200)
    required_materials: list[RequiredMaterial] = Field(max_length=300)
    unknowns: list[UnknownItem] = Field(max_length=200)
    analysis_meta: AnalysisMeta
