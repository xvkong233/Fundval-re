type CandidateItem = {
  fund_code: string;
  star_count?: number | null;
  max_drawdown?: string | null;
  year_growth?: string | null;
};

function toNumber(value: string | null | undefined): number | null {
  if (!value) return null;
  const n = Number.parseFloat(String(value));
  return Number.isFinite(n) ? n : null;
}

export function selectSnifferSignalCandidateCodes(items: CandidateItem[], limit = 50): string[] {
  const normalized = items
    .map((it, idx) => {
      const fundCode = String(it.fund_code ?? "").trim();
      const stars = typeof it.star_count === "number" && Number.isFinite(it.star_count) ? it.star_count : 0;
      const drawdown = toNumber(it.max_drawdown) ?? 0;
      const yearGrowth = toNumber(it.year_growth) ?? 0;
      const score = stars * 10000 + Math.min(drawdown, 80) * 100 + Math.min(yearGrowth, 80);
      const primary = stars >= 3 || drawdown >= 10;
      return { fundCode, score, primary, idx };
    })
    .filter((x) => Boolean(x.fundCode));

  const sort = (a: (typeof normalized)[number], b: (typeof normalized)[number]) => {
    if (b.score !== a.score) return b.score - a.score;
    // 旧实现用 idx 作为 tie-break，会导致“相同 score 的候选集合”在输入顺序变化时不稳定，
    // 进而使嗅探页 signalCandidateKey 波动、触发重复入队。这里改为按 fundCode 字典序稳定排序。
    const c = a.fundCode.localeCompare(b.fundCode);
    if (c !== 0) return c;
    return a.idx - b.idx;
  };

  const primary = normalized.filter((x) => x.primary).sort(sort);
  const secondary = normalized.filter((x) => !x.primary).sort(sort);

  const out: string[] = [];
  const seen = new Set<string>();
  for (const x of [...primary, ...secondary]) {
    if (seen.has(x.fundCode)) continue;
    seen.add(x.fundCode);
    out.push(x.fundCode);
    if (out.length >= limit) break;
  }
  return out;
}

