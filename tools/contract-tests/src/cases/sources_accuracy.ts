import { assertSameSchema } from "../diff.js";
import { getJson } from "../http.js";

export async function runSourcesAccuracy(goldenBase: string, candidateBase: string): Promise<void> {
  const golden = await getJson(`${goldenBase}/api/sources/eastmoney/accuracy/`);
  const candidate = await getJson(`${candidateBase}/api/sources/eastmoney/accuracy/`);

  if (golden.status !== candidate.status) {
    throw new Error(
      `sources.accuracy 状态码不一致: golden=${golden.status} candidate=${candidate.status}`
    );
  }
  assertSameSchema(golden.json, candidate.json, "$");
}

