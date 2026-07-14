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
});
