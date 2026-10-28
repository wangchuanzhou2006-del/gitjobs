"""FastAPI application — internal tender analysis service.

No CORS, no user auth, no database.  Only internal Bearer-authenticated
endpoints: parse streams the upload after auth, and analyze turns a parsed
document into a cited TenderAnalysisV1 report.
"""

from __future__ import annotations

import json
from collections.abc import AsyncGenerator
from contextlib import asynccontextmanager
from typing import Annotated

from fastapi import Depends, FastAPI, Request
from fastapi.responses import JSONResponse
from pydantic import ValidationError
from python_multipart.multipart import MultipartParseError, MultipartParser, parse_options_header

from app.analysis import analyze_document as run_analysis
from app.auth import require_internal_token
from app.config import Settings, get_settings
from app.contracts import AnalyzeRequest, ParseResponse
from app.llm import AnalysisError, LiteLlmProvider, LlmProvider
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
MAX_HEADER_BYTES = 16 * 1024  # cumulative multipart header budget
MAX_HEADER_COUNT = 100  # per-part header count bound


class _FileTooLargeError(Exception):
    """Raised when the file field exceeds MAX_UPLOAD_BYTES."""


class _HeaderTooLargeError(Exception):
    """Raised when cumulative multipart headers exceed MAX_HEADER_BYTES."""


class _MultipartFileAccumulator:
    """Collect the single ``file`` field while tracking multipart structure."""

    def __init__(self, max_file_bytes: int, max_header_bytes: int) -> None:
        self.max_file_bytes = max_file_bytes
        self.max_header_bytes = max_header_bytes
        self.file_data = bytearray()
        self.file_part_count = 0
        self.total_part_count = 0
        self.completed_part_count = 0
        self.message_completed = False
        self._is_file_part = False
        self._header_name = bytearray()
        self._header_value = bytearray()
        self._total_header_bytes = 0

    def on_part_begin(self) -> None:
        self.total_part_count += 1
        self._is_file_part = False
        self._header_name = bytearray()
        self._header_value = bytearray()

    def on_header_field(self, data, start, end) -> None:
        self._total_header_bytes += end - start
        if self._total_header_bytes > self.max_header_bytes:
            raise _HeaderTooLargeError()
        self._header_name += data[start:end]

    def on_header_value(self, data, start, end) -> None:
        self._total_header_bytes += end - start
        if self._total_header_bytes > self.max_header_bytes:
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

    def on_headers_finished(self) -> None:
        if self._is_file_part:
            self.file_part_count += 1

    def on_part_data(self, data, start, end) -> None:
        if not self._is_file_part:
            return
        self.file_data += data[start:end]
        if len(self.file_data) > self.max_file_bytes:
            raise _FileTooLargeError()

    def on_part_end(self) -> None:
        self.completed_part_count += 1

    def on_end(self) -> None:
        self.message_completed = True

    def callbacks(self) -> dict:
        return {
            "on_part_begin": self.on_part_begin,
            "on_header_field": self.on_header_field,
            "on_header_value": self.on_header_value,
            "on_header_end": self.on_header_end,
            "on_headers_finished": self.on_headers_finished,
            "on_part_data": self.on_part_data,
            "on_part_end": self.on_part_end,
            "on_end": self.on_end,
        }


def _error(status_code: int, code: str) -> JSONResponse:
    return JSONResponse(status_code=status_code, content={"detail": code})


async def _read_body(request: Request, max_bytes: int) -> bytes | None:
    """Read the request body up to ``max_bytes``; return None when over limit."""
    data = bytearray()
    async for chunk in request.stream():
        data += chunk
        if len(data) > max_bytes:
            return None
    return bytes(data)


def _build_provider(settings: Settings) -> LlmProvider:
    model = settings.llm_model
    if "/" not in model:
        model = f"{settings.llm_provider}/{model}"
    return LiteLlmProvider(
        model=model,
        api_key=settings.llm_api_key,
        timeout=settings.llm_timeout_seconds,
    )


@app.post("/internal/v1/parse")
async def parse_document(
    _token: Annotated[None, Depends(require_internal_token)],
    request: Request,
):
    """Stream and parse an uploaded PDF, after authentication.

    The multipart body is parsed incrementally and must contain exactly one
    complete ``file`` part terminated by the correct final boundary. Size and
    header budgets are enforced on actual streamed bytes; no temporary files
    are used.
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
    parser = MultipartParser(
        boundary,
        accumulator.callbacks(),
        max_header_count=MAX_HEADER_COUNT,
        max_header_size=float("inf"),
    )

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

    try:
        parser.finalize()
    except MultipartParseError:
        return _error(400, "invalid_multipart")

    if not accumulator.message_completed:
        return _error(400, "invalid_multipart")
    if accumulator.total_part_count != 1:
        return _error(400, "invalid_multipart")
    if accumulator.file_part_count != 1:
        return _error(400, "invalid_multipart")
    if accumulator.completed_part_count != 1:
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
    request: Request,
):
    """Analyze a parsed document and return a cited TenderAnalysisV1 report.

    Authentication runs before the JSON body is read; the body is bounded to
    ``MAX_REQUEST_BYTES`` by actual streamed bytes, then parsed and validated.
    """
    data = await _read_body(request, MAX_REQUEST_BYTES)
    if data is None:
        return _error(413, "request_too_large")

    try:
        text = data.decode("utf-8")
    except UnicodeDecodeError:
        return _error(400, "invalid_request")

    try:
        payload = json.loads(text)
        analyze_request = AnalyzeRequest.model_validate(payload)
    except (json.JSONDecodeError, ValidationError):
        return _error(400, "invalid_request")

    settings = get_settings()
    provider = _build_provider(settings)
    try:
        report = await run_analysis(
            analyze_request.document,
            provider,
            settings.llm_model,
            model_provider=settings.llm_provider,
        )
    except AnalysisError as exc:
        return JSONResponse(status_code=502, content={"detail": exc.code})

    return report
