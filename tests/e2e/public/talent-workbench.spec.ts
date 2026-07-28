import { expect, test } from "@playwright/test";
import type { Page } from "@playwright/test";

import { navigateWithRetries } from "../shared/helpers";

const browserErrors = new WeakMap<Page, string[]>();
const SEEDED_PENDING_TITLES = ["E2E 种子费用待核招聘", "E2E 种子高相似招聘"];

type TalentRecord = {
  id: string;
  mode: "hiring" | "seeking";
  title: string;
  scene: string | null;
  region: string;
  district: string;
  locationTerms: string[];
  salaryMin: number | null;
  salaryMax: number | null;
  roles: string[];
  skills: string[];
  experienceYears: number | null;
  hoursPerDay: number | null;
  restDays: number | null;
  lodging: string | null;
  housingAllowance: boolean | null;
  availability: string | null;
  urgency: string | null;
  credibility: number | null;
  sourceLabel: string | null;
  updatedLabel: string;
  riskFlags: string[];
  contactMasked: string | null;
  prototype: boolean;
};

type TalentDataset = { hiring: TalentRecord[]; seeking: TalentRecord[] };

const readTalentDataset = async (page: Page): Promise<TalentDataset> =>
  page.locator("#talent-data").evaluate((node) => JSON.parse(node.textContent || ""));

const expectFormalSeedDataset = async (page: Page): Promise<TalentDataset> => {
  const dataset = await readTalentDataset(page);
  const records = [...dataset.hiring, ...dataset.seeking];
  expect(dataset.hiring.length).toBeGreaterThan(0);
  expect(dataset.seeking.length).toBeGreaterThan(0);
  expect(records.every((record) => record.prototype === false)).toBe(true);
  for (const title of SEEDED_PENDING_TITLES) {
    expect(records.map((record) => record.title)).not.toContain(title);
    await expect(page.locator("body")).not.toContainText(title);
  }
  await expect(page.getByRole("note")).toHaveCount(0);
  await expect(page.locator(".talent-results > header")).toContainText("经过确认后发布");
  await expect(page.locator(".talent-results > header")).not.toContainText("虚构演示数据");
  return dataset;
};

const visibleCardCount = (page: Page) => page.locator("[data-talent-card]:visible").count();

const salaryLabel = (record: TalentRecord) => {
  if (record.salaryMin !== null && record.salaryMax !== null)
    return `${record.salaryMin}–${record.salaryMax} 元/月`;
  if (record.salaryMin !== null) return `${record.salaryMin} 元/月起`;
  if (record.salaryMax !== null) return `最高 ${record.salaryMax} 元/月`;
  return "待确认";
};

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
    await expectFormalSeedDataset(page);
  });

  test.afterEach(async ({ page }) => {
    expect(browserErrors.get(page) ?? []).toEqual([]);
  });

  test("禁用 JavaScript 时正式发布数据仍由服务端完整呈现", async ({ browser }) => {
    const context = await browser.newContext({ javaScriptEnabled: false });
    const page = await context.newPage();
    const errors = captureBrowserErrors(page);
    await navigateWithRetries(page, "/talent", "talent workbench without JavaScript");

    expect(await page.locator('[data-talent-card][data-talent-mode="hiring"]:visible').count()).toBeGreaterThan(0);
    expect(await page.locator('[data-talent-card][data-talent-mode="seeking"]:visible').count()).toBeGreaterThan(0);
    await expect(page.getByRole("note")).toHaveCount(0);
    await expect(page.locator(".talent-results > header")).toContainText("经过确认后发布");
    await expect(page.getByRole("link", { name: "机会" }).first()).toHaveAttribute("href", "/opportunities");
    expect(errors).toEqual([]);
    await context.close();
  });

  test("交互数据损坏时播报降级状态并保留全部服务端结果", async ({ page }) => {
    const dataset = await readTalentDataset(page);
    const expectedCount = dataset.hiring.length + dataset.seeking.length;
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
    await expect(page.locator("[data-talent-card]:visible")).toHaveCount(expectedCount);
    await expect(page.getByRole("link", { name: "机会" }).first()).toHaveAttribute("href", "/opportunities");
  });

  test("交互数据节点缺失时保留全部服务端结果", async ({ page }) => {
    const dataset = await readTalentDataset(page);
    const expectedCount = dataset.hiring.length + dataset.seeking.length;
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
    await expect(page.locator("[data-talent-card]:visible")).toHaveCount(expectedCount);
    await expect(page.getByRole("link", { name: "机会" }).first()).toHaveAttribute("href", "/opportunities");
  });

  test("交互数据形状错误时保留全部服务端结果", async ({ page }) => {
    const dataset = await readTalentDataset(page);
    const expectedCount = dataset.hiring.length + dataset.seeking.length;
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
    await expect(page.locator("[data-talent-card]:visible")).toHaveCount(expectedCount);
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
    const dataset = await expectFormalSeedDataset(page);
    await expect(page.locator('[data-talent-card][data-talent-mode="hiring"]:visible')).toHaveCount(
      dataset.hiring.length,
    );
    const toggle = page.locator("#opportunity-theme-toggle");
    await toggle.click();
    await expect(page.locator("#talent-app")).toHaveAttribute("data-theme", "light");
    await expect(toggle).toHaveAttribute("aria-label", "切换深色沉浸模式");
    expect(await page.evaluate(() => window.__talentThemeEvents)).toEqual(["light"]);
    expect(errors).toEqual([]);
    await context.close();
  });

  test("默认展示招聘机会并根据自然语言生成可解释匹配", async ({ page }) => {
    const dataset = await readTalentDataset(page);
    await expect(page.getByRole("button", { name: "招聘机会" })).toHaveAttribute("aria-pressed", "true");
    await expect(page.locator('[data-talent-card][data-talent-mode="hiring"]:visible')).toHaveCount(
      dataset.hiring.length,
    );
    await expect(page.getByText("等待输入条件").first()).toBeVisible();

    await page.getByLabel("描述招聘或求职需求").fill("北京找切配，工资7000以上，包吃住，月休4天");
    await page.getByRole("button", { name: "生成匹配" }).click();

    await expect(page.locator("#talent-condition-chips button")).toHaveCount(5);
    await expect(page.locator("#talent-status")).toContainText("条匹配结果");
    await expect(page.locator("[data-talent-match-score]:visible").first()).not.toHaveText("等待输入条件");

    await page.getByRole("button", { name: "求职人才" }).click();
    expect(
      await page.locator('[data-talent-card][data-talent-mode="seeking"]:visible').count(),
    ).toBeLessThanOrEqual(dataset.seeking.length);
    await expect(page.locator("#talent-condition-chips")).toContainText("北京");
    await expect(page.locator("#talent-condition-chips")).toContainText("切配");
    await expect(page.locator("#talent-condition-chips button")).toHaveCount(3);
    await expect(page.locator("#talent-status")).toContainText(/匹配结果|没有匹配结果/);
  });

  test("数据集提供扩展地区或岗位时可直接生成匹配", async ({ page }) => {
    const dataset = await readTalentDataset(page);
    const oldLocations = new Set(["北京", "天津", "河北", "广东", "江苏", "浙江", "四川", "安徽"]);
    const oldRoles = new Set(["炒锅", "切配", "打荷", "洗碗", "面点", "烧烤", "服务员", "店长"]);
    const candidates = [...dataset.hiring, ...dataset.seeking].map((record) => ({
      record,
      location: record.locationTerms
        .filter((term) => !oldLocations.has(term))
        .sort((left, right) => right.length - left.length)[0],
      role: [...record.roles, record.title]
        .filter((term) => !oldRoles.has(term))
        .sort((left, right) => right.length - left.length)[0],
    }));
    const candidate = candidates.find(({ location, role }) => location || role);
    if (!candidate) {
      test.skip(true, "当前数据集没有旧默认词表之外的地区或岗位");
      return;
    }

    if (candidate.record.mode === "seeking")
      await page.getByRole("button", { name: "求职人才" }).click();
    const terms = [candidate.location, candidate.role].filter(
      (term): term is string => Boolean(term),
    );
    await page.getByLabel("描述招聘或求职需求").fill(terms.join("找"));
    await page.getByRole("button", { name: "生成匹配" }).click();

    for (const term of terms) await expect(page.locator("#talent-condition-chips")).toContainText(term);
    expect(
      await page.locator("[data-talent-card]:visible").evaluateAll(
        (cards, id) => cards.some((card) => card.getAttribute("data-talent-id") === id),
        candidate.record.id,
      ),
    ).toBe(true);
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
    const dataset = await readTalentDataset(page);
    const knownRegions = ["北京", "天津", "河北", "广东", "江苏", "浙江", "四川", "安徽"];
    const knownRoles = ["炒锅", "切配", "打荷", "洗碗", "面点", "烧烤", "服务员", "店长"];
    const target = dataset.seeking.find(
      (record) =>
        Number.isFinite(record.salaryMin) &&
        knownRegions.some((region) => record.locationTerms.includes(region)) &&
        knownRoles.some(
          (role) => record.title.includes(role) || record.skills.some((skill) => skill.includes(role)),
        ),
    );
    expect(target).toBeTruthy();
    const region = knownRegions.find((value) => target!.locationTerms.includes(value))!;
    const role = knownRoles.find(
      (value) => target!.title.includes(value) || target!.skills.some((skill) => skill.includes(value)),
    )!;
    await page
      .getByLabel("描述招聘或求职需求")
      .fill(`${region}找${role}，工资${target!.salaryMin}以上，3年经验，包吃住，月休4天，随时上岗，急招`);
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
    await expect(chips).toContainText(region);
    await expect(chips).toContainText(role);
    await expect(chips).toContainText(String(target!.salaryMin));
    await expect(chips).not.toContainText("3年经验");
    await expect(chips).not.toContainText("包吃住");
    expect(await visibleCardCount(page)).toBeGreaterThan(0);
    await expect(page.locator("[data-talent-match-score]:visible").first()).toContainText("匹配度");
    await page.locator("[data-open-talent-detail]:visible").first().click();
    const switchedSelection = await page.evaluate(() => window.__talentSelection);
    expect(switchedSelection.record.mode).toBe("seeking");
    expect(dataset.seeking.map((record) => record.id)).toContain(switchedSelection.record.id);
    expect(switchedSelection.match.score).toEqual(expect.any(Number));
    await page.getByRole("button", { name: "关闭人才详情" }).click();

    await page.getByRole("button", { name: "撤销条件更改" }).click();
    await expect(page.getByRole("button", { name: "招聘机会" })).toHaveAttribute("aria-pressed", "true");
    await expect(chips.locator("button")).toHaveCount(8);
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
      const base = {
        mode: "hiring",
        title: "切配岗位（原型）",
        scene: "门店（原型）",
        region: "北京",
        district: "朝阳区",
        locationTerms: ["北京", "朝阳区"],
        salaryMin: 7000,
        salaryMax: 9000,
        roles: ["切配"],
        skills: ["切配", "川湘菜"],
        experienceYears: 4,
        hoursPerDay: 8,
        restDays: 4,
        lodging: "提供（原型）",
        housingAllowance: false,
        availability: "随时到岗（原型）",
        urgency: "较高（原型）",
        sourceLabel: "平台演示数据",
        updatedLabel: "近期更新（原型）",
        riskFlags: ["岗位细节待确认（原型）"],
        prototype: true,
      };
      const conditions = matching.parseTalentCommand(
        "北京找切配，工资7000以上，川湘菜，3年经验，包吃住，月休4天，随时上岗，急招",
        "hiring",
        [base],
      );
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
        locationTerms: ["天津"],
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
      const unknownScore = matching.scoreTalentRecord(
        {
          ...exact,
          id: "unknown-values",
          salaryMin: null,
          salaryMax: null,
          experienceYears: null,
          restDays: null,
          lodging: null,
          availability: null,
          urgency: null,
          credibility: null,
        },
        conditions,
      );
      const provinceCityScore = matching.scoreTalentRecord(
        {
          ...exact,
          region: "河北 石家庄",
          district: "长安区",
          locationTerms: ["河北", "石家庄", "长安区"],
        },
        [{ key: "region", value: "河北", label: "河北" }],
      );
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
        unknownScore,
        provinceCityScore,
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
    expect(result.unknownScore.score).toBe(100);
    expect(result.unknownScore.misses).toEqual([]);
    expect(result.provinceCityScore.score).toBe(100);
    expect(result.provinceCityScore.reasons).toContain("地区符合：河北");
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
    await page.evaluate(() => {
      window.__talentDrawerSelection = null;
      window.addEventListener("talent:selection", (event) => {
        window.__talentDrawerSelection = event.detail;
      });
    });
    const trigger = page.locator("[data-open-talent-detail]:visible").first();
    await trigger.click();
    const selected = (await page.evaluate(() => window.__talentDrawerSelection)).record as TalentRecord;

    const drawer = page.locator("#talent-detail-drawer");
    const dialog = drawer.locator("[data-talent-dialog]");
    await expect(drawer).toHaveAttribute("data-open", "true");
    await expect(dialog).toHaveAttribute("role", "dialog");
    await expect(dialog).toHaveAttribute("aria-modal", "true");
    await expect(dialog).toContainText(selected.title);
    await expect(dialog).toContainText(selected.scene || "待确认");
    await expect(dialog).toContainText(
      [selected.region, selected.district].filter(Boolean).join(" ") || "待确认",
    );
    await expect(dialog).toContainText(salaryLabel(selected));
    await expect(dialog).toContainText(selected.skills.join("、") || "待确认");
    await expect(dialog.getByRole("heading", { name: "命中条件", exact: true })).toBeVisible();
    await expect(dialog.getByRole("heading", { name: "未命中条件" })).toBeVisible();
    await expect(dialog.getByRole("heading", { name: "风险与缺失字段" })).toBeVisible();
    for (const risk of selected.riskFlags) await expect(dialog).toContainText(risk);
    await expect(dialog).toContainText(`来源说明：${selected.sourceLabel || "待补充"}`);
    await expect(dialog).toContainText(`更新时间：${selected.updatedLabel}`);
    await expect(dialog).toContainText(`联系方式：${selected.contactMasked || "暂未提供"}`);
    await expect(dialog).toContainText("等待输入条件");
    await expect(dialog).toContainText("未生成 AI 结论");
    await expect(page.locator("body")).toHaveClass(/talent-detail-scroll-locked/);

    for (let index = 0; index < 6; index += 1) {
      await page.keyboard.press("Tab");
      expect(await dialog.evaluate((node) => node.contains(document.activeElement))).toBe(true);
    }
    await page.keyboard.press("Shift+Tab");
    expect(await dialog.evaluate((node) => node.contains(document.activeElement))).toBe(true);

    await dialog.getByRole("heading", { name: selected.title }).click();
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
