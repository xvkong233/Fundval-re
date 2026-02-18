import { assertSameSchema } from "../diff.js";
import { getJson, postJson } from "../http.js";

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

  assertSameSchema(golden.json as any, candidate.json as any, "$");

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

  assertSameSchema(goldenOne.json as any, candidateOne.json as any, "$");

  // estimate: fund 不存在 -> 404 + {detail:"Not found."}
  const missingFundCode = "999999";
  const goldenEstimateMissing = await getJson(
    `${goldenBase}/api/funds/${encodeURIComponent(missingFundCode)}/estimate/`
  );
  const candidateEstimateMissing = await getJson(
    `${candidateBase}/api/funds/${encodeURIComponent(missingFundCode)}/estimate/`
  );
  if (goldenEstimateMissing.status !== candidateEstimateMissing.status) {
    throw new Error(
      `funds.estimate(404) 状态码不一致: golden=${goldenEstimateMissing.status} candidate=${candidateEstimateMissing.status}`
    );
  }
  assertSameSchema(goldenEstimateMissing.json as any, candidateEstimateMissing.json as any, "$");

  // accuracy: fund 不存在 -> 404 + {detail:"Not found."}
  const goldenAccMissing = await getJson(
    `${goldenBase}/api/funds/${encodeURIComponent(missingFundCode)}/accuracy/`
  );
  const candidateAccMissing = await getJson(
    `${candidateBase}/api/funds/${encodeURIComponent(missingFundCode)}/accuracy/`
  );
  if (goldenAccMissing.status !== candidateAccMissing.status) {
    throw new Error(
      `funds.accuracy(404) 状态码不一致: golden=${goldenAccMissing.status} candidate=${candidateAccMissing.status}`
    );
  }
  assertSameSchema(goldenAccMissing.json as any, candidateAccMissing.json as any, "$");

  // accuracy(success): 需要 seed 的 estimate_accuracy 数据；并且至少包含两个数据源，防止只覆盖“单一 key”情况
  const goldenAccOk = await getJson(`${goldenBase}/api/funds/${encodeURIComponent(firstCode)}/accuracy/?days=100`);
  const candidateAccOk = await getJson(`${candidateBase}/api/funds/${encodeURIComponent(firstCode)}/accuracy/?days=100`);
  if (goldenAccOk.status !== candidateAccOk.status) {
    throw new Error(
      `funds.accuracy(ok) 状态码不一致: golden=${goldenAccOk.status} candidate=${candidateAccOk.status}`
    );
  }
  if (goldenAccOk.status !== 200) {
    throw new Error(`funds.accuracy(ok) 状态码非 200: ${goldenAccOk.status}`);
  }
  assertSameSchema(goldenAccOk.json as any, candidateAccOk.json as any, "$");
  const goldenSources = Object.keys((goldenAccOk.json as any) ?? {});
  if (goldenSources.length < 2) {
    throw new Error(`funds.accuracy(ok) 期望至少 2 个 source，但得到 ${goldenSources.length}`);
  }
  if (!goldenSources.includes("eastmoney")) {
    throw new Error(`funds.accuracy(ok) 期望包含 source=eastmoney，但 sources=[${goldenSources.join(",")}]`);
  }

  // batch_estimate: 缺少 fund_codes -> 400 + {error:"缺少 fund_codes 参数"}
  const goldenBatchEstimateBad = await postJson(`${goldenBase}/api/funds/batch_estimate/`, {});
  const candidateBatchEstimateBad = await postJson(`${candidateBase}/api/funds/batch_estimate/`, {});
  if (goldenBatchEstimateBad.status !== candidateBatchEstimateBad.status) {
    throw new Error(
      `funds.batch_estimate(bad) 状态码不一致: golden=${goldenBatchEstimateBad.status} candidate=${candidateBatchEstimateBad.status}`
    );
  }
  assertSameSchema(goldenBatchEstimateBad.json as any, candidateBatchEstimateBad.json as any, "$");

  // batch_estimate: 空库 + 不存在 fund -> {code:{error:"基金不存在"}}
  const goldenBatchEstimateMissing = await postJson(`${goldenBase}/api/funds/batch_estimate/`, {
    fund_codes: [missingFundCode],
  });
  const candidateBatchEstimateMissing = await postJson(`${candidateBase}/api/funds/batch_estimate/`, {
    fund_codes: [missingFundCode],
  });
  if (goldenBatchEstimateMissing.status !== candidateBatchEstimateMissing.status) {
    throw new Error(
      `funds.batch_estimate(missing fund) 状态码不一致: golden=${goldenBatchEstimateMissing.status} candidate=${candidateBatchEstimateMissing.status}`
    );
  }
  assertSameSchema(goldenBatchEstimateMissing.json as any, candidateBatchEstimateMissing.json as any, "$");

  // batch_update_nav: 缺少 fund_codes -> 400
  const goldenBatchNavBad = await postJson(`${goldenBase}/api/funds/batch_update_nav/`, {});
  const candidateBatchNavBad = await postJson(`${candidateBase}/api/funds/batch_update_nav/`, {});
  if (goldenBatchNavBad.status !== candidateBatchNavBad.status) {
    throw new Error(
      `funds.batch_update_nav(bad) 状态码不一致: golden=${goldenBatchNavBad.status} candidate=${candidateBatchNavBad.status}`
    );
  }
  assertSameSchema(goldenBatchNavBad.json as any, candidateBatchNavBad.json as any, "$");

  // batch_update_nav: 空库 + 不存在 fund -> 返回空对象（Python 仅对存在的 fund 发起并发获取）
  const goldenBatchNavMissing = await postJson(`${goldenBase}/api/funds/batch_update_nav/`, {
    fund_codes: [missingFundCode],
  });
  const candidateBatchNavMissing = await postJson(`${candidateBase}/api/funds/batch_update_nav/`, {
    fund_codes: [missingFundCode],
  });
  if (goldenBatchNavMissing.status !== candidateBatchNavMissing.status) {
    throw new Error(
      `funds.batch_update_nav(missing fund) 状态码不一致: golden=${goldenBatchNavMissing.status} candidate=${candidateBatchNavMissing.status}`
    );
  }
  assertSameSchema(goldenBatchNavMissing.json as any, candidateBatchNavMissing.json as any, "$");

  // query_nav: fund 不存在 -> 404
  const goldenQueryNavMissing = await postJson(`${goldenBase}/api/funds/query_nav/`, {
    fund_code: missingFundCode,
    operation_date: "2024-01-15",
    before_15: true,
  });
  const candidateQueryNavMissing = await postJson(`${candidateBase}/api/funds/query_nav/`, {
    fund_code: missingFundCode,
    operation_date: "2024-01-15",
    before_15: true,
  });
  if (goldenQueryNavMissing.status !== candidateQueryNavMissing.status) {
    throw new Error(
      `funds.query_nav(404) 状态码不一致: golden=${goldenQueryNavMissing.status} candidate=${candidateQueryNavMissing.status}`
    );
  }
  assertSameSchema(goldenQueryNavMissing.json as any, candidateQueryNavMissing.json as any, "$");

  // query_nav(success history): 用 seed 的 nav_history 命中 history 分支，避免触发外网同步
  const seedFundCode = "000001";
  const goldenQueryNavOk = await postJson(`${goldenBase}/api/funds/query_nav/`, {
    fund_code: seedFundCode,
    operation_date: "2026-02-12",
    before_15: true,
  });
  const candidateQueryNavOk = await postJson(`${candidateBase}/api/funds/query_nav/`, {
    fund_code: seedFundCode,
    operation_date: "2026-02-12",
    before_15: true,
  });
  if (goldenQueryNavOk.status !== candidateQueryNavOk.status) {
    throw new Error(
      `funds.query_nav(ok) 状态码不一致: golden=${goldenQueryNavOk.status} candidate=${candidateQueryNavOk.status}`
    );
  }
  if (goldenQueryNavOk.status !== 200) {
    throw new Error(`funds.query_nav(ok) 状态码非 200: ${goldenQueryNavOk.status}`);
  }
  assertSameSchema(goldenQueryNavOk.json as any, candidateQueryNavOk.json as any, "$");
  const navDate = (goldenQueryNavOk.json as any)?.nav_date as string | undefined;
  if (navDate !== "2026-02-11") {
    throw new Error(`funds.query_nav(ok) 期望 nav_date=2026-02-11，但得到 ${String(navDate)}`);
  }
}
