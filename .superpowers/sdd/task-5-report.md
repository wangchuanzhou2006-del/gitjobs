# Task 5 实施报告：机会星图与同步降级卡片流

## 状态

完成。桌面正常能力使用 ECharts canvas 机会星图；移动端、低动效、`saveData`、低内存与 ECharts 不可用时自动切换到同步卡片流。星图与卡片流消费同一批节点与筛选事件，并保留明显的“界面原型数据，不代表真实项目”说明。

## TDD：RED

Windows 环境无 `just`，使用 justfile 的等价命令：

```powershell
$env:BASE_URL='http://127.0.0.1:8080'
npx playwright test --config tests/e2e/playwright.config.ts tests/e2e/public/opportunity-home.spec.ts --project=chromium -g 'constellation|reduced motion|card flow'
```

结果：4 项既有测试通过，3 项新增测试按预期失败，服务连接正常。

- 桌面星图：`#opportunity-graph canvas` 不存在。
- 低动效：`#opportunity-app` 缺少 `data-experience="fallback"`（收到 `null`）。
- 移动端：`#opportunity-app` 缺少 `data-experience="fallback"`（收到 `null`）。

## 构建 RED 与根因修复

生产文件首次落盘后，主代理重启服务时 Tailwind 稳定报错：`Missing closing } at .opportunity-visual-heading, .opportunity-list-heading`。检查 Task 5 新增 CSS diff 与文件尾部，确认补丁被截断在 `flex-direction: column;`，缺少该规则和末尾 `@media` 的两个闭合花括号。

按系统化调试只补入两个 `}`，未改动其他 CSS。Prettier 随后可成功解析 CSS（仅报告整份历史文件存在风格差异，未执行全文件写入），`git diff --check` 无错误；主代理再次启动服务成功，8080 返回 200 且页面包含 `opportunity-graph`。

## GREEN

服务重启确认后运行完整 Chromium：

```powershell
$env:BASE_URL='http://127.0.0.1:8080'
npx playwright test --config tests/e2e/playwright.config.ts tests/e2e/public/opportunity-home.spec.ts --project=chromium
```

结果：`7 passed (4.0s)`，0 failed。

Scoped 格式与差异检查：

- 新增 JS 与 E2E：scoped Prettier 写入并复查。
- CSS：Prettier 成功解析；为避免机械格式化整份历史 CSS，未执行全文件写入。
- 模板：本机没有 `djlint` 可执行文件，未安装新依赖；服务重启编译与完整 E2E 已验证模板。
- `git diff --check`：通过。

## 改动文件

- `gitjobs-server/templates/platform/components/opportunity_graph.html`
- `gitjobs-server/templates/platform/components/opportunity_fallback.html`
- `gitjobs-server/templates/platform/opportunity_home.html`
- `gitjobs-server/static/js/platform/opportunity-graph.js`
- `gitjobs-server/static/css/styles.src.css`
- `tests/e2e/public/opportunity-home.spec.ts`
- `.superpowers/sdd/task-5-report.md`

## 提交

- `feat: add accessible opportunity constellation`（本报告与实现同一 Signed-off-by 提交）

## 自检

- 单一 module bootstrap，严格按 exploration rail → graph → command bar 顺序初始化，未丢失 command bar 首次发布事件。
- `chartNode` 使用 serde camelCase 字段 `daysLeft`。
