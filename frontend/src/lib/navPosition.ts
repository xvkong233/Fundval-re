import type { NavHistoryRow } from "./navChart";

function toFiniteNumber(vRaw: unknown): number {
  const v = typeof vRaw === "number" ? vRaw : typeof vRaw === "string" ? Number.parseFloat(vRaw) : NaN;
  return Number.isFinite(v) ? v : NaN;
}

export function computeRangePositionPct(rows: NavHistoryRow[]): number | null {
  const cleaned = (Array.isArray(rows) ? rows : [])
    .filter((r) => typeof (r as any)?.nav_date === "string" && (r as any).nav_date)
    .map((r) => ({ date: String((r as any).nav_date), value: toFiniteNumber((r as any).unit_nav) }))
    .filter((x) => Number.isFinite(x.value));

  if (cleaned.length < 2) return null;

  cleaned.sort((a, b) => a.date.localeCompare(b.date));
  const latest = cleaned[cleaned.length - 1]?.value;
  if (!Number.isFinite(latest)) return null;

  let minV = Number.POSITIVE_INFINITY;
  let maxV = Number.NEGATIVE_INFINITY;
  for (const p of cleaned) {
    if (p.value < minV) minV = p.value;
    if (p.value > maxV) maxV = p.value;
  }

  if (!Number.isFinite(minV) || !Number.isFinite(maxV) || maxV <= minV) return null;

  const pct = ((latest - minV) / (maxV - minV)) * 100;
  return Math.max(0, Math.min(100, pct));
}

export function bucketForRangePosition(pct0To100: number): "low" | "medium" | "high" {
  const p = Number.isFinite(pct0To100) ? Math.max(0, Math.min(100, pct0To100)) : 50;
  if (p <= 20) return "low";
  if (p <= 80) return "medium";
  return "high";
}

