export type SnifferAdviceInputItem = {
  fund_code: string;
  fund_name: string;
  star_count?: number | null;
  max_drawdown?: string | null;
  week_growth?: string | null;
  year_growth?: string | null;
};

export type SnifferAdvice = {
  focus: SnifferAdviceInputItem[];
  dipBuy: SnifferAdviceInputItem[];
};

function toNumber(value: string | null | undefined): number | null {
  if (!value) return null;
  const n = Number.parseFloat(String(value));
  return Number.isFinite(n) ? n : null;
}

export function buildSnifferAdvice(items: SnifferAdviceInputItem[]): SnifferAdvice {
  const focus = [...items].sort((a, b) => {
    const sa = typeof a.star_count === "number" ? a.star_count : -1;
    const sb = typeof b.star_count === "number" ? b.star_count : -1;
    if (sb !== sa) return sb - sa;
    const ya = toNumber(a.year_growth) ?? -Infinity;
    const yb = toNumber(b.year_growth) ?? -Infinity;
    return yb - ya;
  });

  const dipBuy = items
    .filter((it) => {
      const s = typeof it.star_count === "number" ? it.star_count : 0;
      const dd = toNumber(it.max_drawdown) ?? 0;
      return s >= 3 && dd >= 15;
    })
    .sort((a, b) => {
      const dda = toNumber(a.max_drawdown) ?? -Infinity;
      const ddb = toNumber(b.max_drawdown) ?? -Infinity;
      if (ddb !== dda) return ddb - dda;
      const sa = typeof a.star_count === "number" ? a.star_count : -1;
      const sb = typeof b.star_count === "number" ? b.star_count : -1;
      return sb - sa;
    });

  return { focus, dipBuy };
}

