# 给 Claude 的 Task 01 修正任务 01：收紧证据契约并验证单一事实源

Task 01 尚未通过 Codex 审核。不要开始 Task 02，不要实现 PDF 解析、RAG 或 LLM 调用。本次只修正已经提交的契约、测试和工作区卫生问题。

## 必须先阅读

1. `docs/superpowers/specs/2026-08-10-ai-tender-document-analysis-integration-design.md`
2. `docs/superpowers/plans/2026-08-10-ai-tender-document-analysis-integration.md` 的 Global Constraints 与 Task 1
3. `docs/superpowers/prompts/2026-08-10-tender-analysis-task-01.md`
4. 提交 `66dd7e7` 的实际差异

先报告当前分支、`git status --short` 和本次计划修改的准确文件。不得清理或覆盖用户已有的其他未跟踪文件。

## 审核发现与复现证据

当前实现错误接受以下内容：

```text
ACCEPTED confirmed_fact_without_citation
ACCEPTED qualification_without_citation
ACCEPTED risk_without_citation
ACCEPTED material_without_citation
ACCEPTED arbitrary_prompt_versions
```

另外，Pydantic 的 `ParseResponse.parsed_document` 和 `error_code` 接受显式 `null`，手写 JSON Schema 对应字段不接受 `null`。现有测试只用 Pydantic 验证 fixture，从未使用提交的 JSON Schema，因此“JSON Schema、Pydantic 与 fixture 完全一致”的完成声明没有证据。

Fresh verification also found:

```text
python -m pytest tests/test_auth.py tests/test_contracts.py -q -> 35 passed
python -m ruff check app tests -> passed
python -m ruff format --check app tests -> failed; 4 files would be reformatted
```

运行测试后还产生未跟踪的 `services/tender-analysis/**/__pycache__/`，根 `.gitignore` 当前没有 Python 缓存规则。

## 允许修改的文件

- `contracts/tender-analysis.v1.schema.json`
- `contracts/fixtures/tender-analysis.v1.json`（仅当收紧契约需要同步）
- `services/tender-analysis/pyproject.toml`
- `services/tender-analysis/app/contracts.py`
- `services/tender-analysis/tests/test_contracts.py`
- `services/tender-analysis/app/auth.py`、`config.py`、`main.py`（仅运行 Ruff 格式化，不改行为）
- 根目录 `.gitignore`

不得修改其他文件；不得修改旧原型 `C:\Users\25351\ai-bid-assistant`。

## 必须修正 1：关键结论必须有证据

实现并测试以下不变量：

- `EvidenceValue.value` 非 `None` 时，`citations` 至少一项。
- `EvidenceValue.value` 为 `None` 时，`requires_review` 必须为 `True`；`citations` 可以为空，也可以引用发生冲突的原文。
- `Qualification.citations` 至少一项。
- `Risk.citations` 至少一项，因为风险事实必须可追溯；判断、影响和建议仍与事实字段分开。
- `RequiredMaterial.citations` 至少一项。
- `UnknownItem.reason == "missing"` 时允许空引用；其他未知原因可以有引用，不强制伪造引用。
- `Citation.page_number` 必须与 `chunk_id` 中的页码一致。
- `DocumentChunk.page_number` 必须与自身 `chunk_id` 中的页码一致。

使用 `Field(min_length=1, max_length=20)` 和必要的 `@model_validator(mode="after")`，不得只依赖提示词约束。

先增加会在当前实现上失败的测试，并实际运行以留下红阶段证据。

## 必须修正 2：解析结构不能自相矛盾

实现并测试：

- `ParsedDocument.page_count` 至少为1，最多1000。
- `pages` 长度必须等于 `page_count`。
- 页面必须按1开始连续编号，不能重复、跳号或乱序。
- 每个 `ParsedPage` 内的 chunk 必须属于该页。
- `ParseResponse.status` 只能是 `completed` 或 `failed`。
- `completed` 必须携带 `parsed_document` 且不能携带 `error_code`。
- `failed` 必须携带已批准的安全错误码且不能携带 `parsed_document`。
- 安全错误码固定为 `invalid_pdf`、`encrypted_pdf`、`too_many_pages`、`extracted_text_too_large`、`ocr_required`。

可使用模型校验器实现状态与字段组合约束。测试必须覆盖合法成功、合法失败和所有矛盾组合。

## 必须修正 3：版本字段必须固定

将 `PromptVersions` 改为固定 Literal：

- `tender-project-info-v1`
- `tender-qualifications-v1`
- `tender-risks-v1`
- `tender-materials-v1`
- `tender-review-v1`

`AnalysisMeta.completeness` 只能是 `complete` 或 `partial`。增加测试证明任意其他提示词版本和完整度会被拒绝。

## 必须修正 4：JSON Schema 由 Pydantic 契约生成并得到真实验证

不要继续人工维护一份可能漂移的报告 Schema。建立一个确定性的 schema 生成函数，以 `TenderAnalysisV1.model_json_schema(mode="validation")` 为唯一结构来源，并补充固定的 `$schema`、`$id`、标题和描述。提交的 `contracts/tender-analysis.v1.schema.json` 必须等于该函数生成的 JSON 对象。

测试必须：

1. 比较已提交 JSON Schema 与生成结果，任何字段漂移都失败。
2. 使用声明在 dev dependencies 中的 `jsonschema` 验证 fixture。
3. 同一 fixture 同时通过 Pydantic 和 JSON Schema。
4. 无引用的资格、风险、材料和已确认事实同时被两者拒绝。
5. 错误提示词版本同时被两者拒绝。

如果公共 JSON Schema 只描述最终 `TenderAnalysisV1`，不要在其中保留未被报告根类型引用的 `ParseResponse` 或 `ParsedDocument` 定义；这些是内部接口模型，由 Pydantic测试负责。Schema 文件名和根契约仍为 `tender-analysis.v1`。

## 必须修正 5：格式和缓存卫生

- 在根 `.gitignore` 增加通用 Python 忽略项：`__pycache__/`、`*.py[cod]`、`.pytest_cache/`、`.ruff_cache/`、`.venv/`。
- 不要删除或改动其他未跟踪用户文件。
- 对 `services/tender-analysis/app` 和 `tests` 运行 Ruff formatter，使 format check 通过。

## 验证命令

依次运行并报告退出码：

```text
cd services/tender-analysis
python -m pytest tests/test_auth.py tests/test_contracts.py -q
python -m ruff check app tests
python -m ruff format --check app tests
```

再运行一个最小负例脚本，证明以下五项均输出 `REJECTED`：

- confirmed fact without citation
- qualification without citation
- risk without citation
- material without citation
- arbitrary prompt versions

最后运行 `git diff --check`。

## 提交和停止

只提交本次允许修改的文件：

```text
git commit -s -m "fix: harden tender analysis contract"
```

提交后停止，不得开始 Task 02。最终报告必须包含文件清单、红阶段证据、全部绿阶段命令与退出码、负例输出、提交哈希和偏差说明。
