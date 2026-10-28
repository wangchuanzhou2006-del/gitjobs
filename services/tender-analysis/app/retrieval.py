"""Deterministic hybrid chunk retrieval for tender document analysis.

Selects a bounded, document-ordered window of chunks for each analysis stage by
scoring keyword hits, heading/clause-number patterns, and negation terms, while
always keeping the first two pages and table-of-contents hits. No external
embeddings or persistent store are used.
"""

from __future__ import annotations

import re
from collections.abc import Sequence

from app.contracts import DocumentChunk, ParsedDocument

MAX_CHUNKS = 60

QUERY_PACKS: dict[str, list[str]] = {
    "project_info": [
        "项目名称",
        "采购人",
        "采购单位",
        "预算",
        "金额",
        "币种",
        "人民币",
        "服务期",
        "服务期限",
        "投标截止",
        "开标时间",
        "截止时间",
    ],
    "qualifications": [
        "资格",
        "资质",
        "许可证",
        "营业执照",
        "认证",
        "证书",
        "业绩",
        "注册资本",
        "人员",
        "须具备",
        "应具备",
        "不得",
    ],
    "risks": [
        "风险",
        "违约",
        "废标",
        "无效投标",
        "保证金",
        "罚款",
        "责任",
        "终止",
        "解除",
        "不得",
        "须承担",
        "不予",
    ],
    "materials": [
        "提供",
        "提交",
        "复印件",
        "加盖公章",
        "承诺函",
        "证明",
        "材料",
        "清单",
        "原件",
        "投标文件",
        "格式",
    ],
}

_HEADING_RE = re.compile(
    r"(?:^|\n)\s*(?:第[一二三四五六七八九十百千0-9]+[章节条]|"
    r"[一二三四五六七八九十]+[、.．]|"
    r"\([一二三四五六七八九十]+\)|（[一二三四五六七八九十]+）|"
    r"[0-9]+[、.．]|[0-9]+\.[0-9]+)"
)

_NEGATION_TERMS = (
    "不得",
    "禁止",
    "必须",
    "应当",
    "应",
    "不予",
    "不得低于",
    "不得高于",
    "否则",
)

_TOC_MARKERS = ("目录", "目 录", "Table of Contents")


def _chunk_key(chunk: DocumentChunk) -> tuple[int, int]:
    m = re.search(r"-chunk-([0-9]+)$", chunk.chunk_id)
    idx = int(m.group(1)) if m else 0
    return (chunk.page_number, idx)


def _score(chunk: DocumentChunk, query_terms: Sequence[str]) -> int:
    text = chunk.text
    score = 0
    for term in query_terms:
        score += text.count(term)
    if _HEADING_RE.search(text):
        score += 2
    for term in _NEGATION_TERMS:
        if term in text:
            score += 1
    return score


def _looks_like_toc(text: str) -> bool:
    return any(marker in text for marker in _TOC_MARKERS)


def _high_in_priority_order(
    chunks: list[DocumentChunk], scores: dict[str, int]
) -> list[DocumentChunk]:
    """Order high-scoring chunks by score, then round-robin across pages.

    Round-robining within a score tier keeps a later-page chunk that ties with
    many earlier-page chunks from being crowded out by a stable document-order
    sort.
    """
    by_score: dict[int, list[DocumentChunk]] = {}
    for chunk in chunks:
        score = scores[chunk.chunk_id]
        if score > 0:
            by_score.setdefault(score, []).append(chunk)

    ordered: list[DocumentChunk] = []
    for score in sorted(by_score, reverse=True):
        bucket = sorted(by_score[score], key=_chunk_key)
        by_page: dict[int, list[DocumentChunk]] = {}
        for chunk in bucket:
            by_page.setdefault(chunk.page_number, []).append(chunk)
        pages = sorted(by_page)
        idx = 0
        while any(idx < len(by_page[page]) for page in pages):
            for page in pages:
                if idx < len(by_page[page]):
                    ordered.append(by_page[page][idx])
            idx += 1
    return ordered


def retrieve_chunks(
    document: ParsedDocument,
    query_terms: Sequence[str],
    *,
    max_chunks: int = MAX_CHUNKS,
) -> list[DocumentChunk]:
    """Return up to ``max_chunks`` chunks, always in document order.

    High-scoring chunks are selected first (so later-page clauses are never
    crowded out), then their neighbors, then a bounded, representative slice of
    the first two pages and table-of-contents chunks fills any remaining quota.
    """
    chunks = [c for page in document.pages for c in page.chunks]
    if not chunks:
        return []

    scores = {c.chunk_id: _score(c, query_terms) for c in chunks}
    high_ids = {c.chunk_id for c in chunks if scores[c.chunk_id] > 0}

    neighbor_ids: set[str] = set()
    for i, chunk in enumerate(chunks):
        if chunk.chunk_id in high_ids:
            if i > 0:
                neighbor_ids.add(chunks[i - 1].chunk_id)
            if i + 1 < len(chunks):
                neighbor_ids.add(chunks[i + 1].chunk_id)

    intro_ids = {c.chunk_id for c in chunks if c.page_number <= 2 or _looks_like_toc(c.text)}

    high_ordered = _high_in_priority_order(chunks, scores)
    neighbors = [c for c in chunks if c.chunk_id in neighbor_ids and c.chunk_id not in high_ids]
    intro = [
        c
        for c in chunks
        if c.chunk_id in intro_ids and c.chunk_id not in high_ids and c.chunk_id not in neighbor_ids
    ]

    selected: list[DocumentChunk] = []
    seen: set[str] = set()

    def add(candidate: DocumentChunk) -> None:
        if len(selected) >= max_chunks or candidate.chunk_id in seen:
            return
        selected.append(candidate)
        seen.add(candidate.chunk_id)

    for chunk in high_ordered:
        add(chunk)
    for chunk in neighbors:
        add(chunk)
    for chunk in intro:
        add(chunk)

    selected.sort(key=_chunk_key)
    return selected[:max_chunks]
