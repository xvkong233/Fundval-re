import { assertSameSchema } from "../diff.js";
import { getJson, postJson } from "../http.js";

export async function runPositionsHistory(
  goldenBase: string,
  candidateBase: string
): Promise<void> {
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
  if (goldenLogin.status !== 200) return;

  const goldenAccessToken = (goldenLogin.json as any)?.access_token as string;
  const candidateAccessToken = (candidateLogin.json as any)?.access_token as string;

  // 创建父/子账户
  const name = `acct_hist_${Date.now()}_${Math.floor(Math.random() * 1e6)}`;
  const goldenParent = await postJsonWithBearer(
    `${goldenBase}/api/accounts/`,
    goldenAccessToken,
    { name, parent: null, is_default: false }
  );
  const candidateParent = await postJsonWithBearer(
    `${candidateBase}/api/accounts/`,
    candidateAccessToken,
    { name, parent: null, is_default: false }
  );
  if (goldenParent.status !== candidateParent.status) {
    throw new Error(
      `accounts.create(for positions_history) 状态码不一致: golden=${goldenParent.status} candidate=${candidateParent.status}`
    );
  }
  if (goldenParent.status !== 201) return;
  const goldenParentId = (goldenParent.json as any)?.id as string;
  const candidateParentId = (candidateParent.json as any)?.id as string;

  const goldenChild = await postJsonWithBearer(
    `${goldenBase}/api/accounts/`,
    goldenAccessToken,
    { name: `${name}_child`, parent: goldenParentId, is_default: false }
  );
  const candidateChild = await postJsonWithBearer(
    `${candidateBase}/api/accounts/`,
    candidateAccessToken,
    { name: `${name}_child`, parent: candidateParentId, is_default: false }
  );
  if (goldenChild.status !== candidateChild.status) {
    throw new Error(
      `accounts.create(child for positions_history) 状态码不一致: golden=${goldenChild.status} candidate=${candidateChild.status}`
    );
  }
  if (goldenChild.status !== 201) return;
  const goldenChildId = (goldenChild.json as any)?.id as string;
  const candidateChildId = (candidateChild.json as any)?.id as string;

  // history missing account_id -> 400
  const goldenMissing = await getJsonWithBearer(
    `${goldenBase}/api/positions/history/`,
    goldenAccessToken
  );
  const candidateMissing = await getJsonWithBearer(
    `${candidateBase}/api/positions/history/`,
    candidateAccessToken
  );
  if (goldenMissing.status !== candidateMissing.status) {
    throw new Error(
      `positions.history(missing account_id) 状态码不一致: golden=${goldenMissing.status} candidate=${candidateMissing.status}`
    );
  }
  assertSameSchema(goldenMissing.json, candidateMissing.json, "$");

  // parent account -> 400
  const goldenParentBad = await getJsonWithBearer(
    `${goldenBase}/api/positions/history/?account_id=${encodeURIComponent(goldenParentId)}&days=7`,
    goldenAccessToken
  );
  const candidateParentBad = await getJsonWithBearer(
    `${candidateBase}/api/positions/history/?account_id=${encodeURIComponent(candidateParentId)}&days=7`,
    candidateAccessToken
  );
  if (goldenParentBad.status !== candidateParentBad.status) {
    throw new Error(
      `positions.history(parent account) 状态码不一致: golden=${goldenParentBad.status} candidate=${candidateParentBad.status}`
    );
  }
  assertSameSchema(goldenParentBad.json, candidateParentBad.json, "$");

  // 为子账户创建一条操作流水（依赖 seed 基金：000001；若未 seed，可能返回 400，仍做 schema 对照）
  const today = new Date().toISOString().slice(0, 10);
  const fundCode = "000001";
  const goldenOp = await postJsonWithBearer(
    `${goldenBase}/api/positions/operations/`,
    goldenAccessToken,
    {
      account: goldenChildId,
      fund_code: fundCode,
      operation_type: "BUY",
      operation_date: today,
      before_15: true,
      amount: "1000",
      share: "100",
      nav: "1.0000",
    }
  );
  const candidateOp = await postJsonWithBearer(
    `${candidateBase}/api/positions/operations/`,
    candidateAccessToken,
    {
      account: candidateChildId,
      fund_code: fundCode,
      operation_type: "BUY",
      operation_date: today,
      before_15: true,
      amount: "1000",
      share: "100",
      nav: "1.0000",
    }
  );
  if (goldenOp.status !== candidateOp.status) {
    throw new Error(
      `operations.create(for positions_history) 状态码不一致: golden=${goldenOp.status} candidate=${candidateOp.status}`
    );
  }

  // history ok: schema + 长度对照（只在返回 list 时检查）
  const goldenOk = await getJsonWithBearer(
    `${goldenBase}/api/positions/history/?account_id=${encodeURIComponent(goldenChildId)}&days=7`,
    goldenAccessToken
  );
  const candidateOk = await getJsonWithBearer(
    `${candidateBase}/api/positions/history/?account_id=${encodeURIComponent(candidateChildId)}&days=7`,
    candidateAccessToken
  );

  if (goldenOk.status !== candidateOk.status) {
    throw new Error(
      `positions.history(ok) 状态码不一致: golden=${goldenOk.status} candidate=${candidateOk.status}`
    );
  }
  assertSameSchema(goldenOk.json, candidateOk.json, "$");

  if (goldenOk.status === 200 && Array.isArray(goldenOk.json) && Array.isArray(candidateOk.json)) {
    if (goldenOk.json.length !== candidateOk.json.length) {
      throw new Error(
        `positions.history length 不一致: golden=${goldenOk.json.length} candidate=${candidateOk.json.length}`
      );
    }
  }
}

async function getJsonWithBearer(url: string, token: string) {
  const res = await fetch(url, {
    method: "GET",
    headers: { Accept: "application/json", Authorization: `Bearer ${token}` },
  });
  const text = await res.text();
  return { status: res.status, json: JSON.parse(text) };
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
  return { status: res.status, json: JSON.parse(text) };
}
