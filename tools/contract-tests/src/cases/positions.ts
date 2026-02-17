import { assertSameSchema } from "../diff.js";
import { getJson, postJson } from "../http.js";

export async function runPositions(goldenBase: string, candidateBase: string): Promise<void> {
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

  // 为 operations.create 的错误分支准备一个“子账户”
  const name = `acct_pos_${Date.now()}_${Math.floor(Math.random() * 1e6)}`;
  const goldenParent = await postJsonWithBearer(`${goldenBase}/api/accounts/`, goldenAccessToken, {
    name,
    parent: null,
    is_default: false,
  });
  const candidateParent = await postJsonWithBearer(`${candidateBase}/api/accounts/`, candidateAccessToken, {
    name,
    parent: null,
    is_default: false,
  });
  if (goldenParent.status !== candidateParent.status) {
    throw new Error(
      `accounts.create(for positions) 状态码不一致: golden=${goldenParent.status} candidate=${candidateParent.status}`
    );
  }
  if (goldenParent.status !== 201) {
    throw new Error(`accounts.create(for positions) 状态码非 201: ${goldenParent.status}`);
  }
  const goldenParentId = (goldenParent.json as any)?.id as string;
  const candidateParentId = (candidateParent.json as any)?.id as string;

  const goldenChild = await postJsonWithBearer(`${goldenBase}/api/accounts/`, goldenAccessToken, {
    name: `${name}_child`,
    parent: goldenParentId,
    is_default: false,
  });
  const candidateChild = await postJsonWithBearer(`${candidateBase}/api/accounts/`, candidateAccessToken, {
    name: `${name}_child`,
    parent: candidateParentId,
    is_default: false,
  });
  if (goldenChild.status !== candidateChild.status) {
    throw new Error(
      `accounts.create(child for positions) 状态码不一致: golden=${goldenChild.status} candidate=${candidateChild.status}`
    );
  }
  if (goldenChild.status !== 201) {
    throw new Error(`accounts.create(child for positions) 状态码非 201: ${goldenChild.status}`);
  }
  const goldenChildId = (goldenChild.json as any)?.id as string;
  const candidateChildId = (candidateChild.json as any)?.id as string;

  // positions.list
  const goldenPositions = await getJsonWithBearer(`${goldenBase}/api/positions/`, goldenAccessToken);
  const candidatePositions = await getJsonWithBearer(
    `${candidateBase}/api/positions/`,
    candidateAccessToken
  );
  if (goldenPositions.status !== candidatePositions.status) {
    throw new Error(
      `positions.list 状态码不一致: golden=${goldenPositions.status} candidate=${candidatePositions.status}`
    );
  }
  if (goldenPositions.status !== 200) {
    throw new Error(`positions.list 状态码非 200: ${goldenPositions.status}`);
  }
  assertSameSchema(goldenPositions.json, candidatePositions.json, "$");

  // positions.list filtered by account
  const goldenFiltered = await getJsonWithBearer(
    `${goldenBase}/api/positions/?account=${encodeURIComponent(goldenChildId)}`,
    goldenAccessToken
  );
  const candidateFiltered = await getJsonWithBearer(
    `${candidateBase}/api/positions/?account=${encodeURIComponent(candidateChildId)}`,
    candidateAccessToken
  );
  if (goldenFiltered.status !== candidateFiltered.status) {
    throw new Error(
      `positions.list(filter) 状态码不一致: golden=${goldenFiltered.status} candidate=${candidateFiltered.status}`
    );
  }
  assertSameSchema(goldenFiltered.json, candidateFiltered.json, "$");

  // operations.list
  const goldenOps = await getJsonWithBearer(`${goldenBase}/api/positions/operations/`, goldenAccessToken);
  const candidateOps = await getJsonWithBearer(
    `${candidateBase}/api/positions/operations/`,
    candidateAccessToken
  );
  if (goldenOps.status !== candidateOps.status) {
    throw new Error(
      `operations.list 状态码不一致: golden=${goldenOps.status} candidate=${candidateOps.status}`
    );
  }
  if (goldenOps.status !== 200) {
    throw new Error(`operations.list 状态码非 200: ${goldenOps.status}`);
  }
  assertSameSchema(goldenOps.json, candidateOps.json, "$");

  // operations.create: fund 不存在 -> 400 + {fund_code:["基金不存在"]}
  const missingFundCode = `no_such_${Date.now()}`;
  const goldenCreateMissing = await postJsonWithBearer(
    `${goldenBase}/api/positions/operations/`,
    goldenAccessToken,
    {
      account: goldenChildId,
      fund_code: missingFundCode,
      operation_type: "BUY",
      operation_date: "2024-02-11",
      before_15: true,
      amount: "1000",
      share: "100",
      nav: "10",
    }
  );
  const candidateCreateMissing = await postJsonWithBearer(
    `${candidateBase}/api/positions/operations/`,
    candidateAccessToken,
    {
      account: candidateChildId,
      fund_code: missingFundCode,
      operation_type: "BUY",
      operation_date: "2024-02-11",
      before_15: true,
      amount: "1000",
      share: "100",
      nav: "10",
    }
  );
  if (goldenCreateMissing.status !== candidateCreateMissing.status) {
    throw new Error(
      `operations.create(missing fund) 状态码不一致: golden=${goldenCreateMissing.status} candidate=${candidateCreateMissing.status}`
    );
  }
  assertSameSchema(goldenCreateMissing.json, candidateCreateMissing.json, "$");

  // positions.retrieve(not found)
  const missingId = "00000000-0000-0000-0000-000000000000";
  const goldenMissing = await getJsonWithBearer(
    `${goldenBase}/api/positions/${missingId}/`,
    goldenAccessToken
  );
  const candidateMissing = await getJsonWithBearer(
    `${candidateBase}/api/positions/${missingId}/`,
    candidateAccessToken
  );
  if (goldenMissing.status !== candidateMissing.status) {
    throw new Error(
      `positions.retrieve(404) 状态码不一致: golden=${goldenMissing.status} candidate=${candidateMissing.status}`
    );
  }
  assertSameSchema(goldenMissing.json, candidateMissing.json, "$");

  // operations.retrieve(not found)
  const goldenMissingOp = await getJsonWithBearer(
    `${goldenBase}/api/positions/operations/${missingId}/`,
    goldenAccessToken
  );
  const candidateMissingOp = await getJsonWithBearer(
    `${candidateBase}/api/positions/operations/${missingId}/`,
    candidateAccessToken
  );
  if (goldenMissingOp.status !== candidateMissingOp.status) {
    throw new Error(
      `operations.retrieve(404) 状态码不一致: golden=${goldenMissingOp.status} candidate=${candidateMissingOp.status}`
    );
  }
  assertSameSchema(goldenMissingOp.json, candidateMissingOp.json, "$");

  // positions.recalculate (admin-only; admin 用户应成功)
  const goldenRecalc = await postJsonWithBearer(`${goldenBase}/api/positions/recalculate/`, goldenAccessToken, {});
  const candidateRecalc = await postJsonWithBearer(
    `${candidateBase}/api/positions/recalculate/`,
    candidateAccessToken,
    {}
  );
  if (goldenRecalc.status !== candidateRecalc.status) {
    throw new Error(
      `positions.recalculate 状态码不一致: golden=${goldenRecalc.status} candidate=${candidateRecalc.status}`
    );
  }
  assertSameSchema(goldenRecalc.json, candidateRecalc.json, "$");
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
