import { expect, test } from "@playwright/test";
import { navigateWithRetries } from "../shared/helpers";

const parseRgb = (value: string) =>
  (value.match(/[\d.]+/g) ?? []).slice(0, 3).map((channel) => Number(channel) / 255);

const relativeLuminance = (color: string) => {
  const [red, green, blue] = parseRgb(color).map((channel) =>
    channel <= 0.04045 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4,
  );
  return 0.2126 * red + 0.7152 * green + 0.0722 * blue;
};

const contrastRatio = (first: string, second: string) => {
  const lighter = Math.max(relativeLuminance(first), relativeLuminance(second));
  const darker = Math.min(relativeLuminance(first), relativeLuminance(second));
  return (lighter + 0.05) / (darker + 0.05);
};

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
    await expect(page.locator("#opportunity-graph canvas")).toBeVisible();
    const graphNodeCount = await page.evaluate(() => {
      const host = document.getElementById("opportunity-graph");
      const chart = (window as any).echarts.getInstanceByDom(host);
      return chart
        .getOption()
        .series.reduce((count: number, series: { data: unknown[] }) => count + series.data.length, 0);
    });
    expect(graphNodeCount).toBe(1);
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
    const encoding = await page.evaluate(() => {
      const host = document.getElementById("opportunity-graph");
      const option = (window as any).echarts.getInstanceByDom(host).getOption();
      return {
        renderMode: option.tooltip[0].renderMode,
        periods: option.series.map(
          (series: { rippleEffect: { period: number } }) => series.rippleEffect.period,
        ),
        nodeCount: option.series.reduce(
          (count: number, series: { data: unknown[] }) => count + series.data.length,
          0,
        ),
      };
    });
    expect(encoding).toEqual({ renderMode: "richText", periods: [1.8, 4.2], nodeCount: 6 });
  });


  test("presents complete opportunity facts without a fake action", async ({ page }) => {
    const card = page.locator("[data-opportunity-card]").first();
    await expect(card.getByRole("heading", { name: "广东高校食堂合作机会" })).toBeVisible();
    await expect(card).toContainText("服务规模");
    await expect(card).toContainText("12000 人");
    await expect(card).toContainText("92 / 100（证据评分，非平台担保）");
    await expect(card).toContainText("5 天后");
    await expect(card).toContainText("来源：学校官网（原型字段）");
    await expect(card.getByRole("button")).toHaveCount(0);
  });
  test("makes the card flow primary with reduced motion", async ({ browser }) => {
    const context = await browser.newContext({
      reducedMotion: "reduce",
      viewport: { width: 1280, height: 800 },
    });
    const page = await context.newPage();
    const echartsRequests: string[] = [];
    page.on("request", (request) => {
      if (request.url().includes("echarts.v6.0.0.min.js")) echartsRequests.push(request.url());
    });
    await navigateWithRetries(page, "/opportunities", "reduced-motion opportunities");
    await expect(page.locator("#opportunity-app")).toHaveAttribute("data-experience", "fallback");
    await expect(page.locator("#opportunity-graph")).toBeHidden();
    await expect(page.locator("#opportunity-graph canvas")).toHaveCount(0);
    await expect(page.locator("[data-opportunity-card]:visible")).toHaveCount(6);
    await expect(page.getByText("界面原型数据，不代表真实项目", { exact: true })).toBeVisible();
    expect(echartsRequests).toEqual([]);
    await context.close();
  });

  test("uses the opportunity card flow on mobile", async ({ browser }) => {
    const context = await browser.newContext({ viewport: { width: 390, height: 844 } });
    const page = await context.newPage();
    const echartsRequests: string[] = [];
    page.on("request", (request) => {
      if (request.url().includes("echarts.v6.0.0.min.js")) echartsRequests.push(request.url());
    });
    await navigateWithRetries(page, "/opportunities", "mobile opportunities");
    await expect(page.locator("#opportunity-app")).toHaveAttribute("data-experience", "fallback");
    await expect(page.locator("[data-opportunity-card]:visible").first()).toContainText("可信度");
    expect(echartsRequests).toEqual([]);
    await context.close();
  });

  test("uses fallback when data saver is enabled", async ({ browser }) => {
    const context = await browser.newContext({ viewport: { width: 1280, height: 800 } });
    await context.addInitScript(() => {
      Object.defineProperty(navigator, "connection", {
        configurable: true,
        value: { saveData: true },
      });
    });
    const page = await context.newPage();
    const echartsRequests: string[] = [];
    page.on("request", (request) => {
      if (request.url().includes("echarts.v6.0.0.min.js")) echartsRequests.push(request.url());
    });
    await navigateWithRetries(page, "/opportunities", "data-saver opportunities");
    await expect(page.locator("#opportunity-app")).toHaveAttribute("data-experience", "fallback");
    await expect(page.locator("#opportunity-graph canvas")).toHaveCount(0);
    await expect(page.getByText("界面原型数据，不代表真实项目", { exact: true })).toBeVisible();
    expect(echartsRequests).toEqual([]);
    await context.close();
  });

  test("uses fallback on low-memory devices", async ({ browser }) => {
    const context = await browser.newContext({ viewport: { width: 1280, height: 800 } });
    await context.addInitScript(() => {
      Object.defineProperty(navigator, "deviceMemory", { configurable: true, value: 4 });
    });
    const page = await context.newPage();
    const echartsRequests: string[] = [];
    page.on("request", (request) => {
      if (request.url().includes("echarts.v6.0.0.min.js")) echartsRequests.push(request.url());
    });
    await navigateWithRetries(page, "/opportunities", "low-memory opportunities");
    await expect(page.locator("#opportunity-app")).toHaveAttribute("data-experience", "fallback");
    await expect(page.locator("#opportunity-graph canvas")).toHaveCount(0);
    await expect(page.getByText("界面原型数据，不代表真实项目", { exact: true })).toBeVisible();
    expect(echartsRequests).toEqual([]);
    await context.close();
  });

  test("uses fallback when ECharts is unavailable", async ({ browser }) => {
    const context = await browser.newContext({ viewport: { width: 1280, height: 800 } });
    const page = await context.newPage();
    const errors: string[] = [];
    page.on("console", (message) => {
      if (
        message.type() === "error" &&
        message.text() !== "Failed to load resource: net::ERR_FAILED"
      ) errors.push(message.text());
    });
    await page.route("**/echarts.v6.0.0.min.js", (route) => route.abort());
    await navigateWithRetries(page, "/opportunities", "no-echarts opportunities");
    await expect(page.locator("#opportunity-app")).toHaveAttribute("data-experience", "fallback");
    await expect(page.locator("#opportunity-status")).toContainText("6 条");
    await expect(page.locator("[data-opportunity-card]:visible")).toHaveCount(6);
    await expect(page.locator("#opportunity-graph canvas")).toHaveCount(0);
    await expect(page.getByText("界面原型数据，不代表真实项目", { exact: true })).toBeVisible();
    expect(errors).toEqual([]);
    await context.close();
  });

  test("disposes and rebuilds the constellation across responsive fallback", async ({ page }) => {
    await expect(page.locator("#opportunity-graph canvas")).toBeVisible();
    await page.setViewportSize({ width: 390, height: 844 });
    await expect(page.locator("#opportunity-app")).toHaveAttribute("data-experience", "fallback");
    await expect(page.locator("#opportunity-graph canvas")).toHaveCount(0);
    await page.setViewportSize({ width: 1280, height: 800 });
    await expect(page.locator("#opportunity-app")).toHaveAttribute("data-experience", "constellation");
    await expect(page.locator("#opportunity-graph canvas")).toBeVisible();
  });

  test("responds immediately when reduced-motion preference changes", async ({ page }) => {
    await expect(page.locator("#opportunity-graph canvas")).toBeVisible();
    await page.emulateMedia({ reducedMotion: "reduce" });
    await expect(page.locator("#opportunity-app")).toHaveAttribute("data-experience", "fallback");
    await expect(page.locator("#opportunity-graph canvas")).toHaveCount(0);
    await page.emulateMedia({ reducedMotion: "no-preference" });
    await expect(page.locator("#opportunity-app")).toHaveAttribute("data-experience", "constellation");
    await expect(page.locator("#opportunity-graph canvas")).toBeVisible();
  });

  test("exposes landmarks, visible keyboard focus, and the non-production boundary", async ({
    page,
  }) => {
    await expect(page.getByRole("main")).toBeVisible();
    await expect(
      page.locator('[role="note"]', { hasText: "界面原型数据，不代表真实项目" }),
    ).toBeVisible();
    await page.keyboard.press("Tab");
    const skipLink = page.locator('a[href="#main-content"]');
    await expect(skipLink).toBeFocused();
    await expect(skipLink).toBeVisible();
    await expect(skipLink).toHaveCSS("outline-style", "solid");
    await expect(page.getByText("证据评分，非平台担保").first()).toBeVisible();
  });

  test("meets contrast thresholds for interactive text and keyboard focus", async ({ page }) => {
    const input = page.getByLabel("描述你要寻找的餐饮项目");
    await input.focus();
    const colors = await page.evaluate(() => {
      const app = document.getElementById("opportunity-app");
      const focusTarget = document.getElementById("ai-command-input");
      const submit = document.querySelector(".opportunity-command-row button");
      const backgroundProbe = document.createElement("span");
      backgroundProbe.style.color = "var(--opp-bg)";
      app.append(backgroundProbe);
      const lightBackground = getComputedStyle(backgroundProbe).color;
      backgroundProbe.remove();
      const focusStyle = getComputedStyle(focusTarget);
      const submitStyle = getComputedStyle(submit);
      return {
        focus: focusStyle.outlineColor,
        lightBackground,
        text: submitStyle.color,
        buttonBackground: submitStyle.backgroundColor,
      };
    });
    expect(contrastRatio(colors.focus, colors.lightBackground)).toBeGreaterThanOrEqual(3);
    expect(contrastRatio(colors.text, colors.buttonBackground)).toBeGreaterThanOrEqual(4.5);
  });

  test("loads without browser console errors", async ({ page }) => {
    const errors: string[] = [];
    page.on("console", (message) => {
      if (message.type() === "error") errors.push(message.text());
    });
    await page.reload();
    await page.waitForLoadState("networkidle");
    expect(errors).toEqual([]);
  });
});
