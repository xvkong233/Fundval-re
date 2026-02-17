import { describe, expect, it } from "vitest";
import { pickDefaultParentAccountId } from "../accounts";

describe("pickDefaultParentAccountId", () => {
  it("returns default parent account id when present", () => {
    const accounts = [
      { id: "p1", parent: null, is_default: false },
      { id: "p2", parent: null, is_default: true },
      { id: "c1", parent: "p2", is_default: false },
    ];

    expect(pickDefaultParentAccountId(accounts as any)).toBe("p2");
  });

  it("returns first parent when no default present", () => {
    const accounts = [
      { id: "p1", parent: null, is_default: false },
      { id: "p2", parent: null, is_default: false },
    ];

    expect(pickDefaultParentAccountId(accounts as any)).toBe("p1");
  });

  it("returns null when no parent accounts exist", () => {
    const accounts = [
      { id: "c1", parent: "p1", is_default: false },
      { id: "c2", parent: "p1", is_default: false },
    ];

    expect(pickDefaultParentAccountId(accounts as any)).toBeNull();
  });
});

