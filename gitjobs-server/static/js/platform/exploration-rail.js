const THEME_KEY = "opportunity-theme";

export const initializeExplorationRail = () => {
  const app = document.getElementById("opportunity-app");
  const toggle = document.getElementById("opportunity-theme-toggle");
  if (!app || !toggle) return;

  const stored = localStorage.getItem(THEME_KEY);
  const initial = stored === "dark" || stored === "light" ? stored : "light";
  app.dataset.theme = initial;

  const syncLabel = () =>
    toggle.setAttribute("aria-label", app.dataset.theme === "dark" ? "切换浅色专业模式" : "切换深色沉浸模式");
  syncLabel();

  toggle.addEventListener("click", () => {
    app.dataset.theme = app.dataset.theme === "dark" ? "light" : "dark";
    localStorage.setItem(THEME_KEY, app.dataset.theme);
    syncLabel();
    window.dispatchEvent(new CustomEvent("opportunity:theme", { detail: app.dataset.theme }));
  });
};
