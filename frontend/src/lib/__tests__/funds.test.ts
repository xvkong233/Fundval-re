import { describe, expect, test } from "vitest";

import { mergeBatchEstimate, mergeBatchNav, normalizeFundList } from "../funds";

describe("funds helpers", () => {
  test("normalizeFundList supports paginated response", () => {
    const json = {
      count: 2,
      results: [{ fund_code: "000001", fund_name: "A" }, { fund_code: "000002", fund_name: "B" }],
    };
    const out = normalizeFundList(json as any);
    expect(out.total).toBe(2);
    expect(out.results).toHaveLength(2);
    expect(out.results[0].fund_code).toBe("000001");
  });

  test("normalizeFundList supports array response", () => {
    const json = [{ fund_code: "000001", fund_name: "A" }];
    const out = normalizeFundList(json as any);
    expect(out.total).toBe(1);
    expect(out.results).toHaveLength(1);
  });

  test("mergeBatchNav merges latest nav fields", () => {
    const funds = [{ fund_code: "000001", fund_name: "A" }];
    const batch = { "000001": { latest_nav: "1.2345", latest_nav_date: "2026-02-17" } };
    const out = mergeBatchNav(funds as any, batch as any);
    expect(out[0].latest_nav).toBe("1.2345");
    expect(out[0].latest_nav_date).toBe("2026-02-17");
  });

  test("mergeBatchEstimate merges estimate fields", () => {
    const funds = [{ fund_code: "000001", fund_name: "A" }];
    const batch = { "000001": { estimate_nav: "1.0000", estimate_growth: "0.12" } };
    const out = mergeBatchEstimate(funds as any, batch as any);
    expect(out[0].estimate_nav).toBe("1.0000");
    expect(out[0].estimate_growth).toBe("0.12");
  });
});

