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


def retrieve_chunks(
    document: ParsedDocument,
    query_terms: Sequence[str],
    *,
    max_chunks: int = MAX_CHUNKS,
) -> list[DocumentChunk]:
    """Return up to ``max_chunks`` chunks, always in document order."""
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

    selected: list[DocumentChunk] = []
    seen: set[str] = set()
    for chunk in chunks:
        if chunk.page_number <= 2 or _looks_like_toc(chunk.text):
            selected.append(chunk)
            seen.add(chunk.chunk_id)

    by_score = sorted(chunks, key=lambda c: scores[c.chunk_id], reverse=True)
    for chunk in by_score:
        if len(selected) >= max_chunks:
            break
        if chunk.chunk_id in seen:
            continue
        if chunk.chunk_id in high_ids or chunk.chunk_id in neighbor_ids:
            selected.append(chunk)
            seen.add(chunk.chunk_id)

    selected.sort(key=_chunk_key)
    return selected[:max_chunks]
