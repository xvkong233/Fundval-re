export type NavHistoryLike = Record<string, any> & {
  nav_date?: string;
  unit_nav?: string | number;
  accumulated_nav?: string | number | null;
  daily_growth?: string | number | null;
  // legacy
  accum_nav?: string | number | null;
};

export type NormalizedNavHistoryRow = {
  nav_date?: string;
  unit_nav?: string | number;
  accumulated_nav?: string | number | null;
  daily_growth?: string | number | null;
};

export const normalizeNavHistoryRows = (rows: NavHistoryLike[]): NormalizedNavHistoryRow[] => {
  if (!Array.isArray(rows)) return [];
  return rows.map((r) => ({
    nav_date: r?.nav_date,
    unit_nav: r?.unit_nav,
    accumulated_nav: r?.accumulated_nav ?? r?.accum_nav ?? null,
    daily_growth: r?.daily_growth ?? null,
  }));
};

