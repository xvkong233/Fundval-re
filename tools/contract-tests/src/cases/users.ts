import { assertSameShape } from "../diff.js";
import { getJson, postJson } from "../http.js";

export async function runUsers(goldenBase: string, candidateBase: string): Promise<void> {
  const goldenHealth = await getJson(`${goldenBase}/api/health/`);
  const candidateHealth = await getJson(`${candidateBase}/api/health/`);

  const goldenInit = (goldenHealth.json as any)?.system_initialized;
  const candidateInit = (candidateHealth.json as any)?.system_initialized;
  if (goldenInit !== true || candidateInit !== true) return;

  const username = `newuser_${Date.now()}_${Math.floor(Math.random() * 1e6)}`;
  const password = "password123";

  const goldenRegister = await postJson(`${goldenBase}/api/users/register/`, {
    username,
    password,
    password_confirm: password,
  });
  const candidateRegister = await postJson(`${candidateBase}/api/users/register/`, {
    username,
    password,
    password_confirm: password,
  });

  if (goldenRegister.status !== candidateRegister.status) {
    throw new Error(
      `register 状态码不一致: golden=${goldenRegister.status} candidate=${candidateRegister.status}`
    );
  }

  if (goldenRegister.status === 201) {
    assertSameShape(goldenRegister.json, candidateRegister.json, "$", {
      allowValueDiffAtPaths: new Set(["$.access_token", "$.refresh_token", "$.user.id"]),
    });
  } else {
    // 可能是 403（未开放）或 400（校验失败）；只比较形状
    assertSameShape(goldenRegister.json, candidateRegister.json, "$", {});
    return;
  }

  // 重复用户名 -> 400 + {username:["用户名已存在"]}
  const goldenDup = await postJson(`${goldenBase}/api/users/register/`, {
    username,
    password,
    password_confirm: password,
  });
  const candidateDup = await postJson(`${candidateBase}/api/users/register/`, {
    username,
    password,
    password_confirm: password,
  });

  if (goldenDup.status !== candidateDup.status) {
    throw new Error(
      `register(duplicate) 状态码不一致: golden=${goldenDup.status} candidate=${candidateDup.status}`
    );
  }
  assertSameShape(goldenDup.json, candidateDup.json, "$", {});
}

