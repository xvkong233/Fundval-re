import { assertSameShape } from "../diff.js";
import { getJson, postJson } from "../http.js";

export async function runAuth(goldenBase: string, candidateBase: string): Promise<void> {
  const goldenHealth = await getJson(`${goldenBase}/api/health/`);
  const candidateHealth = await getJson(`${candidateBase}/api/health/`);

  const goldenInit = (goldenHealth.json as any)?.system_initialized;
  const candidateInit = (candidateHealth.json as any)?.system_initialized;
  if (goldenInit !== true || candidateInit !== true) return;

  const goldenLogin = await postJson(`${goldenBase}/api/auth/login`, {
    username: "admin",
    password: "admin123",
  });
  const candidateLogin = await postJson(`${candidateBase}/api/auth/login`, {
    username: "admin",
    password: "admin123",
  });

  if (goldenLogin.status !== candidateLogin.status) {
    throw new Error(
      `login 状态码不一致: golden=${goldenLogin.status} candidate=${candidateLogin.status}`
    );
  }
  assertSameShape(goldenLogin.json, candidateLogin.json, "$", {
    allowValueDiffAtPaths: new Set(["$.access_token", "$.refresh_token", "$.user.id"]),
  });

  const goldenRefreshToken = (goldenLogin.json as any)?.refresh_token as string;
  const candidateRefreshToken = (candidateLogin.json as any)?.refresh_token as string;

  const goldenRefresh = await postJson(`${goldenBase}/api/auth/refresh`, {
    refresh_token: goldenRefreshToken,
  });
  const candidateRefresh = await postJson(`${candidateBase}/api/auth/refresh`, {
    refresh_token: candidateRefreshToken,
  });

  if (goldenRefresh.status !== candidateRefresh.status) {
    throw new Error(
      `refresh 状态码不一致: golden=${goldenRefresh.status} candidate=${candidateRefresh.status}`
    );
  }
  assertSameShape(goldenRefresh.json, candidateRefresh.json, "$", {
    allowValueDiffAtPaths: new Set(["$.access_token"]),
  });

  const goldenAccessToken = (goldenLogin.json as any)?.access_token as string;
  const candidateAccessToken = (candidateLogin.json as any)?.access_token as string;

  const goldenMe = await fetchJsonWithBearer(`${goldenBase}/api/auth/me`, goldenAccessToken);
  const candidateMe = await fetchJsonWithBearer(`${candidateBase}/api/auth/me`, candidateAccessToken);

  if (goldenMe.status !== candidateMe.status) {
    throw new Error(`me 状态码不一致: golden=${goldenMe.status} candidate=${candidateMe.status}`);
  }
  assertSameShape(goldenMe.json, candidateMe.json, "$", {
    allowValueDiffAtPaths: new Set(["$.id", "$.created_at"]),
  });
}

async function fetchJsonWithBearer(url: string, token: string) {
  const res = await fetch(url, {
    method: "GET",
    headers: { Accept: "application/json", Authorization: `Bearer ${token}` },
  });
  const text = await res.text();
  return { status: res.status, json: JSON.parse(text) };
}

