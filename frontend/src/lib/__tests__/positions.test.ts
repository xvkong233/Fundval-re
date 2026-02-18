import { describe, expect, it } from "vitest";
import { pickDefaultChildAccountId } from "../positions";

describe("pickDefaultChildAccountId", () => {
  it("returns preferred id if it is a child account", () => {
    const accounts = [
      { id: "p1", parent: null },
      { id: "c1", parent: "p1" },
      { id: "c2", parent: "p1" },
    ];

    expect(pickDefaultChildAccountId(accounts as any, "c2")).toBe("c2");
  });

  it("falls back to first child account when preferred is not a child", () => {
    const accounts = [
      { id: "p1", parent: null },
      { id: "c1", parent: "p1" },
      { id: "c2", parent: "p1" },
    ];

    expect(pickDefaultChildAccountId(accounts as any, "p1")).toBe("c1");
  });

  it("returns null when no child accounts exist", () => {
    const accounts = [{ id: "p1", parent: null }];

    expect(pickDefaultChildAccountId(accounts as any, "whatever")).toBeNull();
  });
});

