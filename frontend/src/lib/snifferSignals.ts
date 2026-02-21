import type { SnifferSignalsSummary } from "./snifferAdvice";

type HorizonProbaOut = {
  p_5t?: number | null;
  p_20t?: number | null;
};

type PeerSignalsOut = {
  peer_code: string;
  peer_name: string;
  position_bucket?: string | null;
  dip_buy?: HorizonProbaOut | null;
  magic_rebound?: HorizonProbaOut | null;
  model_sample_size_20t?: number | null;
  computed_at?: string | null;
};

type FundSignalsLiteOut = {
  fund_code: string;
  source: string;
  as_of_date?: string | null;
  best_peer?: PeerSignalsOut | null;
  computed_at: string;
};

export function liteListToSignalsSummaryByFund(
  list: FundSignalsLiteOut[]
): Record<string, SnifferSignalsSummary | null> {
  const out: Record<string, SnifferSignalsSummary | null> = {};
  for (const x of Array.isArray(list) ? list : []) {
    const code = String((x as any)?.fund_code ?? "").trim();
    if (!code) continue;
    const best = (x as any)?.best_peer ?? null;
    if (!best) {
      out[code] = null;
      continue;
    }
    out[code] = {
      peer_name: typeof best.peer_name === "string" ? best.peer_name : null,
      position_bucket: typeof best.position_bucket === "string" ? best.position_bucket : null,
      dip_buy_p_5t: typeof best?.dip_buy?.p_5t === "number" ? best.dip_buy.p_5t : null,
      dip_buy_p_20t: typeof best?.dip_buy?.p_20t === "number" ? best.dip_buy.p_20t : null,
      magic_rebound_p_5t: typeof best?.magic_rebound?.p_5t === "number" ? best.magic_rebound.p_5t : null,
      magic_rebound_p_20t: typeof best?.magic_rebound?.p_20t === "number" ? best.magic_rebound.p_20t : null,
      model_sample_size_20t: typeof best.model_sample_size_20t === "number" ? best.model_sample_size_20t : null,
      as_of_date: typeof (x as any)?.as_of_date === "string" ? (x as any).as_of_date : null,
    };
  }
  return out;
}

