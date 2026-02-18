export type PositionLike = {
  fund_code?: string;
  fund_name?: string;
  fund_type?: string | null;
  holding_share?: string | number | null;
  pnl?: string | number | null;
  fund?: {
    fund_name?: string;
    fund_type?: string | null;
    latest_nav?: string | number | null;
  };
};

export type DistributionRow = { name: string; value: number; percent: number };
export type PnlRow = { name: string; pnl: number };

function toNumber(v: any): number | null {
  if (v === null || v === undefined || v === "") return null;
  const n = Number(v);
  return Number.isFinite(n) ? n : null;
}

function mapFundType(raw: any): string {
  const text = typeof raw === "string" ? raw.trim() : "";
  if (!text) return "其他";

  const lower = text.toLowerCase();
  if (text.includes("股票") || text.includes("指数") || lower.includes("equity") || lower.includes("index")) return "股票型";
  if (text.includes("债券") || text.includes("中短债") || text.includes("固收") || lower.includes("bond")) return "债券型";
  if (text.includes("混合") || lower.includes("balanced")) return "混合型";
  if (text.includes("货币") || lower.includes("money")) return "货币型";
  if (text.toLowerCase().includes("qdii") || text.includes("海外") || lower.includes("overseas")) return "QDII";
  return "其他";
}

export function computeDistribution(positions: PositionLike[]): DistributionRow[] {
  const list = Array.isArray(positions) ? positions : [];
  const byType = new Map<string, number>();

  for (const p of list) {
    const share = toNumber(p?.holding_share);
    const nav = toNumber(p?.fund?.latest_nav);
    if (share === null || nav === null) continue;
    const value = share * nav;
    if (!Number.isFinite(value) || value <= 0) continue;

    const t = mapFundType(p?.fund_type ?? p?.fund?.fund_type);
    byType.set(t, (byType.get(t) ?? 0) + value);
  }

  const total = Array.from(byType.values()).reduce((sum, v) => sum + v, 0);
  const out = Array.from(byType.entries())
    .map(([name, value]) => ({
      name,
      value: Number(value.toFixed(2)),
      percent: total > 0 ? Number(((value / total) * 100).toFixed(2)) : 0,
    }))
    .sort((a, b) => b.value - a.value);

  return out;
}

function isPnlRow(row: { name: string; pnl: number | null }): row is PnlRow {
  return !!row.name && typeof row.pnl === "number" && Number.isFinite(row.pnl);
}

export function computePnlRanking(positions: PositionLike[], limit = 10): PnlRow[] {
  const list = Array.isArray(positions) ? positions : [];
  const rows = list
    .map((p) => {
      const pnl = toNumber(p?.pnl);
      const name = String(p?.fund_name ?? p?.fund?.fund_name ?? p?.fund_code ?? "").trim();
      return { name, pnl };
    })
    .filter(isPnlRow)
    .sort((a, b) => b.pnl - a.pnl);

  return rows.slice(0, Math.max(0, limit));
}

export function buildDistributionChartOption(
  data: DistributionRow[],
  opts?: { compact?: boolean }
) {
  const compact = opts?.compact ?? false;
  const cleaned = (Array.isArray(data) ? data : []).filter((d) => d?.name && Number.isFinite(d.value) && d.value > 0);
  return {
    tooltip: {
      trigger: "item",
      formatter: (params: any) => {
        const name = String(params?.name ?? "");
        const value = Number(params?.value ?? 0);
        const percent = Number(params?.percent ?? 0);
        return `${name}<br/>金额：¥${value.toFixed(2)}<br/>占比：${percent.toFixed(2)}%`;
      },
    },
    legend: { orient: compact ? "horizontal" : "vertical", left: compact ? "center" : "left" },
    series: [
      {
        name: "仓位分布",
        type: "pie",
        radius: compact ? ["30%", "70%"] : ["25%", "75%"],
        center: ["60%", "50%"],
        data: cleaned.map((d) => ({ name: d.name, value: d.value })),
        emphasis: { itemStyle: { shadowBlur: 10, shadowOffsetX: 0, shadowColor: "rgba(0, 0, 0, 0.3)" } },
      },
    ],
  };
}

export function buildPnlRankingChartOption(
  data: PnlRow[],
  opts?: { compact?: boolean }
) {
  const compact = opts?.compact ?? false;
  const cleaned = (Array.isArray(data) ? data : []).filter((d) => d?.name && Number.isFinite(d.pnl));
  return {
    tooltip: {
      trigger: "axis",
      axisPointer: { type: "shadow" },
      formatter: (params: any) => {
        const p = Array.isArray(params) ? params[0] : params;
        const name = String(p?.name ?? "");
        const value = Number(p?.value ?? 0);
        return `${name}<br/>盈亏：¥${value.toFixed(2)}`;
      },
    },
    grid: { left: "3%", right: "4%", bottom: compact ? "16%" : "10%", containLabel: true },
    xAxis: { type: "category", data: cleaned.map((d) => d.name), axisLabel: { rotate: compact ? 45 : 0 } },
    yAxis: { type: "value" },
    series: [
      {
        name: "盈亏",
        type: "bar",
        data: cleaned.map((d) => d.pnl),
        itemStyle: {
          color: (params: any) => {
            const v = Number(params?.value ?? 0);
            return v >= 0 ? "#cf1322" : "#3f8600";
          },
        },
      },
    ],
  };
}
