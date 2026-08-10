"""Internal Bearer authentication using constant-time comparison."""

from __future__ import annotations

import secrets
from typing import Annotated

from fastapi import Depends, HTTPException
from fastapi.security import HTTPAuthorizationCredentials, HTTPBearer

from app.config import Settings, get_settings  # noqa: F401 — re-exported for main

_bearer_scheme = HTTPBearer(auto_error=False)


async def require_internal_token(
    credentials: Annotated[
        HTTPAuthorizationCredentials | None,
        Depends(_bearer_scheme),
    ],
    settings: Annotated[Settings, Depends(get_settings)],
) -> None:
    supplied = (
        credentials.credentials
        if credentials and credentials.scheme.lower() == "bearer"
        else ""
    )
    if not secrets.compare_digest(
        supplied, settings.internal_service_token.get_secret_value()
    ):
        raise HTTPException(
            status_code=401, detail="invalid internal credentials"
        )
