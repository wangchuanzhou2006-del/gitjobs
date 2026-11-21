import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const sourceUrl = new URL(
  "../../gitjobs-server/static/js/platform/talent-matching.js",
  import.meta.url,
);
const source = await readFile(sourceUrl, "utf8");
const matching = await import(
  `data:text/javascript;base64,${Buffer.from(source).toString("base64")}`
);

const publishedRecord = {
  id: "published-outside-old-vocabulary",
  mode: "hiring",
  title: "宴会厨师",
  region: "河北 石家庄",
  district: "长安区",
  locationTerms: ["河北", "石家庄", "石家庄市", "长安区"],
  roles: ["厨师", "高级厨师"],
  skills: ["厨师", "高级厨师", "宴会出品", "前厅[主管]"],
  salaryMax: 9_000,
  riskFlags: [],
  credibility: null,
};

test("published dataset vocabulary matches arbitrary literal locations and roles", () => {
  const conditions = matching.parseTalentCommand(
    "石家庄市找高级厨师，要求前厅[主管]，工资8000以上",
    "hiring",
    [publishedRecord],
  );

  assert.deepEqual(
    conditions.map(({ key, value }) => [key, value]),
    [
      ["region", "石家庄市"],
      ["role", "高级厨师"],
      ["skills", ["前厅[主管]"]],
      ["salaryMin", 8_000],
    ],
  );
  assert.equal(matching.filterAndRankTalent([publishedRecord], conditions).length, 1);
});

test("prototype commands remain available through their delivered fixture vocabulary", () => {
  const prototypeRecord = {
    ...publishedRecord,
    region: "北京",
    district: "朝阳区",
    locationTerms: ["北京", "朝阳区"],
    title: "切配岗位（原型）",
    roles: ["切配"],
    skills: ["切配", "川湘菜"],
  };

  const conditions = matching.parseTalentCommand(
    "北京找切配和川湘菜，工资7000以上",
    "hiring",
    [prototypeRecord],
  );

  assert.deepEqual(
    conditions.map(({ key, value }) => [key, value]),
    [
      ["region", "北京"],
      ["role", "切配"],
      ["skills", ["川湘菜"]],
      ["salaryMin", 7_000],
    ],
  );
});
