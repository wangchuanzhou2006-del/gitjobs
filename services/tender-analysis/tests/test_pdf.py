"""PDF parsing tests for the internal tender analysis service.

PDFs are generated at runtime with PyMuPDF; no real tender documents are
committed. Chinese text uses PyMuPDF's built-in CJK font, and the pure
``normalize_text`` function is tested separately so it does not depend on a
system CJK font being installed.
"""

from __future__ import annotations

import secrets

import fitz
import pytest
from httpx import ASGITransport, AsyncClient


@pytest.fixture
def anyio_backend():
    return "asyncio"


@pytest.fixture
def valid_token():
    return "test-token-" + secrets.token_hex(16)


@pytest.fixture
def auth_env(monkeypatch, valid_token):
    """Set the internal token and return an ASGI transport plus the token."""
    monkeypatch.setenv("INTERNAL_SERVICE_TOKEN", valid_token)
    from app.main import app

    return ASGITransport(app=app), valid_token


def _make_pdf(page_texts, *, fontname="helv", encrypt=False, user_pw="user"):
    doc = fitz.open()
    for text in page_texts:
        page = doc.new_page()
        if text:
            page.insert_text((72, 72), text, fontname=fontname, fontsize=12)
    if encrypt:
        return doc.tobytes(encryption=fitz.PDF_ENCRYPT_AES_256, owner_pw="owner", user_pw=user_pw)
    return doc.tobytes()


# ═══════════════════════════════════════════════════════════════
# ── Page-preserving parsing ────────────────────────────────────
# ═══════════════════════════════════════════════════════════════


def test_two_page_pdf_preserves_page_evidence():
    """Two pages of text must produce 1-based pages with page-local chunks."""
    from app.pdf import parse_pdf

    data = _make_pdf(["budget 1000000", "deadline 2026-09-01"])
    parsed = parse_pdf(data)

    assert parsed.page_count == 2
    assert [p.page_number for p in parsed.pages] == [1, 2]
    assert "budget 1000000" in parsed.pages[0].text
    assert "deadline 2026-09-01" in parsed.pages[1].text
    assert parsed.pages[0].chunks[0].chunk_id == "page-1-chunk-0"
    assert parsed.pages[1].chunks[0].chunk_id == "page-2-chunk-0"


def test_page_and_chunk_ids_match_page_number():
    """Every chunk's page_number must match its own page-{page}-chunk-{seq} ID."""
    from app.pdf import parse_pdf

    parsed = parse_pdf(_make_pdf(["one", "two", "three"]))
    for page in parsed.pages:
        for idx, chunk in enumerate(page.chunks):
            assert chunk.page_number == page.page_number
            assert chunk.chunk_id == f"page-{page.page_number}-chunk-{idx}"


def test_chunk_page_splits_with_overlap():
    """The chunker must emit stable, overlapping page-local chunks."""
    from app.pdf import chunk_page

    text = "".join(chr(65 + (i % 26)) for i in range(4000))
    chunks = chunk_page(1, text, size=1600, overlap=200)

    assert [c.chunk_id for c in chunks] == [
        "page-1-chunk-0",
        "page-1-chunk-1",
        "page-1-chunk-2",
    ]
    assert [c.page_number for c in chunks] == [1, 1, 1]
    assert chunks[0].text == text[0:1600]
    assert chunks[1].text == text[1400:3000]
    assert chunks[2].text == text[2800:4000]
    assert chunks[0].text + chunks[1].text[200:] + chunks[2].text[200:] == text


def test_chinese_pdf_preserves_text():
    """A Chinese PDF rendered with the built-in CJK font keeps its text."""
    from app.pdf import parse_pdf

    parsed = parse_pdf(_make_pdf(["第一页：预算100万元"], fontname="china-s"))
    assert "预算100万元" in parsed.pages[0].text
    assert "第一页" in parsed.pages[0].text


# ═══════════════════════════════════════════════════════════════
# ── Text normalization ─────────────────────────────────────────
# ═══════════════════════════════════════════════════════════════


def test_normalize_text_removes_controls_and_preserves_newlines():
    """NUL and other unsafe control chars must go; newlines and text must stay."""
    from app.pdf import normalize_text

    out = normalize_text("预算\x00100万\r\n截止\x1b日期\ncafé")

    assert "\x00" not in out
    assert "\x1b" not in out
    assert "\r" not in out
    assert "预算100万" in out
    assert "截止日期" in out
    assert "café" in out  # NFC-composed from e + combining acute


# ═══════════════════════════════════════════════════════════════
# ── Parser failure codes ───────────────────────────────────────
# ═══════════════════════════════════════════════════════════════


def test_spoofed_pdf_signature_raises_invalid_pdf():
    """A fake ``%PDF-`` header must be rejected as invalid_pdf."""
    from app.pdf import ParseError, parse_pdf

    with pytest.raises(ParseError) as exc:
        parse_pdf(b"%PDF-1.7\nnot really a pdf")
    assert exc.value.code == "invalid_pdf"


def test_corrupt_bytes_raise_invalid_pdf():
    """Bytes that are not a PDF must be rejected as invalid_pdf."""
    from app.pdf import ParseError, parse_pdf

    with pytest.raises(ParseError) as exc:
        parse_pdf(b"definitely not a pdf")
    assert exc.value.code == "invalid_pdf"


def test_encrypted_pdf_raises_encrypted_pdf():
    """A password-protected PDF must be rejected as encrypted_pdf."""
    from app.pdf import ParseError, parse_pdf

    data = _make_pdf(["secret"], encrypt=True, user_pw="user")
    with pytest.raises(ParseError) as exc:
        parse_pdf(data)
    assert exc.value.code == "encrypted_pdf"


def test_more_than_1000_pages_raises_too_many_pages():
    """PDFs over 1,000 pages must be rejected as too_many_pages."""
    from app.pdf import ParseError, parse_pdf

    data = _make_pdf([""] * 1001)
    with pytest.raises(ParseError) as exc:
        parse_pdf(data)
    assert exc.value.code == "too_many_pages"


def test_blank_page_raises_ocr_required():
    """A page with no text layer must be rejected as ocr_required."""
    from app.pdf import ParseError, parse_pdf

    with pytest.raises(ParseError) as exc:
        parse_pdf(_make_pdf([""]))
    assert exc.value.code == "ocr_required"


def test_text_below_ten_percent_raises_ocr_required():
    """Fewer than 10% of pages carrying text must be rejected as ocr_required."""
    from app.pdf import ParseError, parse_pdf

    data = _make_pdf(["has text"] + [""] * 19)  # 1 of 20 pages = 5%
    with pytest.raises(ParseError) as exc:
        parse_pdf(data)
    assert exc.value.code == "ocr_required"


def test_text_at_ten_percent_is_accepted():
    """Exactly 10% of pages carrying text is acceptable."""
    from app.pdf import parse_pdf

    data = _make_pdf(["has text", "more text"] + [""] * 18)  # 2 of 20 = 10%
    parsed = parse_pdf(data)
    assert parsed.page_count == 20


def test_extracted_text_limit(monkeypatch):
    """Extracted text over the byte budget must be rejected as extracted_text_too_large."""
    import app.pdf as pdf

    monkeypatch.setattr(pdf, "MAX_EXTRACTED_BYTES", 10)
    data = _make_pdf(["x" * 50])
    with pytest.raises(pdf.ParseError) as exc:
        pdf.parse_pdf(data)
    assert exc.value.code == "extracted_text_too_large"


# ═══════════════════════════════════════════════════════════════
# ── Parse endpoint ─────────────────────────────────────────────
# ═══════════════════════════════════════════════════════════════


@pytest.mark.anyio
async def test_parse_requires_auth(auth_env):
    """POST /internal/v1/parse without a token must return 401."""
    transport, _ = auth_env
    async with AsyncClient(transport=transport, base_url="http://test") as client:
        response = await client.post(
            "/internal/v1/parse",
            files={"file": ("a.pdf", b"%PDF-", "application/pdf")},
        )
    assert response.status_code == 401


@pytest.mark.anyio
async def test_parse_success_returns_parse_response(auth_env):
    """Authenticated parse of a valid PDF returns a completed ParseResponse."""
    from app.contracts import ParseResponse

    transport, token = auth_env
    data = _make_pdf(["budget 1000000"])
    async with AsyncClient(transport=transport, base_url="http://test") as client:
        response = await client.post(
            "/internal/v1/parse",
            headers={"Authorization": f"Bearer {token}"},
            files={"file": ("t.pdf", data, "application/pdf")},
        )
    assert response.status_code == 200
    parsed_response = ParseResponse.model_validate(response.json())
    assert parsed_response.status == "completed"
    assert parsed_response.parsed_document is not None
    assert parsed_response.parsed_document.page_count == 1
    assert "budget 1000000" in parsed_response.parsed_document.pages[0].text


@pytest.mark.anyio
async def test_parse_invalid_pdf_returns_422(auth_env):
    """Authenticated parse of corrupt bytes returns 422 with invalid_pdf."""
    from app.contracts import ParseResponse

    transport, token = auth_env
    async with AsyncClient(transport=transport, base_url="http://test") as client:
        response = await client.post(
            "/internal/v1/parse",
            headers={"Authorization": f"Bearer {token}"},
            files={"file": ("fake.pdf", b"not a pdf", "application/pdf")},
        )
    assert response.status_code == 422
    parsed_response = ParseResponse.model_validate(response.json())
    assert parsed_response.status == "failed"
    assert parsed_response.error_code == "invalid_pdf"


@pytest.mark.anyio
async def test_parse_encrypted_pdf_returns_422(auth_env):
    """Authenticated parse of an encrypted PDF returns 422 with encrypted_pdf."""
    from app.contracts import ParseResponse

    transport, token = auth_env
    data = _make_pdf(["secret"], encrypt=True, user_pw="user")
    async with AsyncClient(transport=transport, base_url="http://test") as client:
        response = await client.post(
            "/internal/v1/parse",
            headers={"Authorization": f"Bearer {token}"},
            files={"file": ("enc.pdf", data, "application/pdf")},
        )
    assert response.status_code == 422
    parsed_response = ParseResponse.model_validate(response.json())
    assert parsed_response.status == "failed"
    assert parsed_response.error_code == "encrypted_pdf"


@pytest.mark.anyio
async def test_parse_oversized_upload_returns_413(auth_env, monkeypatch):
    """An upload over the byte budget must return 413 with file_too_large."""
    from app import main

    monkeypatch.setattr(main, "MAX_UPLOAD_BYTES", 8)
    transport, token = auth_env
    async with AsyncClient(transport=transport, base_url="http://test") as client:
        response = await client.post(
            "/internal/v1/parse",
            headers={"Authorization": f"Bearer {token}"},
            files={"file": ("big.pdf", b"x" * 100, "application/pdf")},
        )
    assert response.status_code == 413
    assert response.json()["detail"] == "file_too_large"
