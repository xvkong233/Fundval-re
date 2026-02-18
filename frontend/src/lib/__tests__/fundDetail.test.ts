import { describe, expect, test } from "vitest";

import { buildFundPositionRows, filterPositionsByFund, sortOperationsDesc } from "../fundDetail";

describe("filterPositionsByFund", () => {
  test("keeps only matching fund_code", () => {
    const input = [
      { fund_code: "000001", holding_share: "1" },
      { fund_code: "000002", holding_share: "2" },
    ];
    expect(filterPositionsByFund(input, "000001")).toEqual([{ fund_code: "000001", holding_share: "1" }]);
  });
});

describe("buildFundPositionRows", () => {
  test("calculates market_value, pnl and pnl_rate", () => {
    const rows = buildFundPositionRows(
      [
        {
          account_name: "子账户A",
          fund_code: "000001",
          holding_share: "100.0",
          holding_cost: "1000.00",
          fund: { latest_nav: "12.0" },
        },
      ],
      "000001",
      "12.0"
    );

    expect(rows).toHaveLength(1);
    expect(rows[0]).toMatchObject({
      account_name: "子账户A",
      holding_share: 100,
      holding_cost: 1000,
      latest_nav: 12,
      market_value: 1200,
      pnl: 200,
      pnl_rate: 20,
    });
  });

  test("sorts by market_value desc", () => {
    const rows = buildFundPositionRows(
      [
        { account_name: "A", fund_code: "000001", holding_share: "10", holding_cost: "10", fund: { latest_nav: "1" } },
        { account_name: "B", fund_code: "000001", holding_share: "20", holding_cost: "20", fund: { latest_nav: "1" } },
      ],
      "000001",
      "1"
    );
    expect(rows.map((r) => r.account_name)).toEqual(["B", "A"]);
  });
});

describe("sortOperationsDesc", () => {
  test("sorts by operation_date desc, then created_at desc", () => {
    const input = [
      { id: "1", operation_date: "2024-01-01", created_at: "2024-01-01T10:00:00Z" },
      { id: "2", operation_date: "2024-01-02", created_at: "2024-01-02T09:00:00Z" },
      { id: "3", operation_date: "2024-01-02", created_at: "2024-01-02T10:00:00Z" },
    ];
    const out = sortOperationsDesc(input);
    expect(out.map((r) => r.id)).toEqual(["3", "2", "1"]);
  });
});

