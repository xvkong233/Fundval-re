export type NavHistoryRow = {
  nav_date?: string;
  unit_nav?: string | number;
};

export type ForecastPoint = {
  step: number;
  nav: number;
  ci_low?: number;
  ci_high?: number;
  mu?: number;
};

export type ForecastExtrema = { step: number; nav: number };

export type ForecastSwingPoint = { kind: "low" | "high"; step: number; nav: number };

export type ForecastCurve = {
  base_nav?: number;
  points: ForecastPoint[];
  low?: ForecastExtrema;
  high?: ForecastExtrema;
  swing_points?: ForecastSwingPoint[];
};

type SwingPoint = {
  date: string;
  value: number;
  kind: "low" | "high";
  prominence: number;
};

function toFiniteNumber(vRaw: unknown): number {
  const v = typeof vRaw === "number" ? vRaw : typeof vRaw === "string" ? Number.parseFloat(vRaw) : NaN;
  return Number.isFinite(v) ? v : NaN;
}

function computeSwingPoints(points: { date: string; value: number }[], windowSize: number): SwingPoint[] {
  const w = Math.max(1, Math.floor(windowSize));
  if (points.length < w * 2 + 3) return [];

  const out: SwingPoint[] = [];
  for (let i = w; i < points.length - w; i++) {
    const center = points[i];
    let isLow = true;
    let isHigh = true;
    let localMin = Number.POSITIVE_INFINITY;
    let localMax = Number.NEGATIVE_INFINITY;

    for (let j = i - w; j <= i + w; j++) {
      const v = points[j].value;
      if (v < localMin) localMin = v;
      if (v > localMax) localMax = v;
      if (j !== i) {
        if (v >= center.value) isHigh = false;
        if (v <= center.value) isLow = false;
      }
    }

    if (isLow && Number.isFinite(localMax) && localMax > 0) {
      out.push({
        date: center.date,
        value: center.value,
        kind: "low",
        prominence: Math.max(0, (localMax - center.value) / localMax),
      });
    } else if (isHigh && Number.isFinite(localMin) && localMin > 0) {
      out.push({
        date: center.date,
        value: center.value,
        kind: "high",
        prominence: Math.max(0, (center.value - localMin) / center.value),
      });
    }
  }

  return out;
}

function pickTopSwingPoints(points: SwingPoint[], maxPointsPerKind: number): SwingPoint[] {
  const maxN = Math.max(0, Math.floor(maxPointsPerKind));
  if (maxN <= 0) return [];

  const lows = points
    .filter((p) => p.kind === "low")
    .sort((a, b) => b.prominence - a.prominence)
    .slice(0, maxN);
  const highs = points
    .filter((p) => p.kind === "high")
    .sort((a, b) => b.prominence - a.prominence)
    .slice(0, maxN);

  const merged = [...lows, ...highs].sort((a, b) => a.date.localeCompare(b.date));
  const uniq = new Map<string, SwingPoint>();
  for (const p of merged) uniq.set(`${p.kind}:${p.date}`, p);
  return Array.from(uniq.values());
}

function parseIsoDateUTC(sRaw: string): Date | null {
  const s = (sRaw || "").trim();
  if (!s || s.length < 10) return null;
  const y = Number.parseInt(s.slice(0, 4), 10);
  const m = Number.parseInt(s.slice(5, 7), 10);
  const d = Number.parseInt(s.slice(8, 10), 10);
  if (!Number.isFinite(y) || !Number.isFinite(m) || !Number.isFinite(d)) return null;
  if (m < 1 || m > 12 || d < 1 || d > 31) return null;
  return new Date(Date.UTC(y, m - 1, d, 0, 0, 0));
}

function formatIsoDateUTC(dt: Date): string {
  return dt.toISOString().slice(0, 10);
}

function nextTradingDates(lastDate: string, n: number): string[] {
  const out: string[] = [];
  const base = parseIsoDateUTC(lastDate);
  if (!base) return out;

  let cur = base;
  while (out.length < Math.max(0, Math.floor(n))) {
    cur = new Date(cur.getTime() + 24 * 60 * 60 * 1000);
    const wd = cur.getUTCDay(); // 0 Sun, 6 Sat
    if (wd === 0 || wd === 6) continue;
    out.push(formatIsoDateUTC(cur));
  }

  return out;
}

export function buildNavChartOption(
  rows: NavHistoryRow[],
  opts?: {
    compact?: boolean;
    color?: string;
    swing?: { enabled?: boolean; window?: number; maxPointsPerKind?: number };
    forecast?: ForecastCurve;
  }
) {
  const compact = opts?.compact ?? false;
  const color = opts?.color ?? "#2563EB";
  const swingEnabled = opts?.swing?.enabled ?? false;
  const swingWindow = opts?.swing?.window ?? 5;
  const swingMaxPointsPerKind = opts?.swing?.maxPointsPerKind ?? 6;

  const cleaned = (Array.isArray(rows) ? rows : [])
    .filter((r) => typeof r?.nav_date === "string" && r.nav_date)
    .map((r) => {
      const v = toFiniteNumber((r as any).unit_nav);
      return { date: r.nav_date as string, value: v };
    })
    .filter((r) => Number.isFinite(r.value));

  const dates = cleaned.map((r) => r.date);
  const values = cleaned.map((r) => r.value);

  const swingPoints = swingEnabled
    ? pickTopSwingPoints(computeSwingPoints(cleaned, swingWindow), swingMaxPointsPerKind)
    : [];

  const forecast = opts?.forecast;
  const forecastPointsRaw = Array.isArray(forecast?.points) ? (forecast!.points as any[]) : null;
  const forecastPoints = forecastPointsRaw
    ? forecastPointsRaw
        .map((p) => ({
          step: Number(p?.step),
          nav: toFiniteNumber(p?.nav),
          ci_low: toFiniteNumber(p?.ci_low),
          ci_high: toFiniteNumber(p?.ci_high),
        }))
        .filter((p) => Number.isFinite(p.step) && p.step > 0 && Number.isFinite(p.nav))
        .sort((a, b) => a.step - b.step)
    : [];

  const horizon = forecastPoints.length;
  const lastDate = dates.length ? dates[dates.length - 1] : "";
  const lastNav = values.length ? values[values.length - 1] : NaN;
  const futureDates = horizon && lastDate ? nextTradingDates(lastDate, horizon) : [];
  const xDates = futureDates.length ? [...dates, ...futureDates] : dates;

  const baseSeriesData = futureDates.length ? [...values, ...Array(futureDates.length).fill(null)] : values;

  let forecastSeries: any[] = [];
  if (futureDates.length && Number.isFinite(lastNav)) {
    const fNavs = forecastPoints.map((p) => p.nav);
    const fLow = forecastPoints.map((p) => (Number.isFinite(p.ci_low) ? p.ci_low : p.nav));
    const fHigh = forecastPoints.map((p) => (Number.isFinite(p.ci_high) ? p.ci_high : p.nav));

    const prefixNulls = Array(Math.max(0, values.length - 1)).fill(null);

    const forecastData = [...prefixNulls, lastNav, ...fNavs];
    const lowData = [...prefixNulls, lastNav, ...fLow];
    const highData = [...prefixNulls, lastNav, ...fHigh];

    const extremaPoints: any[] = [];
    const lowStep = Number((forecast as any)?.low?.step);
    const lowNav = toFiniteNumber((forecast as any)?.low?.nav);
    if (Number.isFinite(lowStep) && lowStep > 0 && lowStep <= futureDates.length && Number.isFinite(lowNav)) {
      extremaPoints.push({
        name: "预测低点",
        coord: [futureDates[lowStep - 1], lowNav],
        value: lowNav,
        itemStyle: { color: "#16A34A" },
        label: { formatter: "低" },
      });
    }
    const highStep = Number((forecast as any)?.high?.step);
    const highNav = toFiniteNumber((forecast as any)?.high?.nav);
    if (
      Number.isFinite(highStep) &&
      highStep > 0 &&
      highStep <= futureDates.length &&
      Number.isFinite(highNav)
    ) {
      extremaPoints.push({
        name: "预测高点",
        coord: [futureDates[highStep - 1], highNav],
        value: highNav,
        itemStyle: { color: "#DC2626" },
        label: { formatter: "高" },
      });
    }

    const swing = Array.isArray((forecast as any)?.swing_points) ? ((forecast as any).swing_points as any[]) : [];
    for (const s of swing.slice(0, 12)) {
      const step = Number(s?.step);
      const nav = toFiniteNumber(s?.nav);
      const kind = String(s?.kind ?? "");
      if (!Number.isFinite(step) || step <= 0 || step > futureDates.length || !Number.isFinite(nav)) continue;
      if (kind !== "low" && kind !== "high") continue;
      extremaPoints.push({
        name: kind === "low" ? "预测拐点(低)" : "预测拐点(高)",
        coord: [futureDates[step - 1], nav],
        value: nav,
        itemStyle: { color: kind === "low" ? "#22C55E" : "#EF4444" },
        symbol: "circle",
        symbolSize: 18,
        label: { show: false },
      });
    }

    forecastSeries = [
      {
        name: "预测净值",
        type: "line",
        data: forecastData,
        smooth: true,
        showSymbol: false,
        lineStyle: { width: 2, color: "#8B5CF6" },
        itemStyle: { color: "#8B5CF6" },
        markPoint: extremaPoints.length
          ? {
              symbol: "pin",
              symbolSize: 44,
              data: extremaPoints,
            }
          : undefined,
      },
      {
        name: "预测区间下界",
        type: "line",
        data: lowData,
        smooth: true,
        showSymbol: false,
        lineStyle: { type: "dashed", width: 1, color: "#94A3B8" },
      },
      {
        name: "预测区间上界",
        type: "line",
        data: highData,
        smooth: true,
        showSymbol: false,
        lineStyle: { type: "dashed", width: 1, color: "#94A3B8" },
      },
    ];
  }

  return {
    tooltip: { trigger: "axis", axisPointer: { type: "cross" } },
    legend: forecastSeries.length ? { top: 0 } : undefined,
    xAxis: {
      type: "category",
      data: xDates,
      axisLabel: { rotate: compact ? 45 : 0 },
    },
    yAxis: { type: "value", scale: true },
    grid: { left: "3%", right: "4%", bottom: compact ? "12%" : "10%", containLabel: true },
    series: [
      {
        name: "单位净值",
        type: "line",
        data: baseSeriesData,
        smooth: true,
        showSymbol: false,
        lineStyle: { color },
        itemStyle: { color },
        markPoint: swingPoints.length
          ? {
              symbol: "pin",
              symbolSize: 44,
              label: {
                formatter: (p: any) => {
                  const kind = String(p?.data?.kind ?? "");
                  return kind === "low" ? "低" : kind === "high" ? "高" : "";
                },
              },
              data: swingPoints.map((p) => ({
                coord: [p.date, p.value],
                value: p.value,
                kind: p.kind,
                itemStyle: { color: p.kind === "low" ? "#16A34A" : "#DC2626" },
              })),
            }
          : undefined,
      },
      ...forecastSeries,
    ],
  };
}
