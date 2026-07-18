const REGION_PATTERN = /(北京|天津|河北|广东|江苏|浙江|四川|安徽)/;
const ROLE_PATTERN = /(炒锅|切配|打荷|洗碗|面点|烧烤|服务员|店长)/;
const SKILL_PATTERN = /(川湘菜|家常菜|清真|火锅|早餐|面食|刀工)/g;
export const TALENT_DIMENSION_WEIGHTS = Object.freeze({
  role: 20,
  region: 20,
  salaryMin: 20,
  skills: 15,
  experienceYears: 10,
  lodging: 5,
  restDays: 5,
  timing: 5,
});
const CONDITION_DIMENSIONS = {
  role: "role",
  region: "region",
  salaryMin: "salaryMin",
  skills: "skills",
  experienceYears: "experienceYears",
  lodging: "lodging",
  restDays: "restDays",
  availability: "timing",
  urgency: "timing",
};

const includesRole = (record, role) =>
  record.title.includes(role) || record.skills.some((skill) => skill.includes(role));

const evaluate = (record, condition) => {
  const value = condition.value;
  if (condition.key === "region")
    return [
      record.region === value,
      "地区符合：" + value,
      "地区不符：需要" + value + "，当前" + record.region,
    ];
  if (condition.key === "role")
    return [includesRole(record, value), "岗位符合：" + value, "岗位不符：需要" + value];
  if (condition.key === "skills") {
    const missing = value.filter((wanted) => !record.skills.some((skill) => skill.includes(wanted)));
    return [missing.length === 0, "技能符合：" + value.join("、"), "技能未满足：" + missing.join("、")];
  }
  if (condition.key === "salaryMin")
    return [
      record.salaryMax >= value,
      "薪资范围达到" + value + "元以上",
      "薪资未达到：需要" + value + "元以上，当前最高" + record.salaryMax + "元",
    ];
  if (condition.key === "experienceYears")
    return [
      record.experienceYears >= value,
      "经验符合：" + record.experienceYears + "年",
      "经验未满足：需要" + value + "年，当前" + record.experienceYears + "年",
    ];
  if (condition.key === "restDays")
    return [
      record.restDays >= value,
      "月休符合：" + record.restDays + "天",
      "月休未满足：需要" + value + "天，当前" + record.restDays + "天",
    ];
  if (condition.key === "lodging") {
    const hit = !record.lodging.includes("不提供") && /(提供|需要)/.test(record.lodging);
    return [hit, "住宿偏好符合：可包吃住", "住宿偏好未满足：需要包吃住"];
  }
  if (condition.key === "availability")
    return [
      record.availability.includes("随时"),
      "到岗时间符合：可随时到岗",
      "到岗时间未满足：当前" + record.availability,
    ];
  if (condition.key === "urgency")
    return [/(较高|急)/.test(record.urgency), "紧急度符合：急招急聘", "紧急度未满足：当前" + record.urgency];
  return null;
};

export const parseTalentCommand = (input, mode) => {
  if (!["hiring", "seeking"].includes(mode)) return [];
  const conditions = [];
  const region = input.match(REGION_PATTERN)?.[1];
  const role = input.match(ROLE_PATTERN)?.[1];
  const skills = [...new Set(input.match(SKILL_PATTERN) || [])];
  const salary = input.match(/(?:工资|薪资)?\s*(\d{4,5})\s*(?:元|以上|起)?/);
  const experience = input.match(/(\d+)\s*年(?:经验|从业)?/);
  const restDays = input.match(/月休\s*(\d+)\s*天/);
  if (region) conditions.push({ key: "region", label: region, value: region });
  if (role) conditions.push({ key: "role", label: role, value: role });
  if (skills.length) conditions.push({ key: "skills", label: skills.join("、"), value: skills });
  if (salary)
    conditions.push({ key: "salaryMin", label: "工资" + salary[1] + "元以上", value: Number(salary[1]) });
  if (experience)
    conditions.push({
      key: "experienceYears",
      label: experience[1] + "年经验",
      value: Number(experience[1]),
    });
  if (restDays)
    conditions.push({ key: "restDays", label: "月休" + restDays[1] + "天", value: Number(restDays[1]) });
  if (/包吃住|包食宿/.test(input)) conditions.push({ key: "lodging", label: "包吃住", value: true });
  if (/随时上岗|随时到岗/.test(input))
    conditions.push({ key: "availability", label: "随时到岗", value: "immediate" });
  if (/急招|急聘/.test(input)) conditions.push({ key: "urgency", label: "急招", value: "urgent" });
  return conditions;
};

export const scoreTalentRecord = (record, conditions) => {
  if (conditions.length === 0) return { score: null, reasons: [], misses: [], risks: record.riskFlags };
  let activeWeight = 0;
  let hitWeight = 0;
  const reasons = [];
  const misses = [];
  const resultsByDimension = new Map();
  for (const condition of conditions) {
    const result = evaluate(record, condition);
    const dimension = CONDITION_DIMENSIONS[condition.key];
    if (!result || !dimension) continue;
    const dimensionResults = resultsByDimension.get(dimension) || [];
    dimensionResults.push(result);
    resultsByDimension.set(dimension, dimensionResults);
  }
  for (const [dimension, results] of resultsByDimension) {
    const weight = TALENT_DIMENSION_WEIGHTS[dimension];
    activeWeight += weight;
    if (results.every(([hit]) => hit)) hitWeight += weight;
    for (const [hit, reason, miss] of results) {
      if (hit) reasons.push(reason);
      else misses.push(miss);
    }
  }
  return {
    score: activeWeight ? Math.round((hitWeight / activeWeight) * 100) : null,
    reasons,
    misses,
    risks: record.riskFlags,
  };
};

const hasHardConflict = (record, conditions) =>
  conditions.some(
    (condition) =>
      (condition.key === "region" && record.region !== condition.value) ||
      (condition.key === "role" && !includesRole(record, condition.value)) ||
      (condition.key === "salaryMin" && record.salaryMax < condition.value),
  );

export const filterAndRankTalent = (records, conditions) =>
  records
    .map((record, index) => ({ record, match: scoreTalentRecord(record, conditions), index }))
    .filter(({ record }) => !hasHardConflict(record, conditions))
    .sort(
      (left, right) =>
        (right.match.score ?? -1) - (left.match.score ?? -1) ||
        right.record.credibility - left.record.credibility ||
        left.index - right.index,
    )
    .map(({ record, match }) => ({ record, match }));
