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
}
