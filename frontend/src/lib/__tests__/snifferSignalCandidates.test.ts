import { describe, expect, test } from "vitest";
import { selectSnifferSignalCandidateCodes } from "../snifferSignalCandidates";

describe("selectSnifferSignalCandidateCodes", () => {
  test("dedupes, drops empty codes, and prioritizes stars/drawdown", () => {
    const items = [
      { fund_code: "", star_count: 5, max_drawdown: "10" },
      { fund_code: "A", star_count: 5, max_drawdown: "2" },
      { fund_code: "B", star_count: 2, max_drawdown: "30" },
      { fund_code: "C", star_count: 1, max_drawdown: "5" },
      { fund_code: "A", star_count: 4, max_drawdown: "20" },
    ];

    expect(selectSnifferSignalCandidateCodes(items, 50)).toEqual(["A", "B", "C"]);
    expect(selectSnifferSignalCandidateCodes(items, 2)).toEqual(["A", "B"]);
  });
});

