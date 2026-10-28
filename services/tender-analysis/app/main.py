"""FastAPI application — internal tender analysis service.

No CORS, no user auth, no database.  Only internal Bearer-authenticated
endpoints with Task 1 returning 501 shells for parse/analyze.
"""

from __future__ import annotations

from collections.abc import AsyncGenerator
from contextlib import asynccontextmanager
from typing import Annotated

from fastapi import Depends, FastAPI, File, UploadFile
from fastapi.responses import JSONResponse

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


MAX_UPLOAD_BYTES = 50 * 1024 * 1024  # 50 MiB


@app.post("/internal/v1/parse")
async def parse_document(
    _token: Annotated[None, Depends(require_internal_token)],
    file: Annotated[UploadFile, File(...)],
):
    """Parse an uploaded PDF into page-preserving text.

    Authentication runs before any file bytes are read. Content-level failures
    return a ``ParseResponse(status="failed")`` with a safe fixed code; an
    oversized upload is rejected before parsing with a fixed 413 response
    carrying ``file_too_large``.
    """
    data = await file.read(MAX_UPLOAD_BYTES + 1)
    if len(data) > MAX_UPLOAD_BYTES:
        return JSONResponse(status_code=413, content={"detail": "file_too_large"})

    try:
        parsed = parse_pdf(data)
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
