import { expect, test } from "@playwright/test";
import { navigateWithRetries } from "../shared/helpers";

test.describe("opportunity constellation foundation", () => {
  test.beforeEach(async ({ page }) => navigateWithRetries(page, "/opportunities", "opportunity home"));

  test("shows the desktop exploration rail and persists dark mode", async ({ page }) => {
    await expect(page.getByTestId("desktop-exploration-rail")).toBeVisible();
    await expect(
      page.getByTestId("desktop-exploration-rail").getByText("机会", { exact: true }),
    ).toBeVisible();
    await page.getByRole("button", { name: "切换深色沉浸模式" }).click();
    await expect(page.locator("#opportunity-app")).toHaveAttribute("data-theme", "dark");
    await page.reload();
    await expect(page.locator("#opportunity-app")).toHaveAttribute("data-theme", "dark");
  });

  test("uses the five-item mobile navigation", async ({ page }) => {
    await page.setViewportSize({ width: 390, height: 844 });
    const nav = page.getByTestId("mobile-opportunity-nav");
    await expect(nav).toBeVisible();
    await expect(nav.locator(":scope > *")).toHaveCount(5);
    await expect(nav.getByRole("link")).toHaveCount(3);
    await expect(nav.locator('[aria-disabled="true"]')).toHaveCount(2);
    await expect(nav.locator(":scope > *")).toHaveText(["机会", "人才", "AI", "消息", "我的"]);
    await expect(page.getByTestId("desktop-exploration-rail")).toBeHidden();
  });

  test("turns a natural-language request into removable conditions and synchronized results", async ({
    page,
  }) => {
    await page.getByLabel("描述你要寻找的餐饮项目").fill("广东，高校食堂，1万人以上，我有300万资金和8年经验");
    await page.getByRole("button", { name: "生成机会视图" }).click();
    const chips = page.locator("#opportunity-condition-chips button");
    await expect(chips).toHaveCount(5);
    await expect(page.locator("#opportunity-status")).toContainText("1 条");
    await expect(page.locator("[data-opportunity-card]:visible")).toHaveCount(1);
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

  test("renders explainable constellation encodings on desktop", async ({ page }) => {
    await expect(page.locator("#opportunity-graph canvas")).toBeVisible();
    await expect(page.getByText("颜色表示项目类型")).toBeVisible();
    await expect(page.getByText("光环表示可信度")).toBeVisible();
    await expect(page.getByText("脉冲表示截止紧迫度")).toBeVisible();
    await expect(page.locator("#opportunity-status")).toContainText("6 条");
  });

  test("makes the card flow primary with reduced motion", async ({ browser }) => {
    const context = await browser.newContext({
      reducedMotion: "reduce",
      viewport: { width: 1280, height: 800 },
    });
    const page = await context.newPage();
    await navigateWithRetries(page, "/opportunities", "reduced-motion opportunities");
    await expect(page.locator("#opportunity-app")).toHaveAttribute("data-experience", "fallback");
    await expect(page.locator("#opportunity-graph")).toBeHidden();
    await expect(page.locator("[data-opportunity-card]:visible")).toHaveCount(6);
    await context.close();
  });

  test("uses the opportunity card flow on mobile", async ({ page }) => {
    await page.setViewportSize({ width: 390, height: 844 });
    await expect(page.locator("#opportunity-app")).toHaveAttribute("data-experience", "fallback");
    await expect(page.locator("[data-opportunity-card]:visible").first()).toContainText("可信度");
  });
});
