export const initializeExplorationRail = ({
  appId = "opportunity-app",
  storageKey = "opportunity-theme",
  eventName = "opportunity:theme",
} = {}) => {
  const app = document.getElementById(appId);
  const toggle = document.getElementById("opportunity-theme-toggle");
  if (!app || !toggle) return;

  const stored = localStorage.getItem(storageKey);
  const initial = stored === "dark" || stored === "light" ? stored : "light";
  app.dataset.theme = initial;

  const syncLabel = () =>
    toggle.setAttribute("aria-label", app.dataset.theme === "dark" ? "切换浅色专业模式" : "切换深色沉浸模式");
  syncLabel();

  toggle.addEventListener("click", () => {
    app.dataset.theme = app.dataset.theme === "dark" ? "light" : "dark";
    localStorage.setItem(storageKey, app.dataset.theme);
    syncLabel();
    window.dispatchEvent(new CustomEvent(eventName, { detail: app.dataset.theme }));
  });
};
