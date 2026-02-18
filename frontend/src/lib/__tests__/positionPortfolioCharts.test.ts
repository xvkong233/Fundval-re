import { describe, expect, it } from "vitest";

import { computeDistribution, computePnlRanking } from "../positionPortfolioCharts";

describe("positionPortfolioCharts", () => {
  it("computes distribution by mapped fund type", () => {
    const out = computeDistribution([
      { fund_type: "混合型", holding_share: "10.0000", fund: { latest_nav: "1.0000" } },
      { fund_type: "股票型", holding_share: "5.0000", fund: { latest_nav: "2.0000" } },
    ]);

    const byName = new Map(out.map((r) => [r.name, r]));
    expect(byName.get("混合型")?.value).toBe(10);
    expect(byName.get("股票型")?.value).toBe(10);
  });

  it("computes pnl ranking sorted desc and limited", () => {
    const out = computePnlRanking(
      [
        { fund_name: "A", pnl: "-1" },
        { fund_name: "B", pnl: "2" },
        { fund_name: "C", pnl: "1" },
      ],
      2
    );

    expect(out.map((r) => r.name)).toEqual(["B", "C"]);
  });
});

