# Opportunity Constellation UI Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a separately accessible, responsive `/opportunities` prototype that proves the AI opportunity constellation, exploration rail, command-driven filters, synchronized fallback list, and accessible light/dark presentation without adding payment or subscription enforcement.

**Architecture:** Add a focused `platform` page slice beside the existing job board so the prototype cannot regress the current `/` recruitment experience. Rust owns a deterministic, explicitly labelled prototype view model and renders accessible HTML through Askama; browser JavaScript progressively enhances the same data into an ECharts constellation and local command interaction, while the server-rendered list remains the source of accessible fallback content.

**Tech Stack:** Rust 1.96.1, Axum, Askama, Serde, Tailwind CSS 4.0.17 CLI, vanilla ES modules, ECharts 6.0.0, Playwright 1.61.1.

## Global Constraints

- Work only in the existing linked worktree on branch `codex/foundation-validation`; do not develop directly on `main`.
- Preserve the current GitJobs job board at `/`; this UI-foundation phase is exposed at `/opportunities` until real catering opportunity data is connected in a later phase.
- Use only deterministic prototype records defined in Rust and visibly label them `界面原型数据，不代表真实项目`; do not present prototype counts as production statistics.
- The constellation uses project type for color, credibility for halo strength, and deadline urgency for pulse speed; each visual encoding must also have a text equivalent.
- The AI command interaction is a local parsing prototype only: conditions must be visible, editable by removal, undoable, and synchronized with graph, list, and summary. Do not call an LLM API in this phase.
- Desktop primary navigation is `机会 / 人才 / 情报 / 企业`; mobile navigation is `机会 / 人才 / AI / 消息 / 我的`.
- Use deep ink blue and warm porcelain as the base; electric violet-blue for AI, signal orange for urgency, and emerald green for trustworthy sources.
- Provide independently designed light professional and dark immersive modes, and persist the user's explicit choice in `localStorage` under `opportunity-theme`.
- When ECharts is unavailable, `prefers-reduced-motion: reduce` is active, `navigator.connection.saveData` is true, `navigator.deviceMemory <= 4`, or the viewport is below 768 px, show the card flow as the primary experience.
- Search, condition removal, undo, result understanding, and project access must work without WebGL and without decorative animation.
- Keyboard focus must be visible; the graph must not be the only way to select a project; status changes use `aria-live="polite"`.
- Do not implement project detail, contact unlocking, entitlement checks, orders, payment, automatic renewal, or subscription paywalls.
- Do not rename the whole inherited product or replace all GitJobs metadata in this phase; add page-specific Chinese metadata through Askama blocks.
- Add no new runtime dependency. Reuse the vendored `/static/vendor/js/echarts.v6.0.0.min.js` and the existing hashed-asset build pipeline.
- Every commit uses sign-off: `git commit -s`.

## File Map

**Create**

- `gitjobs-server/src/templates/platform/mod.rs` — exports the platform template slice.
- `gitjobs-server/src/templates/platform/opportunities.rs` — owns `OpportunityHomePage`, `OpportunityNode`, `OpportunityKind`, and deterministic prototype records.
- `gitjobs-server/src/handlers/platform/mod.rs` — exports platform HTTP handlers.
- `gitjobs-server/src/handlers/platform/opportunities.rs` — renders the `/opportunities` page and contains its HTTP unit test.
- `gitjobs-server/templates/platform/opportunity_home.html` — assembles page metadata, shell, constellation, fallback list, and scripts.
- `gitjobs-server/templates/platform/components/exploration_rail.html` — desktop rail and mobile bottom navigation.
- `gitjobs-server/templates/platform/components/ai_command_bar.html` — command form, condition chips, undo, and live result status.
- `gitjobs-server/templates/platform/components/opportunity_graph.html` — graph host, legend, evidence explanation, and non-production data notice.
- `gitjobs-server/templates/platform/components/opportunity_fallback.html` — server-rendered accessible opportunity cards.
- `gitjobs-server/static/js/platform/ai-command-bar.js` — pure parsing/state functions and command-bar DOM controller.
- `gitjobs-server/static/js/platform/opportunity-graph.js` — ECharts rendering, text-equivalent synchronization, and fallback selection.
- `gitjobs-server/static/js/platform/exploration-rail.js` — rail collapse, theme selection, and persistence.
- `tests/e2e/public/opportunity-home.spec.ts` — desktop, keyboard, reduced-motion/fallback, persistence, and mobile coverage.

**Modify**

- `gitjobs-server/src/templates/mod.rs` — export `platform` and add `PageId::Opportunities`.
- `gitjobs-server/src/handlers/mod.rs` — export `platform`.
- `gitjobs-server/src/router.rs` — import `platform` and route `GET /opportunities`.
- `gitjobs-server/templates/base.html` — add overridable language/title/description/theme-color blocks while preserving defaults.
- `gitjobs-server/static/css/styles.src.css` — add opportunity tokens, shell primitives, graph/list states, focus, responsive, and reduced-motion rules.

---

### Task 1: Page Identity and Design Tokens

**Files:**
- Modify: `gitjobs-server/src/templates/mod.rs`
- Modify: `gitjobs-server/templates/base.html`
- Modify: `gitjobs-server/static/css/styles.src.css`
- Test: `gitjobs-server/src/templates/platform/opportunities.rs` (created in Task 2; Task 1 verification is compile/build based)

**Interfaces:**
- Consumes: existing `PageId`, Askama inheritance, and Tailwind `@theme` configuration.
- Produces: `PageId::Opportunities`; blocks `document_language`, `page_title`, `page_description`, and `theme_color`; CSS variables `--color-opportunity-*` and reusable `.opportunity-*` primitives.

- [ ] **Step 1: Add the page identity and overridable metadata blocks**

In `gitjobs-server/src/templates/mod.rs`, add the enum variant (the module declaration is added with its file in Task 2):

```rust
pub(crate) enum PageId {
    About,
    EmployerDashboard,
    JobBoard,
    JobSeekerDashboard,
    LogIn,
    ModeratorDashboard,
    NotFound,
    Opportunities,
    SignUp,
    Stats,
}
```

In `gitjobs-server/templates/base.html`, replace the fixed language, title, description, and theme color lines with blocks that keep current pages unchanged:

```html
<html lang="{% block document_language %}en{% endblock document_language %}"
      class="h-full min-h-dvh has-[div[role='dialog'][data-open='true']]:overflow-hidden">
  <head>
    <title>{% block page_title %}GitJobs{% endblock page_title %}</title>
    <meta name="description"
          content="{% block page_description %}GitJobs is an open source job board focused on open source job opportunities.{% endblock page_description %}">
    <meta name="theme-color"
          content="{% block theme_color %}#f9fafb{% endblock theme_color %}" />
```

Delete the old duplicate fixed `<meta name="theme-color" content="#f9fafb" />` lower in `<head>`.

- [ ] **Step 2: Add the complete visual token layer**

Append these values inside the existing `@theme` block in `gitjobs-server/static/css/styles.src.css`:

```css
  --font-opportunity: "Inter", "PingFang SC", "Microsoft YaHei", sans-serif;
  --color-opportunity-ink: #071426;
  --color-opportunity-ink-soft: #10243d;
  --color-opportunity-porcelain: #f7f3ea;
  --color-opportunity-porcelain-deep: #ece5d8;
  --color-opportunity-ai: #7357ff;
  --color-opportunity-ai-bright: #9c8cff;
  --color-opportunity-urgent: #ff7a3d;
  --color-opportunity-trust: #1ab887;
  --color-opportunity-line: #bdc8d5;
```

Append the following non-Tailwind component rules after the existing component layer:

```css
.opportunity-app {
  --opp-bg: var(--color-opportunity-porcelain);
  --opp-surface: rgb(255 253 248 / 82%);
  --opp-text: var(--color-opportunity-ink);
  --opp-muted: #526174;
  --opp-border: rgb(7 20 38 / 14%);
  min-height: 100dvh;
  color: var(--opp-text);
  background:
    radial-gradient(circle at 72% 16%, rgb(115 87 255 / 12%), transparent 30rem),
    linear-gradient(145deg, var(--opp-bg), #fffdf8 58%, #e8eee9);
  font-family: var(--font-opportunity);
}

.opportunity-app[data-theme="dark"] {
  --opp-bg: var(--color-opportunity-ink);
  --opp-surface: rgb(16 36 61 / 72%);
  --opp-text: #f7f3ea;
  --opp-muted: #aebbd0;
  --opp-border: rgb(247 243 234 / 16%);
  background:
    radial-gradient(circle at 70% 18%, rgb(115 87 255 / 22%), transparent 32rem),
    linear-gradient(145deg, #06101e, #0b1a2d 55%, #071426);
}

.opportunity-surface {
  border: 1px solid var(--opp-border);
  background: var(--opp-surface);
  box-shadow: 0 20px 55px rgb(7 20 38 / 10%);
  backdrop-filter: blur(18px);
}

.opportunity-focus:focus-visible {
  outline: 3px solid var(--color-opportunity-ai-bright);
  outline-offset: 3px;
}

.opportunity-app [data-urgency="high"] { --node-accent: var(--color-opportunity-urgent); }
.opportunity-app [data-credibility="high"] { --node-trust: var(--color-opportunity-trust); }

@media (prefers-reduced-motion: reduce) {
  .opportunity-app *,
  .opportunity-app *::before,
  .opportunity-app *::after {
    scroll-behavior: auto !important;
    animation-duration: 0.01ms !important;
    animation-iteration-count: 1 !important;
    transition-duration: 0.01ms !important;
  }
}
```

- [ ] **Step 3: Run formatting and the production asset build**

Run from the repository root:

```bash
just frontend-fmt-and-lint
cargo check -p gitjobs-server
```

Expected: `djlint` reports no template errors; Tailwind runs through `build.rs`; `cargo check` exits 0 with no missing `PageId` match.

- [ ] **Step 4: Commit the identity and token foundation**

```bash
git add gitjobs-server/src/templates/mod.rs gitjobs-server/templates/base.html gitjobs-server/static/css/styles.src.css
git commit -s -m "feat: add opportunity experience design tokens"
```

### Task 2: Deterministic Server View Model and Route

**Files:**
- Create: `gitjobs-server/src/templates/platform/mod.rs`
- Create: `gitjobs-server/src/templates/platform/opportunities.rs`
- Create: `gitjobs-server/src/handlers/platform/mod.rs`
- Create: `gitjobs-server/src/handlers/platform/opportunities.rs`
- Modify: `gitjobs-server/src/handlers/mod.rs`
- Modify: `gitjobs-server/src/router.rs`
- Create: `gitjobs-server/templates/platform/opportunity_home.html`

**Interfaces:**
- Consumes: `Config`, `PageId`, `User`, `AUTH_PROVIDER_KEY`, `Session`, `HttpServerConfig`.
- Produces: `OpportunityHomePage { auth_provider, cfg, nodes, nodes_json, page_id, user }`, `OpportunityNode`, `OpportunityKind`, `prototype_nodes() -> Vec<OpportunityNode>`, and `GET /opportunities`.

- [ ] **Step 1: Write the failing handler test**

Create `gitjobs-server/src/handlers/platform/opportunities.rs` with the handler signature and test first:

```rust
use anyhow::Result;
use askama::Template;
use axum::{extract::State, response::{Html, IntoResponse}};
use chrono::Duration;
use tower_sessions::Session;

use crate::{
    config::HttpServerConfig,
    handlers::{auth::AUTH_PROVIDER_KEY, error::HandlerError, prepare_headers},
    templates::{PageId, auth::User, platform::opportunities::{OpportunityHomePage, prototype_nodes}},
};

pub(crate) async fn page(
    session: Session,
    State(cfg): State<HttpServerConfig>,
) -> Result<impl IntoResponse, HandlerError> {
    let nodes = prototype_nodes();
    let template = OpportunityHomePage {
        auth_provider: session.get(AUTH_PROVIDER_KEY).await?,
        cfg: cfg.into(),
        nodes_json: serde_json::to_string(&nodes)?,
        nodes,
        page_id: PageId::Opportunities,
        user: User::default(),
    };
    let headers = prepare_headers(Duration::minutes(10), &[])?;
    Ok((headers, Html(template.render()?)))
}

#[cfg(test)]
mod tests {
    use axum::{body::{Body, to_bytes}, http::{Request, StatusCode}};
    use tower::ServiceExt;
    use crate::{db::mock::MockDB, handlers::tests::TestRouterBuilder, notifications::MockNotificationsManager};

    #[tokio::test]
    async fn opportunity_page_labels_prototype_data_and_renders_accessible_fallback() {
        let router = TestRouterBuilder::new(MockDB::new(), MockNotificationsManager::new()).build().await;
        let response = router.oneshot(Request::builder().uri("/opportunities").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let html = String::from_utf8(to_bytes(response.into_body(), usize::MAX).await.unwrap().to_vec()).unwrap();
        assert!(html.contains("界面原型数据，不代表真实项目"));
        assert!(html.contains("id=\"opportunity-fallback\""));
        assert!(html.contains("aria-live=\"polite\""));
    }
}
```

- [ ] **Step 2: Register empty modules and route, then confirm the test fails on missing templates/types**

Create both module files:

```rust
// gitjobs-server/src/templates/platform/mod.rs
pub(crate) mod opportunities;
```

```rust
// gitjobs-server/src/handlers/platform/mod.rs
pub(crate) mod opportunities;
```

Add `pub(crate) mod platform;` to both `gitjobs-server/src/templates/mod.rs` and `gitjobs-server/src/handlers/mod.rs`. In `gitjobs-server/src/router.rs`, add `platform` beside the existing handler imports and add this public route after `/`:

```rust
.route("/opportunities", get(platform::opportunities::page))
```

Run:

```bash
cargo test -p gitjobs-server opportunity_page_labels_prototype_data_and_renders_accessible_fallback
```

Expected: FAIL because `templates::platform::opportunities` and `platform/opportunity_home.html` do not exist yet.

- [ ] **Step 3: Implement the complete view model**

Create `gitjobs-server/src/templates/platform/opportunities.rs`:

```rust
use askama::Template;
use serde::{Deserialize, Serialize};
use crate::templates::{Config, PageId, auth::User};

#[derive(Debug, Clone, Template, Serialize, Deserialize)]
#[template(path = "platform/opportunity_home.html")]
pub(crate) struct OpportunityHomePage {
    pub auth_provider: Option<String>,
    pub cfg: Config,
    pub nodes: Vec<OpportunityNode>,
    pub nodes_json: String,
    pub page_id: PageId,
    pub user: User,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OpportunityNode {
    pub id: &'static str,
    pub title: &'static str,
    pub region: &'static str,
    pub city: &'static str,
    pub kind: OpportunityKind,
    pub headcount: u32,
    pub credibility: u8,
    pub days_left: u8,
    pub source_label: &'static str,
    pub prototype: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum OpportunityKind { University, Enterprise, Hospital, Park, Stall }

impl OpportunityKind {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::University => "高校",
            Self::Enterprise => "企业",
            Self::Hospital => "医院",
            Self::Park => "园区",
            Self::Stall => "档口",
        }
    }
}

pub(crate) fn prototype_nodes() -> Vec<OpportunityNode> {
    vec![
        OpportunityNode { id: "proto-gd-01", title: "广东高校食堂合作机会", region: "广东", city: "广州", kind: OpportunityKind::University, headcount: 12_000, credibility: 92, days_left: 5, source_label: "学校官网（原型字段）", prototype: true },
        OpportunityNode { id: "proto-bj-01", title: "北京园区团餐托管机会", region: "北京", city: "北京", kind: OpportunityKind::Park, headcount: 6_500, credibility: 84, days_left: 12, source_label: "园区公告（原型字段）", prototype: true },
        OpportunityNode { id: "proto-js-01", title: "江苏医院营养食堂合作机会", region: "江苏", city: "南京", kind: OpportunityKind::Hospital, headcount: 3_200, credibility: 88, days_left: 8, source_label: "医院官网（原型字段）", prototype: true },
        OpportunityNode { id: "proto-zj-01", title: "浙江企业员工餐服务机会", region: "浙江", city: "杭州", kind: OpportunityKind::Enterprise, headcount: 8_000, credibility: 76, days_left: 20, source_label: "企业公告（原型字段）", prototype: true },
        OpportunityNode { id: "proto-sc-01", title: "四川高校档口招商机会", region: "四川", city: "成都", kind: OpportunityKind::Stall, headcount: 18_000, credibility: 70, days_left: 3, source_label: "招商公告（原型字段）", prototype: true },
        OpportunityNode { id: "proto-hb-01", title: "湖北高校食堂合作机会", region: "湖北", city: "武汉", kind: OpportunityKind::University, headcount: 15_000, credibility: 81, days_left: 16, source_label: "学校官网（原型字段）", prototype: true },
    ]
}
```

- [ ] **Step 4: Add a minimal compiling page template**

Create `gitjobs-server/templates/platform/opportunity_home.html`:

```html
{% extends "base.html" %}
{% block document_language %}zh-CN{% endblock document_language %}
{% block page_title %}AI 餐饮机会星图{% endblock page_title %}
{% block page_description %}AI 驱动的餐饮项目发现与可信情报原型。{% endblock page_description %}
{% block theme_color %}#071426{% endblock theme_color %}
{% block content %}
  <div id="opportunity-app" class="opportunity-app" data-theme="light">
    <p role="note">界面原型数据，不代表真实项目</p>
    <section id="opportunity-fallback" aria-label="机会列表">
      {% for node in nodes %}<article data-opportunity-id="{{ node.id }}">{{ node.title }}</article>{% endfor %}
    </section>
    <p id="opportunity-status" aria-live="polite">当前展示 {{ nodes.len() }} 条原型机会</p>
    <script id="opportunity-data" type="application/json">{{ nodes_json|safe }}</script>
  </div>
{% endblock content %}
```

- [ ] **Step 5: Run the focused and full Rust checks**

```bash
cargo test -p gitjobs-server opportunity_page_labels_prototype_data_and_renders_accessible_fallback
cargo fmt --check
cargo clippy -p gitjobs-server --all-targets --all-features -- --deny warnings
```

Expected: focused test passes; formatting and Clippy exit 0.

- [ ] **Step 6: Commit the server slice**

```bash
git add gitjobs-server/src/templates gitjobs-server/src/handlers gitjobs-server/src/router.rs gitjobs-server/templates/platform/opportunity_home.html
git commit -s -m "feat: add opportunity prototype route"
```

### Task 3: Exploration Shell and Responsive Navigation

**Files:**
- Create: `gitjobs-server/templates/platform/components/exploration_rail.html`
- Modify: `gitjobs-server/templates/platform/opportunity_home.html`
- Create: `gitjobs-server/static/js/platform/exploration-rail.js`
- Modify: `gitjobs-server/static/css/styles.src.css`
- Create: `tests/e2e/public/opportunity-home.spec.ts`

**Interfaces:**
- Consumes: `#opportunity-app`, `data-theme`, `localStorage`.
- Produces: `initializeExplorationRail()`, `[data-testid="desktop-exploration-rail"]`, `[data-testid="mobile-opportunity-nav"]`, and persisted `opportunity-theme`.

- [ ] **Step 1: Write the failing desktop/mobile shell tests**

Create `tests/e2e/public/opportunity-home.spec.ts`:

```ts
import { expect, test } from "@playwright/test";
import { navigateWithRetries } from "../shared/helpers";

test.describe("opportunity constellation foundation", () => {
  test.beforeEach(async ({ page }) => navigateWithRetries(page, "/opportunities", "opportunity home"));

  test("shows the desktop exploration rail and persists dark mode", async ({ page }) => {
    await expect(page.getByTestId("desktop-exploration-rail")).toBeVisible();
    await expect(page.getByTestId("desktop-exploration-rail").getByText("机会", { exact: true })).toBeVisible();
    await page.getByRole("button", { name: "切换深色沉浸模式" }).click();
    await expect(page.locator("#opportunity-app")).toHaveAttribute("data-theme", "dark");
    await page.reload();
    await expect(page.locator("#opportunity-app")).toHaveAttribute("data-theme", "dark");
  });

  test("uses the five-item mobile navigation", async ({ page }) => {
    await page.setViewportSize({ width: 390, height: 844 });
    const nav = page.getByTestId("mobile-opportunity-nav");
    await expect(nav).toBeVisible();
    await expect(nav.getByRole("link")).toHaveCount(5);
    await expect(nav).toContainText("机会人才AI消息我的");
    await expect(page.getByTestId("desktop-exploration-rail")).toBeHidden();
  });
});
```

- [ ] **Step 2: Run the tests and verify the missing shell failure**

With the normal local test server running, run:

```bash
just e2e-tests tests/e2e/public/opportunity-home.spec.ts --project=chromium
```

Expected: FAIL because the two navigation test IDs and theme button do not exist.

- [ ] **Step 3: Create the complete navigation partial**

Create `gitjobs-server/templates/platform/components/exploration_rail.html` with semantic links and non-functional future entries explicitly marked:

```html
<aside class="opportunity-rail opportunity-surface" data-testid="desktop-exploration-rail" aria-label="主要探索">
  <a class="opportunity-brand opportunity-focus" href="/opportunities" aria-label="餐饮情报机会首页">餐饮<br>情报</a>
  <nav aria-label="桌面主导航">
    <a class="opportunity-focus" href="/opportunities" aria-current="page">机会</a>
    <a class="opportunity-focus" href="/" title="现有招聘与求职">人才</a>
    <span aria-disabled="true" title="后续阶段开放">情报</span>
    <span aria-disabled="true" title="后续阶段开放">企业</span>
  </nav>
  <button id="opportunity-theme-toggle" class="opportunity-focus" type="button" aria-label="切换深色沉浸模式">明暗</button>
</aside>

<nav class="opportunity-mobile-nav opportunity-surface" data-testid="mobile-opportunity-nav" aria-label="移动主导航">
  <a class="opportunity-focus" href="/opportunities" aria-current="page">机会</a>
  <a class="opportunity-focus" href="/">人才</a>
  <a class="opportunity-focus" href="#ai-command-input">AI</a>
  <span aria-disabled="true">消息</span>
  <span aria-disabled="true">我的</span>
</nav>
```

- [ ] **Step 4: Implement rail/theme behavior**

Create `gitjobs-server/static/js/platform/exploration-rail.js`:

```js
const THEME_KEY = "opportunity-theme";

export const initializeExplorationRail = () => {
  const app = document.getElementById("opportunity-app");
  const toggle = document.getElementById("opportunity-theme-toggle");
  if (!app || !toggle) return;

  const stored = localStorage.getItem(THEME_KEY);
  const initial = stored === "dark" || stored === "light" ? stored : "light";
  app.dataset.theme = initial;

  const syncLabel = () => toggle.setAttribute(
    "aria-label",
    app.dataset.theme === "dark" ? "切换浅色专业模式" : "切换深色沉浸模式",
  );
  syncLabel();

  toggle.addEventListener("click", () => {
    app.dataset.theme = app.dataset.theme === "dark" ? "light" : "dark";
    localStorage.setItem(THEME_KEY, app.dataset.theme);
    syncLabel();
    window.dispatchEvent(new CustomEvent("opportunity:theme", { detail: app.dataset.theme }));
  });
};
```

Replace the body of the page block with a shell containing `{% include "platform/components/exploration_rail.html" %}`, a `<main id="main-content" class="opportunity-main">`, the existing prototype notice/list/status/data, and this module bootstrap:

```html
<script type="module">
  import { initializeExplorationRail } from "/static/js/platform/exploration-rail.js";
  initializeExplorationRail();
</script>
```

- [ ] **Step 5: Add exact responsive shell rules**

Append to `styles.src.css`:

```css
.opportunity-rail { position: fixed; inset: 1rem auto 1rem 1rem; z-index: 30; width: 5.5rem; border-radius: 1.5rem; padding: 1rem .65rem; display: flex; flex-direction: column; align-items: center; justify-content: space-between; }
.opportunity-rail nav { display: grid; gap: 1.2rem; text-align: center; }
.opportunity-rail [aria-current="page"] { color: var(--color-opportunity-ai); font-weight: 750; }
.opportunity-rail [aria-disabled="true"] { color: var(--opp-muted); opacity: .58; cursor: not-allowed; }
.opportunity-main { min-height: 100dvh; padding: 1.25rem 1.25rem 2rem 7.75rem; }
.opportunity-mobile-nav { display: none; }

@media (max-width: 767px) {
  .opportunity-rail { display: none; }
  .opportunity-main { padding: 1rem 1rem 6.5rem; }
  .opportunity-mobile-nav { position: fixed; z-index: 40; display: grid; grid-template-columns: repeat(5, 1fr); inset: auto .75rem .75rem; border-radius: 1.25rem; padding: .8rem .35rem max(.8rem, env(safe-area-inset-bottom)); text-align: center; }
  .opportunity-mobile-nav [aria-current="page"] { color: var(--color-opportunity-ai); font-weight: 750; }
  .opportunity-mobile-nav [aria-disabled="true"] { color: var(--opp-muted); opacity: .58; }
}
```

- [ ] **Step 6: Verify and commit**

```bash
just frontend-fmt-and-lint
just e2e-tests tests/e2e/public/opportunity-home.spec.ts --project=chromium
git add gitjobs-server/templates/platform gitjobs-server/static/js/platform/exploration-rail.js gitjobs-server/static/css/styles.src.css tests/e2e/public/opportunity-home.spec.ts
git commit -s -m "feat: add responsive opportunity exploration shell"
```

Expected: both Chromium tests pass.

### Task 4: AI Command Bar and Reversible Filter State

**Files:**
- Create: `gitjobs-server/templates/platform/components/ai_command_bar.html`
- Modify: `gitjobs-server/templates/platform/opportunity_home.html`
- Create: `gitjobs-server/static/js/platform/ai-command-bar.js`
- Modify: `gitjobs-server/static/css/styles.src.css`
- Modify: `tests/e2e/public/opportunity-home.spec.ts`

**Interfaces:**
- Consumes: natural-language string and `OpportunityNode[]`.
- Produces: `parseOpportunityCommand(input) -> Condition[]`, `filterOpportunities(nodes, conditions) -> OpportunityNode[]`, event `opportunity:filters-changed` with `{ conditions, nodes }`, removable chips, and one-level undo.
- `Condition` shape: `{ key: "region"|"kind"|"headcountMin"|"capitalWan"|"experienceYears", label: string, value: string|number }`.

- [ ] **Step 1: Add failing command, chip, synchronization, and undo tests**

Append inside the existing `describe`:

```ts
  test("turns a natural-language request into removable conditions and synchronized results", async ({ page }) => {
    await page.getByLabel("描述你要寻找的餐饮项目").fill("广东，高校食堂，1万人以上，我有300万资金和8年经验");
    await page.getByRole("button", { name: "生成机会视图" }).click();
    const chips = page.locator("#opportunity-condition-chips button");
    await expect(chips).toHaveCount(5);
    await expect(page.locator("#opportunity-status")).toContainText("1 条");
    await expect(page.locator('[data-opportunity-card]:visible')).toHaveCount(1);
    await chips.filter({ hasText: "广东" }).click();
    await expect(page.locator("#opportunity-status")).toContainText("2 条");
    await page.getByRole("button", { name: "撤销上一步条件修改" }).click();
    await expect(page.locator("#opportunity-status")).toContainText("1 条");
  });

  test("supports keyboard submission and announces no matching result", async ({ page }) => {
    const input = page.getByLabel("描述你要寻找的餐饮项目");
    await input.fill("海南，高校食堂，5万人以上");
    await input.press("Enter");
    await expect(page.locator("#opportunity-status")).toHaveText(/没有符合条件的原型机会/);
    await expect(page.getByRole("button", { name: "撤销上一步条件修改" })).toBeEnabled();
  });
```

- [ ] **Step 2: Run and confirm the missing command UI failure**

```bash
just e2e-tests tests/e2e/public/opportunity-home.spec.ts --project=chromium -g "natural-language|keyboard"
```

Expected: FAIL because the labelled input and command controller are absent.

- [ ] **Step 3: Create the command partial**

Create `gitjobs-server/templates/platform/components/ai_command_bar.html`:

```html
<section class="opportunity-command opportunity-surface" aria-labelledby="opportunity-command-title">
  <div>
    <p class="opportunity-eyebrow">AI 机会指令</p>
    <h1 id="opportunity-command-title">把你的承接能力，变成可探索的机会</h1>
    <p>本阶段在浏览器内演示条件提取，不会上传文本或调用 AI。</p>
  </div>
  <form id="opportunity-command-form">
    <label for="ai-command-input">描述你要寻找的餐饮项目</label>
    <div class="opportunity-command-row">
      <input id="ai-command-input" class="opportunity-focus" type="text" autocomplete="off" placeholder="例如：广东，高校食堂，1万人以上，我有300万资金和8年经验">
      <button class="opportunity-focus" type="submit">生成机会视图</button>
    </div>
  </form>
  <div class="opportunity-condition-row">
    <div id="opportunity-condition-chips" aria-label="已识别条件"></div>
    <button id="opportunity-undo" class="opportunity-focus" type="button" aria-label="撤销上一步条件修改" disabled>撤销</button>
  </div>
</section>
```

- [ ] **Step 4: Implement the parser, filter, chips, and undo exactly**

Create `gitjobs-server/static/js/platform/ai-command-bar.js`:

```js
const REGION_PATTERN = /(广东|北京|江苏|浙江|四川|海南|广西|安徽|天津)/;
const KIND_PATTERNS = [["university", /高校|大学/], ["enterprise", /企业/], ["hospital", /医院/], ["park", /园区/], ["stall", /档口/]];

export const parseOpportunityCommand = (input) => {
  const conditions = [];
  const region = input.match(REGION_PATTERN)?.[1];
  if (region) conditions.push({ key: "region", label: region, value: region });
  const kind = KIND_PATTERNS.find(([, pattern]) => pattern.test(input));
  if (kind) conditions.push({ key: "kind", label: input.match(kind[1])?.[0] || kind[0], value: kind[0] });
  const headcount = input.match(/(\d+(?:\.\d+)?)\s*万\s*人(?:以上)?/);
  if (headcount) conditions.push({ key: "headcountMin", label: `${headcount[1]}万人以上`, value: Number(headcount[1]) * 10000 });
  const capital = input.match(/(\d+(?:\.\d+)?)\s*万\s*资金/);
  if (capital) conditions.push({ key: "capitalWan", label: `资金${capital[1]}万`, value: Number(capital[1]) });
  const experience = input.match(/(\d+)\s*年\s*经验/);
  if (experience) conditions.push({ key: "experienceYears", label: `${experience[1]}年经验`, value: Number(experience[1]) });
  return conditions;
};

export const filterOpportunities = (nodes, conditions) => nodes.filter((node) => conditions.every((condition) => {
  if (condition.key === "region") return node.region === condition.value;
  if (condition.key === "kind") return node.kind === condition.value;
  if (condition.key === "headcountMin") return node.headcount >= condition.value;
  return true; // capability conditions affect later ranking, not hard filtering in this prototype
}));

export const initializeAICommandBar = (nodes) => {
  const form = document.getElementById("opportunity-command-form");
  const input = document.getElementById("ai-command-input");
  const chipHost = document.getElementById("opportunity-condition-chips");
  const undo = document.getElementById("opportunity-undo");
  if (!form || !input || !chipHost || !undo) return;
  let conditions = [];
  let previous = [];

  const publish = () => {
    const filtered = filterOpportunities(nodes, conditions);
    chipHost.replaceChildren(...conditions.map((condition) => {
      const button = document.createElement("button");
      button.type = "button";
      button.className = "opportunity-chip opportunity-focus";
      button.textContent = `${condition.label} ×`;
      button.setAttribute("aria-label", `移除条件：${condition.label}`);
      button.addEventListener("click", () => {
        previous = [...conditions];
        conditions = conditions.filter((item) => item !== condition);
        publish();
      });
      return button;
    }));
    undo.disabled = previous.length === 0;
    window.dispatchEvent(new CustomEvent("opportunity:filters-changed", { detail: { conditions, nodes: filtered } }));
  };

  form.addEventListener("submit", (event) => {
    event.preventDefault();
    previous = [...conditions];
    conditions = parseOpportunityCommand(input.value);
    publish();
  });
  undo.addEventListener("click", () => {
    const current = [...conditions];
    conditions = previous;
    previous = current;
    publish();
  });
  publish();
};
```

Include the partial above the graph/list area. Parse `#opportunity-data`, call `initializeAICommandBar(nodes)`, and retain the rail initializer in the page's single module script.

- [ ] **Step 5: Style command hierarchy and chips**

Append:

```css
.opportunity-command { max-width: 78rem; margin: 0 auto 1rem; border-radius: 1.75rem; padding: clamp(1.2rem, 3vw, 2.4rem); }
.opportunity-eyebrow { color: var(--color-opportunity-ai); font-size: .75rem; font-weight: 800; letter-spacing: .18em; text-transform: uppercase; }
.opportunity-command h1 { max-width: 18ch; margin-top: .35rem; font-size: clamp(1.8rem, 4vw, 3.6rem); font-weight: 760; line-height: 1.06; letter-spacing: -.04em; }
.opportunity-command form { margin-top: 1.4rem; }
.opportunity-command label { display: block; margin-bottom: .55rem; color: var(--opp-muted); font-size: .875rem; }
.opportunity-command-row { display: grid; grid-template-columns: 1fr auto; gap: .65rem; }
.opportunity-command-row input { min-width: 0; border: 1px solid var(--opp-border); border-radius: 1rem; padding: .9rem 1rem; color: var(--opp-text); background: rgb(255 255 255 / 9%); }
.opportunity-command-row button { border-radius: 1rem; padding: .9rem 1.15rem; color: white; background: var(--color-opportunity-ai); font-weight: 750; }
.opportunity-condition-row { display: flex; align-items: center; justify-content: space-between; gap: 1rem; margin-top: .85rem; }
#opportunity-condition-chips { display: flex; flex-wrap: wrap; gap: .45rem; }
.opportunity-chip { border: 1px solid rgb(115 87 255 / 32%); border-radius: 999px; padding: .45rem .7rem; background: rgb(115 87 255 / 10%); }
#opportunity-undo:disabled { opacity: .4; cursor: not-allowed; }
@media (max-width: 640px) { .opportunity-command-row { grid-template-columns: 1fr; } }
```

- [ ] **Step 6: Verify and commit**

```bash
just frontend-fmt-and-lint
just e2e-tests tests/e2e/public/opportunity-home.spec.ts --project=chromium
git add gitjobs-server/templates/platform gitjobs-server/static/js/platform/ai-command-bar.js gitjobs-server/static/css/styles.src.css tests/e2e/public/opportunity-home.spec.ts
git commit -s -m "feat: add reversible opportunity commands"
```

### Task 5: Opportunity Constellation and Synchronized Fallback

**Files:**
- Create: `gitjobs-server/templates/platform/components/opportunity_graph.html`
- Create: `gitjobs-server/templates/platform/components/opportunity_fallback.html`
- Modify: `gitjobs-server/templates/platform/opportunity_home.html`
- Create: `gitjobs-server/static/js/platform/opportunity-graph.js`
- Modify: `gitjobs-server/static/css/styles.src.css`
- Modify: `tests/e2e/public/opportunity-home.spec.ts`

**Interfaces:**
- Consumes: `OpportunityNode[]` and `opportunity:filters-changed`.
- Produces: `shouldUseFallback() -> boolean`, `initializeOpportunityGraph(nodes)`, `#opportunity-graph`, synchronized `[data-opportunity-card]` visibility, and text status.

- [ ] **Step 1: Add failing graph-encoding and fallback tests**

Append:

```ts
  test("renders explainable constellation encodings on desktop", async ({ page }) => {
    await expect(page.locator("#opportunity-graph canvas")).toBeVisible();
    await expect(page.getByText("颜色表示项目类型")).toBeVisible();
    await expect(page.getByText("光环表示可信度")).toBeVisible();
    await expect(page.getByText("脉冲表示截止紧迫度")).toBeVisible();
    await expect(page.locator("#opportunity-status")).toContainText("6 条");
  });

  test("makes the card flow primary with reduced motion", async ({ browser }) => {
    const context = await browser.newContext({ reducedMotion: "reduce", viewport: { width: 1280, height: 800 } });
    const page = await context.newPage();
    await navigateWithRetries(page, "/opportunities", "reduced-motion opportunities");
    await expect(page.locator("#opportunity-app")).toHaveAttribute("data-experience", "fallback");
    await expect(page.locator("#opportunity-graph")).toBeHidden();
    await expect(page.locator('[data-opportunity-card]:visible')).toHaveCount(6);
    await context.close();
  });

  test("uses the opportunity card flow on mobile", async ({ page }) => {
    await page.setViewportSize({ width: 390, height: 844 });
    await expect(page.locator("#opportunity-app")).toHaveAttribute("data-experience", "fallback");
    await expect(page.locator('[data-opportunity-card]:visible').first()).toContainText("可信度");
  });
```

- [ ] **Step 2: Run and verify the missing graph failure**

```bash
just e2e-tests tests/e2e/public/opportunity-home.spec.ts --project=chromium -g "constellation|reduced motion|card flow"
```

Expected: FAIL because no chart canvas or `data-experience` exists.

- [ ] **Step 3: Create graph and fallback HTML partials**

Create `opportunity_graph.html`:

```html
<section class="opportunity-visual opportunity-surface" aria-labelledby="opportunity-visual-title">
  <div class="opportunity-visual-heading">
    <div><p class="opportunity-eyebrow">机会星图</p><h2 id="opportunity-visual-title">从区域密度进入具体项目</h2></div>
    <p role="note">界面原型数据，不代表真实项目</p>
  </div>
  <ul class="opportunity-legend" aria-label="星图视觉含义">
    <li>颜色表示项目类型</li><li>光环表示可信度</li><li>脉冲表示截止紧迫度</li>
  </ul>
  <div id="opportunity-graph" role="img" aria-label="餐饮项目机会星图；可使用下方列表访问相同项目"></div>
</section>
```

Create `opportunity_fallback.html`:

```html
<section id="opportunity-fallback" class="opportunity-fallback" aria-labelledby="opportunity-list-title">
  <div class="opportunity-list-heading">
    <div><p class="opportunity-eyebrow">同步机会流</p><h2 id="opportunity-list-title">所有星点都有可读取的项目卡片</h2></div>
    <p id="opportunity-status" aria-live="polite">当前展示 {{ nodes.len() }} 条原型机会</p>
  </div>
  <div class="opportunity-card-grid">
    {% for node in nodes %}
      <article class="opportunity-card opportunity-surface" data-opportunity-card data-opportunity-id="{{ node.id }}" data-credibility="{% if node.credibility >= 85 %}high{% else %}standard{% endif %}" data-urgency="{% if node.days_left <= 5 %}high{% else %}standard{% endif %}">
        <div><span>{{ node.kind.label() }}</span><span>{{ node.region }} · {{ node.city }}</span></div>
        <h3>{{ node.title }}</h3>
        <dl><div><dt>服务规模</dt><dd>{{ node.headcount }} 人</dd></div><div><dt>可信度</dt><dd>{{ node.credibility }} / 100（证据评分，非平台担保）</dd></div><div><dt>截止</dt><dd>{{ node.days_left }} 天后</dd></div></dl>
        <p>来源：{{ node.source_label }}</p>
        <button class="opportunity-focus" type="button" aria-label="查看{{ node.title }}的原型摘要">查看原型摘要</button>
      </article>
    {% endfor %}
  </div>
</section>
```

- [ ] **Step 4: Implement progressive graph enhancement and fallback selection**

Create `gitjobs-server/static/js/platform/opportunity-graph.js`:

```js
const TYPE_COLORS = { university: "#7357ff", enterprise: "#2f8cff", hospital: "#1ab887", park: "#f0b44d", stall: "#ff7a3d" };

export const shouldUseFallback = () =>
  window.innerWidth < 768 ||
  window.matchMedia("(prefers-reduced-motion: reduce)").matches ||
  navigator.connection?.saveData === true ||
  (typeof navigator.deviceMemory === "number" && navigator.deviceMemory <= 4) ||
  !window.echarts;

const POSITIONS = [[16, 64], [36, 30], [53, 70], [69, 38], [84, 67], [28, 82]];

const chartNode = (node, index) => ({
  id: node.id,
  name: node.title,
  value: [...POSITIONS[index % POSITIONS.length], node.headcount],
  symbolSize: Math.max(28, Math.min(62, 24 + node.headcount / 500)),
  itemStyle: {
    color: TYPE_COLORS[node.kind],
    borderColor: node.credibility >= 85 ? "#1ab887" : "rgba(255,255,255,.55)",
    borderWidth: Math.max(2, Math.round(node.credibility / 20)),
    shadowColor: node.daysLeft <= 5 ? "#ff7a3d" : TYPE_COLORS[node.kind],
    shadowBlur: node.daysLeft <= 5 ? 28 : 12,
  },
  label: { show: true, color: "#f7f3ea", formatter: node.city },
});

export const initializeOpportunityGraph = (initialNodes) => {
  const app = document.getElementById("opportunity-app");
  const host = document.getElementById("opportunity-graph");
  const status = document.getElementById("opportunity-status");
  const cards = [...document.querySelectorAll("[data-opportunity-card]")];
  if (!app || !host || !status) return;
  let chart;
  let currentNodes = initialNodes;

  const render = (nodes) => {
    currentNodes = nodes;
    const visibleIds = new Set(nodes.map(({ id }) => id));
    cards.forEach((card) => { card.hidden = !visibleIds.has(card.dataset.opportunityId); });
    status.textContent = nodes.length === 0 ? "没有符合条件的原型机会，请减少一个条件后重试" : `当前展示 ${nodes.length} 条原型机会`;
    const fallback = shouldUseFallback();
    app.dataset.experience = fallback ? "fallback" : "constellation";
    if (fallback) { chart?.dispose(); chart = undefined; return; }
    chart ||= window.echarts.init(host, null, { renderer: "canvas" });
    const data = nodes.map(chartNode);
    chart.setOption({
      animation: true,
      backgroundColor: "transparent",
      tooltip: { trigger: "item", formatter: ({ data: item }) => item.name },
      xAxis: { min: 0, max: 100, show: false },
      yAxis: { min: 0, max: 100, show: false },
      series: [
        { type: "effectScatter", coordinateSystem: "cartesian2d", rippleEffect: { period: 1.8, scale: 2.6 }, data: data.filter((_, index) => nodes[index].daysLeft <= 5) },
        { type: "effectScatter", coordinateSystem: "cartesian2d", rippleEffect: { period: 4.2, scale: 1.8 }, data: data.filter((_, index) => nodes[index].daysLeft > 5) },
      ],
    }, true);
  };

  render(initialNodes);
  window.addEventListener("opportunity:filters-changed", ({ detail }) => render(detail.nodes));
  window.addEventListener("opportunity:theme", () => chart?.resize());
  window.addEventListener("resize", () => { render(currentNodes); chart?.resize(); });
};
```

In `opportunity_home.html`, load ECharts before the module, include the two partials, and bootstrap in this order so the initial event is not missed:

```html
<script src="/static/vendor/js/echarts.v6.0.0.min.js"></script>
<script id="opportunity-data" type="application/json">{{ nodes_json|safe }}</script>
<script type="module">
  import { initializeExplorationRail } from "/static/js/platform/exploration-rail.js";
  import { initializeAICommandBar } from "/static/js/platform/ai-command-bar.js";
  import { initializeOpportunityGraph } from "/static/js/platform/opportunity-graph.js";
  const nodes = JSON.parse(document.getElementById("opportunity-data").textContent);
  initializeExplorationRail();
  initializeOpportunityGraph(nodes);
  initializeAICommandBar(nodes);
</script>
```

- [ ] **Step 5: Add graph and list layout rules**

Append:

```css
.opportunity-visual { max-width: 78rem; margin: 0 auto; border-radius: 1.75rem; padding: 1rem; overflow: hidden; }
.opportunity-visual-heading, .opportunity-list-heading { display: flex; align-items: end; justify-content: space-between; gap: 1rem; }
.opportunity-visual h2, .opportunity-fallback h2 { font-size: clamp(1.25rem, 2vw, 2rem); font-weight: 740; }
.opportunity-legend { display: flex; flex-wrap: wrap; gap: .6rem 1rem; margin: .8rem 0; color: var(--opp-muted); font-size: .78rem; }
#opportunity-graph { height: min(56dvh, 38rem); min-height: 28rem; border-radius: 1.2rem; background: radial-gradient(circle, rgb(115 87 255 / 12%), transparent 62%); }
.opportunity-fallback { max-width: 78rem; margin: 1rem auto 0; }
.opportunity-card-grid { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: .8rem; margin-top: .8rem; }
.opportunity-card { border-radius: 1.25rem; padding: 1rem; }
.opportunity-card h3 { margin: .75rem 0; font-size: 1.05rem; font-weight: 740; }
.opportunity-card dl { display: grid; gap: .45rem; color: var(--opp-muted); font-size: .84rem; }
.opportunity-card dl div { display: flex; justify-content: space-between; gap: 1rem; }
.opportunity-card button { margin-top: .9rem; color: var(--color-opportunity-ai-bright); font-weight: 700; }
.opportunity-app[data-experience="constellation"] .opportunity-fallback { margin-top: 1.25rem; }
.opportunity-app[data-experience="fallback"] .opportunity-visual { display: none; }
@media (max-width: 1000px) { .opportunity-card-grid { grid-template-columns: repeat(2, minmax(0, 1fr)); } }
@media (max-width: 640px) { .opportunity-card-grid { grid-template-columns: 1fr; } .opportunity-visual-heading, .opportunity-list-heading { align-items: start; flex-direction: column; } }
```

- [ ] **Step 6: Verify and commit**

```bash
just frontend-fmt-and-lint
just e2e-tests tests/e2e/public/opportunity-home.spec.ts --project=chromium
git add gitjobs-server/templates/platform gitjobs-server/static/js/platform/opportunity-graph.js gitjobs-server/static/css/styles.src.css tests/e2e/public/opportunity-home.spec.ts
git commit -s -m "feat: add accessible opportunity constellation"
```

### Task 6: Cross-Browser, Accessibility, and Regression Gate

**Files:**
- Modify: `tests/e2e/public/opportunity-home.spec.ts`
- Modify: `gitjobs-server/static/js/platform/opportunity-graph.js` only if the verification exposes a concrete cross-browser issue.
- Modify: `gitjobs-server/static/css/styles.src.css` only if the verification exposes a concrete layout/focus issue.

**Interfaces:**
- Consumes: all prior UI-foundation interfaces.
- Produces: an independently verifiable first-phase prototype with no console errors, accessible landmarks/focus, cross-browser behavior, and no regression to current job-board tests.

- [ ] **Step 1: Add the final accessibility and boundary tests**

Append:

```ts
  test("exposes landmarks, visible keyboard focus, and the non-production boundary", async ({ page }) => {
    await expect(page.getByRole("main")).toBeVisible();
    await expect(page.locator('[role="note"]', { hasText: "界面原型数据，不代表真实项目" })).toBeVisible();
    await page.keyboard.press("Tab");
    const focused = page.locator(":focus");
    await expect(focused).toBeVisible();
    await expect(focused).toHaveCSS("outline-style", "solid");
    await expect(page.getByText("证据评分，非平台担保").first()).toBeVisible();
  });

  test("loads without browser console errors", async ({ page }) => {
    const errors: string[] = [];
    page.on("console", (message) => { if (message.type() === "error") errors.push(message.text()); });
    await page.reload();
    await page.waitForLoadState("networkidle");
    expect(errors).toEqual([]);
  });
```

- [ ] **Step 2: Run the complete new suite on all configured browsers**

```bash
just e2e-tests tests/e2e/public/opportunity-home.spec.ts
```

Expected: all opportunity tests pass on Chromium, Firefox, and WebKit with zero retries and no console errors.

- [ ] **Step 3: Run inherited job-board regression coverage**

```bash
just e2e-tests tests/e2e/public/jobboard.spec.ts --project=chromium
```

Expected: existing job-board tests pass because `/` and its templates were not replaced.

- [ ] **Step 4: Run the complete code-quality and Rust test gate**

```bash
just frontend-fmt-and-lint
cargo fmt --check
cargo clippy --all-targets --all-features -- --deny warnings
cargo test
cargo build -p gitjobs-server
```

Expected: every command exits 0; the existing Rust test count does not decrease; the server binary embeds the newly hashed platform JavaScript and updated template.

- [ ] **Step 5: Confirm scope boundaries from the diff**

```bash
git diff --check
git status --short
git diff --stat HEAD~5..HEAD
rg -n "payment|支付回调|自动续费|EntitlementService|unlock_contact" gitjobs-server/src/handlers/platform gitjobs-server/src/templates/platform gitjobs-server/templates/platform gitjobs-server/static/js/platform
```

Expected: `git diff --check` is silent; status contains only intended final-test edits if not yet committed; the search returns no implementation of payment, entitlement, renewal, or contact unlocking.

- [ ] **Step 6: Commit the verification coverage**

```bash
git add tests/e2e/public/opportunity-home.spec.ts gitjobs-server/static/js/platform/opportunity-graph.js gitjobs-server/static/css/styles.src.css
git commit -s -m "test: verify opportunity experience across browsers"
```

If no production file needed a verification fix, stage and commit only `tests/e2e/public/opportunity-home.spec.ts`.

## Completion Evidence

Before reporting completion, record these outputs in the handoff message:

- `cargo test`: total passed/failed count.
- `cargo clippy --all-targets --all-features -- --deny warnings`: exit status.
- `just e2e-tests tests/e2e/public/opportunity-home.spec.ts`: passed count for Chromium, Firefox, and WebKit.
- `just e2e-tests tests/e2e/public/jobboard.spec.ts --project=chromium`: passed count.
- `git status --short --branch`: branch and cleanliness.
- Manual boundary statement: `/opportunities` contains clearly labelled prototype data; payment, entitlement, contact unlocking, and production AI calls remain absent.
