# Running UI Screenshots Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Capture honest screenshots of the running app from the existing synthetic-data browser test environment.

**Architecture:** Add screenshot capture to the already working Chromium Playwright paths for public opportunities and authenticated tender report. Upload only named preview PNGs as an artifact from the existing E2E workflow, then download and inspect them before showing the user.

**Tech Stack:** Playwright, GitHub Actions, Rust/Axum test server, PostgreSQL test service.

**Spec:** `docs/superpowers/specs/2026-11-20-running-ui-screenshots-design.md`

## Global Constraints

- No public deployment, real user data, or new model calls.
- Keep the PR draft and preserve unrelated worktree files.
- Screenshots must come from rendered HTTP pages, not source templates.

## Review Focus

- Missing CSS or template rendering: assert a styled app root and absence of raw `{%` markers before capture.
- Unready dynamic content: wait for opportunity cards and report sections before capture.
- Sensitive output: use only synthetic fixtures and narrow artifact glob.
- Cross-browser duplication: capture only in Chromium, while all three browser test suites still run.
- Screenshot delivery: verify PNG dimensions/content after artifact download, not just upload success.

---

### Task 1: Capture running pages

**Files:** Modify `tests/e2e/public/opportunity-home.spec.ts` and `tests/e2e/public/tender-analysis.spec.ts`.

**Interfaces:** Use Playwright `testInfo.outputPath("preview-*.png")` and `page.screenshot({ path, fullPage: true })`; Chromium only.

- [ ] Add assertions for rendered, styled content and no raw template markers in the relevant existing tests.
- [ ] Run the focused test in CI and confirm the assertions protect the pages.
- [ ] Capture desktop opportunities, authenticated tender list, and synthetic tender report after content readiness.
- [ ] Check screenshots contain no real credentials or personal data.

### Task 2: Publish and inspect test artifacts

**Files:** Modify `.github/workflows/e2e.yml`.

**Interfaces:** Upload only `test-results/**/preview-*.png` from the Chromium job with `actions/upload-artifact@v4`.

- [ ] Add artifact upload after browser tests, including on test failure so diagnostics remain available.
- [ ] Validate YAML formatting, `git diff --check`, and the available local checks.
- [ ] Commit and push only the intended files to the existing draft PR.
- [ ] Wait for CI, download the artifact, inspect each PNG, and present the actual images with a clear synthetic-data disclaimer.
