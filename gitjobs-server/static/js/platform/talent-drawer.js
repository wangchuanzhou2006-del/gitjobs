const FOCUSABLE_SELECTOR = [
  "button:not([disabled])",
  "a[href]",
  "input:not([disabled])",
  "select:not([disabled])",
  "textarea:not([disabled])",
  '[tabindex]:not([tabindex="-1"])',
].join(",");

const makeListItems = (values, emptyLabel) => {
  const items = values.length ? values : [emptyLabel];
  return items.map((value) => {
    const item = document.createElement("li");
    item.textContent = value;
    return item;
  });
};

const makeFact = (label, value) => {
  const row = document.createElement("div");
  const term = document.createElement("dt");
  const description = document.createElement("dd");
  term.textContent = label;
  description.textContent = value;
  row.replaceChildren(term, description);
  return row;
};

const textOrUnknown = (value) => (typeof value === "string" && value.trim() ? value : "待确认");

const numberOrUnknown = (value, suffix) => (Number.isFinite(value) ? `${value} ${suffix}` : "待确认");

const salaryLabel = (record) => {
  if (Number.isFinite(record.salaryMin) && Number.isFinite(record.salaryMax))
    return `${record.salaryMin}–${record.salaryMax} 元/月`;
  if (Number.isFinite(record.salaryMin)) return `${record.salaryMin} 元/月起`;
  if (Number.isFinite(record.salaryMax)) return `最高 ${record.salaryMax} 元/月`;
  return "待确认";
};

const locationLabel = (record) =>
  [record.region, record.district].filter((value) => typeof value === "string" && value.trim()).join(" ") ||
  "待确认";

export const initializeTalentDrawer = () => {
  const root = document.getElementById("talent-detail-drawer");
  const panel = root?.querySelector("[data-talent-dialog]");
  const closeButton = root?.querySelector("[data-close-talent-detail]");
  const title = root?.querySelector("[data-talent-detail-title]");
  const summary = root?.querySelector("[data-talent-match-summary]");
  const explanation = root?.querySelector("[data-talent-match-explanation]");
  const facts = root?.querySelector("[data-talent-detail-facts]");
  const reasons = root?.querySelector("[data-talent-hit-list]");
  const misses = root?.querySelector("[data-talent-miss-list]");
  const risks = root?.querySelector("[data-talent-risk-list]");
  const source = root?.querySelector("[data-talent-source]");
  const updated = root?.querySelector("[data-talent-updated]");
  const contact = root?.querySelector("[data-talent-contact]");
  if (
    !root ||
    !panel ||
    !closeButton ||
    !title ||
    !summary ||
    !explanation ||
    !facts ||
    !reasons ||
    !misses ||
    !risks ||
    !source ||
    !updated ||
    !contact
  )
    return null;

  let trigger = null;

  const close = () => {
    if (root.hidden) return;
    root.hidden = true;
    root.dataset.open = "false";
    document.body.classList.remove("talent-detail-scroll-locked");
    document.removeEventListener("keydown", handleKeydown);
    const previousTrigger = trigger;
    trigger = null;
    if (previousTrigger?.isConnected) previousTrigger.focus();
  };

  const handleKeydown = (event) => {
    if (event.key === "Escape") {
      event.preventDefault();
      close();
      return;
    }
    if (event.key !== "Tab") return;
    const focusable = [...panel.querySelectorAll(FOCUSABLE_SELECTOR)].filter((element) => !element.hidden);
    if (!focusable.length) {
      event.preventDefault();
      panel.focus();
      return;
    }
    const first = focusable[0];
    const last = focusable.at(-1);
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault();
      first.focus();
    } else if (!panel.contains(document.activeElement)) {
      event.preventDefault();
      first.focus();
    }
  };

  const open = (record, match, opener) => {
    trigger = opener;
    title.textContent = record.title;
    summary.textContent = match.score === null ? "等待输入条件" : `匹配度 ${match.score}%`;
    explanation.textContent =
      match.score === null
        ? "未生成 AI 结论：请输入匹配条件后查看可解释结果。"
        : "已根据当前条件生成匹配结论。";
    facts.replaceChildren(
      makeFact("场景", textOrUnknown(record.scene)),
      makeFact("地区", locationLabel(record)),
      makeFact(record.mode === "hiring" ? "薪资" : "期望薪资", salaryLabel(record)),
      makeFact(
        "技能",
        Array.isArray(record.skills) && record.skills.length ? record.skills.join("、") : "待确认",
      ),
      makeFact("经验", numberOrUnknown(record.experienceYears, "年")),
      makeFact(
        record.mode === "hiring" ? "工时" : "期望工时",
        numberOrUnknown(record.hoursPerDay, "小时/天"),
      ),
      makeFact(record.mode === "hiring" ? "休息" : "期望休息", numberOrUnknown(record.restDays, "天/月")),
      makeFact("住宿", textOrUnknown(record.lodging)),
      makeFact(
        "住房补贴",
        record.housingAllowance === true ? "提供" : record.housingAllowance === false ? "不提供" : "待确认",
      ),
      makeFact("到岗", textOrUnknown(record.availability)),
      makeFact("紧急度", textOrUnknown(record.urgency)),
      makeFact(
        "可信度",
        Number.isFinite(record.credibility)
          ? `${record.credibility} / 100${record.prototype ? "（原型字段）" : ""}`
          : "待确认",
      ),
    );
    reasons.replaceChildren(...makeListItems(match.reasons, "暂无命中条件"));
    misses.replaceChildren(...makeListItems(match.misses, "暂无未命中条件"));
    risks.replaceChildren(...makeListItems(match.risks, "暂无风险与缺失字段"));
    source.textContent = `来源说明：${record.sourceLabel || "待补充"}`;
    updated.textContent = `更新时间：${record.updatedLabel}`;
    contact.textContent = `联系方式：${record.contactMasked || "暂未提供"}`;

    root.hidden = false;
    root.dataset.open = "true";
    document.body.classList.add("talent-detail-scroll-locked");
    document.addEventListener("keydown", handleKeydown);
    closeButton.focus();
  };

  closeButton.addEventListener("click", close);
  root.addEventListener("click", (event) => {
    if (event.target === root) close();
  });

  return { open, close };
};
