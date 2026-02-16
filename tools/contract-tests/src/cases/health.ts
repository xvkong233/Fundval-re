import { assertSameShape } from "../diff.js";
import { getJson } from "../http.js";

export async function runHealth(goldenBase: string, candidateBase: string): Promise<void> {
  const golden = await getJson(`${goldenBase}/api/health/`);
  const candidate = await getJson(`${candidateBase}/api/health/`);

  if (golden.status !== candidate.status) {
    throw new Error(`状态码不一致: golden=${golden.status} candidate=${candidate.status}`);
  }

  // 允许 database 值不同（但必须存在且是 string）
  assertSameShape(golden.json, candidate.json, "$", {
    allowValueDiffAtPaths: new Set(["$.database"])
  });

  const candidateDatabase = (candidate.json as any)?.database;
  if (typeof candidateDatabase !== "string") {
    throw new Error(`candidate.database 不是 string: ${String(candidateDatabase)}`);
  }
}

