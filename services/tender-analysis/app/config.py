"""Fail-closed environment settings for the internal tender analysis service."""

from __future__ import annotations

from pydantic import SecretStr, field_validator
from pydantic_settings import BaseSettings, SettingsConfigDict


class Settings(BaseSettings):
    model_config = SettingsConfigDict(
        env_file=".env",
        env_file_encoding="utf-8",
        extra="forbid",
    )

    internal_service_token: SecretStr

    llm_provider: str = "openai"
    llm_model: str = ""
    llm_api_key: SecretStr | None = None
    llm_timeout_seconds: float = 90.0

    @field_validator("internal_service_token")
    @classmethod
    def _token_min_length(cls, v: SecretStr) -> SecretStr:
        if len(v.get_secret_value()) < 32:
            raise ValueError("internal_service_token must be at least 32 characters")
        return v

    def __repr__(self) -> str:
        return (
            "Settings("
            f"internal_service_token={self.internal_service_token!r}, "
            f"llm_provider={self.llm_provider!r}, "
            f"llm_model={self.llm_model!r}, "
            f"llm_api_key={self.llm_api_key!r})"
        )


def get_settings() -> Settings:
    return Settings()
