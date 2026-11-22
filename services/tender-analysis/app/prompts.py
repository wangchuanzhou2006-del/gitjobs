"""Five immutable, versioned system prompts for tender analysis."""

from __future__ import annotations

_COMMON_RULES = (
    "只陈述输入片段明确支持的事实。禁止使用常识补全。\n"
    "每个关键结论必须引用 chunk_id、1-based 页码和逐字原文。\n"
    "资格准入、评分加分、提交材料、合同履约和普通说明必须分开。\n"
    "信息缺失、互相冲突或置信度不足时写入 unknowns，禁止猜测。\n"
    "事实依据、风险判断、影响和建议必须分别输出。"
)

PROJECT_INFO_PROMPT = (
    "tender-project-info-v1\n"
    + _COMMON_RULES
    + "\n只提取项目名称、采购单位、预算金额、预算币种、服务期限、投标截止时间。"
    "缺失的字段将 value 置为 null 且 requires_review=true。只输出严格 JSON。"
)

QUALIFICATIONS_PROMPT = (
    "tender-qualifications-v1\n"
    + _COMMON_RULES
    + "\n提取资格要求，kind 只能是 hard_qualification、scoring_item、submission_material、"
    "contract_performance、general_note 之一。硬性准入与评分加分必须分开。只输出严格 JSON。"
)

RISKS_PROMPT = (
    "tender-risks-v1\n"
    + _COMMON_RULES
    + "\n基于事实提取风险，severity 只能是 low、medium、high。事实、判断、影响、建议分开。"
    "只输出严格 JSON。"
)

MATERIALS_PROMPT = (
    "tender-materials-v1\n" + _COMMON_RULES + "\n提取需要提交的材料清单。只输出严格 JSON。"
)

REVIEW_PROMPT = (
    "tender-review-v1\n"
    + _COMMON_RULES
    + "\n复核上述提取结果：只能保留、删除或降级已有结论，不能新增事实。"
    "冲突和缺失写入 unknowns。只输出严格 JSON。"
)

PROMPT_BY_STAGE = {
    "project_info": PROJECT_INFO_PROMPT,
    "qualifications": QUALIFICATIONS_PROMPT,
    "risks": RISKS_PROMPT,
    "materials": MATERIALS_PROMPT,
    "review": REVIEW_PROMPT,
}
