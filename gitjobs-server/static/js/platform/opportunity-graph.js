const TYPE_COLORS = {
  university: "#7357ff",
  enterprise: "#2f8cff",
  hospital: "#1ab887",
  park: "#f0b44d",
  stall: "#ff7a3d",
};

const reducedMotionQuery = window.matchMedia("(prefers-reduced-motion: reduce)");

export const shouldUseFallback = () =>
  window.innerWidth < 768 ||
  reducedMotionQuery.matches ||
  navigator.connection?.saveData === true ||
  (typeof navigator.deviceMemory === "number" && navigator.deviceMemory <= 4) ||
  !window.echarts;

const POSITIONS = [
  [16, 64],
  [36, 30],
  [53, 70],
  [69, 38],
  [84, 67],
  [28, 82],
];

const chartNode = (node, index) => ({
  id: node.id,
  name: node.title,
  value: [...POSITIONS[index % POSITIONS.length], node.headcount],
  symbolSize: Math.max(28, Math.min(62, 24 + node.headcount / 500)),
  itemStyle: {
    color: TYPE_COLORS[node.kind],
    borderColor: node.credibility >= 85 ? "#1ab887" : "rgba(255,255,255,.55)",
    borderWidth: Math.max(2, Math.round(node.credibility / 20)),
    shadowColor: node.daysLeft <= 5 ? "#ff7a3d" : TYPE_COLORS[node.kind],
    shadowBlur: node.daysLeft <= 5 ? 28 : 12,
  },
  label: { show: true, color: "#f7f3ea", formatter: node.city },
});

export const initializeOpportunityGraph = (initialNodes) => {
  const app = document.getElementById("opportunity-app");
  const host = document.getElementById("opportunity-graph");
  const status = document.getElementById("opportunity-status");
  const cards = [...document.querySelectorAll("[data-opportunity-card]")];
  if (!app || !host || !status) return;
  let chart;
  let currentNodes = initialNodes;

  const render = (nodes) => {
    currentNodes = nodes;
    const visibleIds = new Set(nodes.map(({ id }) => id));
    cards.forEach((card) => {
      card.hidden = !visibleIds.has(card.dataset.opportunityId);
    });
    status.textContent =
      nodes.length === 0
        ? "没有符合条件的原型机会，请减少一个条件后重试"
        : `当前展示 ${nodes.length} 条原型机会`;
    const fallback = shouldUseFallback();
    app.dataset.experience = fallback ? "fallback" : "constellation";
    if (fallback) {
      chart?.dispose();
      chart = undefined;
      return;
    }
    chart ||= window.echarts.init(host, null, { renderer: "canvas" });
    const data = nodes.map(chartNode);
    chart.setOption(
      {
        animation: true,
        backgroundColor: "transparent",
        tooltip: {
          trigger: "item",
          renderMode: "richText",
          formatter: ({ data: item }) => item.name,
        },
        xAxis: { min: 0, max: 100, show: false },
        yAxis: { min: 0, max: 100, show: false },
        series: [
          {
            type: "effectScatter",
            coordinateSystem: "cartesian2d",
            rippleEffect: { period: 1.8, scale: 2.6 },
            data: data.filter((_, index) => nodes[index].daysLeft <= 5),
          },
          {
            type: "effectScatter",
            coordinateSystem: "cartesian2d",
            rippleEffect: { period: 4.2, scale: 1.8 },
            data: data.filter((_, index) => nodes[index].daysLeft > 5),
          },
        ],
      },
      true,
    );
  };

  render(initialNodes);
  window.addEventListener("opportunity:filters-changed", ({ detail }) => render(detail.nodes));
  reducedMotionQuery.addEventListener("change", () => render(currentNodes));
  window.addEventListener("opportunity:theme", () => chart?.resize());
  window.addEventListener("resize", () => {
    render(currentNodes);
    chart?.resize();
  });
};
