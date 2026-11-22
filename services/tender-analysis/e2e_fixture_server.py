"""Test-only ASGI entry point using the real parser and analyzer.

The provider below is deterministic and cannot call an external model. This
module is outside the packaged ``app`` package and is used only by E2E CI.
"""

from __future__ import annotations

import re

from app import main
from app.config import Settings
from app.llm import AnalysisError

_CHUNK = re.compile(r"\[(page-\d+-chunk-\d+) \| page (\d+)\]\n(.*?)(?=\n\n\[|\Z)", re.S)
_BUDGET = "Budget amount: CNY 2800000"
_DEADLINE = "Bid deadline: 2026-09-01"


def _evidence(context: str, phrase: str, value: str, *, forged: bool = False) -> dict:
    for chunk_id, page, text in _CHUNK.findall(context):
        if phrase in text:
            return {
                "value": value,
                "confidence": 0.95,
                "requires_review": False,
                "citations": [
                    {
                        "chunk_id": chunk_id,
                        "page_number": int(page),
                        "quote": "Invented budget: CNY 9999999" if forged else phrase,
                    }
                ],
            }
    raise AnalysisError("analysis_unavailable")


def _missing() -> dict:
    return {"value": None, "confidence": 0, "requires_review": True, "citations": []}


class _FixtureProvider:
    async def complete_structured(self, system_prompt, user_payload, response_model):
        version = system_prompt.splitlines()[0].strip()
        if version == "tender-project-info-v1":
            return response_model.model_validate(
                {
                    "project_name": _missing(),
                    "procurement_entity": _missing(),
                    "budget_amount": _evidence(
                        user_payload,
                        _BUDGET,
                        "CNY 2800000",
                        forged="E2E_FORGED_BID" in user_payload,
                    ),
                    "budget_currency": _evidence(user_payload, _BUDGET, "CNY"),
                    "service_period": _missing(),
                    "bid_deadline": _evidence(user_payload, _DEADLINE, "2026-09-01"),
                }
            )
        if version in {
            "tender-qualifications-v1",
            "tender-risks-v1",
            "tender-materials-v1",
        }:
            return response_model.model_validate({"items": []})
        if version == "tender-review-v1":
            return response_model.model_validate({})
        raise AnalysisError("analysis_unavailable")


def _build_fixture_provider(settings: Settings) -> _FixtureProvider:
    if settings.llm_provider != "e2e" or settings.llm_model != "fixture-model":
        raise AnalysisError("analysis_unavailable")
    return _FixtureProvider()


main._build_provider = _build_fixture_provider
app = main.app
