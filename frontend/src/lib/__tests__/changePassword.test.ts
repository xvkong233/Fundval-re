import { describe, expect, test } from "vitest";

import { getChangePasswordErrorMessage } from "../changePassword";

describe("getChangePasswordErrorMessage", () => {
  test("returns backend error when provided", () => {
    const error = { response: { status: 400, data: { error: "旧密码错误" } } };
    expect(getChangePasswordErrorMessage(error)).toBe("旧密码错误");
  });

  test("returns generic message on 401", () => {
    const error = { response: { status: 401, data: { detail: "Authentication credentials were not provided." } } };
    expect(getChangePasswordErrorMessage(error)).toContain("登录");
  });

  test("returns network message when response missing", () => {
    expect(getChangePasswordErrorMessage(new Error("Network Error"))).toContain("无法连接");
  });
});

