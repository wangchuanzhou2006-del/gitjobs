"""Four-stage extraction plus review, with deterministic citation validation."""

from __future__ import annotations

import json
from typing import Literal

from pydantic import BaseModel, ConfigDict, Field, ValidationError

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
from app.llm import AnalysisError, LlmProvider
from app.prompts import (
    MATERIALS_PROMPT,
    PROJECT_INFO_PROMPT,
    QUALIFICATIONS_PROMPT,
    REVIEW_PROMPT,
    RISKS_PROMPT,
)
from app.retrieval import QUERY_PACKS, order_by_relevance, retrieve_chunks

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


def _dedupe_chunks(chunks: list[DocumentChunk]) -> list[DocumentChunk]:
    seen: dict[str, DocumentChunk] = {}
    for chunk in chunks:
        seen.setdefault(chunk.chunk_id, chunk)
    return sorted(
        seen.values(),
        key=lambda c: (c.page_number, int(c.chunk_id.rsplit("-chunk-", 1)[1])),
    )


def _source_chunk_view(chunks: list[DocumentChunk]) -> list[dict]:
    return [{"chunk_id": c.chunk_id, "page_number": c.page_number, "text": c.text} for c in chunks]


def _build_risk_context(
    project_info: ProjectInfo,
    qualifications: list[Qualification],
    risk_chunks: list[DocumentChunk],
) -> str:
    return json.dumps(
        {
            "project_info": project_info.model_dump(),
            "qualifications": [q.model_dump() for q in qualifications],
            "source_chunks": _source_chunk_view(risk_chunks),
        },
        ensure_ascii=False,
    )


def _build_review_context(
    project_info: ProjectInfo,
    qualifications: list[Qualification],
    risks: list[Risk],
    materials: list[RequiredMaterial],
    source_chunks: list[DocumentChunk],
) -> str:
    return json.dumps(
        {
            "project_info": project_info.model_dump(),
            "qualifications": [q.model_dump() for q in qualifications],
            "risks": [r.model_dump() for r in risks],
            "materials": [m.model_dump() for m in materials],
            "source_chunks": _source_chunk_view(source_chunks),
        },
        ensure_ascii=False,
    )


def _all_citations_valid(citations: list[Citation], chunk_index: dict[str, DocumentChunk]) -> bool:
    for citation in citations:
        chunk = chunk_index.get(citation.chunk_id)
        if chunk is None:
            return False
        if chunk.page_number != citation.page_number:
            return False
        if citation.quote not in chunk.text:
            return False
    return True


def _unknown_for_item(field_name: str) -> UnknownItem:
    return UnknownItem(
        field_name=field_name,
        reason="unsupported",
        detail="结论引用无效或缺失",
        citations=[],
    )


def _validate_list(items, chunk_index, field_prefix, unknowns):
    kept = []
    for item in items:
        if _all_citations_valid(item.citations, chunk_index):
            kept.append(item)
        else:
            unknowns.append(_unknown_for_item(field_prefix))
    return kept


_VALID_UNKNOWN_FIELDS = {
    "project_name",
    "procurement_entity",
    "budget_amount",
    "budget_currency",
    "service_period",
    "bid_deadline",
    "qualifications",
    "risks",
    "materials",
}


def _is_valid_conflict(citations: list[Citation], chunk_index: dict[str, DocumentChunk]) -> bool:
    if not citations or not _all_citations_valid(citations, chunk_index):
        return False
    return len({c.chunk_id for c in citations}) >= 2


def _validate_review_unknown(unknown: UnknownItem, chunk_index: dict[str, DocumentChunk]) -> bool:
    if unknown.field_name not in _VALID_UNKNOWN_FIELDS:
        return False
    if unknown.reason == "missing":
        return not unknown.citations or _all_citations_valid(unknown.citations, chunk_index)
    if unknown.reason == "conflict":
        return _is_valid_conflict(unknown.citations, chunk_index)
    return not unknown.citations or _all_citations_valid(unknown.citations, chunk_index)


def _select_review_source_chunks(
    project_info: ProjectInfo,
    qualifications: list[Qualification],
    risks: list[Risk],
    materials: list[RequiredMaterial],
    retrieved_chunks: list[DocumentChunk],
    *,
    max_chunks: int = 60,
) -> list[DocumentChunk]:
    """Cap review source chunks and prefer chunks already cited by conclusions."""
    cited_ids: set[str] = set()
    for field in ProjectInfo.model_fields:
        for citation in getattr(project_info, field).citations:
            cited_ids.add(citation.chunk_id)
    for item in qualifications + risks + materials:
        for citation in item.citations:
            cited_ids.add(citation.chunk_id)

    retrieved = _dedupe_chunks(retrieved_chunks)
    cited = [c for c in retrieved if c.chunk_id in cited_ids]
    others = [c for c in retrieved if c.chunk_id not in cited_ids]

    all_terms = sorted({term for terms in QUERY_PACKS.values() for term in terms})
    others_ordered = order_by_relevance(others, all_terms)

    selected = cited[:max_chunks]
    for chunk in others_ordered:
        if len(selected) >= max_chunks:
            break
        selected.append(chunk)
    return sorted(selected, key=lambda c: (c.page_number, int(c.chunk_id.rsplit("-chunk-", 1)[1])))


def _apply_pi_action(action, pi_fields, unknowns):
    if action.field is None or action.field not in pi_fields:
        return
    if action.action == "demote" and action.demote_reason == "conflict":
        return  # conflict needs citations the action cannot carry
    evidence = pi_fields[action.field]
    if action.action == "demote":
        unknowns.append(
            UnknownItem(
                field_name=action.field,
                reason=action.demote_reason or "unsupported",
                detail=action.demote_detail,
                citations=[],
            )
        )
    pi_fields[action.field] = EvidenceValue(
        value=None, confidence=evidence.confidence, requires_review=True, citations=[]
    )


def _apply_list_actions(actions, items, stage_name, unknowns):
    drop_indices: set[int] = set()
    demotes: dict[int, tuple[str, str]] = {}
    for action in actions:
        if action.index is None or action.index < 0 or action.index >= len(items):
            continue
        if action.action == "drop":
            drop_indices.add(action.index)
        elif action.action == "demote":
            if action.demote_reason == "conflict":
                continue
            demotes[action.index] = (action.demote_reason or "unsupported", action.demote_detail)

    kept = []
    for index, item in enumerate(items):
        if index in demotes:
            reason, detail = demotes[index]
            unknowns.append(
                UnknownItem(field_name=stage_name, reason=reason, detail=detail, citations=[])
            )
            continue
        if index in drop_indices:
            continue
        kept.append(item)
    return kept


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

    project_chunks = retrieve_chunks(document, QUERY_PACKS["project_info"])
    qual_chunks = retrieve_chunks(document, QUERY_PACKS["qualifications"])
    risk_chunks = retrieve_chunks(document, QUERY_PACKS["risks"])
    material_chunks = retrieve_chunks(document, QUERY_PACKS["materials"])

    project_info = await provider.complete_structured(
        PROJECT_INFO_PROMPT, _build_context(project_chunks), ProjectInfo
    )
    qualifications = await provider.complete_structured(
        QUALIFICATIONS_PROMPT, _build_context(qual_chunks), QualificationList
    )
    risks = await provider.complete_structured(
        RISKS_PROMPT,
        _build_risk_context(project_info, qualifications.items, risk_chunks),
        RiskList,
    )
    materials = await provider.complete_structured(
        MATERIALS_PROMPT, _build_context(material_chunks), MaterialList
    )

    source_chunks = _select_review_source_chunks(
        project_info,
        qualifications.items,
        risks.items,
        materials.items,
        project_chunks + qual_chunks + risk_chunks + material_chunks,
    )
    review = await provider.complete_structured(
        REVIEW_PROMPT,
        _build_review_context(
            project_info, qualifications.items, risks.items, materials.items, source_chunks
        ),
        ReviewResult,
    )

    pi_fields = {name: getattr(project_info, name) for name in ProjectInfo.model_fields}
    qual_items = list(qualifications.items)
    risk_items = list(risks.items)
    material_items = list(materials.items)

    # Review unknowns must reference known fields/items and carry valid citations.
    unknowns = [u for u in review.unknowns if _validate_review_unknown(u, chunk_index)]

    # Apply review actions; indices are validated and deduplicated.
    for action in (a for a in review.actions if a.stage == "project_info"):
        _apply_pi_action(action, pi_fields, unknowns)
    qual_items = _apply_list_actions(
        [a for a in review.actions if a.stage == "qualifications"],
        qual_items,
        "qualifications",
        unknowns,
    )
    risk_items = _apply_list_actions(
        [a for a in review.actions if a.stage == "risks"], risk_items, "risks", unknowns
    )
    material_items = _apply_list_actions(
        [a for a in review.actions if a.stage == "materials"],
        material_items,
        "materials",
        unknowns,
    )

    # Deterministic citation validation for confirmed project facts.
    for field in list(pi_fields):
        evidence = pi_fields[field]
        if evidence.value is not None and not _all_citations_valid(evidence.citations, chunk_index):
            unknowns.append(_unknown_for_item(field))
            pi_fields[field] = EvidenceValue(
                value=None, confidence=evidence.confidence, requires_review=True, citations=[]
            )

    qual_items = _validate_list(qual_items, chunk_index, "qualifications", unknowns)
    risk_items = _validate_list(risk_items, chunk_index, "risks", unknowns)
    material_items = _validate_list(material_items, chunk_index, "materials", unknowns)

    # A valid conflict demotes its project field to unconfirmed.
    for unknown in unknowns:
        if unknown.reason == "conflict" and unknown.field_name in pi_fields:
            evidence = pi_fields[unknown.field_name]
            if evidence.value is not None:
                pi_fields[unknown.field_name] = EvidenceValue(
                    value=None, confidence=evidence.confidence, requires_review=True, citations=[]
                )

    # Unconfirmed project fields become missing unknowns unless already conflicted.
    for field, evidence in pi_fields.items():
        if evidence.value is None and not any(
            u.field_name == field and u.reason in ("missing", "conflict") for u in unknowns
        ):
            unknowns.append(
                UnknownItem(
                    field_name=field, reason="missing", detail="未在文件中确认", citations=[]
                )
            )

    # A confirmed field cannot be reported missing or conflicted.
    unknowns = [
        u
        for u in unknowns
        if not (
            u.reason in ("missing", "conflict")
            and u.field_name in pi_fields
            and pi_fields[u.field_name].value is not None
        )
    ]

    # Category unknowns must relate to an actually-extracted category.
    category_has_items = {
        "qualifications": bool(qualifications.items),
        "risks": bool(risks.items),
        "materials": bool(materials.items),
    }
    unknowns = [
        u
        for u in unknowns
        if u.field_name not in category_has_items or category_has_items[u.field_name]
    ]

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

    try:
        return TenderAnalysisV1(
            schema_version="tender-analysis.v1",
            project_info=ProjectInfo(**pi_fields),
            qualifications=qual_items,
            risks=risk_items,
            required_materials=material_items,
            unknowns=unknowns,
            analysis_meta=meta,
        )
    except ValidationError:
        raise AnalysisError("analysis_unavailable")
