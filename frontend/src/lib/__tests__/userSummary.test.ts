import { describe, expect, test } from "vitest";

import { normalizeUserSummary } from "../userSummary";

describe("normalizeUserSummary", () => {
  test("parses numeric fields and calculates pnl_rate", () => {
    const normalized = normalizeUserSummary({
      account_count: 3,
      position_count: 10,
      total_cost: "100.00",
      total_value: "120.00",
      total_pnl: "20.00",
    });

    expect(normalized).toMatchObject({
      account_count: 3,
      position_count: 10,
      total_cost: 100,
      total_value: 120,
      total_pnl: 20,
      total_pnl_rate: 20,
    });
  });

  test("handles zero cost safely", () => {
    const normalized = normalizeUserSummary({
      account_count: 0,
      position_count: 0,
      total_cost: 0,
      total_value: 0,
      total_pnl: 0,
    });
    expect(normalized.total_pnl_rate).toBeNull();
  });
});

