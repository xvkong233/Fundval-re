export type PositionHistoryRow = {
  date?: string;
  value?: number | string | null;
  cost?: number | string | null;
};

export function buildPositionHistoryChartOption(
  rows: PositionHistoryRow[],
  opts?: { compact?: boolean; colorValue?: string; colorCost?: string }
) {
  const compact = opts?.compact ?? false;
  const colorValue = opts?.colorValue ?? "#2563EB";
  const colorCost = opts?.colorCost ?? "#ff4d4f";

  const cleaned = (Array.isArray(rows) ? rows : [])
    .filter((r) => typeof r?.date === "string" && r.date)
    .map((r) => {
      const vRaw = (r as any).value;
      const cRaw = (r as any).cost;
      const value =
        typeof vRaw === "number" ? vRaw : typeof vRaw === "string" ? Number.parseFloat(vRaw) : NaN;
      const cost =
        typeof cRaw === "number" ? cRaw : typeof cRaw === "string" ? Number.parseFloat(cRaw) : NaN;
      return { date: r.date as string, value, cost };
    })
    .filter((r) => Number.isFinite(r.value) && Number.isFinite(r.cost));

  const dates = cleaned.map((r) => r.date);
  const values = cleaned.map((r) => r.value);
  const costs = cleaned.map((r) => r.cost);

  return {
    tooltip: { trigger: "axis", axisPointer: { type: "cross" } },
    xAxis: {
      type: "category",
      data: dates,
      axisLabel: { rotate: compact ? 45 : 0 },
    },
    yAxis: { type: "value", scale: true },
    grid: { left: "3%", right: "4%", bottom: compact ? "12%" : "10%", containLabel: true },
    legend: { data: ["账户市值", "持仓成本"] },
    series: [
      {
        name: "账户市值",
        type: "line",
        data: values,
        smooth: true,
        showSymbol: false,
        lineStyle: { color: colorValue },
        itemStyle: { color: colorValue },
      },
      {
        name: "持仓成本",
        type: "line",
        data: costs,
        smooth: true,
        showSymbol: false,
        lineStyle: { color: colorCost, type: "dashed" },
        itemStyle: { color: colorCost },
      },
    ],
  };
}

