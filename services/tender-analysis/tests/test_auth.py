"""Authentication tests for the internal tender analysis service."""

from __future__ import annotations

import secrets

import pytest
from httpx import ASGITransport, AsyncClient


@pytest.fixture
def anyio_backend():
    return "asyncio"


@pytest.fixture
def app():
    from app.main import app

    return app


@pytest.fixture
def valid_token():
    return "test-token-" + secrets.token_hex(16)


@pytest.fixture
def settings_with_token(monkeypatch, valid_token):
    monkeypatch.setenv("INTERNAL_SERVICE_TOKEN", valid_token)
    from app.config import Settings

    return Settings()


# ── Token redaction ───────────────────────────────────────────


def test_settings_token_repr_does_not_leak_cleartext(settings_with_token, valid_token):
    """Settings repr must not expose the cleartext token."""
    r = repr(settings_with_token)
    assert valid_token not in r


def test_settings_token_str_does_not_leak_cleartext(settings_with_token, valid_token):
    """Settings str must not expose the cleartext token."""
    s = str(settings_with_token)
    assert valid_token not in s


def test_settings_token_access_via_get_secret_value(settings_with_token, valid_token):
    """The real token is only accessible via get_secret_value()."""
    assert settings_with_token.internal_service_token.get_secret_value() == valid_token


# ── Health endpoint (no auth) ─────────────────────────────────


@pytest.mark.anyio
async def test_health_returns_200(app, monkeypatch, valid_token):
    """GET /internal/health requires no authentication."""
    monkeypatch.setenv("INTERNAL_SERVICE_TOKEN", valid_token)
    transport = ASGITransport(app=app)
    async with AsyncClient(transport=transport, base_url="http://test") as client:
        response = await client.get("/internal/health")
    assert response.status_code == 200
    data = response.json()
    assert data["status"] == "ok"


# ── Missing token → 401 ──────────────────────────────────────


@pytest.mark.anyio
async def test_parse_no_token_returns_401(app, monkeypatch, valid_token):
    """POST /internal/v1/parse without a token must return 401."""
    monkeypatch.setenv("INTERNAL_SERVICE_TOKEN", valid_token)
    transport = ASGITransport(app=app)
    async with AsyncClient(transport=transport, base_url="http://test") as client:
        response = await client.post("/internal/v1/parse")
    assert response.status_code == 401


@pytest.mark.anyio
async def test_analyze_no_token_returns_401(app, monkeypatch, valid_token):
    """POST /internal/v1/analyze without a token must return 401."""
    monkeypatch.setenv("INTERNAL_SERVICE_TOKEN", valid_token)
    transport = ASGITransport(app=app)
    async with AsyncClient(transport=transport, base_url="http://test") as client:
        response = await client.post("/internal/v1/analyze")
    assert response.status_code == 401


# ── Wrong token → 401 ─────────────────────────────────────────


@pytest.mark.anyio
async def test_parse_wrong_token_returns_401(app, monkeypatch, valid_token):
    """POST /internal/v1/parse with a wrong token must return 401."""
    monkeypatch.setenv("INTERNAL_SERVICE_TOKEN", valid_token)
    transport = ASGITransport(app=app)
    async with AsyncClient(transport=transport, base_url="http://test") as client:
        response = await client.post(
            "/internal/v1/parse",
            headers={"Authorization": "Bearer wrong-token-value"},
        )
    assert response.status_code == 401


@pytest.mark.anyio
async def test_analyze_wrong_token_returns_401(app, monkeypatch, valid_token):
    """POST /internal/v1/analyze with a wrong token must return 401."""
    monkeypatch.setenv("INTERNAL_SERVICE_TOKEN", valid_token)
    transport = ASGITransport(app=app)
    async with AsyncClient(transport=transport, base_url="http://test") as client:
        response = await client.post(
            "/internal/v1/analyze",
            headers={"Authorization": "Bearer wrong-token-value"},
        )
    assert response.status_code == 401


# ── Malformed auth header → 401 ───────────────────────────────


@pytest.mark.anyio
async def test_parse_non_bearer_scheme_returns_401(app, monkeypatch, valid_token):
    """A non-Bearer Authorization scheme must return 401."""
    monkeypatch.setenv("INTERNAL_SERVICE_TOKEN", valid_token)
    transport = ASGITransport(app=app)
    async with AsyncClient(transport=transport, base_url="http://test") as client:
        response = await client.post(
            "/internal/v1/parse",
            headers={"Authorization": f"Basic {valid_token}"},
        )
    assert response.status_code == 401


@pytest.mark.anyio
async def test_parse_malformed_auth_returns_401(app, monkeypatch, valid_token):
    """A malformed Authorization header must return 401."""
    monkeypatch.setenv("INTERNAL_SERVICE_TOKEN", valid_token)
    transport = ASGITransport(app=app)
    async with AsyncClient(transport=transport, base_url="http://test") as client:
        response = await client.post(
            "/internal/v1/parse",
            headers={"Authorization": "Bearer"},
        )
    assert response.status_code == 401


# ── Valid token → 501 (shell) ─────────────────────────────────


@pytest.mark.anyio
async def test_parse_valid_token_requires_file(app, monkeypatch, valid_token):
    """POST /internal/v1/parse with a valid token but no file returns 422."""
    monkeypatch.setenv("INTERNAL_SERVICE_TOKEN", valid_token)
    transport = ASGITransport(app=app)
    async with AsyncClient(transport=transport, base_url="http://test") as client:
        response = await client.post(
            "/internal/v1/parse",
            headers={"Authorization": f"Bearer {valid_token}"},
        )
    assert response.status_code == 422


@pytest.mark.anyio
async def test_analyze_valid_token_returns_501(app, monkeypatch, valid_token):
    """POST /internal/v1/analyze with valid token returns 501."""
    monkeypatch.setenv("INTERNAL_SERVICE_TOKEN", valid_token)
    transport = ASGITransport(app=app)
    async with AsyncClient(transport=transport, base_url="http://test") as client:
        response = await client.post(
            "/internal/v1/analyze",
            headers={"Authorization": f"Bearer {valid_token}"},
        )
    assert response.status_code == 501


# ── Consistent error message ──────────────────────────────────


@pytest.mark.anyio
async def test_401_response_body_is_consistent(app, monkeypatch, valid_token):
    """All 401 responses must return the same fixed detail message."""
    monkeypatch.setenv("INTERNAL_SERVICE_TOKEN", valid_token)
    transport = ASGITransport(app=app)
    async with AsyncClient(transport=transport, base_url="http://test") as client:
        r1 = await client.post("/internal/v1/parse")
        r2 = await client.post(
            "/internal/v1/parse",
            headers={"Authorization": "Bearer wrong"},
        )
        r3 = await client.post(
            "/internal/v1/analyze",
            headers={"Authorization": "Basic xyz"},
        )
    assert r1.status_code == 401
    assert r2.status_code == 401
    assert r3.status_code == 401
    detail = r1.json()["detail"]
    assert detail == "invalid internal credentials"
    assert r2.json()["detail"] == detail
    assert r3.json()["detail"] == detail
