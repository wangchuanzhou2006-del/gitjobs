"""Four-stage extraction plus review, with deterministic citation validation."""

from __future__ import annotations

import json
from typing import Literal

from pydantic import BaseModel, ConfigDict, Field

from app.contracts import (
    AnalysisMeta,
    Citation,
    DocumentChunk,
    EvidenceValue,
    ParsedDocument,
    ProjectInfo,
    PromptVersions,
    Qualification,
    RequiredMaterial,
    Risk,
    TenderAnalysisV1,
    UnknownItem,
)
from app.llm import LlmProvider
from app.prompts import (
    MATERIALS_PROMPT,
    PROJECT_INFO_PROMPT,
    QUALIFICATIONS_PROMPT,
    REVIEW_PROMPT,
    RISKS_PROMPT,
)
from app.retrieval import QUERY_PACKS, retrieve_chunks

# ── Intermediate extraction/review models ──────────────────────


class QualificationList(BaseModel):
    model_config = ConfigDict(extra="forbid")

    items: list[Qualification] = Field(max_length=200)


class RiskList(BaseModel):
    model_config = ConfigDict(extra="forbid")

    items: list[Risk] = Field(max_length=200)


class MaterialList(BaseModel):
    model_config = ConfigDict(extra="forbid")

    items: list[RequiredMaterial] = Field(max_length=300)


class ReviewAction(BaseModel):
    model_config = ConfigDict(extra="forbid")

    stage: Literal["project_info", "qualifications", "risks", "materials"]
    index: int | None = None
    field: str | None = None
    action: Literal["drop", "demote"]
    demote_reason: Literal["missing", "conflict", "low_confidence", "unsupported"] | None = None
    demote_detail: str = ""


class ReviewResult(BaseModel):
    model_config = ConfigDict(extra="forbid")

    actions: list[ReviewAction] = Field(default_factory=list, max_length=500)
    unknowns: list[UnknownItem] = Field(default_factory=list, max_length=200)


# ── Helpers ────────────────────────────────────────────────────


def _build_context(chunks: list[DocumentChunk]) -> str:
    return "\n\n".join(f"[{c.chunk_id} | page {c.page_number}]\n{c.text}" for c in chunks)


def _build_review_context(
    project_info: ProjectInfo,
    qualifications: list[Qualification],
    risks: list[Risk],
    materials: list[RequiredMaterial],
) -> str:
    return json.dumps(
        {
            "project_info": project_info.model_dump(),
            "qualifications": [q.model_dump() for q in qualifications],
            "risks": [r.model_dump() for r in risks],
            "materials": [m.model_dump() for m in materials],
        },
        ensure_ascii=False,
    )


def _has_valid_citation(citations: list[Citation], chunk_index: dict[str, DocumentChunk]) -> bool:
    for citation in citations:
        chunk = chunk_index.get(citation.chunk_id)
        if chunk is None:
            continue
        if chunk.page_number != citation.page_number:
            continue
        if citation.quote not in chunk.text:
            continue
        return True
    return False


def _unknown_for_item(field_name: str, item) -> UnknownItem:
    return UnknownItem(
        field_name=field_name,
        reason="unsupported",
        detail="结论引用无效或缺失",
        citations=item.citations,
    )


def _validate_list(items, chunk_index, field_prefix, unknowns):
    kept = []
    for item in items:
        if _has_valid_citation(item.citations, chunk_index):
            kept.append(item)
        else:
            unknowns.append(_unknown_for_item(field_prefix, item))
    return kept


def _apply_review_action(action, pi_fields, qual_items, risk_items, material_items, unknowns):
    if action.stage == "project_info":
        if action.field is None or action.field not in pi_fields:
            return
        evidence = pi_fields[action.field]
        if action.action == "demote":
            unknowns.append(
                UnknownItem(
                    field_name=action.field,
                    reason=action.demote_reason or "unsupported",
                    detail=action.demote_detail,
                    citations=evidence.citations,
                )
            )
        pi_fields[action.field] = EvidenceValue(
            value=None, confidence=evidence.confidence, requires_review=True, citations=[]
        )
        return

    items = {
        "qualifications": qual_items,
        "risks": risk_items,
        "materials": material_items,
    }[action.stage]
    if action.index is None or action.index >= len(items):
        return
    item = items.pop(action.index)
    if action.action == "demote":
        unknowns.append(
            UnknownItem(
                field_name=action.stage,
                reason=action.demote_reason or "unsupported",
                detail=action.demote_detail,
                citations=item.citations,
            )
        )


# ── Orchestration ──────────────────────────────────────────────


async def analyze_document(
    document: ParsedDocument,
    provider: LlmProvider,
    model_name: str,
    *,
    model_provider: str = "",
) -> TenderAnalysisV1:
    """Run the four extraction stages, review, and evidence validation."""
    chunk_index = {c.chunk_id: c for page in document.pages for c in page.chunks}

    project_info = await provider.complete_structured(
        PROJECT_INFO_PROMPT,
        _build_context(retrieve_chunks(document, QUERY_PACKS["project_info"])),
        ProjectInfo,
    )
    qualifications = await provider.complete_structured(
        QUALIFICATIONS_PROMPT,
        _build_context(retrieve_chunks(document, QUERY_PACKS["qualifications"])),
        QualificationList,
    )
    risks = await provider.complete_structured(
        RISKS_PROMPT,
        _build_context(retrieve_chunks(document, QUERY_PACKS["risks"])),
        RiskList,
    )
    materials = await provider.complete_structured(
        MATERIALS_PROMPT,
        _build_context(retrieve_chunks(document, QUERY_PACKS["materials"])),
        MaterialList,
    )
    review = await provider.complete_structured(
        REVIEW_PROMPT,
        _build_review_context(project_info, qualifications.items, risks.items, materials.items),
        ReviewResult,
    )

    pi_fields = {name: getattr(project_info, name) for name in ProjectInfo.model_fields}
    qual_items = list(qualifications.items)
    risk_items = list(risks.items)
    material_items = list(materials.items)
    unknowns = list(review.unknowns)

    # Apply review actions highest-index-first so list pops do not shift earlier indices.
    for action in sorted(review.actions, key=lambda a: a.index or 0, reverse=True):
        _apply_review_action(action, pi_fields, qual_items, risk_items, material_items, unknowns)

    # Deterministic citation validation for confirmed project facts.
    for field in list(pi_fields):
        evidence = pi_fields[field]
        if evidence.value is not None and not _has_valid_citation(evidence.citations, chunk_index):
            unknowns.append(_unknown_for_item(field, evidence))
            pi_fields[field] = EvidenceValue(
                value=None, confidence=evidence.confidence, requires_review=True, citations=[]
            )

    qual_items = _validate_list(qual_items, chunk_index, "qualification", unknowns)
    risk_items = _validate_list(risk_items, chunk_index, "risk", unknowns)
    material_items = _validate_list(material_items, chunk_index, "material", unknowns)

    meta = AnalysisMeta(
        model_provider=model_provider,
        model_name=model_name,
        prompt_versions=PromptVersions(
            project_info="tender-project-info-v1",
            qualifications="tender-qualifications-v1",
            risks="tender-risks-v1",
            materials="tender-materials-v1",
            review="tender-review-v1",
        ),
        contract_version="tender-analysis.v1",
        completeness="complete" if not unknowns else "partial",
    )

    return TenderAnalysisV1(
        schema_version="tender-analysis.v1",
        project_info=ProjectInfo(**pi_fields),
        qualifications=qual_items,
        risks=risk_items,
        required_materials=material_items,
        unknowns=unknowns,
        analysis_meta=meta,
    )
