export type FundPositionLike = Record<string, any> & {
  fund_code?: string;
  account_name?: string;
  holding_share?: string | number;
  holding_cost?: string | number;
  fund?: { latest_nav?: string | number } | null;
};

export type FundPositionRow = {
  account_name: string;
  holding_share: number;
  holding_cost: number;
  latest_nav: number | null;
  market_value: number | null;
  pnl: number | null;
  pnl_rate: number | null;
};

export const filterPositionsByFund = <T extends { fund_code?: string }>(
  positions: T[],
  fundCode: string
): T[] => {
  if (!Array.isArray(positions) || !fundCode) return [];
  return positions.filter((p) => String(p?.fund_code ?? "") === fundCode);
};

const toNumberOrNull = (value: unknown): number | null => {
  if (value === null || value === undefined || value === "") return null;
  const n = Number(value);
  return Number.isFinite(n) ? n : null;
};

export const buildFundPositionRows = (
  positions: FundPositionLike[],
  fundCode: string,
  fallbackLatestNav?: string | number | null
): FundPositionRow[] => {
  const filtered = filterPositionsByFund(positions ?? [], fundCode);
  const fallbackNav = toNumberOrNull(fallbackLatestNav);

  const rows = filtered.map((p) => {
    const share = toNumberOrNull(p.holding_share) ?? 0;
    const cost = toNumberOrNull(p.holding_cost) ?? 0;
    const latestNav = toNumberOrNull(p?.fund?.latest_nav) ?? fallbackNav;
    const marketValue = latestNav === null ? null : share * latestNav;
    const pnl = marketValue === null ? null : marketValue - cost;
    const pnlRate = cost > 0 && pnl !== null ? (pnl / cost) * 100 : null;

    return {
      account_name: String(p.account_name ?? "-"),
      holding_share: share,
      holding_cost: cost,
      latest_nav: latestNav,
      market_value: marketValue,
      pnl,
      pnl_rate: pnlRate,
    };
  });

  rows.sort((a, b) => (b.market_value ?? -Infinity) - (a.market_value ?? -Infinity));
  return rows;
};

export const sortOperationsDesc = <T extends { operation_date?: string; created_at?: string }>(
  operations: T[]
): T[] => {
  if (!Array.isArray(operations)) return [];
  return [...operations].sort((a, b) => {
    const d = String(b.operation_date ?? "").localeCompare(String(a.operation_date ?? ""));
    if (d !== 0) return d;
    return String(b.created_at ?? "").localeCompare(String(a.created_at ?? ""));
  });
};

