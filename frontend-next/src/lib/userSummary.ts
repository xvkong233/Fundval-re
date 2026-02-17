export type UserSummaryLike = Record<string, any> & {
  account_count?: number;
  position_count?: number;
  total_cost?: string | number;
  total_value?: string | number;
  total_pnl?: string | number;
};

export type NormalizedUserSummary = {
  account_count: number;
  position_count: number;
  total_cost: number;
  total_value: number;
  total_pnl: number;
  total_pnl_rate: number | null;
};

const toNumber = (value: unknown): number => {
  const n = Number(value);
  return Number.isFinite(n) ? n : 0;
};

export const normalizeUserSummary = (input: UserSummaryLike): NormalizedUserSummary => {
  const totalCost = toNumber(input?.total_cost);
  const totalValue = toNumber(input?.total_value);
  const totalPnl = toNumber(input?.total_pnl);

  const totalPnlRate = totalCost > 0 ? (totalPnl / totalCost) * 100 : null;

  return {
    account_count: Number.isFinite(Number(input?.account_count)) ? Number(input?.account_count) : 0,
    position_count: Number.isFinite(Number(input?.position_count)) ? Number(input?.position_count) : 0,
    total_cost: totalCost,
    total_value: totalValue,
    total_pnl: totalPnl,
    total_pnl_rate: totalPnlRate,
  };
};

