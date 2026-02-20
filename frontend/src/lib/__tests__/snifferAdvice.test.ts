import { describe, expect, test } from "vitest";
import { buildSnifferAdvice } from "../snifferAdvice";

describe("buildSnifferAdvice", () => {
  test("selects focus and dip-buy candidates", () => {
    const items = [
      { fund_code: "A", fund_name: "A", star_count: 5, max_drawdown: "10", year_growth: "20" },
      { fund_code: "B", fund_name: "B", star_count: 4, max_drawdown: "18", year_growth: "5" },
      { fund_code: "C", fund_name: "C", star_count: 3, max_drawdown: "25", year_growth: "1" },
      { fund_code: "D", fund_name: "D", star_count: 2, max_drawdown: "30", year_growth: "-3" },
    ];

    const out = buildSnifferAdvice(items);
    expect(out.focus.map((x) => x.fund_code)).toEqual(["A", "B", "C", "D"]);
    expect(out.dipBuy.map((x) => x.fund_code)).toEqual(["C", "B"]);
  });
});

