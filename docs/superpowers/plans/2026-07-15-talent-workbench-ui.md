# 餐饮人才 AI 双向匹配工作台实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在公开 `/talent` 路径交付可切换招聘/求职、可解析自然语言条件、可解释匹配并可在当前页面查看详情的餐饮人才 UI 原型。

**Architecture:** 继续使用 Axum 路由与 Askama 服务端渲染，Rust 提供两组确定性原型记录和安全转义 JSON，原生 ES modules 负责条件解析、筛选、评分、模式切换与无障碍抽屉。页面复用机会模块的主题令牌和探索轨道，所有基础卡片由服务端渲染，使 JavaScript 不可用时仍能浏览。

**Tech Stack:** Rust 1.96.1、Axum 0.8.9、Askama 0.16.0、Serde、原生 JavaScript ES modules、Tailwind CSS 源样式、Playwright（Chromium/Firefox/WebKit）。

## Global Constraints

- 页面路径固定为 `/talent`，公开可访问；全局“人才”入口不再指向旧 `/`。
- 页面持续显示原文：`界面原型数据，不代表真实招聘或求职信息`。
- 招聘与求职各至少 8 条确定性原型记录；不得使用聊天记录中的真实姓名、电话、微信、企业名称或可识别照片。
- 联系方式只显示脱敏占位与“登录后申请联系”的禁用说明，不实现解锁。
- 不实现真实大模型 API、语义检索、n8n/Tavily/Firecrawl、数据库迁移、登录、收藏、消息、认证、会员、支付或订阅。
- 无条件时显示“等待输入条件”，不得生成虚假高匹配结论。
- 匹配权重固定为：岗位 20、地区 20、薪资 20、技能/菜系 15、经验 10、食宿 5、月休 5、到岗/紧急度 5，总分 100；风险提示不参与加分。
- 桌面、390px 手机、键盘、屏幕阅读器、`prefers-reduced-motion`、Chromium、Firefox、WebKit 均需覆盖。
- 不新增运行时依赖，不访问生产网络，不删除或迁移旧 GitJobs 招聘页面。

---

## 文件结构

**新增**

- `gitjobs-server/src/templates/platform/talent.rs`：人才记录类型、原型数据、页面模板上下文和 JSON 序列化。
- `gitjobs-server/src/handlers/platform/talent.rs`：公开人才页面处理器及服务端渲染测试。
- `gitjobs-server/templates/platform/talent_home.html`：页面壳、原型边界、JSON 数据与模块初始化。
- `gitjobs-server/templates/platform/components/talent_command_bar.html`：模式切换、AI 指令、条件标签、撤销和结果状态。
- `gitjobs-server/templates/platform/components/talent_results.html`：招聘/求职服务端卡片和无脚本基础内容。
- `gitjobs-server/templates/platform/components/talent_detail_drawer.html`：详情抽屉语义结构和脱敏联系状态。
- `gitjobs-server/static/js/platform/talent-matching.js`：纯函数解析、筛选、评分、原因与风险生成。
- `gitjobs-server/static/js/platform/talent-workbench.js`：模式、条件历史、卡片排序和详情数据协调。
- `gitjobs-server/static/js/platform/talent-drawer.js`：抽屉打开/关闭、焦点陷阱、滚动锁定与焦点恢复。
- `tests/e2e/public/talent-workbench.spec.ts`：三浏览器、移动、键盘、降级和回归验收。

**修改**

- `gitjobs-server/src/router.rs`：注册公开 `/talent` 路由。
- `gitjobs-server/src/handlers/platform/mod.rs`：导出 `talent` 处理器模块。
- `gitjobs-server/src/templates/platform/mod.rs`：导出 `talent` 模板模块。
- `gitjobs-server/src/templates/mod.rs`：为 `PageId` 增加 `Talent`。
- `gitjobs-server/templates/platform/components/exploration_rail.html`：机会/人才链接统一并根据 `PageId` 设置当前项。
- `gitjobs-server/static/js/platform/exploration-rail.js`：让机会页和人才页共享主题根节点及事件。
- `gitjobs-server/static/css/styles.src.css`：增加人才工作台、卡片、匹配摘要、抽屉和响应式样式。
- `tests/e2e/public/opportunity-home.spec.ts`：验证机会页“人才”入口改为 `/talent` 且原体验不回退。

---

### Task 1: 公开路由、确定性数据与服务端基础页面

**Files:**
- Create: `gitjobs-server/src/templates/platform/talent.rs`
- Create: `gitjobs-server/src/handlers/platform/talent.rs`
- Create: `gitjobs-server/templates/platform/talent_home.html`
- Create: `gitjobs-server/templates/platform/components/talent_command_bar.html`
- Create: `gitjobs-server/templates/platform/components/talent_results.html`
- Create: `gitjobs-server/templates/platform/components/talent_detail_drawer.html`
- Modify: `gitjobs-server/src/templates/platform/mod.rs`
- Modify: `gitjobs-server/src/handlers/platform/mod.rs`
- Modify: `gitjobs-server/src/templates/mod.rs`
- Modify: `gitjobs-server/src/router.rs`

**Interfaces:**
- Produces: `TalentHomePage`, `TalentRecord`, `TalentMode`, `prototype_hiring_records()`, `prototype_seeking_records()`, `talent::page`, `GET /talent`。
- `TalentRecord` fields: `id`, `mode`, `title`, `scene`, `region`, `district`, `salary_min`, `salary_max`, `skills`, `experience_years`, `hours_per_day`, `rest_days`, `lodging`, `housing_allowance`, `availability`, `urgency`, `credibility`, `source_label`, `updated_label`, `risk_flags`, `prototype`。
- Later tasks consume one JSON object shaped as `{ hiring: TalentRecord[], seeking: TalentRecord[] }` with camelCase keys.

- [ ] **Step 1: Write failing handler and data-contract tests**

Add tests in `gitjobs-server/src/templates/platform/talent.rs` that assert both fixture lists have exactly 8 unique IDs, every record has `prototype == true`, no record contains a digit run matching `\d{7,}`, and serialized `</script>` becomes `\u003c/script>`. Add a handler test in `gitjobs-server/src/handlers/platform/talent.rs`:

```rust
#[tokio::test]
async fn talent_page_renders_public_prototype_and_accessible_fallback() {
    let router = TestRouterBuilder::new(MockDB::new(), MockNotificationsManager::new())
        .build()
        .await;
    let response = router
        .oneshot(Request::builder().uri("/talent").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let html = String::from_utf8(
        to_bytes(response.into_body(), usize::MAX).await.unwrap().to_vec(),
    )
    .unwrap();
    assert!(html.contains("界面原型数据，不代表真实招聘或求职信息"));
    assert!(html.contains("data-talent-mode=\"hiring\""));
    assert!(html.contains("data-talent-mode=\"seeking\""));
    assert_eq!(html.matches("data-talent-card").count(), 16);
    assert!(html.contains("id=\"talent-detail-drawer\""));
    assert!(html.contains("aria-live=\"polite\""));
    assert!(html.contains("<html lang=\"zh-CN\""));
    assert!(html.contains("<title>AI 餐饮人才匹配工作台</title>"));
}
```

- [ ] **Step 2: Run the focused tests and verify RED**

Run:

```bash
cargo test -p gitjobs-server talent
```

Expected: compilation fails because `platform::talent`, `/talent`, `TalentHomePage` and fixture functions do not exist.

- [ ] **Step 3: Implement the data model and exact prototype fixture policy**

Create `TalentMode` with serialized values `hiring` and `seeking`, and `TalentRecord` using `#[serde(rename_all = "camelCase")]`. Populate eight hiring IDs `proto-hire-01`…`proto-hire-08` and eight seeking IDs `proto-seek-01`…`proto-seek-08` across 北京、天津、河北、广东、江苏、浙江、四川、安徽. Cover 炒锅、切配、打荷、洗碗、面点、烧烤、服务员、店长. Use only generic scenes such as `连锁餐饮门店（原型）` and aliases such as `候选人 A（原型）`; set `source_label` to `平台演示数据`, contact content nowhere in the records, and at least two explicit risk flags such as `薪资构成未拆分（原型）` or `技能证明未核验（原型）`.

Use this page context:

```rust
#[derive(Debug, Clone, Template)]
#[template(path = "platform/talent_home.html")]
pub(crate) struct TalentHomePage {
    pub auth_provider: Option<String>,
    pub cfg: Config,
    pub hiring_records: Vec<TalentRecord>,
    pub seeking_records: Vec<TalentRecord>,
    pub records_json: String,
    pub page_id: PageId,
    pub user: User,
}
```

Reuse `serialize_json_for_script` from `platform::opportunities`; do not duplicate escaping logic.

- [ ] **Step 4: Implement route, handler, template and no-JavaScript cards**

Add `PageId::Talent`, export both new modules, add:

```rust
.route("/talent", get(platform::talent::page))
```

The handler constructs both fixture lists once, serializes `{ "hiring": ..., "seeking": ... }`, renders `TalentHomePage`, and uses the same ten-minute response cache policy as `/opportunities`.

The page template must include `exploration_rail.html`, the three new talent components, the exact prototype note, and a `<script id="talent-data" type="application/json">`. Render all 16 cards server-side; initial `hiring` cards are visible and `seeking` cards use `hidden`. Each card includes its factual fields, a `<button data-open-talent-detail>` and the neutral non-score state `等待输入条件`.

- [ ] **Step 5: Run focused tests and verify GREEN**

Run:

```bash
cargo test -p gitjobs-server talent
```

Expected: all tests whose names contain `talent` pass.

- [ ] **Step 6: Commit Task 1**

```bash
git add gitjobs-server/src gitjobs-server/templates/platform
git commit -s -m "feat: add talent workbench route and prototype data"
```

---

### Task 2: 双向模式、指令解析、筛选与可解释评分

**Files:**
- Create: `gitjobs-server/static/js/platform/talent-matching.js`
- Create: `gitjobs-server/static/js/platform/talent-workbench.js`
- Modify: `gitjobs-server/templates/platform/talent_home.html`
- Modify: `gitjobs-server/templates/platform/components/talent_command_bar.html`
- Modify: `gitjobs-server/templates/platform/components/talent_results.html`
- Test: `tests/e2e/public/talent-workbench.spec.ts`

**Interfaces:**
- `parseTalentCommand(input: string, mode: "hiring" | "seeking"): TalentCondition[]`.
- `scoreTalentRecord(record: TalentRecord, conditions: TalentCondition[]): MatchResult`.
- `filterAndRankTalent(records: TalentRecord[], conditions: TalentCondition[]): RankedTalent[]`.
- `MatchResult` shape: `{ score: number | null, reasons: string[], misses: string[], risks: string[] }`.
- `initializeTalentWorkbench(data)` emits `talent:selection` with `{ record, match }` for Task 3.

- [ ] **Step 1: Write failing browser tests for modes and filtering**

Create tests that assert:

```ts
await expect(page.getByRole("button", { name: "招聘机会" })).toHaveAttribute("aria-pressed", "true");
await expect(page.locator('[data-talent-card][data-talent-mode="hiring"]:visible')).toHaveCount(8);
await expect(page.getByText("等待输入条件").first()).toBeVisible();

await page.getByLabel("描述招聘或求职需求").fill("北京找切配，工资7000以上，包吃住，月休4天");
await page.getByRole("button", { name: "生成匹配" }).click();
await expect(page.locator("#talent-condition-chips button")).toHaveCount(5);
await expect(page.locator("#talent-status")).toContainText("条匹配结果");
await expect(page.locator("[data-talent-match-score]:visible").first()).not.toHaveText("等待输入条件");

await page.getByRole("button", { name: "求职人才" }).click();
await expect(page.locator('[data-talent-card][data-talent-mode="seeking"]:visible')).toHaveCount(8);
await expect(page.locator("#talent-condition-chips")).toContainText("北京");
await expect(page.locator("#talent-condition-chips")).toContainText("切配");
```

Also assert one chip can be removed, undo restores it, Enter submits, an impossible condition announces no result, and switching mode preserves only common keys `region`, `role`, `salaryMin`, `skills`.

- [ ] **Step 2: Run Chromium test and verify RED**

Run:

```bash
npx playwright test --config tests/e2e/playwright.config.ts tests/e2e/public/talent-workbench.spec.ts --project=chromium
```

Expected: tests fail because the workbench modules and live conditions do not exist.

- [ ] **Step 3: Implement deterministic parsing**

Recognize these exact groups without remote calls:

```js
const REGION_PATTERN = /(北京|天津|河北|广东|江苏|浙江|四川|安徽)/;
const ROLE_PATTERN = /(炒锅|切配|打荷|洗碗|面点|烧烤|服务员|店长)/;
const SKILL_PATTERN = /(川湘菜|家常菜|清真|火锅|早餐|面食|刀工)/g;
const salary = input.match(/(?:工资|薪资)?\s*(\d{4,5})\s*(?:元|以上|起)?/);
const experience = input.match(/(\d+)\s*年(?:经验|从业)?/);
const restDays = input.match(/月休\s*(\d+)\s*天/);
```

Map `包吃住|包食宿` to `lodging=true`, `随时上岗|随时到岗` to `availability=immediate`, and `急招|急聘` to `urgency=urgent`. Return stable keys so chips can be compared and preserved across mode changes.

- [ ] **Step 4: Implement the fixed 100-point scoring contract**

Only score dimensions that have a corresponding condition. For each present condition, add its fixed weight on a hit and append a Chinese hit/miss explanation. Compute display percentage as `round(hitWeight / activeWeight * 100)` so a single satisfied condition is 100 without pretending unrequested dimensions were assessed. With zero active conditions return:

```js
{ score: null, reasons: [], misses: [], risks: record.riskFlags }
```

Sort scored records by descending `score`, then descending `credibility`, then stable original order. Filter out a record only for hard conflicts in role, region, or salary minimum; preferences such as lodging and monthly rest remain visible with a miss explanation.

- [ ] **Step 5: Implement workbench state, chips, undo and synchronized cards**

Keep state in one object:

```js
const state = {
  mode: "hiring",
  conditions: [],
  history: [],
  selectedId: null,
};
```

Every mutation pushes the previous conditions to `history`. Rendering must update mode buttons, chip buttons, `hidden` card state, DOM order, result count, score text, up to two recommendation reasons, risks, and an `aria-live="polite"` status from the same `filterAndRankTalent` result. An unrecognized command retains the input, keeps existing conditions, and displays `未识别新条件，可尝试“北京找切配，工资7000以上”`.

- [ ] **Step 6: Run Chromium test and verify GREEN**

Run the same Playwright command from Step 2.

Expected: all talent workbench Chromium tests written in this task pass.

- [ ] **Step 7: Commit Task 2**

```bash
git add gitjobs-server/static/js/platform gitjobs-server/templates/platform tests/e2e/public/talent-workbench.spec.ts
git commit -s -m "feat: add explainable talent matching interactions"
```

---

### Task 3: 当前页详情抽屉、脱敏联系状态与键盘焦点

**Files:**
- Create: `gitjobs-server/static/js/platform/talent-drawer.js`
- Modify: `gitjobs-server/static/js/platform/talent-workbench.js`
- Modify: `gitjobs-server/templates/platform/components/talent_detail_drawer.html`
- Modify: `tests/e2e/public/talent-workbench.spec.ts`

**Interfaces:**
- `initializeTalentDrawer()` returns `{ open(record, match, trigger), close() }`.
- `open` consumes the exact `TalentRecord` and `MatchResult` interfaces from Task 2.
- Drawer root is `#talent-detail-drawer`, dialog panel is `[data-talent-dialog]`, close control is `[data-close-talent-detail]`.

- [ ] **Step 1: Write failing drawer accessibility tests**

Add one desktop test and one 390px test. The desktop test opens the first card and asserts `role="dialog"`, `aria-modal="true"`, exact facts, hit/miss sections, risk section, `联系方式：1**********（原型占位）`, and disabled text `登录后申请联系（功能未开放）`. Press Tab repeatedly and assert focus never leaves the dialog; press Escape and assert the drawer closes, body scrolling is restored, and focus returns to the originating card button.

The mobile test asserts the dialog bounding box occupies at least 90% of viewport width and no horizontal page overflow exists.

- [ ] **Step 2: Run focused drawer tests and verify RED**

Run:

```bash
npx playwright test --config tests/e2e/playwright.config.ts tests/e2e/public/talent-workbench.spec.ts --project=chromium --grep "详情抽屉|drawer"
```

Expected: tests fail because no drawer controller opens or manages focus.

- [ ] **Step 3: Implement drawer rendering and safe text insertion**

Render all record-derived values with `textContent` and `replaceChildren`; never use `innerHTML`. Populate definition-list facts, `命中条件`, `未命中条件`, `风险与缺失字段`, `来源说明`, and `更新时间`. If `match.score === null`, show `等待输入条件` and explain that no AI conclusion was generated.

- [ ] **Step 4: Implement dialog behavior**

On open: store the trigger, remove `hidden`, set `data-open="true"`, add `overflow: hidden` through a body class, focus the close button, and install `keydown`. Tab/Shift+Tab cycles through enabled buttons/links/inputs inside the panel. Escape closes. Clicking backdrop closes; clicking inside the panel does not. On close: set `hidden`, remove the body class and listener, then focus the stored trigger if still connected.

- [ ] **Step 5: Run focused drawer tests and verify GREEN**

Run the same command from Step 2.

Expected: desktop and mobile drawer tests pass with no console errors.

- [ ] **Step 6: Commit Task 3**

```bash
git add gitjobs-server/static/js/platform/talent-drawer.js gitjobs-server/static/js/platform/talent-workbench.js gitjobs-server/templates/platform/components/talent_detail_drawer.html tests/e2e/public/talent-workbench.spec.ts
git commit -s -m "feat: add accessible talent detail drawer"
```

---

### Task 4: 共享导航、专业视觉、明暗主题与响应式降级

**Files:**
- Modify: `gitjobs-server/templates/platform/components/exploration_rail.html`
- Modify: `gitjobs-server/static/js/platform/exploration-rail.js`
- Modify: `gitjobs-server/static/css/styles.src.css`
- Modify: `tests/e2e/public/talent-workbench.spec.ts`
- Modify: `tests/e2e/public/opportunity-home.spec.ts`

**Interfaces:**
- Exploration rail chooses current link from `PageId::Opportunities` or `PageId::Talent`.
- `initializeExplorationRail({ appId, storageKey, eventName })` defaults to existing opportunity values, preserving backward compatibility.
- Talent root is `#talent-app`, uses existing `--opp-*` design tokens, and dispatches `talent:theme`.

- [ ] **Step 1: Write failing navigation, theme, responsive and reduced-motion tests**

Assert the opportunity page talent links resolve to `/talent`; talent desktop rail marks 人才 current and 390px mobile nav has five items `机会、人才、AI、消息、我的`. Toggle dark mode, reload, and assert persistence under `talent-theme`. At 1280px cards use a two-column workspace with a compact match rail; at 390px cards are one column and the drawer becomes a near-full-screen bottom panel. Under reduced motion, computed transition and animation durations are at most `0.01ms` and all facts remain visible.

Add keyboard focus contrast assertions: focus outline ratio ≥ 3:1 against the page background and primary button text ratio ≥ 4.5:1 against its background.

- [ ] **Step 2: Run targeted browser tests and verify RED**

Run:

```bash
npx playwright test --config tests/e2e/playwright.config.ts tests/e2e/public/talent-workbench.spec.ts tests/e2e/public/opportunity-home.spec.ts --project=chromium
```

Expected: new navigation and talent presentation expectations fail.

- [ ] **Step 3: Make the exploration rail reusable**

Change both talent links from `/` to `/talent`. Use Askama conditions so only the current page receives `aria-current="page"`. Parameterize theme initialization while retaining these opportunity defaults:

```js
export const initializeExplorationRail = ({
  appId = "opportunity-app",
  storageKey = "opportunity-theme",
  eventName = "opportunity:theme",
} = {}) => {
  const app = document.getElementById(appId);
  const toggle = document.getElementById("opportunity-theme-toggle");
  if (!app || !toggle) return;
  const stored = localStorage.getItem(storageKey);
  app.dataset.theme = stored === "dark" || stored === "light" ? stored : "light";
  const syncLabel = () =>
    toggle.setAttribute(
      "aria-label",
      app.dataset.theme === "dark" ? "切换浅色专业模式" : "切换深色沉浸模式",
    );
  syncLabel();
  toggle.addEventListener("click", () => {
    app.dataset.theme = app.dataset.theme === "dark" ? "light" : "dark";
    localStorage.setItem(storageKey, app.dataset.theme);
    syncLabel();
    window.dispatchEvent(new CustomEvent(eventName, { detail: app.dataset.theme }));
  });
};
```

Talent initializes it with `{ appId: "talent-app", storageKey: "talent-theme", eventName: "talent:theme" }`.

- [ ] **Step 4: Implement talent styles with the existing design system**

Add `.talent-*` rules after the opportunity rules. Reuse `--opp-bg`, `--opp-surface`, `--opp-text`, `--opp-muted`, `--opp-border`, `--opp-interactive`, and `.opportunity-focus`. Desktop uses `max-width: 78rem`, dense fact grids, restrained purple AI accents, green trust labels and orange urgency labels. Do not use decorative gradients behind every card, gendered colors, stock photos or a second font family.

At `max-width: 1000px`, collapse supporting columns; at `max-width: 767px`, hide the desktop rail and show mobile navigation; at `max-width: 640px`, use one card column and a bottom drawer. Under `prefers-reduced-motion: reduce`, disable displacement, pulse and slide while keeping immediate visibility changes.

- [ ] **Step 5: Run targeted browser tests and verify GREEN**

Run the same command from Step 2.

Expected: talent and opportunity Chromium tests pass.

- [ ] **Step 6: Commit Task 4**

```bash
git add gitjobs-server/templates/platform/components/exploration_rail.html gitjobs-server/static/js/platform/exploration-rail.js gitjobs-server/static/css/styles.src.css tests/e2e/public
git commit -s -m "feat: polish responsive talent workbench UI"
```

---

### Task 5: 三浏览器验收、旧页面回归与质量门禁

**Files:**
- Modify only when a verification defect requires it: `gitjobs-server/src/templates/platform/talent.rs`
- Modify only when a verification defect requires it: `gitjobs-server/src/handlers/platform/talent.rs`
- Modify only when a verification defect requires it: `gitjobs-server/templates/platform/talent_home.html`
- Modify only when a verification defect requires it: `gitjobs-server/templates/platform/components/talent_command_bar.html`
- Modify only when a verification defect requires it: `gitjobs-server/templates/platform/components/talent_results.html`
- Modify only when a verification defect requires it: `gitjobs-server/templates/platform/components/talent_detail_drawer.html`
- Modify only when a verification defect requires it: `gitjobs-server/static/js/platform/talent-matching.js`
- Modify only when a verification defect requires it: `gitjobs-server/static/js/platform/talent-workbench.js`
- Modify only when a verification defect requires it: `gitjobs-server/static/js/platform/talent-drawer.js`
- Modify only when a verification defect requires it: `gitjobs-server/static/css/styles.src.css`
- Verify: `tests/e2e/public/talent-workbench.spec.ts`
- Verify: `tests/e2e/public/opportunity-home.spec.ts`
- Verify: `tests/e2e/public/jobboard.spec.ts`

**Interfaces:**
- No new interfaces; this task only fixes defects exposed by the complete acceptance matrix without expanding scope.

- [ ] **Step 1: Run Rust format, tests and strict Clippy**

Run:

```bash
cargo fmt --all -- --check
cargo test
cargo clippy --all-targets --all-features -- --deny warnings
```

Expected: all commands exit 0; Rust test count is at least the 213-test baseline plus the new talent tests.

- [ ] **Step 2: Run template and JavaScript formatting checks**

Run:

```bash
djlint --check --configuration gitjobs-server/templates/.djlintrc gitjobs-server/templates
prettier --config gitjobs-server/static/js/.prettierrc.yaml --check "gitjobs-server/static/js/**/*.js" "tests/e2e/**/*.ts"
```

Expected: both commands exit 0 with no file requiring formatting.

- [ ] **Step 3: Run the talent suite in all three browsers**

Run:

```bash
npx playwright test --config tests/e2e/playwright.config.ts tests/e2e/public/talent-workbench.spec.ts --project=chromium
npx playwright test --config tests/e2e/playwright.config.ts tests/e2e/public/talent-workbench.spec.ts --project=firefox
npx playwright test --config tests/e2e/playwright.config.ts tests/e2e/public/talent-workbench.spec.ts --project=webkit
```

Expected: every talent test passes in each project, with zero browser console errors.

- [ ] **Step 4: Run opportunity and legacy recruitment regression**

Run:

```bash
npx playwright test --config tests/e2e/playwright.config.ts tests/e2e/public/opportunity-home.spec.ts
npx playwright test --config tests/e2e/playwright.config.ts tests/e2e/public/jobboard.spec.ts --project=chromium
```

Expected: the opportunity suite passes in Chromium, Firefox and WebKit; the legacy jobboard Chromium suite passes.

- [ ] **Step 5: Run production build and repository hygiene checks**

Run:

```bash
cargo build -p gitjobs-server
git diff --check
git status --short
```

Expected: build exits 0, `git diff --check` emits nothing, and status contains only intentional task files before the final commit.

- [ ] **Step 6: Perform independent review before claiming completion**

Review the final branch against every Global Constraint and the design spec. Reject any real contact detail, misleading AI/verification claim, inaccessible dialog, unmatched score explanation, missing browser coverage or new dependency. Record review findings and address all Critical/Important items before proceeding.

- [ ] **Step 7: Commit verification fixes**

```bash
git add gitjobs-server tests/e2e
git commit -s -m "test: verify talent workbench across browsers"
```

If Step 6 produced no file changes, do not create an empty commit; preserve the existing task commits and report the verified commit hashes.

---

## Self-Review Record

- Spec coverage: route, two modes, 16 fixtures, conditions, undo, shared filters, scoring, explanations, risks, drawer, contact masking, theme, responsive behavior, reduced motion, keyboard, screen-reader semantics, JavaScript fallback and three-browser regression each map to Tasks 1–5.
- Incomplete-instruction scan: every code-changing step names its concrete behavior, command, expected result and neighboring interface.
- Type consistency: Tasks 2 and 3 both consume `TalentRecord`, `TalentCondition` and `MatchResult`; mode values remain exactly `hiring`/`seeking`; DOM IDs and custom event names are defined once and reused consistently.
- Scope control: real AI, collection Agent, database, auth, contact unlock, payment and subscription remain excluded.
