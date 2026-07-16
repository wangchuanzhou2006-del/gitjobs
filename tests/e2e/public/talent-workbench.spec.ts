import { expect, test } from "@playwright/test";

import { navigateWithRetries } from "../shared/helpers";

test.describe("餐饮人才工作台", () => {
  test.beforeEach(async ({ page }) => {
    await navigateWithRetries(page, "/talent", "talent workbench");
  });

  test("默认展示招聘机会并根据自然语言生成可解释匹配", async ({ page }) => {
    await expect(
      page.getByRole("button", { name: "招聘机会" }),
    ).toHaveAttribute("aria-pressed", "true");
    await expect(
      page.locator('[data-talent-card][data-talent-mode="hiring"]:visible'),
    ).toHaveCount(8);
    await expect(page.getByText("等待输入条件").first()).toBeVisible();

    await page
      .getByLabel("描述招聘或求职需求")
      .fill("北京找切配，工资7000以上，包吃住，月休4天");
    await page.getByRole("button", { name: "生成匹配" }).click();

    await expect(page.locator("#talent-condition-chips button")).toHaveCount(5);
    await expect(page.locator("#talent-status")).toContainText("条匹配结果");
    await expect(
      page.locator("[data-talent-match-score]:visible").first(),
    ).not.toHaveText("等待输入条件");

    await page.getByRole("button", { name: "求职人才" }).click();
    await expect(
      page.locator('[data-talent-card][data-talent-mode="seeking"]:visible'),
    ).toHaveCount(0);
    await expect(page.locator("#talent-condition-chips")).toContainText("北京");
    await expect(page.locator("#talent-condition-chips")).toContainText("切配");
    await expect(page.locator("#talent-condition-chips button")).toHaveCount(3);
    await expect(page.locator("#talent-status")).toHaveText("没有匹配结果");
    await expect(page.locator("[data-talent-match-score]:visible")).toHaveCount(
      0,
    );
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
    await expect(page.locator("#talent-status")).toHaveText(
      "未识别新条件，可尝试“北京找切配，工资7000以上”",
    );
  });

  test("切换模式应用公共条件且撤销恢复原模式完整条件", async ({ page }) => {
    await page
      .getByLabel("描述招聘或求职需求")
      .fill(
        "天津找切配，工资5000以上，3年经验，包吃住，月休4天，随时上岗，急招",
      );
    await page.getByRole("button", { name: "生成匹配" }).click();
    await expect(page.locator("#talent-condition-chips button")).toHaveCount(8);
    await page.evaluate(() => {
      window.__talentSelection = null;
      window.addEventListener("talent:selection", (event) => {
        window.__talentSelection = event.detail;
      });
    });

    await page.getByRole("button", { name: "求职人才" }).click();
    const chips = page.locator("#talent-condition-chips");
    await expect(chips.locator("button")).toHaveCount(3);
    await expect(chips).toContainText("天津");
    await expect(chips).toContainText("切配");
    await expect(chips).toContainText("5000");
    await expect(chips).not.toContainText("3年经验");
    await expect(chips).not.toContainText("包吃住");
    await expect(page.locator("#talent-status")).toHaveText("1 条匹配结果");
    await expect(page.locator("[data-talent-card]:visible")).toHaveCount(1);
    await expect(page.locator("[data-talent-match-score]:visible")).toHaveText(
      "匹配度 100%",
    );
    await page.locator("[data-open-talent-detail]:visible").click();
    const switchedSelection = await page.evaluate(
      () => window.__talentSelection,
    );
    expect(switchedSelection.record.id).toBe("proto-seek-02");
    expect(switchedSelection.match.score).toBe(100);

    await page.getByRole("button", { name: "撤销条件更改" }).click();
    await expect(
      page.getByRole("button", { name: "招聘机会" }),
    ).toHaveAttribute("aria-pressed", "true");
    await expect(chips.locator("button")).toHaveCount(8);
    await expect(chips).toContainText("3年经验");
    await expect(chips).toContainText("包吃住");
    await expect(page.locator("#talent-status")).toContainText("条匹配结果");
    await expect(
      page.locator("[data-talent-match-score]:visible").first(),
    ).toContainText("匹配度");
  });

  test("纯函数按活跃维度评分、硬冲突筛选并稳定排序", async ({ page }) => {
    const result = await page.evaluate(async () => {
      const initializer = [...document.scripts].find((script) =>
        script.textContent.includes("initializeTalentWorkbench"),
      );
      const workbenchUrl = initializer?.textContent.match(
        /from "([^"]*talent-workbench[^"]*)"/,
      )?.[1];
      if (!workbenchUrl) throw new Error("talent workbench asset URL missing");
      const workbenchSource = await fetch(workbenchUrl).then((response) =>
        response.text(),
      );
      const matchingUrl = workbenchSource.match(
        /from "([^"]*talent-matching[^"]*)"/,
      )?.[1];
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
      const timingMixedScore = matching.scoreTalentRecord(
        { ...exact, urgency: "常规（原型）" },
        conditions,
      );
      const ranked = matching.filterAndRankTalent(
        [preferredMiss, hardMiss, exact],
        conditions,
      );
      return {
        conditionKeys: conditions.map((condition) => condition.key),
        emptyScore,
        singleScore,
        roleRegionScore,
        timingMissScore,
        timingMixedScore,
        dimensionWeightTotal: Object.values(
          matching.TALENT_DIMENSION_WEIGHTS || {},
        ).reduce((total, weight) => total + weight, 0),
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
    expect(result.dimensionWeightTotal).toBe(100);
    expect(result.timingMissScore.score).toBe(95);
    expect(result.timingMixedScore.score).toBe(95);
    expect(result.timingMixedScore.reasons).toContain(
      "到岗时间符合：可随时到岗",
    );
    expect(result.timingMixedScore.misses).toContain(
      "紧急度未满足：当前常规（原型）",
    );
    expect(result.ranked.map(({ id }) => id)).toEqual([
      "exact",
      "preferred-miss",
    ]);
    expect(result.ranked[1].misses).toEqual(
      expect.arrayContaining([
        "住宿偏好未满足：需要包吃住",
        "月休未满足：需要4天，当前2天",
      ]),
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
});
