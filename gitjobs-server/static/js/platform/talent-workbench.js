import { filterAndRankTalent, parseTalentCommand } from "/static/js/platform/talent-matching.js";
const COMMON_KEYS = new Set(["region", "role", "salaryMin", "skills"]);

export const initializeTalentWorkbench = (data) => {
  const form = document.getElementById("talent-command-form");
  const input = document.getElementById("talent-command-input");
  const chips = document.getElementById("talent-condition-chips");
  const undo = document.getElementById("talent-undo");
  const status = document.getElementById("talent-status");
  const host = document.querySelector(".talent-card-grid");
  const modeButtons = [...document.querySelectorAll("[data-talent-mode-button]")];
  const cards = [...document.querySelectorAll("[data-talent-card]")];
  if (!form || !input || !chips || !undo || !status || !host) return null;

  const records = [...data.hiring, ...data.seeking];
  const recordById = new Map(records.map((record) => [record.id, record]));
  const cardById = new Map(cards.map((card) => [card.dataset.talentId, card]));
  const state = { mode: "hiring", conditions: [], history: [], selectedId: null };
  let matches = new Map();
  const cloneConditions = () => state.conditions.map((item) => ({ ...item }));
  const remember = () => state.history.push({ mode: state.mode, conditions: cloneConditions() });

  const render = () => {
    for (const button of modeButtons)
      button.setAttribute("aria-pressed", String(button.dataset.talentModeButton === state.mode));
    chips.replaceChildren(
      ...state.conditions.map((condition, index) => {
        const button = document.createElement("button");
        button.type = "button";
        button.textContent = condition.label + " ×";
        button.setAttribute("aria-label", "移除条件：" + condition.label);
        button.addEventListener("click", () => {
          remember();
          state.conditions = state.conditions.filter((_, itemIndex) => itemIndex !== index);
          render();
        });
        return button;
      }),
    );
    undo.disabled = state.history.length === 0;

    const ranked = filterAndRankTalent(data[state.mode], state.conditions);
    matches = new Map(ranked.map(({ record, match }) => [record.id, match]));
    const visible = new Set(matches.keys());
    for (const { record, match } of ranked) {
      const card = cardById.get(record.id);
      host.append(card);
      card.querySelector("[data-talent-match-score]").textContent =
        match.score === null ? "等待输入条件" : "匹配度 " + match.score + "%";
      const reasons = card.querySelector("[data-talent-match-reasons]");
      reasons.replaceChildren(
        ...match.reasons.slice(0, 2).map((text) => {
          const item = document.createElement("li");
          item.textContent = text;
          return item;
        }),
      );
      const risks = card.querySelector("[data-talent-risks]");
      risks.replaceChildren(
        ...match.risks.map((text) => {
          const item = document.createElement("li");
          item.textContent = text;
          return item;
        }),
      );
    }
    for (const card of cards.filter((card) => card.dataset.talentMode !== state.mode)) host.append(card);
    for (const card of cards)
      card.hidden = card.dataset.talentMode !== state.mode || !visible.has(card.dataset.talentId);
    if (!state.conditions.length)
      status.textContent =
        "当前展示 " + ranked.length + " 条" + (state.mode === "hiring" ? "招聘机会" : "求职人才");
    else status.textContent = ranked.length ? ranked.length + " 条匹配结果" : "没有匹配结果";
  };

  form.addEventListener("submit", (event) => {
    event.preventDefault();
    const next = parseTalentCommand(input.value, state.mode);
    if (!next.length) {
      status.textContent = "未识别新条件，可尝试“北京找切配，工资7000以上”";
      return;
    }
    remember();
    state.conditions = next;
    render();
  });
  undo.addEventListener("click", () => {
    if (!state.history.length) return;
    const previous = state.history.pop();
    state.mode = previous.mode;
    state.conditions = previous.conditions;
    render();
  });
  for (const button of modeButtons)
    button.addEventListener("click", () => {
      if (button.dataset.talentModeButton === state.mode) return;
      remember();
      state.mode = button.dataset.talentModeButton;
      state.conditions = state.conditions.filter((condition) => COMMON_KEYS.has(condition.key));
      render();
    });
  for (const button of document.querySelectorAll("[data-open-talent-detail]"))
    button.addEventListener("click", () => {
      const record = recordById.get(button.dataset.talentId);
      if (!record) return;
      state.selectedId = record.id;
      const match = matches.get(record.id) || filterAndRankTalent([record], state.conditions)[0]?.match;
      window.dispatchEvent(new CustomEvent("talent:selection", { detail: { record, match } }));
    });
  render();
  return state;
};
