import { describe, expect, test } from "vitest";
import { buildSnifferAdvice } from "../snifferAdvice";

describe("buildSnifferAdvice (neutral)", () => {
  test("puts low+high-proba into buy and high+low-proba into avoid", () => {
    const items = [
      { fund_code: "A", fund_name: "A", star_count: 4, max_drawdown: "18", year_growth: "5" },
      { fund_code: "B", fund_name: "B", star_count: 2, max_drawdown: "10", year_growth: "20" },
    ];

    const out = buildSnifferAdvice(items, {
      A: { position_bucket: "low", dip_buy_p_20t: 0.6, magic_rebound_p_20t: 0.3, model_sample_size_20t: 120 },
      B: { position_bucket: "high", dip_buy_p_20t: 0.3, magic_rebound_p_20t: 0.1, model_sample_size_20t: 120 },
    });

    expect(out.buy.map((x) => x.fund_code)).toEqual(["A"]);
    expect(out.avoid.map((x) => x.fund_code)).toEqual(["B"]);
  });
});

