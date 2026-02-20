import { describe, expect, test } from "vitest";
import { timeRangeToTradingDaysRange } from "../fundAnalytics";

describe("timeRangeToTradingDaysRange", () => {
  test("maps TimeRange to trading-day window", () => {
    expect(timeRangeToTradingDaysRange("1W")).toBe("5T");
    expect(timeRangeToTradingDaysRange("1M")).toBe("20T");
    expect(timeRangeToTradingDaysRange("3M")).toBe("60T");
    expect(timeRangeToTradingDaysRange("6M")).toBe("120T");
    expect(timeRangeToTradingDaysRange("1Y")).toBe("252T");
    expect(timeRangeToTradingDaysRange("ALL")).toBe("252T");
  });
});

