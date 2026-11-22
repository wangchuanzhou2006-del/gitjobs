"""Page-preserving PDF parsing for the tender analysis service.

``parse_pdf`` opens bytes through PyMuPDF, normalizes each page's text, and
splits it into page-local chunks with stable ``page-{page}-chunk-{seq}`` IDs so
later report citations can point back to the exact source page and chunk.

On failure it raises :class:`ParseError` carrying one of the five safe contract
error codes. It never writes to disk, never logs file contents, and never
performs OCR.
"""

from __future__ import annotations

import unicodedata

import fitz

from app.contracts import DocumentChunk, ParsedDocument, ParsedPage

MAX_PAGES = 1000
MAX_EXTRACTED_BYTES = 10 * 1024 * 1024  # 10 MiB
CHUNK_SIZE = 1600
CHUNK_OVERLAP = 200
OCR_TEXT_RATIO = 0.10

_PARSE_ERROR_CODES = {
    "invalid_pdf",
    "encrypted_pdf",
    "too_many_pages",
    "extracted_text_too_large",
    "ocr_required",
}


class ParseError(Exception):
    """A parse failure carrying a safe, fixed contract error code."""

    def __init__(self, code: str) -> None:
        if code not in _PARSE_ERROR_CODES:
            raise ValueError(f"unknown parse error code: {code}")
        self.code = code
        super().__init__(code)


def normalize_text(text: str) -> str:
    """NFC-normalize text, keep newlines/tabs, drop NUL and other controls."""
    text = unicodedata.normalize("NFC", text)
    text = text.replace("\r\n", "\n").replace("\r", "\n")
    return "".join(ch for ch in text if ch in {"\n", "\t"} or unicodedata.category(ch) != "Cc")


def chunk_page(
    page_number: int,
    text: str,
    size: int = CHUNK_SIZE,
    overlap: int = CHUNK_OVERLAP,
) -> list[DocumentChunk]:
    """Split one page's text into overlapping, page-local chunks."""
    chunks: list[DocumentChunk] = []
    start = 0
    while start < len(text):
        end = min(len(text), start + size)
        chunks.append(
            DocumentChunk(
                chunk_id=f"page-{page_number}-chunk-{len(chunks)}",
                page_number=page_number,
                text=text[start:end],
            )
        )
        if end == len(text):
            break
        start = end - overlap
    return chunks


def _extract_page_text(page: fitz.Page) -> str:
    """Keep source order outside ruled tables and serialize table cells by row."""
    tables = page.find_tables().tables
    if not tables:
        return page.get_text("text")

    table_rects = [fitz.Rect(table.bbox) for table in tables]
    table_texts = [
        "".join(
            "\t".join((cell or "").replace("\n", " ").strip() for cell in row) + "\n"
            for row in table.extract()
            if any(cell and cell.strip() for cell in row)
        )
        for table in tables
    ]
    parts: list[str] = []
    emitted: set[int] = set()
    for block in page.get_text("blocks"):
        if block[6] != 0:  # Text only; images do not provide citable source text.
            continue
        midpoint = fitz.Point((block[0] + block[2]) / 2, (block[1] + block[3]) / 2)
        table_index = next(
            (i for i, rect in enumerate(table_rects) if rect.contains(midpoint)), None
        )
        if table_index is None:
            parts.append(block[4])
        elif table_index not in emitted:
            parts.append(table_texts[table_index])
            emitted.add(table_index)

    for index, table_text in enumerate(table_texts):
        if index not in emitted:
            parts.append(table_text)
    return "".join(parts)


def parse_pdf(data: bytes) -> ParsedDocument:
    """Parse PDF bytes into a page-preserving :class:`ParsedDocument`.

    Raises :class:`ParseError` for invalid, encrypted, oversized, or
    text-starved documents instead of returning partial or fabricated text.
    Any unexpected PyMuPDF content error maps to ``invalid_pdf``.
    """
    if not data:
        raise ParseError("invalid_pdf")

    try:
        doc = fitz.open(stream=data, filetype="pdf")
    except Exception:
        # Any failure to open the stream means the bytes are not a parseable PDF.
        raise ParseError("invalid_pdf")

    try:
        try:
            if doc.needs_pass:
                raise ParseError("encrypted_pdf")

            page_count = doc.page_count
            if page_count == 0:
                raise ParseError("invalid_pdf")
            if page_count > MAX_PAGES:
                raise ParseError("too_many_pages")

            total_bytes = 0
            pages: list[ParsedPage] = []
            for page_number in range(1, page_count + 1):
                text = normalize_text(_extract_page_text(doc[page_number - 1]))
                total_bytes += len(text.encode("utf-8"))
                if total_bytes > MAX_EXTRACTED_BYTES:
                    raise ParseError("extracted_text_too_large")
                chunks = chunk_page(page_number, text) if text.strip() else []
                pages.append(ParsedPage(page_number=page_number, text=text, chunks=chunks))

            text_pages = sum(1 for page in pages if page.text.strip())
            if text_pages * 10 < page_count:
                raise ParseError("ocr_required")

            return ParsedDocument(page_count=page_count, pages=pages)
        except ParseError:
            raise
        except Exception:
            raise ParseError("invalid_pdf")
    finally:
        doc.close()
