import { assertSameSchema } from "../diff.js";
import { getJson, postJson } from "../http.js";

export async function runNavHistory(goldenBase: string, candidateBase: string): Promise<void> {
  const golden = await getJson(`${goldenBase}/api/nav-history/`);
  const candidate = await getJson(`${candidateBase}/api/nav-history/`);

  if (golden.status !== candidate.status) {
    throw new Error(
      `nav-history.list 状态码不一致: golden=${golden.status} candidate=${candidate.status}`
    );
  }
  assertSameSchema(golden.json, candidate.json, "$");

  // list/batch_query(success): 需要 DB seed 的 fund_nav_history 数据；否则空数组会导致 schema 检查不深入
  if (process.env.ENABLE_DB_CASES === "true") {
    const fundCode = "000001";
    const startDate = "2026-03-10";
    const endDate = "2026-03-11";

    const goldenSeedList = await getJson(
      `${goldenBase}/api/nav-history/?fund_code=${encodeURIComponent(fundCode)}&start_date=${encodeURIComponent(
        startDate
      )}&end_date=${encodeURIComponent(endDate)}`
    );
    const candidateSeedList = await getJson(
      `${candidateBase}/api/nav-history/?fund_code=${encodeURIComponent(fundCode)}&start_date=${encodeURIComponent(
        startDate
      )}&end_date=${encodeURIComponent(endDate)}`
    );
    if (goldenSeedList.status !== candidateSeedList.status) {
      throw new Error(
        `nav-history.list(seed filter) 状态码不一致: golden=${goldenSeedList.status} candidate=${candidateSeedList.status}`
      );
    }
    if (goldenSeedList.status !== 200) {
      throw new Error(`nav-history.list(seed filter) 状态码非 200: ${goldenSeedList.status}`);
    }
    if (!Array.isArray(goldenSeedList.json) || goldenSeedList.json.length === 0) {
      throw new Error(`nav-history.list(seed filter) 期望非空数组，但得到 length=${(goldenSeedList.json as any)?.length}`);
    }
    assertSameSchema(goldenSeedList.json, candidateSeedList.json, "$");

    const goldenBatchSeed = await postJson(`${goldenBase}/api/nav-history/batch_query/`, {
      fund_codes: [fundCode],
      start_date: startDate,
      end_date: endDate,
    });
    const candidateBatchSeed = await postJson(`${candidateBase}/api/nav-history/batch_query/`, {
      fund_codes: [fundCode],
      start_date: startDate,
      end_date: endDate,
    });
    if (goldenBatchSeed.status !== candidateBatchSeed.status) {
      throw new Error(
        `nav-history.batch_query(seed ok) 状态码不一致: golden=${goldenBatchSeed.status} candidate=${candidateBatchSeed.status}`
      );
    }
    if (goldenBatchSeed.status !== 200) {
      throw new Error(`nav-history.batch_query(seed ok) 状态码非 200: ${goldenBatchSeed.status}`);
    }
    const goldenArr = (goldenBatchSeed.json as any)?.[fundCode];
    if (!Array.isArray(goldenArr) || goldenArr.length === 0) {
      throw new Error(`nav-history.batch_query(seed ok) 期望 ${fundCode} 对应非空数组`);
    }
    assertSameSchema(goldenBatchSeed.json, candidateBatchSeed.json, "$");
  }

  // retrieve missing
  const missingId = "00000000-0000-0000-0000-000000000000";
  const goldenMissing = await getJson(`${goldenBase}/api/nav-history/${missingId}/`);
  const candidateMissing = await getJson(`${candidateBase}/api/nav-history/${missingId}/`);
  if (goldenMissing.status !== candidateMissing.status) {
    throw new Error(
      `nav-history.retrieve(404) 状态码不一致: golden=${goldenMissing.status} candidate=${candidateMissing.status}`
    );
  }
  assertSameSchema(goldenMissing.json, candidateMissing.json, "$");

  // batch_query missing fund_codes
  const goldenBatchBad = await postJson(`${goldenBase}/api/nav-history/batch_query/`, {});
  const candidateBatchBad = await postJson(`${candidateBase}/api/nav-history/batch_query/`, {});
  if (goldenBatchBad.status !== candidateBatchBad.status) {
    throw new Error(
      `nav-history.batch_query(bad) 状态码不一致: golden=${goldenBatchBad.status} candidate=${candidateBatchBad.status}`
    );
  }
  assertSameSchema(goldenBatchBad.json, candidateBatchBad.json, "$");

  // batch_query ok: 不存在 fund_code -> {code: []}
  const missingFundCode = "999999";
  const goldenBatchOk = await postJson(`${goldenBase}/api/nav-history/batch_query/`, {
    fund_codes: [missingFundCode],
  });
  const candidateBatchOk = await postJson(`${candidateBase}/api/nav-history/batch_query/`, {
    fund_codes: [missingFundCode],
  });
  if (goldenBatchOk.status !== candidateBatchOk.status) {
    throw new Error(
      `nav-history.batch_query(ok missing fund) 状态码不一致: golden=${goldenBatchOk.status} candidate=${candidateBatchOk.status}`
    );
  }
  assertSameSchema(goldenBatchOk.json, candidateBatchOk.json, "$");

  // sync missing fund_codes
  const goldenSyncBad = await postJson(`${goldenBase}/api/nav-history/sync/`, {});
  const candidateSyncBad = await postJson(`${candidateBase}/api/nav-history/sync/`, {});
  if (goldenSyncBad.status !== candidateSyncBad.status) {
    throw new Error(
      `nav-history.sync(bad) 状态码不一致: golden=${goldenSyncBad.status} candidate=${candidateSyncBad.status}`
    );
  }
  assertSameSchema(goldenSyncBad.json, candidateSyncBad.json, "$");

  // sync >15 without auth => 403
  const fundCodes = Array.from({ length: 16 }, (_, i) => String(100000 + i));
  const goldenSyncForbidden = await postJson(`${goldenBase}/api/nav-history/sync/`, {
    fund_codes: fundCodes,
  });
  const candidateSyncForbidden = await postJson(`${candidateBase}/api/nav-history/sync/`, {
    fund_codes: fundCodes,
  });
  if (goldenSyncForbidden.status !== candidateSyncForbidden.status) {
    throw new Error(
      `nav-history.sync(>15 forbidden) 状态码不一致: golden=${goldenSyncForbidden.status} candidate=${candidateSyncForbidden.status}`
    );
  }
  assertSameSchema(goldenSyncForbidden.json, candidateSyncForbidden.json, "$");

  // sync >15 with invalid token:
  // DRF 会先做 JWTAuthentication；带无效 token 时应直接 401（而不是走“>15 需要管理员”分支）。
  const goldenSyncInvalidToken = await postJsonWithBearer(
    `${goldenBase}/api/nav-history/sync/`,
    "invalid-token-for-contract-tests",
    { fund_codes: fundCodes }
  );
  const candidateSyncInvalidToken = await postJsonWithBearer(
    `${candidateBase}/api/nav-history/sync/`,
    "invalid-token-for-contract-tests",
    { fund_codes: fundCodes }
  );
  if (goldenSyncInvalidToken.status !== candidateSyncInvalidToken.status) {
    throw new Error(
      `nav-history.sync(>15 invalid token) 状态码不一致: golden=${goldenSyncInvalidToken.status} candidate=${candidateSyncInvalidToken.status}`
    );
  }
  assertSameSchema(goldenSyncInvalidToken.json, candidateSyncInvalidToken.json, "$");
}

async function postJsonWithBearer(url: string, token: string, body: unknown) {
  const res = await fetch(url, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      Accept: "application/json",
      Authorization: `Bearer ${token}`,
    },
    body: JSON.stringify(body),
  });
  const text = await res.text();
  try {
    return { status: res.status, json: JSON.parse(text) };
  } catch {
    throw new Error(`非 JSON 响应: ${url} status=${res.status} body=${text.slice(0, 200)}`);
  }
}
