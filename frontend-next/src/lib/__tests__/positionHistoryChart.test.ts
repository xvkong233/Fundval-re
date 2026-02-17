import { describe, expect, it } from "vitest";

import { buildPositionHistoryChartOption } from "../positionHistoryChart";

describe("buildPositionHistoryChartOption", () => {
  it("builds two series and x-axis dates", () => {
    const option: any = buildPositionHistoryChartOption([
      { date: "2026-02-15", value: 100, cost: 90 },
      { date: "2026-02-16", value: 110, cost: 95 },
    ]);

    expect(option?.xAxis?.data).toEqual(["2026-02-15", "2026-02-16"]);
    expect(option?.series?.length).toBe(2);
    expect(option?.series?.[0]?.name).toBe("账户市值");
    expect(option?.series?.[1]?.name).toBe("持仓成本");
  });
});

