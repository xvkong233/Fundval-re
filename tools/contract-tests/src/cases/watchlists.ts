import { assertSameShape, assertSameSchema } from "../diff.js";
import { getJson, postJson } from "../http.js";

export async function runWatchlists(goldenBase: string, candidateBase: string): Promise<void> {
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

  const name = `wl_${Date.now()}_${Math.floor(Math.random() * 1e6)}`;

  const goldenCreate = await postJsonWithBearer(`${goldenBase}/api/watchlists/`, goldenAccessToken, {
    name,
  });
  const candidateCreate = await postJsonWithBearer(
    `${candidateBase}/api/watchlists/`,
    candidateAccessToken,
    { name }
  );
  if (goldenCreate.status !== candidateCreate.status) {
    throw new Error(
      `watchlists.create 状态码不一致: golden=${goldenCreate.status} candidate=${candidateCreate.status}`
    );
  }
  if (goldenCreate.status !== 201) {
    assertSameSchema(goldenCreate.json, candidateCreate.json, "$");
    return;
  }

  assertSameShape(normalizeWatchlist(goldenCreate.json), normalizeWatchlist(candidateCreate.json), "$", {});

  const goldenId = (goldenCreate.json as any)?.id as string;
  const candidateId = (candidateCreate.json as any)?.id as string;

  const goldenList = await getJsonWithBearer(`${goldenBase}/api/watchlists/`, goldenAccessToken);
  const candidateList = await getJsonWithBearer(`${candidateBase}/api/watchlists/`, candidateAccessToken);
  if (goldenList.status !== candidateList.status) {
    throw new Error(
      `watchlists.list 状态码不一致: golden=${goldenList.status} candidate=${candidateList.status}`
    );
  }
  assertSameSchema(goldenList.json, candidateList.json, "$");

  const goldenInList = findById(goldenList.json, goldenId);
  const candidateInList = findById(candidateList.json, candidateId);
  assertSameShape(
    normalizeWatchlist(goldenInList),
    normalizeWatchlist(candidateInList),
    "$",
    {}
  );

  const goldenDetail = await getJsonWithBearer(`${goldenBase}/api/watchlists/${goldenId}/`, goldenAccessToken);
  const candidateDetail = await getJsonWithBearer(
    `${candidateBase}/api/watchlists/${candidateId}/`,
    candidateAccessToken
  );
  if (goldenDetail.status !== candidateDetail.status) {
    throw new Error(
      `watchlists.retrieve 状态码不一致: golden=${goldenDetail.status} candidate=${candidateDetail.status}`
    );
  }
  assertSameShape(normalizeWatchlist(goldenDetail.json), normalizeWatchlist(candidateDetail.json), "$", {});

  const newName = `${name}_new`;
  const goldenPatch = await patchJsonWithBearer(
    `${goldenBase}/api/watchlists/${goldenId}/`,
    goldenAccessToken,
    { name: newName }
  );
  const candidatePatch = await patchJsonWithBearer(
    `${candidateBase}/api/watchlists/${candidateId}/`,
    candidateAccessToken,
    { name: newName }
  );
  if (goldenPatch.status !== candidatePatch.status) {
    throw new Error(
      `watchlists.patch 状态码不一致: golden=${goldenPatch.status} candidate=${candidatePatch.status}`
    );
  }
  assertSameShape(normalizeWatchlist(goldenPatch.json), normalizeWatchlist(candidatePatch.json), "$", {});

  // add item: fund 不存在 -> 404 + {error:"基金不存在"}
  const missingFundCode = "999999";
  const goldenAddMissing = await postJsonWithBearer(
    `${goldenBase}/api/watchlists/${goldenId}/items/`,
    goldenAccessToken,
    { fund_code: missingFundCode }
  );
  const candidateAddMissing = await postJsonWithBearer(
    `${candidateBase}/api/watchlists/${candidateId}/items/`,
    candidateAccessToken,
    { fund_code: missingFundCode }
  );
  if (goldenAddMissing.status !== candidateAddMissing.status) {
    throw new Error(
      `watchlists.items.add(missing) 状态码不一致: golden=${goldenAddMissing.status} candidate=${candidateAddMissing.status}`
    );
  }
  assertSameShape(goldenAddMissing.json, candidateAddMissing.json, "$", {});

  // remove item: fund 不存在/不在列表 -> 404 + {error:"基金不在自选列表中"}
  const goldenRemoveMissing = await deleteJsonWithBearer(
    `${goldenBase}/api/watchlists/${goldenId}/items/${missingFundCode}/`,
    goldenAccessToken
  );
  const candidateRemoveMissing = await deleteJsonWithBearer(
    `${candidateBase}/api/watchlists/${candidateId}/items/${missingFundCode}/`,
    candidateAccessToken
  );
  if (goldenRemoveMissing.status !== candidateRemoveMissing.status) {
    throw new Error(
      `watchlists.items.remove(missing) 状态码不一致: golden=${goldenRemoveMissing.status} candidate=${candidateRemoveMissing.status}`
    );
  }
  assertSameShape(goldenRemoveMissing.json, candidateRemoveMissing.json, "$", {});

  // reorder: 空列表 -> 400 + {error:"基金代码列表不能为空"}
  const goldenReorderBad = await putJsonWithBearer(
    `${goldenBase}/api/watchlists/${goldenId}/reorder/`,
    goldenAccessToken,
    { fund_codes: [] }
  );
  const candidateReorderBad = await putJsonWithBearer(
    `${candidateBase}/api/watchlists/${candidateId}/reorder/`,
    candidateAccessToken,
    { fund_codes: [] }
  );
  if (goldenReorderBad.status !== candidateReorderBad.status) {
    throw new Error(
      `watchlists.reorder(bad) 状态码不一致: golden=${goldenReorderBad.status} candidate=${candidateReorderBad.status}`
    );
  }
  assertSameShape(goldenReorderBad.json, candidateReorderBad.json, "$", {});

  const goldenDelete = await deleteJsonWithBearer(`${goldenBase}/api/watchlists/${goldenId}/`, goldenAccessToken);
  const candidateDelete = await deleteJsonWithBearer(
    `${candidateBase}/api/watchlists/${candidateId}/`,
    candidateAccessToken
  );
  if (goldenDelete.status !== candidateDelete.status) {
    throw new Error(
      `watchlists.delete 状态码不一致: golden=${goldenDelete.status} candidate=${candidateDelete.status}`
    );
  }

  // delete 是 204，无 body；这里仅验证状态码
  if (goldenDelete.status !== 204) {
    throw new Error(`watchlists.delete 状态码非 204: ${goldenDelete.status}`);
  }
}

function normalizeWatchlist(value: any): any {
  if (value === null || typeof value !== "object") return value;
  const obj = { ...value };
  if (typeof obj.id === "string") obj.id = "uuid";
  if (typeof obj.created_at === "string") obj.created_at = "ts";
  if (Array.isArray(obj.items)) {
    obj.items = obj.items.map((it: any) => normalizeWatchlistItem(it));
  }
  return obj;
}

function normalizeWatchlistItem(value: any): any {
  if (value === null || typeof value !== "object") return value;
  const obj = { ...value };
  if (typeof obj.id === "string") obj.id = "uuid";
  if (typeof obj.fund === "string") obj.fund = "uuid";
  if (typeof obj.created_at === "string") obj.created_at = "ts";
  return obj;
}

function findById(list: any, id: string): any {
  if (!Array.isArray(list)) throw new Error("watchlists.list 响应不是数组");
  const found = list.find((x: any) => x?.id === id);
  if (!found) throw new Error("watchlists.list 未包含新建 watchlist");
  return found;
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

async function patchJsonWithBearer(url: string, token: string, body: unknown) {
  const res = await fetch(url, {
    method: "PATCH",
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

async function putJsonWithBearer(url: string, token: string, body: unknown) {
  const res = await fetch(url, {
    method: "PUT",
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

async function deleteJsonWithBearer(url: string, token: string) {
  const res = await fetch(url, {
    method: "DELETE",
    headers: { Accept: "application/json", Authorization: `Bearer ${token}` },
  });
  const text = await res.text();
  const json = text ? JSON.parse(text) : null;
  return { status: res.status, json };
}

