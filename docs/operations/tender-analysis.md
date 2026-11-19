# 招标文件分析：运行与安全边界

这是 GitJobs 主站的内部能力，不是第二套网站。用户在主站 `/tenders` 登录、建立项目、上传 PDF、查看状态和报告。Rust 服务独占私有文件、任务和报告；Python 服务只在一次请求中解析/分析，不连接业务数据库，也不提供公开用户入口。

## 启用前

1. 在主站配置 `server.tender.enabled: true`，并设置绝对路径 `document_dir`。目录应位于受限磁盘/卷上，只有主站进程可读写；不要把它挂到 Python 容器或公开静态目录。主站会创建目录并在 Unix 上设为 `0700`，新文件为 `0600`。
2. 设置 `analysis_base_url` 为 HTTPS 内部服务源地址（只有 scheme、host、可选端口和 `/`）。仅本机开发可配 `allow_local_http: true` 且地址必须是 `localhost`、`127.0.0.1` 或 `::1`。不要把分析服务直接发布到公网。容器间通信若不是 loopback，需在私有网络前配置 TLS 终止；不能用 `allow_local_http` 放开普通容器域名。
3. 为 Rust 的 `server.tender.internal_service_token` 与 Python 的 `INTERNAL_SERVICE_TOKEN` 注入同一个随机生成、至少 32 字符的密钥。通过密钥管理器或进程环境注入，不写进仓库、日志、镜像或命令行。本文出现的 `<INVALID-...>` 都不是可用凭证。
4. Rust 的 `server.tender.model_provider` / `model_name` 必须与 Python 的 `LLM_PROVIDER` / `LLM_MODEL` 相同。更换供应商或模型会改变新任务的配置指纹；旧报告保留，新上传可创建新的分析任务。Python 的 `LLM_API_KEY` 仅放在服务进程环境中。
5. `max_upload_bytes` 最多 52,428,800（50 MiB），`request_timeout_seconds` 最多 300。Python 端还限制 1,000 页、提取文本 10 MiB、请求体与响应体 64 MiB。主站不把文件名作为磁盘路径，数据库只保存随机存储键和校验值。

## 容器

从仓库根目录构建：`docker build -f services/tender-analysis/Dockerfile -t tender-analysis:local .`。容器以非 root 用户运行，只复制分析服务源码与依赖清单。部署时建议只读根文件系统及临时 `/tmp`，限制网络出口到批准的模型供应商，不挂主站 PDF 卷。`GET /internal/health` 只返回固定状态；解析与分析接口必须带内部 Bearer 密钥。当前 `pyproject.toml` 用最低版本约束而非完整锁文件；生产镜像应固定基础镜像 digest 与依赖解析结果后再发布。

## 用户可见状态与恢复

`queued → parsing → analyzing → completed/partial`。损坏、加密、页数过多、文本过大或扫描版 `ocr_required` 会失败，页面给出固定说明。扫描版不自动伪造 OCR 结果；需要用户提供可检索文本版或后续启用 OCR。分析服务或网络短暂故障会在同一任务内进行一次受租约约束的快速重试；未恢复时由数据库租约过期后重新领取，总领取次数最多三次，随后失败。失败任务可在页面点“重新分析”，创建同文件同配置的新任务；成功或部分完成任务会复用现有报告，避免重复模型费用。

任务领取凭证与版本在数据库中栅栏校验。旧执行器即使晚返回，也不能完成或覆盖新领取者的结果。原始 PDF 由 Rust 保存，暂时无法读取文件时只存安全错误码，不存异常正文。若上传文件已存在，未被数据库采纳的新存储副本会删除。运维仍应定期检查私有目录中不被 `tender_documents.storage_key` 引用的文件；删除前先核对路径、备份及数据库状态，不要对整个目录做盲目递归清理。

## 密钥轮换与供应商

先把新密钥写入密钥管理器，在受控维护窗口同时重启 Rust/Python 服务，确认 `/internal/health` 和带密钥的内部调用。轮换期间旧任务可因 `analysis_unavailable` 延后，失败后再由用户重试。Python LiteLLM 可配置 GPT、Claude、DeepSeek 等模型；切换前审查供应商的数据保留、训练使用、区域与合同条款，尤其是可能包含证照和商业信息的招标文件。未完成数据外发审查时应关闭 `server.tender.enabled`，主站其他功能不受影响。

合同版本固定为 `tender-analysis.v1`；五个提示词版本见 `services/tender-analysis/app/prompts.py`。更改输出结构或提示词时应升级版本并重新运行跨语言契约测试。报告中的结论必须能由页码、chunk ID 和逐字原文验证；缺失、冲突与无依据内容进入“待人工确认”，不得显示为已确认事实。

## 故障排查

- 上传返回 413：检查单份 50 MiB 与整体 64 MiB 限额；不要只信任 `Content-Length`。
- `invalid_pdf` / `encrypted_pdf`：请上传未损坏、未加密的 PDF。
- `ocr_required`：原件主要是图片；当前版本不自动 OCR。
- `analysis_unavailable`：检查私有服务可达性、内部密钥、模型供应商可用性与超时；不要在日志中打印原文件、完整模型响应或密钥。
- `invalid_analysis_contract`：服务输出结构或引用不可信，报告不会以完成状态入库；检查版本配置及脱敏诊断。
- 长时间处于解析/分析中：检查 10 分钟领取租约、worker 日志和数据库任务状态；异常退出后等待租约到期再领取。

验收必须用合成或获授权且脱敏的文件；常规自动化测试不调用付费真实模型。用户做投标决策前仍需核对 PDF 原件和待人工确认项。
