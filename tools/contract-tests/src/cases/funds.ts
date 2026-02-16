import { getJson } from "../http.js";

export async function runFunds(goldenBase: string, candidateBase: string): Promise<void> {
  if (process.env.ENABLE_DB_CASES !== "true") return;

  const golden = await getJson(`${goldenBase}/api/funds/?page=1&page_size=5`);
  const candidate = await getJson(`${candidateBase}/api/funds/?page=1&page_size=5`);

  if (golden.status !== candidate.status) {
    throw new Error(`funds.list 状态码不一致: golden=${golden.status} candidate=${candidate.status}`);
  }
  if (golden.status !== 200) {
    throw new Error(`funds.list 状态码非 200: ${golden.status}`);
  }

  const goldenStr = JSON.stringify(golden.json);
  const candidateStr = JSON.stringify(candidate.json);
  if (goldenStr !== candidateStr) {
    throw new Error("funds.list 响应不一致（JSON stringify 对比失败）");
  }

  const firstCode = (candidate.json as any)?.results?.[0]?.fund_code as string | undefined;
  if (!firstCode) return;

  const goldenOne = await getJson(`${goldenBase}/api/funds/${encodeURIComponent(firstCode)}/`);
  const candidateOne = await getJson(`${candidateBase}/api/funds/${encodeURIComponent(firstCode)}/`);

  if (goldenOne.status !== candidateOne.status) {
    throw new Error(
      `funds.retrieve 状态码不一致: golden=${goldenOne.status} candidate=${candidateOne.status}`
    );
  }
  if (goldenOne.status !== 200) {
    throw new Error(`funds.retrieve 状态码非 200: ${goldenOne.status}`);
  }

  const g1 = JSON.stringify(goldenOne.json);
  const c1 = JSON.stringify(candidateOne.json);
  if (g1 !== c1) {
    throw new Error("funds.retrieve 响应不一致（JSON stringify 对比失败）");
  }
}
