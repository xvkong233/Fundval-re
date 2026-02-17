import { describe, expect, test } from "vitest";

import { buildNavChartOption } from "../navChart";

describe("buildNavChartOption", () => {
  test("builds line chart option from nav history", () => {
    const option = buildNavChartOption(
      [
        { nav_date: "2026-02-15", unit_nav: "1.0000" },
        { nav_date: "2026-02-16", unit_nav: "1.0200" },
        { nav_date: "2026-02-17", unit_nav: "1.0100" },
      ],
      { compact: true }
    );

    expect(option.xAxis).toBeTruthy();
    expect((option as any).xAxis.data).toEqual(["2026-02-15", "2026-02-16", "2026-02-17"]);
    expect((option as any).series[0].type).toBe("line");
    expect((option as any).series[0].data).toEqual([1.0, 1.02, 1.01]);
  });
});

