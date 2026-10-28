"""Deterministic hybrid retrieval tests."""

from __future__ import annotations

from app.contracts import DocumentChunk, ParsedDocument, ParsedPage


def _chunk(page: int, idx: int, text: str) -> DocumentChunk:
    return DocumentChunk(chunk_id=f"page-{page}-chunk-{idx}", page_number=page, text=text)


def _doc(pages: list[list[str]]) -> ParsedDocument:
    parsed_pages = []
    for pno, chunk_texts in enumerate(pages, start=1):
        chunks = [_chunk(pno, i, t) for i, t in enumerate(chunk_texts) if t]
        parsed_pages.append(ParsedPage(page_number=pno, text="\n".join(chunk_texts), chunks=chunks))
    return ParsedDocument(page_count=len(pages), pages=parsed_pages)


def test_first_two_pages_always_included():
    """Chunks from pages 1 and 2 must always be selected."""
    from app.retrieval import retrieve_chunks

    doc = _doc([["首页", "项目名称：测试项目"], ["采购人：某单位"], ["普通内容"]])
    result = retrieve_chunks(doc, ["项目名称"])
    pages = {c.page_number for c in result}
    assert 1 in pages
    assert 2 in pages


def test_later_page_high_score_clause_selected():
    """A later-page clause matching the query must not be crowded out."""
    from app.retrieval import retrieve_chunks

    doc = _doc([["首页"], ["第二页"], ["第三页"], ["第四页"], ["预算金额：人民币壹佰万元整"]])
    result = retrieve_chunks(doc, ["预算金额"])
    assert any("预算金额" in c.text for c in result)
    assert any(c.page_number == 5 for c in result)


def test_max_60_chunk_cap():
    """Retrieval must never exceed the 60-chunk cap."""
    from app.retrieval import retrieve_chunks

    pages = [[f"条款内容 {i}"] for i in range(70)]
    result = retrieve_chunks(_doc(pages), ["条款"], max_chunks=60)
    assert len(result) <= 60


def test_output_in_document_order():
    """Selected chunks must be ordered by page number then chunk index."""
    from app.retrieval import retrieve_chunks

    doc = _doc([["首页"], ["第二页"], ["预算金额：X"], ["项目名称：Y"], ["采购人：Z"]])
    result = retrieve_chunks(doc, ["预算金额", "项目名称", "采购人"])
    keys = [(c.page_number, int(c.chunk_id.rsplit("-chunk-", 1)[1])) for c in result]
    assert keys == sorted(keys)


def test_includes_neighbor_of_high_score_chunk():
    """Neighbors of a high-score chunk must be included for context."""
    from app.retrieval import retrieve_chunks

    doc = _doc(
        [["首页"], ["第二页"], ["第三页"], ["第四页 普通"], ["预算金额：100万"], ["第六页 普通"]]
    )
    result = retrieve_chunks(doc, ["预算金额"])
    ids = {c.chunk_id for c in result}
    assert "page-5-chunk-0" in ids
    assert "page-4-chunk-0" in ids
    assert "page-6-chunk-0" in ids


def test_includes_toc_chunk():
    """A table-of-contents chunk must be retained."""
    from app.retrieval import retrieve_chunks

    doc = _doc([["目录", "第一章 总则"], ["项目名称：测试项目"]])
    result = retrieve_chunks(doc, ["项目名称"])
    ids = {c.chunk_id for c in result}
    assert "page-1-chunk-0" in ids


def test_high_relevance_later_page_not_crowded_by_intro():
    """70 intro chunks plus a high-score page-3 chunk must keep the page-3 chunk."""
    from app.retrieval import retrieve_chunks

    doc = _doc([["普通内容"] * 35, ["普通内容"] * 35, ["预算金额：100万元"]])
    result = retrieve_chunks(doc, ["预算金额"])
    assert len(result) <= 60
    assert any("预算金额" in c.text for c in result)
    assert any(c.page_number == 3 for c in result)
