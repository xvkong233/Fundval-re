import { describe, expect, test } from "vitest";

import { formatErrorRatePercent, normalizeSourceAccuracy } from "../sources";

describe("normalizeSourceAccuracy", () => {
  test("parses avg_error_rate and keeps record_count", () => {
    const normalized = normalizeSourceAccuracy({ avg_error_rate: "0.0123", record_count: 1500 });
    expect(normalized).toEqual({ avg_error_rate: 0.0123, record_count: 1500 });
  });

  test("handles missing fields safely", () => {
    const normalized = normalizeSourceAccuracy({});
    expect(normalized).toEqual({ avg_error_rate: 0, record_count: 0 });
  });
});

describe("formatErrorRatePercent", () => {
  test("formats decimal as percent string", () => {
    expect(formatErrorRatePercent(0.0123)).toBe("1.23%");
  });
});

