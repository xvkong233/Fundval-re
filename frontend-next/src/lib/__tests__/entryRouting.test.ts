import { describe, expect, test } from "vitest";

import { decideEntryRoute, shouldRedirectAuthedPublicPage } from "../entryRouting";

describe("decideEntryRoute", () => {
  test("routes to /initialize when system not initialized", () => {
    expect(decideEntryRoute({ system_initialized: false }, false)).toBe("/initialize");
  });

  test("routes to /dashboard when initialized and authed", () => {
    expect(decideEntryRoute({ system_initialized: true }, true)).toBe("/dashboard");
  });

  test("routes to /login when initialized but not authed", () => {
    expect(decideEntryRoute({ system_initialized: true }, false)).toBe("/login");
  });

  test("defaults to /login when health is missing fields", () => {
    expect(decideEntryRoute({}, false)).toBe("/login");
  });
});

describe("shouldRedirectAuthedPublicPage", () => {
  test("redirects when authed", () => {
    expect(shouldRedirectAuthedPublicPage(true)).toBe(true);
  });

  test("does not redirect when not authed", () => {
    expect(shouldRedirectAuthedPublicPage(false)).toBe(false);
  });
});

