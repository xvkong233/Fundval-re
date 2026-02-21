export type SnifferAdviceInputItem = {
  fund_code: string;
  fund_name: string;
  star_count?: number | null;
  max_drawdown?: string | null;
  week_growth?: string | null;
  year_growth?: string | null;
};

export type SnifferSignalsSummary = {
  peer_name?: string | null;
  position_bucket?: "low" | "medium" | "high" | string | null;
  dip_buy_p_20t?: number | null;
  magic_rebound_p_20t?: number | null;
  dip_buy_p_5t?: number | null;
  magic_rebound_p_5t?: number | null;
  model_sample_size_20t?: number | null;
  as_of_date?: string | null;
};

export type AdviceRow = SnifferAdviceInputItem & { reasons: string[] };

export type SnifferAdvice = {
  buy: AdviceRow[];
  watch: AdviceRow[];
  avoid: AdviceRow[];
};

function toNumber(value: string | null | undefined): number | null {
  if (!value) return null;
  const n = Number.parseFloat(String(value));
  return Number.isFinite(n) ? n : null;
}

function normBucket(bucket: any): "low" | "medium" | "high" | null {
  const s = String(bucket ?? "").toLowerCase().trim();
  if (s === "low") return "low";
  if (s === "medium") return "medium";
  if (s === "high") return "high";
  return null;
}

export function buildSnifferAdvice(
  items: SnifferAdviceInputItem[],
  signalsByFundCode: Record<string, SnifferSignalsSummary | null> = {}
): SnifferAdvice {
  const scored: { item: SnifferAdviceInputItem; bucket: "buy" | "watch" | "avoid"; score: number; reasons: string[] }[] =
    [];

  for (const it of items) {
    const code = String(it.fund_code ?? "").trim();
    const s = signalsByFundCode[code] ?? null;
    const reasons: string[] = [];

    const stars = typeof it.star_count === "number" ? it.star_count : 0;
    const dd = toNumber(it.max_drawdown) ?? 0;

    const bucket = normBucket(s?.position_bucket);
    if (bucket === "low") reasons.push("位置偏低（20/60/20）");
    if (bucket === "high") reasons.push("位置偏高（20/60/20）");

    const dip20 = typeof s?.dip_buy_p_20t === "number" ? s.dip_buy_p_20t : null;
    const reb20 = typeof s?.magic_rebound_p_20t === "number" ? s.magic_rebound_p_20t : null;
    if (dip20 !== null) reasons.push(`抄底概率(20T) ${(dip20 * 100).toFixed(1)}%`);
    if (reb20 !== null) reasons.push(`反转概率(20T) ${(reb20 * 100).toFixed(1)}%`);

    if (stars >= 4) reasons.push("星级较高");
    if (stars >= 3 && dd >= 15) reasons.push("回撤较深");

    // 中性建议：偏低 + 概率更高时才进入买入候选；样本少时要求更高阈值。
    const sampleOk = (typeof s?.model_sample_size_20t === "number" ? s.model_sample_size_20t : 0) >= 100;
    const strongDip = dip20 !== null && dip20 >= (sampleOk ? 0.55 : 0.65);
    const strongReb = reb20 !== null && reb20 >= (sampleOk ? 0.45 : 0.55);

    const buy = bucket === "low" && stars >= 3 && (strongDip || strongReb);
    const avoid = bucket === "high" && stars <= 3 && (dip20 === null || dip20 < 0.45);
    const outBucket: "buy" | "watch" | "avoid" = buy ? "buy" : avoid ? "avoid" : "watch";

    const baseScore = stars * 0.15 + Math.min(dd / 30, 1) * 0.2;
    const probaScore = (dip20 ?? 0) * 0.45 + (reb20 ?? 0) * 0.25;
    const lowBonus = bucket === "low" ? 0.2 : bucket === "high" ? -0.15 : 0;
    const score = baseScore + probaScore + lowBonus + (sampleOk ? 0.05 : 0);

    scored.push({
      item: it,
      bucket: outBucket,
      score,
      reasons: reasons.length ? reasons : ["数据不足（等待更多缓存/训练）"],
    });
  }

  const sortStable = (a: AdviceRow, b: AdviceRow) => {
    const sa = typeof a.star_count === "number" ? a.star_count : -1;
    const sb = typeof b.star_count === "number" ? b.star_count : -1;
    if (sb !== sa) return sb - sa;
    const dda = toNumber(a.max_drawdown) ?? -Infinity;
    const ddb = toNumber(b.max_drawdown) ?? -Infinity;
    if (ddb !== dda) return ddb - dda;
    const ya = toNumber(a.year_growth) ?? -Infinity;
    const yb = toNumber(b.year_growth) ?? -Infinity;
    return yb - ya;
  };

  const buyRows: AdviceRow[] = scored
    .filter((x) => x.bucket === "buy")
    .sort((a, b) => b.score - a.score)
    .map((x) => ({ ...x.item, reasons: x.reasons }))
    .sort(sortStable);

  const watchRows: AdviceRow[] = scored
    .filter((x) => x.bucket === "watch")
    .sort((a, b) => b.score - a.score)
    .map((x) => ({ ...x.item, reasons: x.reasons }))
    .sort(sortStable);

  const avoidRows: AdviceRow[] = scored
    .filter((x) => x.bucket === "avoid")
    .sort((a, b) => b.score - a.score)
    .map((x) => ({ ...x.item, reasons: x.reasons }))
    .sort(sortStable);

  return { buy: buyRows, watch: watchRows, avoid: avoidRows };
}

