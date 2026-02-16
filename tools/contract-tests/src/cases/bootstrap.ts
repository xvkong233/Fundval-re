import { assertSameShape } from "../diff.js";
import { getJson, postJson } from "../http.js";

export async function runBootstrap(goldenBase: string, candidateBase: string): Promise<void> {
  const goldenHealth = await getJson(`${goldenBase}/api/health/`);
  const candidateHealth = await getJson(`${candidateBase}/api/health/`);

  const goldenInit = (goldenHealth.json as any)?.system_initialized;
  const candidateInit = (candidateHealth.json as any)?.system_initialized;
  if (goldenInit !== false || candidateInit !== false) {
    throw new Error(
      `bootstrap 对照要求两端均未初始化。golden=${String(goldenInit)} candidate=${String(candidateInit)}；请清理配置/volume 后重试`
    );
  }

  // 1) verify：错误 key -> 400 + {valid:false,error:"密钥无效"}
  const wrongKey = "WRONG_KEY_FOR_TEST";
  const goldenVerify = await postJson(`${goldenBase}/api/admin/bootstrap/verify`, {
    bootstrap_key: wrongKey,
  });
  const candidateVerify = await postJson(`${candidateBase}/api/admin/bootstrap/verify`, {
    bootstrap_key: wrongKey,
  });

  if (goldenVerify.status !== candidateVerify.status) {
    throw new Error(
      `verify 状态码不一致: golden=${goldenVerify.status} candidate=${candidateVerify.status}`
    );
  }
  assertSameShape(goldenVerify.json, candidateVerify.json, "$", {});

  // 2) initialize：错误 key -> 400 + {error:"密钥无效"}
  const goldenInitRes = await postJson(`${goldenBase}/api/admin/bootstrap/initialize`, {
    bootstrap_key: wrongKey,
    admin_username: "admin",
    admin_password: "admin123",
    allow_register: false,
  });
  const candidateInitRes = await postJson(`${candidateBase}/api/admin/bootstrap/initialize`, {
    bootstrap_key: wrongKey,
    admin_username: "admin",
    admin_password: "admin123",
    allow_register: false,
  });

  if (goldenInitRes.status !== candidateInitRes.status) {
    throw new Error(
      `initialize 状态码不一致: golden=${goldenInitRes.status} candidate=${candidateInitRes.status}`
    );
  }
  assertSameShape(goldenInitRes.json, candidateInitRes.json, "$", {});
}

