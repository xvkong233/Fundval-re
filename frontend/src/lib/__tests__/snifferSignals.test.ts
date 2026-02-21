import { describe, expect, test } from "vitest";
import { liteListToSignalsSummaryByFund } from "../snifferSignals";

describe("liteListToSignalsSummaryByFund", () => {
  test("flattens best_peer into SnifferSignalsSummary", () => {
    const out = liteListToSignalsSummaryByFund([
      {
        fund_code: "018939",
        source: "tiantian",
        as_of_date: "2026-02-20",
        computed_at: "2026-02-21T00:00:00Z",
        best_peer: {
          peer_code: "BK001",
          peer_name: "国防军工",
          position_bucket: "low",
          dip_buy: { p_5t: 0.12, p_20t: 0.62 },
          magic_rebound: { p_5t: 0.08, p_20t: 0.31 },
          model_sample_size_20t: 120,
          computed_at: "2026-02-21T00:00:00Z",
        },
      },
      {
        fund_code: "000000",
        source: "tiantian",
        as_of_date: null,
        computed_at: "2026-02-21T00:00:00Z",
        best_peer: null,
      },
    ] as any);

    expect(out["018939"]?.peer_name).toBe("国防军工");
    expect(out["018939"]?.position_bucket).toBe("low");
    expect(out["018939"]?.dip_buy_p_5t).toBe(0.12);
    expect(out["018939"]?.dip_buy_p_20t).toBe(0.62);
    expect(out["018939"]?.magic_rebound_p_5t).toBe(0.08);
    expect(out["018939"]?.magic_rebound_p_20t).toBe(0.31);
    expect(out["018939"]?.model_sample_size_20t).toBe(120);
    expect(out["018939"]?.as_of_date).toBe("2026-02-20");

    expect(out["000000"]).toBeNull();
  });
});

