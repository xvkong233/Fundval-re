import { assertSameShape } from "../diff.js";
import { getJson, postJson } from "../http.js";

export async function runAccounts(goldenBase: string, candidateBase: string): Promise<void> {
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

  const name = `acct_${Date.now()}_${Math.floor(Math.random() * 1e6)}`;

  const goldenCreateParent = await postJsonWithBearer(
    `${goldenBase}/api/accounts/`,
    goldenAccessToken,
    {
      name,
      parent: null,
      is_default: true,
    }
  );
  const candidateCreateParent = await postJsonWithBearer(
    `${candidateBase}/api/accounts/`,
    candidateAccessToken,
    {
      name,
      parent: null,
      is_default: true,
    }
  );

  if (goldenCreateParent.status !== candidateCreateParent.status) {
    throw new Error(
      `accounts.create(parent) 状态码不一致: golden=${goldenCreateParent.status} candidate=${candidateCreateParent.status}`
    );
  }

  assertSameShape(
    normalizeAccount(goldenCreateParent.json),
    normalizeAccount(candidateCreateParent.json),
    "$",
    {}
  );

  const goldenParentId = (goldenCreateParent.json as any)?.id as string;
  const candidateParentId = (candidateCreateParent.json as any)?.id as string;

  const childName = `${name}_child`;
  const goldenCreateChild = await postJsonWithBearer(`${goldenBase}/api/accounts/`, goldenAccessToken, {
    name: childName,
    parent: goldenParentId,
    is_default: false,
  });
  const candidateCreateChild = await postJsonWithBearer(
    `${candidateBase}/api/accounts/`,
    candidateAccessToken,
    {
      name: childName,
      parent: candidateParentId,
      is_default: false,
    }
  );
  if (goldenCreateChild.status !== candidateCreateChild.status) {
    throw new Error(
      `accounts.create(child) 状态码不一致: golden=${goldenCreateChild.status} candidate=${candidateCreateChild.status}`
    );
  }
  assertSameShape(
    normalizeAccount(goldenCreateChild.json),
    normalizeAccount(candidateCreateChild.json),
    "$",
    {}
  );

  const goldenChildId = (goldenCreateChild.json as any)?.id as string;
  const candidateChildId = (candidateCreateChild.json as any)?.id as string;

  const goldenList = await getJsonWithBearer(`${goldenBase}/api/accounts/`, goldenAccessToken);
  const candidateList = await getJsonWithBearer(`${candidateBase}/api/accounts/`, candidateAccessToken);
  if (goldenList.status !== candidateList.status) {
    throw new Error(
      `accounts.list 状态码不一致: golden=${goldenList.status} candidate=${candidateList.status}`
    );
  }
  const goldenPair = extractParentAndChildFromList(goldenList.json, goldenParentId, goldenChildId);
  const candidatePair = extractParentAndChildFromList(
    candidateList.json,
    candidateParentId,
    candidateChildId
  );

  assertSameShape(normalizeAccount(goldenPair.parent), normalizeAccount(candidatePair.parent), "$", {});
  assertSameShape(normalizeAccount(goldenPair.child), normalizeAccount(candidatePair.child), "$", {});

  const goldenPos = await getJsonWithBearer(
    `${goldenBase}/api/accounts/${goldenParentId}/positions/`,
    goldenAccessToken
  );
  const candidatePos = await getJsonWithBearer(
    `${candidateBase}/api/accounts/${candidateParentId}/positions/`,
    candidateAccessToken
  );
  if (goldenPos.status !== candidatePos.status) {
    throw new Error(
      `accounts.positions 状态码不一致: golden=${goldenPos.status} candidate=${candidatePos.status}`
    );
  }
  assertSameShape(goldenPos.json, candidatePos.json, "$", {});
  if (!Array.isArray(goldenPos.json) || !Array.isArray(candidatePos.json)) {
    throw new Error("accounts.positions 响应不是数组");
  }
  if (goldenPos.json.length !== candidatePos.json.length) {
    throw new Error(
      `accounts.positions 数量不一致: golden=${goldenPos.json.length} candidate=${candidatePos.json.length}`
    );
  }

  const goldenParentDetail = await getJsonWithBearer(
    `${goldenBase}/api/accounts/${goldenParentId}/`,
    goldenAccessToken
  );
  const candidateParentDetail = await getJsonWithBearer(
    `${candidateBase}/api/accounts/${candidateParentId}/`,
    candidateAccessToken
  );
  if (goldenParentDetail.status !== candidateParentDetail.status) {
    throw new Error(
      `accounts.retrieve(parent) 状态码不一致: golden=${goldenParentDetail.status} candidate=${candidateParentDetail.status}`
    );
  }
  const goldenParentObj = goldenParentDetail.json as any;
  const candidateParentObj = candidateParentDetail.json as any;
  if (!Array.isArray(goldenParentObj?.children) || !Array.isArray(candidateParentObj?.children)) {
    throw new Error("accounts.retrieve(parent) children 不是数组");
  }
  assertSameShape(normalizeAccount(goldenParentObj), normalizeAccount(candidateParentObj), "$", {});

  const goldenChildDetail = await getJsonWithBearer(
    `${goldenBase}/api/accounts/${goldenChildId}/`,
    goldenAccessToken
  );
  const candidateChildDetail = await getJsonWithBearer(
    `${candidateBase}/api/accounts/${candidateChildId}/`,
    candidateAccessToken
  );
  if (goldenChildDetail.status !== candidateChildDetail.status) {
    throw new Error(
      `accounts.retrieve(child) 状态码不一致: golden=${goldenChildDetail.status} candidate=${candidateChildDetail.status}`
    );
  }
  const goldenChildObj = goldenChildDetail.json as any;
  const candidateChildObj = candidateChildDetail.json as any;
  if ("children" in goldenChildObj || "children" in candidateChildObj) {
    throw new Error("accounts.retrieve(child) 子账户不应包含 children 字段");
  }
  assertSameShape(normalizeAccount(goldenChildObj), normalizeAccount(candidateChildObj), "$", {});
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

function normalizeAccount(value: any): any {
  if (value === null || typeof value !== "object") return value;
  const obj = { ...value };
  if (typeof obj.id === "string") obj.id = "uuid";
  if (typeof obj.created_at === "string") obj.created_at = "ts";
  if (typeof obj.updated_at === "string") obj.updated_at = "ts";
  if (typeof obj.parent === "string") obj.parent = "uuid";
  if (Array.isArray(obj.children)) obj.children = obj.children.map(normalizeAccount);
  return obj;
}

function normalizeAccountList(value: any): any {
  if (!Array.isArray(value)) return value;
  return value.map(normalizeAccount);
}

function extractParentAndChildFromList(list: any, parentId: string, childId: string) {
  if (!Array.isArray(list)) throw new Error("accounts.list 响应不是数组");

  const parent = list.find((a: any) => a?.id === parentId);
  const child = list.find((a: any) => a?.id === childId);

  if (!parent) throw new Error("accounts.list 未包含父账户");
  if (!child) throw new Error("accounts.list 未包含子账户");

  if (!Array.isArray(parent.children)) {
    throw new Error("accounts.list 父账户缺少 children 数组");
  }
  const childInParent = parent.children.find((c: any) => c?.id === childId);
  if (!childInParent) throw new Error("accounts.list 父账户 children 未包含子账户");

  if ("children" in child) throw new Error("accounts.list 子账户不应包含 children 字段");

  return { parent, child };
}
