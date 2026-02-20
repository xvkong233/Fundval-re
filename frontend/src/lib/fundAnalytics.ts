import type { TimeRange } from "./dateRange";

export type TradingDaysRange = "5T" | "20T" | "60T" | "120T" | "252T";

export function timeRangeToTradingDaysRange(range: TimeRange): TradingDaysRange {
  switch (range) {
    case "1W":
      return "5T";
    case "1M":
      return "20T";
    case "3M":
      return "60T";
    case "6M":
      return "120T";
    case "1Y":
    case "ALL":
      return "252T";
  }
}

