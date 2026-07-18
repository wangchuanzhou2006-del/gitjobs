import { expect, test } from "@playwright/test";
import type { Page } from "@playwright/test";

import { navigateWithRetries } from "../shared/helpers";

const browserErrors = new WeakMap<Page, string[]>();

const captureBrowserErrors = (page: Page): string[] => {
  const errors: string[] = [];
  browserErrors.set(page, errors);
  page.on("console", (message) => {
    if (message.type() === "error") errors.push(`console.error: ${message.text()}`);
  });
  page.on("pageerror", (error) => errors.push(`pageerror: ${error.message}`));
  return errors;
};

test.describe("餐饮人才工作台", () => {
  test.beforeEach(async ({ page }) => {
    captureBrowserErrors(page);
    await navigateWithRetries(page, "/talent", "talent workbench");
  });

  test.afterEach(async ({ page }) => {
    expect(browserErrors.get(page) ?? []).toEqual([]);
  });

  test("禁用 JavaScript 时招聘与求职各有 8 条服务端结果可读", async ({ browser }) => {
    const context = await browser.newContext({ javaScriptEnabled: false });
    const page = await context.newPage();
    const errors = captureBrowserErrors(page);
    await navigateWithRetries(page, "/talent", "talent workbench without JavaScript");

    await expect(page.locator('[data-talent-card][data-talent-mode="hiring"]:visible')).toHaveCount(8);
    await expect(page.locator('[data-talent-card][data-talent-mode="seeking"]:visible')).toHaveCount(8);
    await expect(page.getByRole("link", { name: "机会" }).first()).toHaveAttribute("href", "/opportunities");
    expect(errors).toEqual([]);
    await context.close();
  });

  test("交互数据损坏时播报降级状态并保留全部服务端结果", async ({ page }) => {
    await page.route("**/talent", async (route) => {
      const response = await route.fetch();
      const html = await response.text();
      await route.fulfill({
        response,
        body: html.replace(
          /(<script id="talent-data" type="application\/json">)[\s\S]*?(<\/script>)/,
          "$1{invalid json$2",
        ),
      });
    });
    await page.reload({ waitUntil: "domcontentloaded" });

    await expect(page.locator("#talent-status")).toHaveText("交互数据暂不可用");
    await expect(page.locator('[data-talent-card][data-talent-mode="hiring"]:visible')).toHaveCount(8);
    await expect(page.locator('[data-talent-card][data-talent-mode="seeking"]:visible')).toHaveCount(8);
    await expect(page.getByRole("link", { name: "机会" }).first()).toHaveAttribute("href", "/opportunities");
  });

  test("交互数据节点缺失时保留全部服务端结果", async ({ page }) => {
    await page.route("**/talent", async (route) => {
      const response = await route.fetch();
      const html = await response.text();
      await route.fulfill({
        response,
        body: html.replace(/<script id="talent-data" type="application\/json">[\s\S]*?<\/script>/, ""),
      });
    });
    await page.reload({ waitUntil: "domcontentloaded" });

    await expect(page.locator("#talent-status")).toHaveText("交互数据暂不可用");
    await expect(page.locator("[data-talent-card]:visible")).toHaveCount(16);
    await expect(page.getByRole("link", { name: "机会" }).first()).toHaveAttribute("href", "/opportunities");
  });

  test("交互数据形状错误时保留全部服务端结果", async ({ page }) => {
    await page.route("**/talent", async (route) => {
      const response = await route.fetch();
      const html = await response.text();
      await route.fulfill({
        response,
        body: html.replace(
          /(<script id="talent-data" type="application\/json">)[\s\S]*?(<\/script>)/,
          '$1{"hiring":{},"seeking":[]}$2',
        ),
      });
    });
    await page.reload({ waitUntil: "domcontentloaded" });

    await expect(page.locator("#talent-status")).toHaveText("交互数据暂不可用");
    await expect(page.locator("[data-talent-card]:visible")).toHaveCount(16);
    await expect(page.getByRole("link", { name: "机会" }).first()).toHaveAttribute("href", "/opportunities");
  });
  test("Storage 读写被拒绝时主题和工作台仍完成初始化", async ({ browser }) => {
    const context = await browser.newContext({ colorScheme: "dark" });
    await context.addInitScript(() => {
      for (const method of ["getItem", "setItem"] as const) {
        Object.defineProperty(Storage.prototype, method, {
          configurable: true,
          value: () => {
            throw new DOMException("Storage access denied", "SecurityError");
          },
        });
      }
      window.__talentThemeEvents = [];
      window.addEventListener("talent:theme", (event) => {
        window.__talentThemeEvents.push(event.detail);
      });
    });
    const page = await context.newPage();
    const errors = captureBrowserErrors(page);
    await navigateWithRetries(page, "/talent", "talent workbench without Storage");

    await expect(page.locator("#talent-app")).toHaveAttribute("data-theme", "dark");
    await expect(page.locator('[data-talent-card][data-talent-mode="hiring"]:visible')).toHaveCount(8);
    const toggle = page.locator("#opportunity-theme-toggle");
    await toggle.click();
    await expect(page.locator("#talent-app")).toHaveAttribute("data-theme", "light");
    await expect(toggle).toHaveAttribute("aria-label", "切换深色沉浸模式");
    expect(await page.evaluate(() => window.__talentThemeEvents)).toEqual(["light"]);
    expect(errors).toEqual([]);
    await context.close();
  });

  test("默认展示招聘机会并根据自然语言生成可解释匹配", async ({ page }) => {
    await expect(page.getByRole("button", { name: "招聘机会" })).toHaveAttribute("aria-pressed", "true");
    await expect(page.locator('[data-talent-card][data-talent-mode="hiring"]:visible')).toHaveCount(8);
    await expect(page.getByText("等待输入条件").first()).toBeVisible();

    await page.getByLabel("描述招聘或求职需求").fill("北京找切配，工资7000以上，包吃住，月休4天");
    await page.getByRole("button", { name: "生成匹配" }).click();

    await expect(page.locator("#talent-condition-chips button")).toHaveCount(5);
    await expect(page.locator("#talent-status")).toContainText("条匹配结果");
    await expect(page.locator("[data-talent-match-score]:visible").first()).not.toHaveText("等待输入条件");

    await page.getByRole("button", { name: "求职人才" }).click();
    await expect(page.locator('[data-talent-card][data-talent-mode="seeking"]:visible')).toHaveCount(0);
    await expect(page.locator("#talent-condition-chips")).toContainText("北京");
    await expect(page.locator("#talent-condition-chips")).toContainText("切配");
    await expect(page.locator("#talent-condition-chips button")).toHaveCount(3);
    await expect(page.locator("#talent-status")).toHaveText("没有匹配结果");
    await expect(page.locator("[data-talent-match-score]:visible")).toHaveCount(0);
  });

  test("支持移除条件、撤销和 Enter 提交", async ({ page }) => {
    const input = page.getByLabel("描述招聘或求职需求");
    await input.fill("天津找切配，工资5000以上");
    await input.press("Enter");
    await expect(page.locator("#talent-condition-chips button")).toHaveCount(3);

    await page.getByRole("button", { name: "移除条件：天津" }).click();
    await expect(page.locator("#talent-condition-chips button")).toHaveCount(2);
    await page.getByRole("button", { name: "撤销条件更改" }).click();
    await expect(page.locator("#talent-condition-chips button")).toHaveCount(3);
    await expect(page.locator("#talent-condition-chips")).toContainText("天津");
  });

  test("无结果与未识别指令通过同一实时状态说明", async ({ page }) => {
    const input = page.getByLabel("描述招聘或求职需求");
    await input.fill("北京找店长，工资99999以上");
    await page.getByRole("button", { name: "生成匹配" }).click();
    await expect(page.locator("#talent-status")).toContainText("没有匹配结果");
    await expect(page.locator("[data-talent-card]:visible")).toHaveCount(0);

    await input.fill("请给我最合适的");
    await page.getByRole("button", { name: "生成匹配" }).click();
    await expect(input).toHaveValue("请给我最合适的");
    await expect(page.locator("#talent-condition-chips button")).toHaveCount(3);
    await expect(page.locator("#talent-status")).toHaveText("未识别新条件，可尝试“北京找切配，工资7000以上”");
  });

  test("切换模式应用公共条件且撤销恢复原模式完整条件", async ({ page }) => {
    await page
      .getByLabel("描述招聘或求职需求")
      .fill("天津找切配和川湘菜，工资5000以上，3年经验，包吃住，月休4天，随时上岗，急招");
    await page.getByRole("button", { name: "生成匹配" }).click();
    await expect(page.locator("#talent-condition-chips button")).toHaveCount(9);
    await page.evaluate(() => {
      window.__talentSelection = null;
      window.addEventListener("talent:selection", (event) => {
        window.__talentSelection = event.detail;
      });
    });

    await page.getByRole("button", { name: "求职人才" }).click();
    const chips = page.locator("#talent-condition-chips");
    await expect(chips.locator("button")).toHaveCount(4);
    await expect(chips).toContainText("天津");
    await expect(chips).toContainText("切配");
    await expect(chips).toContainText("5000");
    await expect(chips).toContainText("川湘菜");
    await expect(chips).not.toContainText("3年经验");
    await expect(chips).not.toContainText("包吃住");
    await expect(page.locator("#talent-status")).toHaveText("1 条匹配结果");
    await expect(page.locator("[data-talent-card]:visible")).toHaveCount(1);
    await expect(page.locator("[data-talent-match-score]:visible")).toHaveText("匹配度 80%");
    await page.locator("[data-open-talent-detail]:visible").click();
    const switchedSelection = await page.evaluate(() => window.__talentSelection);
    expect(switchedSelection.record.id).toBe("proto-seek-02");
    expect(switchedSelection.match.score).toBe(80);
    await page.getByRole("button", { name: "关闭人才详情" }).click();

    await page.getByRole("button", { name: "撤销条件更改" }).click();
    await expect(page.getByRole("button", { name: "招聘机会" })).toHaveAttribute("aria-pressed", "true");
    await expect(chips.locator("button")).toHaveCount(9);
    await expect(chips).toContainText("3年经验");
    await expect(chips).toContainText("包吃住");
    await expect(page.locator("#talent-status")).toContainText("条匹配结果");
    await expect(page.locator("[data-talent-match-score]:visible").first()).toContainText("匹配度");
  });

  test("纯函数按活跃维度评分、硬冲突筛选并稳定排序", async ({ page }) => {
    const result = await page.evaluate(async () => {
      const initializer = [...document.scripts].find((script) =>
        script.textContent.includes("initializeTalentWorkbench"),
      );
      const workbenchUrl = initializer?.textContent.match(/from "([^"]*talent-workbench[^"]*)"/)?.[1];
      if (!workbenchUrl) throw new Error("talent workbench asset URL missing");
      const workbenchSource = await fetch(workbenchUrl).then((response) => response.text());
      const matchingUrl = workbenchSource.match(/from "([^"]*talent-matching[^"]*)"/)?.[1];
      if (!matchingUrl) throw new Error("talent matching asset URL missing");
      const matching = await import(matchingUrl);
      const conditions = matching.parseTalentCommand(
        "北京找切配，工资7000以上，川湘菜，3年经验，包吃住，月休4天，随时上岗，急招",
        "hiring",
      );
      const base = {
        mode: "hiring",
        title: "切配岗位（原型）",
        scene: "门店（原型）",
        region: "北京",
        district: "朝阳区",
        salaryMin: 7000,
        salaryMax: 9000,
        skills: ["切配", "川湘菜"],
        experienceYears: 4,
        hoursPerDay: 8,
        restDays: 4,
        lodging: "提供（原型）",
        housingAllowance: 0,
        availability: "随时到岗（原型）",
        urgency: "较高（原型）",
        sourceLabel: "平台演示数据",
        updatedLabel: "近期更新（原型）",
        riskFlags: ["岗位细节待确认（原型）"],
        prototype: true,
      };
      const exact = { ...base, id: "exact", credibility: 80 };
      const sameScoreHighA = { ...base, id: "same-score-high-a", credibility: 95 };
      const sameScoreHighB = { ...base, id: "same-score-high-b", credibility: 95 };
      const preferredMiss = {
        ...base,
        id: "preferred-miss",
        credibility: 99,
        lodging: "不提供（原型）",
        restDays: 2,
      };
      const hardMiss = {
        ...base,
        id: "hard-miss",
        credibility: 100,
        region: "天津",
      };
      const emptyScore = matching.scoreTalentRecord(exact, []);
      const singleScore = matching.scoreTalentRecord(exact, [
        { key: "region", value: "北京", label: "北京" },
      ]);
      const roleRegionScore = matching.scoreTalentRecord(exact, [
        { key: "role", value: "切配", label: "切配" },
        { key: "region", value: "天津", label: "天津" },
      ]);
      const timingMissScore = matching.scoreTalentRecord(
        {
          ...exact,
          availability: "两周后到岗（原型）",
          urgency: "常规（原型）",
        },
        conditions,
      );
      const timingMixedScore = matching.scoreTalentRecord({ ...exact, urgency: "常规（原型）" }, conditions);
      const nativeSort = Array.prototype.sort;
      Array.prototype.sort = function (compare) {
        return nativeSort.call(this, (left, right) => {
          const result = compare(left, right);
          if (result !== 0) return result;
          return String(right?.record?.id ?? "").localeCompare(String(left?.record?.id ?? ""));
        });
      };
      let ranked;
      try {
        ranked = matching.filterAndRankTalent(
          [exact, sameScoreHighA, hardMiss, sameScoreHighB, preferredMiss],
          conditions,
        );
      } finally {
        Array.prototype.sort = nativeSort;
      }
      return {
        conditionKeys: conditions.map((condition) => condition.key),
        emptyScore,
        singleScore,
        roleRegionScore,
        timingMissScore,
        timingMixedScore,
        dimensionWeights: matching.TALENT_DIMENSION_WEIGHTS,
        ranked: ranked.map(({ record, match }) => ({
          id: record.id,
          score: match.score,
          misses: match.misses,
          risks: match.risks,
        })),
      };
    });

    expect(result.conditionKeys).toEqual([
      "region",
      "role",
      "skills",
      "salaryMin",
      "experienceYears",
      "restDays",
      "lodging",
      "availability",
      "urgency",
    ]);
    expect(result.emptyScore).toEqual({
      score: null,
      reasons: [],
      misses: [],
      risks: ["岗位细节待确认（原型）"],
    });
    expect(result.singleScore.score).toBe(100);
    expect(result.singleScore.reasons).toContain("地区符合：北京");
    expect(result.roleRegionScore.score).toBe(50);
    expect(result.dimensionWeights).toEqual({
      role: 20,
      region: 20,
      salaryMin: 20,
      skills: 15,
      experienceYears: 10,
      lodging: 5,
      restDays: 5,
      timing: 5,
    });
    expect(result.timingMissScore.score).toBe(95);
    expect(result.timingMixedScore.score).toBe(95);
    expect(result.timingMixedScore.reasons).toContain("到岗时间符合：可随时到岗");
    expect(result.timingMixedScore.misses).toContain("紧急度未满足：当前常规（原型）");
    expect(result.ranked.map(({ id }) => id)).toEqual([
      "same-score-high-a",
      "same-score-high-b",
      "exact",
      "preferred-miss",
    ]);
    expect(result.ranked[3].misses).toEqual(
      expect.arrayContaining(["住宿偏好未满足：需要包吃住", "月休未满足：需要4天，当前2天"]),
    );
  });

  test("选择卡片发出供详情模块消费的匹配事件", async ({ page }) => {
    await page.evaluate(() => {
      window.__talentSelection = null;
      window.addEventListener("talent:selection", (event) => {
        window.__talentSelection = event.detail;
      });
    });
    await page.locator("[data-open-talent-detail]:visible").first().click();
    const selection = await page.evaluate(() => window.__talentSelection);
    expect(selection.record.id).toBeTruthy();
    expect(selection.match).toEqual({
      score: null,
      reasons: [],
      misses: [],
      risks: expect.any(Array),
    });
  });

  test("详情抽屉展示完整脱敏信息并约束键盘焦点", async ({ page }) => {
    const trigger = page.locator("[data-open-talent-detail]:visible").first();
    await trigger.click();

    const drawer = page.locator("#talent-detail-drawer");
    const dialog = drawer.locator("[data-talent-dialog]");
    await expect(drawer).toHaveAttribute("data-open", "true");
    await expect(dialog).toHaveAttribute("role", "dialog");
    await expect(dialog).toHaveAttribute("aria-modal", "true");
    await expect(dialog).toContainText("店长岗位（原型）");
    await expect(dialog).toContainText("社区餐饮门店（原型）");
    await expect(dialog).toContainText("安徽 合肥蜀山区");
    await expect(dialog).toContainText("7600–10500 元/月");
    await expect(dialog).toContainText("店长、餐饮门店协作（原型）");
    await expect(dialog.getByRole("heading", { name: "命中条件", exact: true })).toBeVisible();
    await expect(dialog.getByRole("heading", { name: "未命中条件" })).toBeVisible();
    await expect(dialog.getByRole("heading", { name: "风险与缺失字段" })).toBeVisible();
    await expect(dialog).toContainText("岗位细节待确认（原型）");
    await expect(dialog).toContainText("来源说明：平台演示数据");
    await expect(dialog).toContainText("更新时间：近期更新（原型）");
    await expect(dialog).toContainText("联系方式：1**********（原型占位）");
    await expect(
      dialog.getByRole("button", {
        name: "登录后申请联系（功能未开放）",
      }),
    ).toBeDisabled();
    await expect(dialog).toContainText("等待输入条件");
    await expect(dialog).toContainText("未生成 AI 结论");
    await expect(page.locator("body")).toHaveClass(/talent-detail-scroll-locked/);

    for (let index = 0; index < 6; index += 1) {
      await page.keyboard.press("Tab");
      expect(await dialog.evaluate((node) => node.contains(document.activeElement))).toBe(true);
    }
    await page.keyboard.press("Shift+Tab");
    expect(await dialog.evaluate((node) => node.contains(document.activeElement))).toBe(true);

    await dialog.getByRole("heading", { name: "店长岗位（原型）" }).click();
    await expect(drawer).toHaveAttribute("data-open", "true");

    await page.keyboard.press("Escape");
    await expect(drawer).toBeHidden();
    await expect(page.locator("body")).not.toHaveClass(/talent-detail-scroll-locked/);
    await expect(trigger).toBeFocused();

    await trigger.click();
    await drawer.click({ position: { x: 8, y: 8 } });
    await expect(drawer).toBeHidden();
    await expect(page.locator("body")).not.toHaveClass(/talent-detail-scroll-locked/);
    await expect(trigger).toBeFocused();
  });

  test("drawer 在 390px 视口保持可用且页面无水平溢出", async ({ page }) => {
    await page.setViewportSize({ width: 390, height: 844 });
    await page.locator("[data-open-talent-detail]:visible").first().click();

    const dialog = page.locator("[data-talent-dialog]");
    await expect(dialog).toBeVisible();
    const bounds = await dialog.boundingBox();
    expect(bounds).not.toBeNull();
    expect(bounds.width).toBeGreaterThanOrEqual(351);
    expect(
      await page.evaluate(() => document.documentElement.scrollWidth <= document.documentElement.clientWidth),
    ).toBe(true);
  });
});
