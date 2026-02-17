import { describe, expect, test } from "vitest";

import { getDateRange } from "../dateRange";

describe("getDateRange", () => {
  test("1W uses past 7 days", () => {
    const now = new Date("2026-02-17T12:00:00Z");
    const { startDate, endDate } = getDateRange("1W", now);
    expect(endDate).toBe("2026-02-17");
    expect(startDate).toBe("2026-02-10");
  });

  test("ALL uses past 10 years", () => {
    const now = new Date("2026-02-17T12:00:00Z");
    const { startDate, endDate } = getDateRange("ALL", now);
    expect(endDate).toBe("2026-02-17");
    expect(startDate).toBe("2016-02-17");
  });
});

