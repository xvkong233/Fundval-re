import { describe, expect, test } from "vitest";

import { clampIndex, getFundCodes, moveInArray } from "../watchlists";

describe("watchlists order helpers", () => {
  test("clampIndex clamps into array bounds", () => {
    expect(clampIndex(-1, 3)).toBe(0);
    expect(clampIndex(0, 3)).toBe(0);
    expect(clampIndex(2, 3)).toBe(2);
    expect(clampIndex(3, 3)).toBe(2);
  });

  test("moveInArray moves item by index", () => {
    expect(moveInArray(["a", "b", "c"], 2, 0)).toEqual(["c", "a", "b"]);
    expect(moveInArray(["a", "b", "c"], 0, 2)).toEqual(["b", "c", "a"]);
  });

  test("getFundCodes returns codes in order", () => {
    expect(getFundCodes([{ fund_code: "000002" }, { fund_code: "000001" }])).toEqual([
      "000002",
      "000001",
    ]);
  });
});

