export function initializeTalentImportForm(root = document) {
  const form = root.getElementById("talent-import-form");
  const input = root.getElementById("talent-raw-text");
  const count = root.getElementById("talent-character-count");
  if (!form || !input || !count || form.dataset.initialized === "true") return;

  form.dataset.initialized = "true";
  const updateCount = () => {
    count.textContent = `${Array.from(input.value).length.toLocaleString("zh-CN")} 字`;
  };
  input.addEventListener("input", updateCount);
  updateCount();
}
