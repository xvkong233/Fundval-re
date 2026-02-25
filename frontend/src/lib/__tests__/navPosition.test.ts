import { describe, expect, test } from "vitest";

import { bucketForRangePosition, computeRangePositionPct } from "../navPosition";

describe("navPosition", () => {
  test("computes range position percent (0 at min, 100 at max)", () => {
    const pct = computeRangePositionPct([
      { nav_date: "2026-02-01", unit_nav: 1.0 },
      { nav_date: "2026-02-02", unit_nav: 2.0 },
      { nav_date: "2026-02-03", unit_nav: 1.5 },
    ]);

    expect(pct).not.toBeNull();
    expect(Math.round(pct as number)).toBe(50);
  });

  test("returns null when insufficient or flat", () => {
    expect(computeRangePositionPct([])).toBeNull();
    expect(computeRangePositionPct([{ nav_date: "2026-02-01", unit_nav: 1.0 }])).toBeNull();
    expect(
      computeRangePositionPct([
        { nav_date: "2026-02-01", unit_nav: 1.0 },
        { nav_date: "2026-02-02", unit_nav: 1.0 },
      ])
    ).toBeNull();
  });

  test("buckets position with 20/60/20", () => {
    expect(bucketForRangePosition(0)).toBe("low");
    expect(bucketForRangePosition(20)).toBe("low");
    expect(bucketForRangePosition(50)).toBe("medium");
    expect(bucketForRangePosition(80)).toBe("medium");
    expect(bucketForRangePosition(81)).toBe("high");
  });
});

