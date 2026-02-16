import { getJson } from "../http.js";

export async function runSources(goldenBase: string, candidateBase: string): Promise<void> {
  const golden = await getJson(`${goldenBase}/api/sources/`);
  const candidate = await getJson(`${candidateBase}/api/sources/`);

  if (golden.status !== candidate.status) {
    throw new Error(`sources 状态码不一致: golden=${golden.status} candidate=${candidate.status}`);
  }
  if (golden.status !== 200) {
    throw new Error(`sources 状态码非 200: ${golden.status}`);
  }

  const goldenNames = (golden.json as any[]).map((x) => x?.name).sort();
  const candidateNames = (candidate.json as any[]).map((x) => x?.name).sort();

  if (JSON.stringify(goldenNames) !== JSON.stringify(candidateNames)) {
    throw new Error(`sources 列表不一致: golden=${goldenNames.join(",")} candidate=${candidateNames.join(",")}`);
  }

  if (!candidateNames.includes("eastmoney")) {
    throw new Error("sources 必须包含 eastmoney");
  }
}

