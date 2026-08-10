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

    @field_validator("internal_service_token")
    @classmethod
    def _token_min_length(cls, v: SecretStr) -> SecretStr:
        if len(v.get_secret_value()) < 32:
            raise ValueError("internal_service_token must be at least 32 characters")
        return v

    def __repr__(self) -> str:
        return (
            f"Settings(internal_service_token={self.internal_service_token!r})"
        )


def get_settings() -> Settings:
    return Settings()
