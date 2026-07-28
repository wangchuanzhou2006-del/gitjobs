import { expect, test } from "@playwright/test";
import type { Page } from "@playwright/test";
import { navigateWithRetries } from "../shared/helpers";

const expectFormalVisualDataset = async (page: Page) => {
  const records = await page.locator("#talent-data").evaluate((node) => {
    const data = JSON.parse(node.textContent || "");
    return [...data.hiring, ...data.seeking];
  });
  expect(records.length).toBeGreaterThan(0);
  expect(records.every((record) => record.prototype === false)).toBe(true);
  await expect(page.getByRole("note")).toHaveCount(0);
  await expect(page.locator(".talent-results > header")).toContainText("经过确认后发布");
};

const rgb = (value: string) =>
  (value.match(/[\d.]+/g) ?? [])
    .slice(0, 3)
    .map((channel) => Number(channel) / 255);
const luminance = (value: string) => {
  const [r, g, b] = rgb(value).map((channel) =>
    channel <= 0.04045 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4,
  );
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
};
const contrast = (a: string, b: string) =>
  (Math.max(luminance(a), luminance(b)) + 0.05) /
  (Math.min(luminance(a), luminance(b)) + 0.05);

test.describe("talent workbench visual contract", () => {
  test.beforeEach(async ({ page }) => {
    await navigateWithRetries(page, "/talent", "talent visual");
    await expectFormalVisualDataset(page);
  });

  test("shared navigation marks talent and keeps five mobile destinations", async ({
    page,
  }) => {
    const rail = page.getByTestId("desktop-exploration-rail");
    await expect(
      rail.getByRole("link", { name: "\u4eba\u624d" }),
    ).toHaveAttribute("aria-current", "page");
    await expect(
      rail.getByRole("link", { name: "\u673a\u4f1a", exact: true }),
    ).not.toHaveAttribute("aria-current", "page");
    await page.setViewportSize({ width: 390, height: 844 });
    const nav = page.getByTestId("mobile-opportunity-nav");
    await expect(nav.locator(":scope > *")).toHaveText([
      "\u673a\u4f1a",
      "\u4eba\u624d",
      "AI",
      "\u6d88\u606f",
      "\u6211\u7684",
    ]);
    await expect(
      nav.getByRole("link", { name: "\u4eba\u624d" }),
    ).toHaveAttribute("aria-current", "page");
    await expect(nav.getByRole("link", { name: "AI" })).toHaveAttribute(
      "href",
      "#talent-command-input",
    );
  });

  test("theme is independent, persistent, and dispatches talent event", async ({
    page,
  }) => {
    await page.evaluate(() => {
      window.__talentTheme = null;
      window.addEventListener("talent:theme", (event) => {
        window.__talentTheme = event.detail;
      });
    });
    await page
      .getByRole("button", {
        name: "\u5207\u6362\u6df1\u8272\u6c89\u6d78\u6a21\u5f0f",
      })
      .click();
    await expect(page.locator("#talent-app")).toHaveAttribute(
      "data-theme",
      "dark",
    );
    expect(
      await page.evaluate(() => localStorage.getItem("talent-theme")),
    ).toBe("dark");
    expect(await page.evaluate(() => window.__talentTheme)).toBe("dark");
    expect(
      await page.evaluate(() => localStorage.getItem("opportunity-theme")),
    ).toBeNull();
    await page.reload();
    await expect(page.locator("#talent-app")).toHaveAttribute(
      "data-theme",
      "dark",
    );
  });

  test("1280 through 390 uses explicit responsive fallbacks", async ({
    page,
  }) => {
    await page.setViewportSize({ width: 1280, height: 900 });
    const grid = page.locator(".talent-card-grid");
    expect(
      await grid.evaluate(
        (node) => getComputedStyle(node).gridTemplateColumns.split(" ").length,
      ),
    ).toBe(2);
    await expect(
      page.locator("[data-talent-match-lens]").first(),
    ).toBeVisible();
    await page.setViewportSize({ width: 1000, height: 900 });
    await expect(
      page.locator("[data-talent-supporting-fact]:visible"),
    ).toHaveCount(0);
    await page.setViewportSize({ width: 767, height: 900 });
    await expect(page.getByTestId("desktop-exploration-rail")).toBeHidden();
    await expect(page.getByTestId("mobile-opportunity-nav")).toBeVisible();
    await page.setViewportSize({ width: 640, height: 900 });
    expect(
      await grid.evaluate(
        (node) => getComputedStyle(node).gridTemplateColumns.split(" ").length,
      ),
    ).toBe(1);
    await page.setViewportSize({ width: 390, height: 844 });
    expect(
      await page.evaluate(
        () => document.documentElement.scrollWidth <= innerWidth,
      ),
    ).toBe(true);
    await page.locator("[data-open-talent-detail]:visible").first().click();
    const box = await page.locator("[data-talent-dialog]").boundingBox();
    expect(box).not.toBeNull();
    expect(box.width).toBeGreaterThanOrEqual(351);
    expect(box.y).toBeGreaterThan(0);
    expect(box.y + box.height).toBeLessThanOrEqual(844);
  });

  test("reduced motion keeps facts visible and durations at 0.01ms", async ({
    browser,
  }) => {
    const context = await browser.newContext({
      reducedMotion: "reduce",
      viewport: { width: 1280, height: 900 },
    });
    const page = await context.newPage();
    await navigateWithRetries(page, "/talent", "reduced talent");
    await expectFormalVisualDataset(page);
    const card = page.locator("[data-talent-card]:visible").first();
    await expect(card.locator("dl")).toBeVisible();
    const values = await card.evaluate((node) => {
      const ms = (value: string) =>
        value
          .split(",")
          .map((part) =>
            part.trim().endsWith("ms")
              ? parseFloat(part)
              : parseFloat(part) * 1000,
          );
      const style = getComputedStyle(node);
      return [...ms(style.transitionDuration), ...ms(style.animationDuration)];
    });
    expect(values.every((value) => value <= 0.01)).toBe(true);

    await card.hover();
    expect(
      await card.evaluate((node) => getComputedStyle(node).transform),
    ).toBe("none");

    const submit = page.locator('#talent-command-form button[type="submit"]');
    await submit.hover();
    await page.mouse.down();
    const activeTransform = await submit.evaluate(
      (node) => getComputedStyle(node).transform,
    );
    await page.mouse.up();
    expect(activeTransform).toBe("none");
    await context.close();
  });

  test("focus and primary action meet contrast thresholds", async ({
    page,
  }) => {
    await page
      .getByLabel("\u63cf\u8ff0\u62db\u8058\u6216\u6c42\u804c\u9700\u6c42")
      .focus();
    const colors = await page.evaluate(() => {
      const app = document.getElementById("talent-app");
      const input = document.getElementById("talent-command-input");
      const button = document.querySelector(
        "#talent-command-form button[type='submit']",
      );
      const probe = document.createElement("span");
      probe.style.color = "var(--opp-bg)";
      app.append(probe);
      const background = getComputedStyle(probe).color;
      probe.remove();
      return {
        focus: getComputedStyle(input).outlineColor,
        background,
        text: getComputedStyle(button).color,
        button: getComputedStyle(button).backgroundColor,
      };
    });
    expect(contrast(colors.focus, colors.background)).toBeGreaterThanOrEqual(3);
    expect(contrast(colors.text, colors.button)).toBeGreaterThanOrEqual(4.5);
  });
});

test("opportunity talent links resolve to /talent", async ({ page }) => {
  await navigateWithRetries(page, "/opportunities", "opportunity talent link");
  const desktop = page.getByTestId("desktop-exploration-rail");
  await expect(
    desktop.getByRole("link", { name: "\u4eba\u624d" }),
  ).toHaveAttribute("href", "/talent");
  await page.setViewportSize({ width: 390, height: 844 });
  const mobile = page.getByTestId("mobile-opportunity-nav");
  await expect(
    mobile.getByRole("link", { name: "\u4eba\u624d" }),
  ).toHaveAttribute("href", "/talent");
});
