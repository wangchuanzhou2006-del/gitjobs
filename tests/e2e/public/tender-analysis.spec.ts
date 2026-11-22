import { expect, test } from "@playwright/test";
import type { Page } from "@playwright/test";

import { loginWithCredentials } from "../shared/utils";
import { syntheticTenderPdf } from "../shared/synthetic-tender-pdf.mjs";

const PROJECT_ID = "00000000-0000-4000-8000-00000000a701";
const browserErrors = new WeakMap<Page, string[]>();
const failedRequests = new WeakMap<Page, string[]>();

function captureBrowserErrors(page: Page): string[] {
  const errors: string[] = [];
  const failures: string[] = [];
  browserErrors.set(page, errors);
  failedRequests.set(page, failures);
  page.on("console", (message) => {
    if (message.type() === "error") errors.push(message.text());
  });
  page.on("pageerror", (error) => errors.push(error.message));
  page.on("requestfailed", (request) => {
    failures.push(
      `${new URL(request.url()).pathname}: ${request.failure()?.errorText ?? "unknown"}`,
    );
  });
  return errors;
}

async function upload(page: Page, pdf: Buffer): Promise<string> {
  await page.goto("/tenders");
  const form = page.locator(`form[action="/tenders/${PROJECT_ID}/documents"]`);
  await expect(form).toBeVisible();
  await form.locator('input[name="file"]').setInputFiles({
    name: "synthetic.pdf",
    mimeType: "application/pdf",
    buffer: pdf,
  });
  await form.getByRole("button", { name: "上传并分析" }).click();
  await expect(page).toHaveURL(/\/tenders\/jobs\/[0-9a-f-]+\/status/);
  return page.url();
}

test.describe("招标文件分析闭环", () => {
  test.skip(
    !process.env.GITJOBS_E2E_TENDER_ACCEPTANCE,
    "requires disposable tender E2E stack",
  );

  test.beforeEach(async ({ page }) => {
    captureBrowserErrors(page);
    await loginWithCredentials(page, "test", "test1234");
    await page.locator("#user-dropdown-button").waitFor({ state: "visible" });
  });

  test.afterEach(async ({ page }) => {
    expect(
      browserErrors.get(page) ?? [],
      `Failed requests: ${failedRequests.get(page)?.join(", ") ?? "none"}`,
    ).toEqual([]);
  });

  test("登录用户上传两页 PDF 后看到正确页码的预算和截止时间", async ({
    page,
    browser,
  }, testInfo) => {
    await page.goto("/opportunities");
    await expect(
      page
        .getByTestId("desktop-exploration-rail")
        .getByRole("link", { name: "招标" }),
    ).toHaveAttribute("href", "/tenders");
    await page.goto("/tenders");
    await expect(
      page.getByRole("heading", { name: "让每一条结论，回到文件原页。" }),
    ).toBeVisible();
    await expect(page.locator("header").first()).toHaveCSS(
      "background-color",
      "rgb(16, 40, 61)",
    );
    await expect(page.locator("body")).not.toContainText("{% if");
    if (testInfo.project.name === "chromium") {
      await page.screenshot({
        path: testInfo.outputPath("preview-tender-list.png"),
        fullPage: true,
      });
    }
    const pdf = syntheticTenderPdf();
    const firstStatusUrl = await upload(page, pdf);
    await expect(page.getByRole("link", { name: "查看分析报告" })).toBeVisible({
      timeout: 30_000,
    });
    await page.getByRole("link", { name: "查看分析报告" }).click();
    await expect(page.getByRole("heading", { name: "项目概况" })).toBeVisible();
    await expect(page.getByText("CNY 2800000", { exact: true })).toBeVisible();
    await expect(page.getByText("2026-09-01", { exact: true })).toBeVisible();
    const overview = page.getByRole("region", { name: "项目概况" });
    const budgetFact = overview
      .getByText("预算金额", { exact: true })
      .locator("..");
    const budgetCitation = budgetFact.locator("details");
    await expect(
      budgetFact.getByText("CNY 2800000", { exact: true }),
    ).toBeVisible();
    await expect(budgetCitation.locator("summary")).toHaveText(
      "查看第 1 页原文",
    );
    await budgetCitation.locator("summary").focus();
    await page.keyboard.press("Enter");
    await expect(budgetCitation.locator("blockquote")).toContainText(
      "Budget amount: CNY 2800000",
    );
    const deadlineFact = overview
      .getByText("投标截止时间", { exact: true })
      .locator("..");
    const deadlineCitation = deadlineFact.locator("details");
    await expect(
      deadlineFact.getByText("2026-09-01", { exact: true }),
    ).toBeVisible();
    await expect(deadlineCitation.locator("summary")).toHaveText(
      "查看第 2 页原文",
    );
    await deadlineCitation.locator("summary").click();
    await expect(deadlineCitation.locator("blockquote")).toContainText(
      "Bid deadline: 2026-09-01",
    );
    await expect(
      page.getByRole("heading", { name: "待人工确认" }),
    ).toBeVisible();
    await expect(page.getByRole("heading", { name: "原文依据" })).toBeVisible();
    await expect(page.locator("header").first()).toHaveCSS(
      "background-color",
      "rgb(16, 40, 61)",
    );
    await expect(page.locator("body")).not.toContainText("{% if");
    if (testInfo.project.name === "chromium") {
      await page.screenshot({
        path: testInfo.outputPath("preview-tender-report.png"),
        fullPage: true,
      });
    }
    const reportUrl = page.url();
    const anonymous = await browser.newContext();
    const anonymousPage = await anonymous.newPage();
    await anonymousPage.goto(reportUrl);
    await expect(anonymousPage).toHaveURL(/\/log-in(?:\?.*)?$/);
    await anonymous.close();
    await page.setViewportSize({ width: 360, height: 800 });
    expect(
      await page.evaluate(() => document.documentElement.scrollWidth),
    ).toBeLessThanOrEqual(360);
    const duplicateStatusUrl = await upload(page, pdf);
    expect(duplicateStatusUrl).toBe(firstStatusUrl);
  });

  test("伪造引用降级为待确认，不能展示为已确认金额", async ({ page }) => {
    await upload(page, syntheticTenderPdf(true));
    await expect(page.getByRole("link", { name: "查看分析报告" })).toBeVisible({
      timeout: 30_000,
    });
    await page.getByRole("link", { name: "查看分析报告" }).click();
    const overview = page.getByRole("region", { name: "项目概况" });
    await expect(
      overview
        .getByText("预算金额", { exact: true })
        .locator("..")
        .getByText("待确认", { exact: true }),
    ).toBeVisible();
    await expect(
      page.getByRole("heading", { name: /budget_amount.*缺少依据/ }),
    ).toBeVisible();
    await expect(page.getByText("Invented budget: CNY 9999999")).toHaveCount(0);
  });

  test("损坏的 PDF 在上传入口安全拒绝", async ({ page }) => {
    await page.goto("/tenders");
    const response = await page.request.post(
      `/tenders/${PROJECT_ID}/documents`,
      {
        headers: { referer: page.url() },
        multipart: {
          file: {
            name: "broken.pdf",
            mimeType: "application/pdf",
            buffer: Buffer.from("%PDF-fictional"),
          },
        },
      },
    );
    expect(response.status()).toBe(422);
    expect(await response.text()).toBe("文件不是有效的 PDF");
    expect(response.headers()["location"]).toBeUndefined();
  });

  test("有效但无文字的 PDF 提示需要文字识别并允许重试", async ({ page }) => {
    const failedStatusUrl = await upload(page, syntheticTenderPdf(false, true));
    await expect(page.getByText("分析失败", { exact: true })).toBeVisible({
      timeout: 30_000,
    });
    await expect(
      page.getByText("这份文件主要是扫描图片，当前尚未启用文字识别。"),
    ).toBeVisible();
    await expect(page.getByRole("link", { name: "查看分析报告" })).toHaveCount(
      0,
    );
    await page.getByRole("button", { name: "重新分析" }).click();
    await expect(page).toHaveURL(/\/tenders\/jobs\/[0-9a-f-]+\/status/);
    expect(page.url()).not.toBe(failedStatusUrl);
    await expect(page.getByText("分析失败", { exact: true })).toBeVisible({
      timeout: 30_000,
    });
  });

  test("无 JavaScript 时可手动刷新状态", async ({ browser }) => {
    const context = await browser.newContext({
      javaScriptEnabled: false,
      viewport: { width: 360, height: 800 },
    });
    const page = await context.newPage();
    const errors = captureBrowserErrors(page);
    await loginWithCredentials(page, "test", "test1234");
    const statusUrl = await upload(page, syntheticTenderPdf());
    await expect
      .poll(
        async () => {
          await page.goto(statusUrl);
          return page.getByRole("link", { name: "查看分析报告" }).count();
        },
        { timeout: 30_000 },
      )
      .toBe(1);
    expect(errors).toEqual([]);
    await context.close();
  });
});
