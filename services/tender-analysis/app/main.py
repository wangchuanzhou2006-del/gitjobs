"""FastAPI application — internal tender analysis service.

No CORS, no user auth, no database.  Only internal Bearer-authenticated
endpoints with Task 1 returning 501 shells for parse/analyze.
"""

from __future__ import annotations

from collections.abc import AsyncGenerator
from contextlib import asynccontextmanager
from typing import Annotated

from fastapi import Depends, FastAPI
from fastapi.responses import JSONResponse

from app.auth import require_internal_token
from app.config import Settings


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


@app.post("/internal/v1/parse")
async def parse_document(
    _token: Annotated[None, Depends(require_internal_token)],
):
    """Parse an uploaded PDF into page-preserving text. (Task 2)"""
    return JSONResponse(
        content={"detail": "not implemented"}, status_code=501
    )


@app.post("/internal/v1/analyze")
async def analyze_document(
    _token: Annotated[None, Depends(require_internal_token)],
):
    """Analyze a parsed document and return TenderAnalysisV1. (Task 3)"""
    return JSONResponse(
        content={"detail": "not implemented"}, status_code=501
    )
