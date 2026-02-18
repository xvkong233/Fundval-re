export type NavHistoryRow = {
  nav_date?: string;
  unit_nav?: string | number;
};

export function buildNavChartOption(
  rows: NavHistoryRow[],
  opts?: { compact?: boolean; color?: string }
) {
  const compact = opts?.compact ?? false;
  const color = opts?.color ?? "#2563EB";

  const cleaned = (Array.isArray(rows) ? rows : [])
    .filter((r) => typeof r?.nav_date === "string" && r.nav_date)
    .map((r) => {
      const vRaw = (r as any).unit_nav;
      const v =
        typeof vRaw === "number" ? vRaw : typeof vRaw === "string" ? Number.parseFloat(vRaw) : NaN;
      return { date: r.nav_date as string, value: v };
    })
    .filter((r) => Number.isFinite(r.value));

  const dates = cleaned.map((r) => r.date);
  const values = cleaned.map((r) => r.value);

  return {
    tooltip: { trigger: "axis", axisPointer: { type: "cross" } },
    xAxis: {
      type: "category",
      data: dates,
      axisLabel: { rotate: compact ? 45 : 0 },
    },
    yAxis: { type: "value", scale: true },
    grid: { left: "3%", right: "4%", bottom: compact ? "12%" : "10%", containLabel: true },
    series: [
      {
        name: "单位净值",
        type: "line",
        data: values,
        smooth: true,
        showSymbol: false,
        lineStyle: { color },
        itemStyle: { color },
      },
    ],
  };
}

