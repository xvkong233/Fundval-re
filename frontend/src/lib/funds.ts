export type Fund = Record<string, any> & {
  fund_code: string;
  fund_name?: string;
};

export type FundListNormalized = {
  total: number;
  results: Fund[];
};

export function normalizeFundList(json: any): FundListNormalized {
  if (Array.isArray(json)) {
    return { total: json.length, results: json as Fund[] };
  }
  const results = (json?.results ?? []) as Fund[];
  const total = typeof json?.count === "number" ? json.count : results.length;
  return { total, results };
}

export function mergeBatchNav(funds: Fund[], batchNav: any): Fund[] {
  if (!batchNav || typeof batchNav !== "object") return funds;
  return funds.map((f) => {
    const nav = batchNav[f.fund_code];
    if (!nav || nav.error) return f;
    return { ...f, latest_nav: nav.latest_nav, latest_nav_date: nav.latest_nav_date };
  });
}

export function mergeBatchEstimate(funds: Fund[], batchEstimate: any): Fund[] {
  if (!batchEstimate || typeof batchEstimate !== "object") return funds;
  return funds.map((f) => {
    const est = batchEstimate[f.fund_code];
    if (!est || est.error) return f;
    return {
      ...f,
      estimate_nav: est.estimate_nav,
      estimate_growth: est.estimate_growth,
      estimate_time: est.estimate_time,
      from_cache: est.from_cache,
    };
  });
}

