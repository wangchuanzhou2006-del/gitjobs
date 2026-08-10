# 给 Claude 的任务 01：建立分析契约与无状态服务外壳

你正在现有 GitJobs 项目中执行一个严格限定的单模块任务。不要重新设计产品，不要扩展范围，不要创建第二套完整项目。

## 工作位置

唯一允许写入的仓库：

`C:\Users\25351\gitjobs-candidate\.worktrees\foundation-validation`

旧原型只允许读取和参考：

`C:\Users\25351\ai-bid-assistant`

不得修改、移动或删除旧原型中的任何文件。

## 开始前必须阅读

完整阅读以下文件后才能修改代码：

1. `CLAUDE.md`
2. `docs/superpowers/specs/2026-08-10-ai-tender-document-analysis-integration-design.md`
3. `docs/superpowers/plans/2026-08-10-ai-tender-document-analysis-integration.md` 中的 Global Constraints、Locked File Structure、Task 1 和 Claude Execution Protocol
4. 只读参考：`C:\Users\25351\ai-bid-assistant\pyproject.toml`
5. 只读参考：`C:\Users\25351\ai-bid-assistant\app\schemas\document.py`
6. 只读参考：`C:\Users\25351\ai-bid-assistant\app\main.py`

阅读后，先报告：

- 当前分支名和 `git status --short`。
- 你理解的任务目标。
- 计划创建的准确文件列表。
- 你将明确排除的内容。

如果当前工作区存在会与本任务文件重叠的未提交改动，立即停止并报告，不要覆盖或清理用户文件。

## 本任务唯一目标

完成实施计划的 Task 1：

- 建立 `tender-analysis.v1` 严格结构化契约。
- 建立无状态 FastAPI 内部服务外壳。
- 实现内部 Bearer 身份验证。
- 添加契约和认证测试。
- parse/analyze 端点在本任务只提供经过认证的 `501 Not Implemented` 外壳。

## 允许创建的文件

只能创建或修改以下文件：

- `contracts/tender-analysis.v1.schema.json`
- `contracts/fixtures/tender-analysis.v1.json`
- `services/tender-analysis/pyproject.toml`
- `services/tender-analysis/app/__init__.py`
- `services/tender-analysis/app/config.py`
- `services/tender-analysis/app/auth.py`
- `services/tender-analysis/app/contracts.py`
- `services/tender-analysis/app/main.py`
- `services/tender-analysis/tests/test_auth.py`
- `services/tender-analysis/tests/test_contracts.py`

如果确实需要新增 `services/tender-analysis/tests/__init__.py` 以解决测试导入问题，必须先说明原因；除此之外不得修改任何文件。

## 明确禁止

- 不得复制旧项目整个目录。
- 不得添加 React、Vite、Ant Design 或其他前端。
- 不得添加注册、登录、JWT、用户表或 `/api/v1/auth`。
- 不得添加 SQLAlchemy、Alembic、PostgreSQL、SQLite 或 `dev.db`。
- 不得添加 Redis、Celery、任务队列或 Chroma 持久化。
- 不得添加 CORS。
- 不得实现 PDF 解析、RAG、LLM 调用或真实分析；这些属于后续任务。
- 不得修改 Rust、数据库迁移、Docker、README 或现有配置。
- 不得顺手格式化、重构或修复无关文件。
- 不得把可用密钥写入代码、测试、示例或提交记录。

## 契约要求

所有 Pydantic 模型必须使用 `ConfigDict(extra="forbid")`。至少定义并组合：

- `Citation`
- `DocumentChunk`
- `ParsedPage`
- `ParsedDocument`
- `ParseResponse`
- `AnalyzeRequest`
- `EvidenceValue`
- `ProjectInfo`
- `Qualification`
- `Risk`
- `RequiredMaterial`
- `UnknownItem`
- `AnalysisMeta`
- `TenderAnalysisV1`

固定要求：

- `schema_version` 只能是 `tender-analysis.v1`。
- 页码为 1–1000。
- `chunk_id` 格式为 `page-{页码}-chunk-{序号}`。
- 置信度为 0–1。
- `Qualification.kind` 只能是 `hard_qualification`、`scoring_item`、`submission_material`、`contract_performance`、`general_note`。
- `Risk.severity` 只能是 `low`、`medium`、`high`。
- `UnknownItem.reason` 只能是 `missing`、`conflict`、`low_confidence`、`unsupported`。
- 关键列表设置明确的最大长度。
- JSON Schema、Pydantic字段和合成fixture必须完全一致。
- fixture必须是虚构中文招标信息，至少包含项目名称、预算、一个硬性资格、一个风险、一项材料、一个未知项和引用；不得使用真实企业或真实个人数据。

## 服务与认证要求

- `GET /internal/health` 不需要认证，只返回固定健康状态，不泄露配置。
- `POST /internal/v1/parse` 和 `POST /internal/v1/analyze` 必须使用同一个内部认证依赖。
- 本任务中两个POST端点通过认证后返回安全的 `501`。
- `internal_service_token` 必填，最少32字符，使用 `SecretStr`，`repr` 和序列化输出不得出现明文。
- Bearer token比较必须使用 `secrets.compare_digest`。
- 缺少、格式错误或不匹配的认证统一返回401和安全固定消息。
- 不得在日志或异常中打印收到的token。

## 测试驱动步骤

严格执行红—绿流程：

1. 先写 `test_auth.py` 和 `test_contracts.py`。
2. 运行测试并保留预期失败证据。
3. 实现最小代码使测试通过。
4. 运行：

   `cd services/tender-analysis && python -m pytest tests/test_auth.py tests/test_contracts.py -q`

5. 运行：

   `cd services/tender-analysis && python -m ruff check app tests`

6. 运行一次凭证泄露检查，确认测试输出、异常字符串和模型 `repr` 不包含测试token明文。

不得为了让测试变绿而放宽 `extra="forbid"`、页码、枚举、置信度或认证约束。

## 完成与停止条件

完成后只提交本任务文件：

`git commit -s -m "feat: add tender analysis service contract"`

然后停止，不得开始 Task 2。最终报告必须包含：

1. 创建/修改文件清单。
2. 红阶段失败命令与失败原因。
3. 绿阶段每条验证命令、退出码和结果。
4. 提交哈希。
5. 与计划的任何偏差；没有偏差时明确写“无偏差”。
6. 明确确认没有修改旧原型、没有加入第二套登录/数据库/前端/队列。
