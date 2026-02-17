import { describe, expect, test } from "vitest";

import { pickDefaultWatchlistId } from "../watchlists";

describe("pickDefaultWatchlistId", () => {
  test("returns null for empty list", () => {
    expect(pickDefaultWatchlistId([])).toBeNull();
  });

  test("returns first id for non-empty list", () => {
    expect(pickDefaultWatchlistId([{ id: "a" }, { id: "b" }])).toBe("a");
  });
});

