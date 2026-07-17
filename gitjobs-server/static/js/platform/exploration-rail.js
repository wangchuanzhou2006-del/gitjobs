export const initializeExplorationRail = ({
  appId = "opportunity-app",
  storageKey = "opportunity-theme",
  eventName = "opportunity:theme",
} = {}) => {
  const app = document.getElementById(appId);
  const toggle = document.getElementById("opportunity-theme-toggle");
  if (!app || !toggle) return;

  let stored;
  try {
    stored = localStorage.getItem(storageKey);
  } catch {
    stored = null;
  }
  const preferred =
    typeof window.matchMedia === "function" && window.matchMedia("(prefers-color-scheme: dark)").matches
      ? "dark"
      : "light";
  app.dataset.theme = stored === "dark" || stored === "light" ? stored : preferred;

  const syncLabel = () =>
    toggle.setAttribute("aria-label", app.dataset.theme === "dark" ? "切换浅色专业模式" : "切换深色沉浸模式");
  syncLabel();

  toggle.addEventListener("click", () => {
    app.dataset.theme = app.dataset.theme === "dark" ? "light" : "dark";
    try {
      localStorage.setItem(storageKey, app.dataset.theme);
    } catch {
      // Storage can be unavailable in privacy-restricted contexts.
    }
    syncLabel();
    window.dispatchEvent(new CustomEvent(eventName, { detail: app.dataset.theme }));
  });
};
