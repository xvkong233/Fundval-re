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

  test("detects swing high correctly", () => {
    const option: any = buildNavChartOption(
      [
        { nav_date: "2026-02-01", unit_nav: 1.0 },
        { nav_date: "2026-02-02", unit_nav: 1.2 },
        { nav_date: "2026-02-03", unit_nav: 1.5 },
        { nav_date: "2026-02-04", unit_nav: 1.2 },
        { nav_date: "2026-02-05", unit_nav: 1.0 },
      ],
      { swing: { enabled: true, window: 1, maxPointsPerKind: 6 } }
    );

    const mp = option.series?.[0]?.markPoint;
    expect(mp).toBeTruthy();
    const data = mp.data ?? [];
    expect(data.some((d: any) => d.kind === "high" && d.coord?.[0] === "2026-02-03")).toBe(true);
    expect(data.some((d: any) => d.kind === "low" && d.coord?.[0] === "2026-02-03")).toBe(false);
  });

  test("detects swing low correctly", () => {
    const option: any = buildNavChartOption(
      [
        { nav_date: "2026-02-01", unit_nav: 1.5 },
        { nav_date: "2026-02-02", unit_nav: 1.2 },
        { nav_date: "2026-02-03", unit_nav: 1.0 },
        { nav_date: "2026-02-04", unit_nav: 1.2 },
        { nav_date: "2026-02-05", unit_nav: 1.5 },
      ],
      { swing: { enabled: true, window: 1, maxPointsPerKind: 6 } }
    );

    const mp = option.series?.[0]?.markPoint;
    expect(mp).toBeTruthy();
    const data = mp.data ?? [];
    expect(data.some((d: any) => d.kind === "low" && d.coord?.[0] === "2026-02-03")).toBe(true);
    expect(data.some((d: any) => d.kind === "high" && d.coord?.[0] === "2026-02-03")).toBe(false);
  });

  test("appends forecast curve and skips weekends", () => {
    const option: any = buildNavChartOption(
      [
        { nav_date: "2026-02-20", unit_nav: 1.0 }, // Fri
        { nav_date: "2026-02-23", unit_nav: 1.02 }, // Mon
      ],
      {
        forecast: {
          base_nav: 1.02,
          points: [
            { step: 1, nav: 1.03, ci_low: 1.01, ci_high: 1.05, mu: 0.0 },
            { step: 2, nav: 1.04, ci_low: 1.02, ci_high: 1.06, mu: 0.0 },
            { step: 3, nav: 1.05, ci_low: 1.03, ci_high: 1.07, mu: 0.0 },
          ],
          low: { step: 1, nav: 1.03 },
          high: { step: 3, nav: 1.05 },
        },
      } as any
    );

    // next trading days after 2026-02-23: 2026-02-24, 2026-02-25, 2026-02-26
    expect(option.xAxis.data.slice(-3)).toEqual(["2026-02-24", "2026-02-25", "2026-02-26"]);
    expect(option.series.length).toBeGreaterThan(1);
    expect(option.series.some((s: any) => s.name === "预测净值")).toBe(true);
  });
});

