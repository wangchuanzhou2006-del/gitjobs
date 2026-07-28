import { expect, test } from "@playwright/test";
import type { Page, TestInfo } from "@playwright/test";

import { navigateHomeWithRetries, navigateWithRetries } from "../../shared/helpers";
import { loginWithCredentials } from "../../shared/utils";

const TRUSTED_SOURCE = "E2E Trusted Talent Source";
const NEW_SOURCE = "E2E New Talent Source";
const MERGE_TARGET_ID = "00000000-0000-4000-8000-000000001211";
const browserDiagnostics = new WeakMap<Page, BrowserDiagnostics>();
const PROTECTED_FIELD_NAMES = [
  "raw_text_ciphertext",
  "raw_fragment_ciphertext",
  "contact_candidates_ciphertext",
  "contact_hashes",
  "encryption_key_version",
  "normalized_fingerprint",
];
const E2E_SECRET_VALUES = [
  "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f",
  "e2e-only-nonproduction-contact-pepper",
  "deliberately-invalid-outside-local-e2e",
  "e2e-deterministic-v1",
  "openai-compatible",
];

type BrowserDiagnostics = {
  allMessages: string[];
  errors: string[];
};

type WorkflowState = {
  batchId: string;
  itemIds: Record<string, string>;
  newItemId: string;
  project: string;
  sensitiveValues: string[];
};

let workflow: WorkflowState;

const titlesFor = (project: string) => ({
  safeHiring: `E2E 安全招聘 · ${project}`,
  safeSeeking: `E2E 安全求职 · ${project}`,
  mediumFee: `E2E 费用待核招聘 · ${project}`,
  highRisk: `E2E 高风险招聘 · ${project}`,
  duplicate: `E2E 重复切配招聘 · ${project}`,
  newSource: `E2E 新来源招聘 · ${project}`,
  pausedSafe: `E2E 暂停后安全招聘 · ${project}`,
});

const projectFixture = (testInfo: TestInfo) => {
  const project = testInfo.project.name.toLowerCase();
  const token = project.toUpperCase();
  const phoneBase = { chromium: 1, firefox: 11, webkit: 21 }[project];
  if (!phoneBase) throw new Error(`unsupported acceptance project: ${project}`);
  return { project, token, phoneBase, titles: titlesFor(project) };
};

const projectProtectedValues = (project: string, token: string, phoneBase: number) => [
  ...[0, 2, 3, 4, 5, 6].map((offset) => syntheticPhone(phoneBase + offset)),
  `test_candidate_${project}_02`,
  ...[
    "SAFE_HIRING",
    "SAFE_SEEKING",
    "MEDIUM_FEE",
    "HIGH_RISK",
    "DUPLICATE",
    "NEW_SOURCE",
    "PAUSED_SAFE",
  ].map((marker) => `E2E_${marker}_${token}`),
  `E2E_PRIVATE_ERROR_${token}_13800009999`,
  ...E2E_SECRET_VALUES,
];

const attachBrowserErrorCapture = (page: Page) => {
  const diagnostics: BrowserDiagnostics = { allMessages: [], errors: [] };
  browserDiagnostics.set(page, diagnostics);
  page.on("console", (message) => {
    const entry = `console.${message.type()}: ${message.text()}`;
    diagnostics.allMessages.push(entry);
    if (message.type() === "error") diagnostics.errors.push(entry);
  });
  page.on("pageerror", (error) => {
    const entry = `pageerror: ${error.message}`;
    diagnostics.allMessages.push(entry);
    diagnostics.errors.push(entry);
  });
};

const submitImport = async (page: Page, sourceName: string, rawText: string) => {
  await navigateWithRetries(page, "/dashboard/moderator/talent/import", "talent import");
  await page.locator("#source-type").selectOption("wechat_group");
  await page.locator("#source-name").fill(sourceName);
  await page.locator("#talent-raw-text").fill(rawText);
  await Promise.all([
    page.waitForURL(/\/dashboard\/moderator\/talent\/review\?batch=[0-9a-f-]+$/),
    page.locator('#talent-import-form button[type="submit"]').click(),
  ]);
  const batchId = new URL(page.url()).searchParams.get("batch");
  expect(batchId).toMatch(/^[0-9a-f-]{36}$/);
  return batchId!;
};

const waitForBatchTitles = async (page: Page, batchId: string, titles: string[]) => {
  await expect
    .poll(
      async () => {
        const response = await page.request.get(
          `/dashboard/moderator/talent/review?status=all&batch=${batchId}`,
        );
        const body = await response.text();
        return titles.every((title) => body.includes(title));
      },
      { timeout: 30_000 },
    )
    .toBe(true);
  await navigateWithRetries(
    page,
    `/dashboard/moderator/talent/review?status=all&batch=${batchId}`,
    "completed talent batch",
  );
  for (const title of titles) await expect(page.getByRole("heading", { name: title })).toBeVisible();
};

const rowFor = (page: Page, title: string) =>
  page.locator("li").filter({ has: page.getByRole("heading", { name: title }) });

const itemIdFor = async (page: Page, title: string) => {
  const href = await rowFor(page, title).locator("a").getAttribute("href");
  const match = href?.match(/\/review\/([0-9a-f-]{36})$/);
  expect(match).toBeTruthy();
  return match![1];
};

const openItem = async (page: Page, itemId: string) => {
  await navigateWithRetries(page, `/dashboard/moderator/talent/review/${itemId}`, "talent review detail");
  expect(await page.locator('input[name="lock_version"]').count()).toBeGreaterThan(0);
};

const syntheticPhone = (suffix: number) => `1380000${String(suffix).padStart(4, "0")}`;

const batchInput = (token: string, phoneBase: number) =>
  [
    `招聘 E2E_SAFE_HIRING_${token}，合成联系电话：${syntheticPhone(phoneBase)}`,
    `求职 E2E_SAFE_SEEKING_${token}，合成微信：test_candidate_${token.toLowerCase()}_02`,
    `招聘 E2E_MEDIUM_FEE_${token}，费用信息需要人工核对，合成联系电话：${syntheticPhone(phoneBase + 2)}`,
    `招聘 E2E_HIGH_RISK_${token}，刷单内容必须人工驳回，合成联系电话：${syntheticPhone(phoneBase + 3)}`,
    `招聘 E2E_DUPLICATE_${token}，与正式种子记录高度相似，合成联系电话：${syntheticPhone(phoneBase + 4)}`,
  ].join("\n\n");

test.describe.serial("Moderator talent import and review acceptance", () => {
  test.beforeEach(async ({ page }) => {
    attachBrowserErrorCapture(page);
    await navigateHomeWithRetries(page);
    await loginWithCredentials(page, "test", "test1234");
  });

  test.afterEach(async ({ page }, testInfo) => {
    const fixture = projectFixture(testInfo);
    const diagnostics = browserDiagnostics.get(page) ?? { allMessages: [], errors: [] };
    expect(diagnostics.errors).toEqual([]);
    const browserOutput = diagnostics.allMessages.join("\n");
    for (const name of PROTECTED_FIELD_NAMES) expect(browserOutput).not.toContain(name);
    for (const secret of projectProtectedValues(
      fixture.project,
      fixture.token,
      fixture.phoneBase,
    ))
      expect(browserOutput).not.toContain(secret);
  });

  test("one pasted trusted batch splits and applies auto-publication, risk, and duplicate gates", async ({
    page,
  }, testInfo) => {
    const fixture = projectFixture(testInfo);
    const batchId = await submitImport(page, TRUSTED_SOURCE, batchInput(fixture.token, fixture.phoneBase));
    const expectedTitles = Object.values(fixture.titles).slice(0, 5);
    await waitForBatchTitles(page, batchId, expectedTitles);

    await expect(page.locator("ol > li")).toHaveCount(5);
    await expect(rowFor(page, fixture.titles.safeHiring)).toContainText("已发布");
    await expect(rowFor(page, fixture.titles.safeSeeking)).toContainText("已发布");
    await expect(rowFor(page, fixture.titles.mediumFee)).toContainText("待查看");
    await expect(rowFor(page, fixture.titles.mediumFee)).toContainText("建议留意");
    await expect(rowFor(page, fixture.titles.highRisk)).toContainText("需要重点核对");
    await expect(rowFor(page, fixture.titles.duplicate)).toContainText("待查看");
    await expect(rowFor(page, fixture.titles.duplicate)).toContainText(/条相似/);

    const itemIds: Record<string, string> = {};
    for (const [name, title] of Object.entries(fixture.titles).slice(0, 5))
      itemIds[name] = await itemIdFor(page, title);
    workflow = {
      batchId,
      itemIds,
      newItemId: "",
      project: fixture.project,
      sensitiveValues: [
        syntheticPhone(fixture.phoneBase),
        syntheticPhone(fixture.phoneBase + 2),
        syntheticPhone(fixture.phoneBase + 3),
        syntheticPhone(fixture.phoneBase + 4),
        `test_candidate_${fixture.project}_02`,
        `E2E_SAFE_HIRING_${fixture.token}`,
        `E2E_HIGH_RISK_${fixture.token}`,
      ],
    };
  });

  test("a new source stays in review and reprocessing uses the same durable pipeline", async ({
    page,
  }, testInfo) => {
    const fixture = projectFixture(testInfo);
    const raw = `招聘 E2E_NEW_SOURCE_${fixture.token}，合成联系电话：${syntheticPhone(fixture.phoneBase + 5)}`;
    const batchId = await submitImport(page, NEW_SOURCE, raw);
    await waitForBatchTitles(page, batchId, [fixture.titles.newSource]);
    const row = rowFor(page, fixture.titles.newSource);
    await expect(row).toContainText("待查看");
    workflow.newItemId = await itemIdFor(page, fixture.titles.newSource);
    workflow.sensitiveValues.push(
      syntheticPhone(fixture.phoneBase + 5),
      `E2E_NEW_SOURCE_${fixture.token}`,
    );

    await openItem(page, workflow.newItemId);
    const before = Number(await page.locator('input[name="lock_version"]').first().inputValue());
    await page.getByRole("button", { name: "重新整理" }).click();
    await expect
      .poll(async () => {
        await page.goto(`/dashboard/moderator/talent/review/${workflow.newItemId}`);
        return Number(await page.locator('input[name="lock_version"]').first().inputValue());
      })
      .toBeGreaterThan(before);
    await expect(page.getByText("待查看", { exact: true })).toBeVisible();
  });

  test("Moderator can edit/publish, reject, merge, and withdraw with one click", async ({
    page,
  }, testInfo) => {
    const fixture = projectFixture(testInfo);
    const editedTitle = `E2E 人工确认招聘 · ${fixture.project}`;

    await openItem(page, workflow.itemIds.mediumFee);
    await page.locator('input[name="title"]').fill(editedTitle);
    await page.locator('input[name="skills"]').fill("服务员、食品安全、人工核对");
    await page.getByRole("button", { name: "保存修改" }).click();
    await openItem(page, workflow.itemIds.mediumFee);
    await expect(page.locator('input[name="title"]')).toHaveValue(editedTitle);
    await page.getByRole("button", { name: "确认发布" }).click();

    await openItem(page, workflow.itemIds.highRisk);
    await page.locator("#reject-reason").selectOption("policy_violation");
    page.once("dialog", (dialog) => dialog.accept());
    await page.getByRole("button", { name: "驳回" }).click();

    await openItem(page, workflow.itemIds.duplicate);
    await page.getByText("更多处理", { exact: true }).click();
    await page.locator('input[name="target_record_id"]').fill(MERGE_TARGET_ID);
    page.once("dialog", (dialog) => dialog.accept());
    await page.getByRole("button", { name: "合并相似信息" }).click();

    await openItem(page, workflow.itemIds.safeHiring);
    await page.getByText("更多处理", { exact: true }).click();
    const withdrawalRequests: string[] = [];
    page.on("request", (request) => {
      if (request.url().includes(`/review/${workflow.itemIds.safeHiring}/withdraw`))
        withdrawalRequests.push(request.url());
    });
    page.once("dialog", (dialog) => dialog.accept());
    await page.getByRole("button", { name: "撤回发布" }).click();
    await expect.poll(() => withdrawalRequests.length).toBe(1);

    await navigateWithRetries(
      page,
      `/dashboard/moderator/talent/review?status=all&batch=${workflow.batchId}`,
      "reviewed talent batch",
    );
    await expect(rowFor(page, editedTitle)).toContainText("已发布");
    await expect(rowFor(page, fixture.titles.highRisk)).toContainText("已驳回");
    await expect(rowFor(page, fixture.titles.duplicate)).toContainText("已合并");

    const publicBody = await (await page.request.get("/talent")).text();
    expect(publicBody).toContain(editedTitle);
    expect(publicBody).toContain(fixture.titles.safeSeeking);
    expect(publicBody).not.toContain(fixture.titles.safeHiring);
    expect(publicBody).not.toContain(fixture.titles.highRisk);
    expect(publicBody).not.toContain(fixture.titles.duplicate);
  });

  test("global pause prevents a later trusted low-risk item from auto-publishing", async ({
    page,
  }, testInfo) => {
    const fixture = projectFixture(testInfo);
    await openItem(page, workflow.newItemId);
    await page.getByText("更多处理", { exact: true }).click();
    await expect(page.getByRole("button", { name: "暂停自动发布" })).toBeVisible();

    page.once("dialog", (dialog) => dialog.accept());
    await page.getByRole("button", { name: "暂停自动发布" }).click();
    const raw = `招聘 E2E_PAUSED_SAFE_${fixture.token}，合成联系电话：${syntheticPhone(fixture.phoneBase + 6)}`;
    const batchId = await submitImport(page, TRUSTED_SOURCE, raw);
    await waitForBatchTitles(page, batchId, [fixture.titles.pausedSafe]);
    await expect(rowFor(page, fixture.titles.pausedSafe)).toContainText("待查看");
    const publicBody = await (await page.request.get("/talent")).text();
    expect(publicBody).not.toContain(fixture.titles.pausedSafe);
    workflow.sensitiveValues.push(
      syntheticPhone(fixture.phoneBase + 6),
      `E2E_PAUSED_SAFE_${fixture.token}`,
    );
  });

  test("public HTML, browser JSON, and safe errors omit protected data and provider secrets", async ({
    page,
  }, testInfo) => {
    const fixture = projectFixture(testInfo);
    const publicResponse = await page.request.get("/talent");
    expect(publicResponse.status()).toBe(200);
    const publicHtml = await publicResponse.text();
    await navigateWithRetries(page, "/talent", "public talent privacy boundary");
    const browserJson = await page.locator("#talent-data").textContent();

    await openItem(page, workflow.newItemId);
    const lockVersion = await page.locator('input[name="lock_version"]').first().inputValue();
    const privateErrorValue = `E2E_PRIVATE_ERROR_${fixture.token}_13800009999`;
    const errorResponse = await page.request.put(
      `/dashboard/moderator/talent/review/${workflow.newItemId}/merge`,
      { form: { lock_version: lockVersion, target_record_id: privateErrorValue } },
    );
    expect(errorResponse.status()).toBe(422);
    const errorBody = await errorResponse.text();

    const forbiddenSecrets = [
      ...workflow.sensitiveValues,
      privateErrorValue,
      ...E2E_SECRET_VALUES,
    ];
    for (const body of [publicHtml, browserJson ?? "", errorBody]) {
      for (const name of PROTECTED_FIELD_NAMES) expect(body).not.toContain(name);
      for (const secret of forbiddenSecrets) expect(body).not.toContain(secret);
    }
  });
});
