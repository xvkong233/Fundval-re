import { describe, expect, test } from "vitest";

import { getBootstrapInitError, maskBootstrapKey } from "../bootstrapInit";

describe("getBootstrapInitError", () => {
  test("maps 410 to already_initialized", () => {
    const error = { response: { status: 410, data: { error: "系统已初始化，接口失效" } } };
    expect(getBootstrapInitError(error)).toMatchObject({
      kind: "already_initialized",
      status: 410,
      message: "系统已初始化，接口失效",
    });
  });

  test("maps 400 to invalid_key when backend returns error", () => {
    const error = { response: { status: 400, data: { error: "密钥无效" } } };
    expect(getBootstrapInitError(error)).toMatchObject({
      kind: "invalid_key",
      status: 400,
      message: "密钥无效",
    });
  });

  test("maps missing response to network error", () => {
    const error = new Error("Network Error");
    expect(getBootstrapInitError(error)).toMatchObject({
      kind: "network",
      message: expect.stringContaining("无法连接"),
    });
  });

  test("falls back to unknown with best-effort message", () => {
    const error = { response: { status: 500, data: { error: "boom" } } };
    expect(getBootstrapInitError(error)).toMatchObject({
      kind: "unknown",
      status: 500,
      message: "boom",
    });
  });
});

describe("maskBootstrapKey", () => {
  test("returns empty string for empty input", () => {
    expect(maskBootstrapKey("")).toBe("");
  });

  test("masks short keys", () => {
    expect(maskBootstrapKey("abcd")).toBe("****");
  });

  test("shows first/last 4 chars for long keys", () => {
    expect(maskBootstrapKey("0123456789abcdef")).toBe("0123…cdef");
  });
});
