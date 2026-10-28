"""FastAPI application — internal tender analysis service.

No CORS, no user auth, no database.  Only internal Bearer-authenticated
endpoints: parse streams the upload after auth; analyze remains a 501 shell.
"""

from __future__ import annotations

from collections.abc import AsyncGenerator
from contextlib import asynccontextmanager
from typing import Annotated

from fastapi import Depends, FastAPI, Request
from fastapi.responses import JSONResponse
from python_multipart.multipart import MultipartParseError, MultipartParser, parse_options_header

from app.auth import require_internal_token
from app.config import Settings
from app.contracts import ParseResponse
from app.pdf import ParseError, parse_pdf


@asynccontextmanager
async def lifespan(app: FastAPI) -> AsyncGenerator[None, None]:
    # Validate token is usable at startup (fail-closed).
    settings = Settings()
    _ = settings.internal_service_token.get_secret_value()
    yield


app = FastAPI(
    title="Tender Analysis Internal Service",
    version="0.1.0",
    lifespan=lifespan,
)


@app.get("/internal/health")
async def health_check():
    """Unauthenticated health check — returns fixed status only."""
    return {"status": "ok"}


MAX_UPLOAD_BYTES = 50 * 1024 * 1024  # 50 MiB file field
MAX_REQUEST_BYTES = 64 * 1024 * 1024  # 64 MiB total body
MAX_HEADER_BYTES = 16 * 1024  # per multipart header line


class _FileTooLargeError(Exception):
    """Raised when the file field exceeds MAX_UPLOAD_BYTES."""


class _HeaderTooLargeError(Exception):
    """Raised when a multipart header exceeds MAX_HEADER_BYTES."""


class _MultipartFileAccumulator:
    """Collect the single ``file`` field while enforcing size limits."""

    def __init__(self, max_file_bytes: int, max_header_bytes: int) -> None:
        self.max_file_bytes = max_file_bytes
        self.max_header_bytes = max_header_bytes
        self.file_data = bytearray()
        self.file_seen = False
        self._is_file_part = False
        self._header_name = bytearray()
        self._header_value = bytearray()
        self._header_size = 0

    def on_part_begin(self) -> None:
        self._is_file_part = False
        self._header_name = bytearray()
        self._header_value = bytearray()
        self._header_size = 0

    def on_header_field(self, data, start, end) -> None:
        self._header_size += end - start
        if self._header_size > self.max_header_bytes:
            raise _HeaderTooLargeError()
        self._header_name += data[start:end]

    def on_header_value(self, data, start, end) -> None:
        self._header_size += end - start
        if self._header_size > self.max_header_bytes:
            raise _HeaderTooLargeError()
        self._header_value += data[start:end]

    def on_header_end(self) -> None:
        if self._header_name.lower() == b"content-disposition":
            try:
                _, options = parse_options_header(self._header_value.decode("latin-1"))
            except Exception:
                options = {}
            if options.get(b"name") == b"file":
                self._is_file_part = True
        self._header_name = bytearray()
        self._header_value = bytearray()
        self._header_size = 0

    def on_part_data(self, data, start, end) -> None:
        if not self._is_file_part:
            return
        self.file_seen = True
        self.file_data += data[start:end]
        if len(self.file_data) > self.max_file_bytes:
            raise _FileTooLargeError()

    def callbacks(self) -> dict:
        return {
            "on_part_begin": self.on_part_begin,
            "on_header_field": self.on_header_field,
            "on_header_value": self.on_header_value,
            "on_header_end": self.on_header_end,
            "on_part_data": self.on_part_data,
        }


def _error(status_code: int, code: str) -> JSONResponse:
    return JSONResponse(status_code=status_code, content={"detail": code})


@app.post("/internal/v1/parse")
async def parse_document(
    _token: Annotated[None, Depends(require_internal_token)],
    request: Request,
):
    """Stream and parse an uploaded PDF, after authentication.

    The multipart body is parsed incrementally and bounded: 50 MiB for the
    ``file`` field, 64 MiB for the whole body, and a cap on header size. No
    temporary files are used, and no exception text is returned to clients.
    """
    content_type = request.headers.get("content-type", "")
    try:
        _, options = parse_options_header(content_type)
    except Exception:
        options = {}
    boundary = options.get(b"boundary")
    if not boundary:
        return _error(400, "invalid_multipart")

    accumulator = _MultipartFileAccumulator(MAX_UPLOAD_BYTES, MAX_HEADER_BYTES)
    parser = MultipartParser(boundary, accumulator.callbacks())

    total = 0
    async for chunk in request.stream():
        total += len(chunk)
        if total > MAX_REQUEST_BYTES:
            return _error(413, "request_too_large")
        try:
            parser.write(chunk)
        except _FileTooLargeError:
            return _error(413, "file_too_large")
        except _HeaderTooLargeError:
            return _error(413, "header_too_large")
        except MultipartParseError:
            return _error(400, "invalid_multipart")

    parser.finalize()

    if not accumulator.file_seen:
        return _error(400, "invalid_multipart")

    try:
        parsed = parse_pdf(bytes(accumulator.file_data))
    except ParseError as exc:
        return JSONResponse(
            status_code=422,
            content=ParseResponse(status="failed", error_code=exc.code).model_dump(mode="json"),
        )
    return ParseResponse(status="completed", parsed_document=parsed)


@app.post("/internal/v1/analyze")
async def analyze_document(
    _token: Annotated[None, Depends(require_internal_token)],
):
    """Analyze a parsed document and return TenderAnalysisV1. (Task 3)"""
    return JSONResponse(content={"detail": "not implemented"}, status_code=501)
