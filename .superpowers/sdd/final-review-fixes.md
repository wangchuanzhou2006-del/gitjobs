# Talent workbench final-review fixes

## Scope

- Branch: `codex/talent-workbench-ui`
- Starting HEAD: `47d8e20494cab9921ce402f6dcfab950dab87279`
- Scope stayed within the final-review findings. No real AI, collection, authentication, payment, subscription, database, dependency, or real-data behavior was added. The scoring contract was not changed.

## Fixes

1. **JavaScript-disabled SSR**
   - Removed the server-rendered `hidden` attribute from seeking cards, so hiring and seeking each expose all eight records without JavaScript.
   - Successful workbench initialization still applies the selected-mode visibility through the existing render path and marks the root with `data-workbench-ready="true"`.

2. **Missing, malformed, or invalid interaction data**
   - Isolated theme initialization from workbench initialization so one optional module cannot block the other.
   - Checked for the `talent-data` node, guarded JSON parsing, validated the `{ hiring: [], seeking: [] }` shape and record IDs, and guarded workbench initialization.
   - Any failure restores all 16 server-rendered cards, removes the ready marker, and announces `交互数据暂不可用` through the existing `aria-live="polite"` status. Dynamic values continue to use text APIs; no dynamic `innerHTML` was added.
   - The existing `/opportunities` navigation remains available in every fallback.

3. **Storage-denied theme behavior**
   - Isolated `localStorage.getItem` and `setItem` in separate `try/catch` blocks.
   - A failed read falls back to `prefers-color-scheme` (`dark` or `light`). A failed write still updates `data-theme`, synchronizes the toggle label, dispatches `talent:theme`, and leaves the workbench initialized.

4. **Matching-test false positives**
   - Asserted the complete `TALENT_DIMENSION_WEIGHTS` object: role 20, region 20, salary minimum 20, skills 15, experience 10, lodging 5, rest days 5, timing 5.
   - Added four ranked records, including three equal-score records, two with equal credibility, and asserted the complete ID order.
   - The test temporarily makes zero-comparison ordering deliberately unstable and restores native sorting afterward. Removing either credibility or explicit original-index tie-breaking now fails the full-order assertion.

5. **Mode-switch and browser-error coverage**
   - Added `川湘菜` to the mode-switch input and asserted all common labels (`region`, `role`, `salaryMin`, `skills`) survive while non-common conditions are removed.
   - All talent tests now collect `console.error` and `pageerror`; the custom Storage context is captured explicitly. The final 13-test Chromium run produced no captured browser errors.

6. **SSR Rust assertion**
   - Tightened the handler test to count rendered `<article class="talent-card">` elements instead of every textual `data-talent-card` occurrence, avoiding the new fallback selector string being miscounted as a card.

## TDD evidence

### RED on the old implementation

Command:

```text
BASE_URL=http://127.0.0.1:9000 npx playwright test --config tests/e2e/playwright.config.ts tests/e2e/public/talent-workbench.spec.ts --project=chromium --grep "禁用 JavaScript|交互数据损坏|Storage 读写"
```

Result: **3 failed, 0 passed**.

- JavaScript disabled: seeking cards expected 8 visible, received 0.
- Malformed JSON: status remained `当前展示 8 条招聘机会`; captured `pageerror` was a JSON `SyntaxError`.
- Storage denied: `data-theme` was unset because `SecurityError` stopped initialization.

### GREEN after the minimal fixes

The same focused command: **3 passed, 0 failed (2.0s)**.

Additional fallback coverage for a missing node and invalid shape also passes. The final complete Chromium command produced **13 passed, 0 failed (5.3s)**. The strengthened pure scoring/ranking test also passed independently (**1 passed, 0 failed**).

## Verification

- Fresh embedded-asset build after the final template formatting: `cargo build -p gitjobs-server` — exit 0.
- Fresh local service: `GET http://127.0.0.1:9000/talent` — HTTP 200.
- Rust talent tests: `cargo test -p gitjobs-server talent` — **5 passed, 0 failed, 213 filtered out**.
- Rust formatting: `cargo fmt --all -- --check` — exit 0.
- Focused Prettier: modified JS and talent E2E — all matched.
- Focused djlint through the existing offline cache (`uvx --offline djlint`): **0 files would be updated**.
- Node syntax: modified exploration rail — exit 0.
- `git diff --check` — exit 0.
- Strict UTF-8 validation: every modified code/test file decoded without error, contains no BOM and no replacement character.

## Concerns and cleanup

- Cargo emitted the existing `num-bigint-dig` future-incompatibility notice and a Windows/WSL incremental-cache finalization warning; build and tests still exited 0. Neither warning was introduced by these changes.
- The temporary PostGIS container had no schema migrations, so unrelated background notification-worker logs reported a missing database function. The public `/talent` route returned HTTP 200 and all browser assertions were independent of database data; no claim of server-log-wide zero errors is made.
- The WSL service on port 9000 was stopped and verified absent.
- The temporary container `gitjobs-talent-final-review-db`, bound only to `127.0.0.1:5432`, was removed and verified absent.