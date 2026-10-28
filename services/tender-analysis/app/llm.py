"""Async LLM provider protocol and LiteLLM adapter."""

from __future__ import annotations

import json
from typing import Protocol, TypeVar

from pydantic import BaseModel, ValidationError

T = TypeVar("T", bound=BaseModel)

MAX_MODEL_OUTPUT_BYTES = 4 * 1024 * 1024  # 4 MiB


class AnalysisError(Exception):
    """A safe, fixed error code for model/provider failures."""

    def __init__(self, code: str) -> None:
        self.code = code
        super().__init__(code)


class LlmProvider(Protocol):
    async def complete_structured(
        self, system_prompt: str, user_payload: str, response_model: type[T]
    ) -> T: ...


class LiteLlmProvider:
    """LiteLLM-backed provider; never leaks model text or provider errors."""

    def __init__(self, model: str, api_key=None, timeout: float = 90.0) -> None:
        self.model = model
        self.api_key = api_key
        self.timeout = timeout

    async def complete_structured(
        self, system_prompt: str, user_payload: str, response_model: type[T]
    ) -> T:
        import litellm  # lazy import so tests never touch litellm unless needed

        api_key = None
        if self.api_key is not None:
            getter = getattr(self.api_key, "get_secret_value", None)
            api_key = getter() if callable(getter) else self.api_key

        try:
            response = await litellm.acompletion(
                model=self.model,
                messages=[
                    {"role": "system", "content": system_prompt},
                    {"role": "user", "content": user_payload},
                ],
                temperature=0,
                timeout=self.timeout,
                api_key=api_key,
            )
            content = response.choices[0].message.content
        except Exception:
            raise AnalysisError("analysis_unavailable")

        if not isinstance(content, str) or len(content.encode("utf-8")) > MAX_MODEL_OUTPUT_BYTES:
            raise AnalysisError("analysis_unavailable")

        try:
            data = json.loads(content)
        except (json.JSONDecodeError, ValueError):
            raise AnalysisError("analysis_unavailable")

        try:
            return response_model.model_validate(data)
        except ValidationError:
            raise AnalysisError("analysis_unavailable")
