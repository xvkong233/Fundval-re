import { describe, expect, test } from "vitest";

import { normalizeNavHistoryRows } from "../navHistoryNormalize";

describe("normalizeNavHistoryRows", () => {
  test("maps accumulated_nav and keeps unit_nav/daily_growth", () => {
    const out = normalizeNavHistoryRows([
      {
        nav_date: "2024-01-01",
        unit_nav: "1.2345",
        accumulated_nav: "2.3456",
        daily_growth: "0.90",
      },
    ]);

    expect(out).toEqual([
      {
        nav_date: "2024-01-01",
        unit_nav: "1.2345",
        accumulated_nav: "2.3456",
        daily_growth: "0.90",
      },
    ]);
  });

  test("accepts legacy accum_nav field by mapping to accumulated_nav", () => {
    const out = normalizeNavHistoryRows([
      { nav_date: "2024-01-01", unit_nav: "1.0", accum_nav: "2.0" as any },
    ]);
    expect(out[0].accumulated_nav).toBe("2.0");
  });
});

