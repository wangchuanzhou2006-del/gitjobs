const REGION_PATTERN = /(广东|北京|江苏|浙江|四川|海南|广西|安徽|天津)/;
const KIND_PATTERNS = [
  ["university", /高校|大学/],
  ["enterprise", /企业/],
  ["hospital", /医院/],
  ["park", /园区/],
  ["stall", /档口/],
];

export const parseOpportunityCommand = (input) => {
  const conditions = [];
  const region = input.match(REGION_PATTERN)?.[1];
  if (region) conditions.push({ key: "region", label: region, value: region });
  const kind = KIND_PATTERNS.find(([, pattern]) => pattern.test(input));
  if (kind) {
    conditions.push({
      key: "kind",
      label: input.match(kind[1])?.[0] || kind[0],
      value: kind[0],
    });
  }
  const headcount = input.match(/(\d+(?:\.\d+)?)\s*万\s*人(?:以上)?/);
  if (headcount) {
    conditions.push({
      key: "headcountMin",
      label: `${headcount[1]}万人以上`,
      value: Number(headcount[1]) * 10000,
    });
  }
  const capital = input.match(/(\d+(?:\.\d+)?)\s*万\s*资金/);
  if (capital) {
    conditions.push({ key: "capitalWan", label: `资金${capital[1]}万`, value: Number(capital[1]) });
  }
  const experience = input.match(/(\d+)\s*年\s*经验/);
  if (experience) {
    conditions.push({
      key: "experienceYears",
      label: `${experience[1]}年经验`,
      value: Number(experience[1]),
    });
  }
  return conditions;
};

export const filterOpportunities = (nodes, conditions) =>
  nodes.filter((node) =>
    conditions.every((condition) => {
      if (condition.key === "region") return node.region === condition.value;
      if (condition.key === "kind") return node.kind === condition.value;
      if (condition.key === "headcountMin") return node.headcount >= condition.value;
      return true;
    }),
  );

export const initializeAICommandBar = (nodes) => {
  const form = document.getElementById("opportunity-command-form");
  const input = document.getElementById("ai-command-input");
  const chipHost = document.getElementById("opportunity-condition-chips");
  const undo = document.getElementById("opportunity-undo");
  const status = document.getElementById("opportunity-status");
  if (!form || !input || !chipHost || !undo) return;
  let conditions = [];
  let previous = [];
  let hasPrevious = false;

  const publish = () => {
    const filtered = filterOpportunities(nodes, conditions);
    chipHost.replaceChildren(
      ...conditions.map((condition) => {
        const button = document.createElement("button");
        button.type = "button";
        button.className = "opportunity-chip opportunity-focus";
        button.textContent = `${condition.label} ×`;
        button.setAttribute("aria-label", `移除条件：${condition.label}`);
        button.addEventListener("click", () => {
          previous = [...conditions];
          hasPrevious = true;
          conditions = conditions.filter((item) => item !== condition);
          publish();
        });
        return button;
      }),
    );
    undo.disabled = !hasPrevious;
    const visibleIds = new Set(filtered.map((node) => node.id));
    document.querySelectorAll("[data-opportunity-card]").forEach((card) => {
      card.hidden = !visibleIds.has(card.dataset.opportunityId);
    });
    if (status) {
      status.textContent = filtered.length
        ? `当前展示 ${filtered.length} 条原型机会`
        : "没有符合条件的原型机会";
    }
    window.dispatchEvent(
      new CustomEvent("opportunity:filters-changed", { detail: { conditions, nodes: filtered } }),
    );
  };

  form.addEventListener("submit", (event) => {
    event.preventDefault();
    previous = [...conditions];
    hasPrevious = true;
    conditions = parseOpportunityCommand(input.value);
    publish();
  });
  undo.addEventListener("click", () => {
    const current = [...conditions];
    conditions = previous;
    previous = current;
    publish();
  });
  publish();
};
