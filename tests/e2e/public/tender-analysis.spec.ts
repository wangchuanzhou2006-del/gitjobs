import { expect, test } from "@playwright/test";
import type { Page } from "@playwright/test";

import { loginWithCredentials } from "../shared/utils";
import { syntheticTenderPdf } from "../shared/synthetic-tender-pdf.mjs";

const PROJECT_ID = "00000000-0000-4000-8000-00000000a701";
const browserErrors = new WeakMap<Page, string[]>();

function captureBrowserErrors(page: Page): string[] {
  const errors: string[] = [];
  browserErrors.set(page, errors);
  page.on("console", (message) => { if (message.type() === "error") errors.push(message.text()); });
  page.on("pageerror", (error) => errors.push(error.message));
  return errors;
}

async function upload(page: Page, pdf: Buffer): Promise<string> {
  await page.goto("/tenders");
  const form = page.locator(`form[action="/tenders/${PROJECT_ID}/documents"]`);
  await expect(form).toBeVisible();
  await form.locator('input[name="file"]').setInputFiles({
    name: "synthetic.pdf", mimeType: "application/pdf", buffer: pdf,
  });
  await form.getByRole("button", { name: "上传并分析" }).click();
  await expect(page).toHaveURL(/\/tenders\/jobs\/[0-9a-f-]+\/status/);
  return page.url();
}

test.describe("招标文件分析闭环", () => {
  test.skip(!process.env.GITJOBS_E2E_TENDER_ACCEPTANCE, "requires disposable tender E2E stack");

  test.beforeEach(async ({ page }) => {
    captureBrowserErrors(page);
    await loginWithCredentials(page, "test", "test1234");
  });

  test.afterEach(async ({ page }) => {
    expect(browserErrors.get(page) ?? []).toEqual([]);
  });

  test("登录用户上传两页 PDF 后看到正确页码的预算和截止时间", async ({ page, browser }) => {
    await page.goto("/opportunities");
    await expect(page.getByTestId("desktop-exploration-rail").getByRole("link", { name: "招标" })).toHaveAttribute("href", "/tenders");
    const pdf = syntheticTenderPdf();
    const firstStatusUrl = await upload(page, pdf);
    await expect(page.getByRole("link", { name: "查看分析报告" })).toBeVisible({ timeout: 30_000 });
    await page.getByRole("link", { name: "查看分析报告" }).click();
    await expect(page.getByRole("heading", { name: "项目概况" })).toBeVisible();
    await expect(page.getByText("CNY 2800000", { exact: true })).toBeVisible();
    await expect(page.getByText("2026-09-01", { exact: true })).toBeVisible();
    const overview = page.getByRole("region", { name: "项目概况" });
    await expect(overview.getByText("查看第 1 页原文").first()).toBeVisible();
    await expect(overview.getByText("查看第 2 页原文")).toBeVisible();
    await overview.locator("summary").first().focus();
    await page.keyboard.press("Enter");
    await expect(overview.locator("blockquote").first()).toBeVisible();
    await expect(page.getByRole("heading", { name: "待人工确认" })).toBeVisible();
    await expect(page.getByRole("heading", { name: "原文依据" })).toBeVisible();
    const reportUrl = page.url();
    const anonymous = await browser.newContext();
    const anonymousPage = await anonymous.newPage();
    await anonymousPage.goto(reportUrl);
    await expect(anonymousPage).toHaveURL(/\/log-in(?:\?.*)?$/);
    await anonymous.close();
    await page.setViewportSize({ width: 360, height: 800 });
    expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(360);
    const duplicateStatusUrl = await upload(page, pdf);
    expect(duplicateStatusUrl).toBe(firstStatusUrl);
  });

  test("伪造引用不能保存为报告，失败任务可以重试", async ({ page }) => {
    await upload(page, syntheticTenderPdf(true));
    await expect(page.getByText("分析失败", { exact: true })).toBeVisible({ timeout: 30_000 });
    await expect(page.getByText("分析结果未通过原文核验，可以重新分析。")).toBeVisible();
    await expect(page.getByRole("link", { name: "查看分析报告" })).toHaveCount(0);
    await page.getByRole("button", { name: "重新分析" }).click();
    await expect(page).toHaveURL(/\/tenders\/jobs\/[0-9a-f-]+\/status/);
    await expect(page.getByText("分析失败", { exact: true })).toBeVisible({ timeout: 30_000 });
  });

  test("无 JavaScript 时可手动刷新状态", async ({ browser }) => {
    const context = await browser.newContext({ javaScriptEnabled: false, viewport: { width: 360, height: 800 } });
    const page = await context.newPage();
    const errors = captureBrowserErrors(page);
    await loginWithCredentials(page, "test", "test1234");
    const statusUrl = await upload(page, syntheticTenderPdf());
    await expect.poll(async () => {
      await page.goto(statusUrl);
      return page.getByRole("link", { name: "查看分析报告" }).count();
    }, { timeout: 30_000 }).toBe(1);
    expect(errors).toEqual([]);
    await context.close();
  });
});
