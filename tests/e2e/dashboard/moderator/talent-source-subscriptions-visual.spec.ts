import { test, expect } from "@playwright/test";
import { loginWithCredentials } from "../../shared/utils";
import {
  navigateHomeWithRetries,
  navigateWithRetries,
} from "../../shared/helpers";

test.describe("GitJobs - 信息雷达移动端", () => {
  test.beforeEach(async ({ page }) => {
    await navigateHomeWithRetries(page);
    await loginWithCredentials(page, "test", "test1234");
    await page.setViewportSize({ width: 360, height: 800 });
    await navigateWithRetries(
      page,
      "/dashboard/moderator/talent/radar",
      "talent radar",
    );
  });

  test("管理员操作清晰且页面没有横向溢出", async ({ page }) => {
    await expect(page.getByRole("heading", { name: "信息雷达" })).toBeVisible();
    await expect(page.getByLabel("关注状态")).toBeVisible();
    await expect(page.getByLabel("从哪里找")).toBeVisible();
    await expect(page.getByLabel("找什么")).toBeVisible();
    await expect(page.getByLabel("多久找一次")).toBeVisible();
    await expect(page.getByText("每天一次", { exact: true })).toBeVisible();
    await expect(page.getByText("每天两次", { exact: true })).toBeAttached();
    await expect(page.getByText("每周一次", { exact: true })).toBeAttached();
    const button = page.getByRole("button", { name: "保存关注" });
    const box = await button.boundingBox();
    expect(box?.height).toBeGreaterThanOrEqual(44);
    const toggleForm = page.locator('form[action$="/toggle"]').first();
    if (await toggleForm.count()) {
      await expect(toggleForm).toHaveAttribute("method", "post");
    }
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  });

  test("后台参数不会出现在页面", async ({ page }) => {
    const body = await page.locator("body").innerText();
    for (const hiddenTerm of ["RSS", "HTTP", "游标", "并发", "请求头", "重试次数"]) {
      expect(body).not.toContain(hiddenTerm);
    }
  });
});
