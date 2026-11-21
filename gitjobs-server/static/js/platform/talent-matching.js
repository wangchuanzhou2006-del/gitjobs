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

const locationTerms = (record) => {
  const terms = [
    ...(Array.isArray(record.locationTerms) ? record.locationTerms : []),
    record.region,
    record.district,
  ];
  return [...new Set(terms.filter((term) => typeof term === "string" && term.trim()))];
};

const literalTerms = (values) =>
  [...new Set(values.filter((value) => typeof value === "string" && value.trim()))].sort(
    (left, right) => right.length - left.length || (left < right ? -1 : left > right ? 1 : 0),
  );

const longestLiteralMatches = (input, values) => {
  const matches = [];
  for (const value of literalTerms(values)) {
    if (!input.includes(value)) continue;
    if (matches.some((longer) => longer.includes(value))) continue;
    matches.push(value);
  }
  return matches;
};

const searchableVocabulary = (records) => {
  const locations = [];
  const roles = [];
  const skills = [];
  for (const record of records) {
    locations.push(...locationTerms(record));
    if (typeof record.title === "string" && record.title.trim()) roles.push(record.title);
    if (Array.isArray(record.roles)) roles.push(...record.roles);
    if (Array.isArray(record.skills)) {
      skills.push(...record.skills);
      if (typeof record.title === "string")
        roles.push(...record.skills.filter((skill) => record.title.includes(skill)));
    }
  }
  const roleTerms = literalTerms(roles);
  const roleSet = new Set(roleTerms);
  return {
    locations: literalTerms(locations),
    roles: roleTerms,
    skills: literalTerms(skills.filter((skill) => !roleSet.has(skill))),
  };
};

const matchesLocation = (record, value) =>
  locationTerms(record).some((term) => term === value || term.includes(value));

const includesRole = (record, role) =>
  (typeof record.title === "string" && record.title.includes(role)) ||
  (Array.isArray(record.skills) && record.skills.some((skill) => skill.includes(role)));

const hasRoleMeaning = (record) =>
  (typeof record.title === "string" && record.title.trim()) ||
  (Array.isArray(record.skills) && record.skills.length);

const evaluate = (record, condition) => {
  const value = condition.value;
  if (condition.key === "region") {
    if (!locationTerms(record).length) return null;
    return [
      matchesLocation(record, value),
      "地区符合：" + value,
      "地区不符：需要" + value + "，当前" + locationTerms(record).join("、"),
    ];
  }
  if (condition.key === "role") {
    if (
      !(typeof record.title === "string" && record.title.trim()) &&
      !(Array.isArray(record.skills) && record.skills.length)
    )
      return null;
    return [includesRole(record, value), "岗位符合：" + value, "岗位不符：需要" + value];
  }
  if (condition.key === "skills") {
    if (!Array.isArray(record.skills) || !record.skills.length) return null;
    const missing = value.filter((wanted) => !record.skills.some((skill) => skill.includes(wanted)));
    return [missing.length === 0, "技能符合：" + value.join("、"), "技能未满足：" + missing.join("、")];
  }
  if (condition.key === "salaryMin") {
    if (!Number.isFinite(record.salaryMax)) return null;
    return [
      record.salaryMax >= value,
      "薪资范围达到" + value + "元以上",
      "薪资未达到：需要" + value + "元以上，当前最高" + record.salaryMax + "元",
    ];
  }
  if (condition.key === "experienceYears") {
    if (!Number.isFinite(record.experienceYears)) return null;
    return [
      record.experienceYears >= value,
      "经验符合：" + record.experienceYears + "年",
      "经验未满足：需要" + value + "年，当前" + record.experienceYears + "年",
    ];
  }
  if (condition.key === "restDays") {
    if (!Number.isFinite(record.restDays)) return null;
    return [
      record.restDays >= value,
      "月休符合：" + record.restDays + "天",
      "月休未满足：需要" + value + "天，当前" + record.restDays + "天",
    ];
  }
  if (condition.key === "lodging") {
    if (typeof record.lodging !== "string" || !record.lodging.trim()) return null;
    const hit = !record.lodging.includes("不提供") && /(提供|需要)/.test(record.lodging);
    return [hit, "住宿偏好符合：可包吃住", "住宿偏好未满足：需要包吃住"];
  }
  if (condition.key === "availability") {
    if (typeof record.availability !== "string" || !record.availability.trim()) return null;
    return [
      record.availability.includes("随时"),
      "到岗时间符合：可随时到岗",
      "到岗时间未满足：当前" + record.availability,
    ];
  }
  if (condition.key === "urgency") {
    if (typeof record.urgency !== "string" || !record.urgency.trim()) return null;
    return [/(较高|急)/.test(record.urgency), "紧急度符合：急招急聘", "紧急度未满足：当前" + record.urgency];
  }
  return null;
};

export const parseTalentCommand = (input, mode, records = []) => {
  if (!["hiring", "seeking"].includes(mode)) return [];
  const conditions = [];
  const vocabulary = searchableVocabulary(records);
  const region = longestLiteralMatches(input, vocabulary.locations)[0];
  const role = longestLiteralMatches(input, vocabulary.roles)[0];
  const skills = longestLiteralMatches(input, vocabulary.skills);
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
  const risks = Array.isArray(record.riskFlags) ? record.riskFlags : [];
  if (conditions.length === 0) return { score: null, reasons: [], misses: [], risks };
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
    risks,
  };
};

const hasHardConflict = (record, conditions) =>
  conditions.some(
    (condition) =>
      (condition.key === "region" &&
        locationTerms(record).length > 0 &&
        !matchesLocation(record, condition.value)) ||
      (condition.key === "role" && hasRoleMeaning(record) && !includesRole(record, condition.value)) ||
      (condition.key === "salaryMin" &&
        Number.isFinite(record.salaryMax) &&
        record.salaryMax < condition.value),
  );

export const filterAndRankTalent = (records, conditions) =>
  records
    .map((record, index) => ({ record, match: scoreTalentRecord(record, conditions), index }))
    .filter(({ record }) => !hasHardConflict(record, conditions))
    .sort(
      (left, right) =>
        (right.match.score ?? -1) - (left.match.score ?? -1) ||
        (right.record.credibility ?? -1) - (left.record.credibility ?? -1) ||
        left.index - right.index,
    )
    .map(({ record, match }) => ({ record, match }));
