import { readFile } from "node:fs/promises";
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

  const wrongKey = "WRONG_KEY_FOR_TEST";

  // 1) verify：错误 key -> 400 + {valid:false,error:"密钥无效"}（并触发两端生成 bootstrap_key）
  const goldenVerifyBad = await postJson(`${goldenBase}/api/admin/bootstrap/verify`, {
    bootstrap_key: wrongKey,
  });
  const candidateVerifyBad = await postJson(`${candidateBase}/api/admin/bootstrap/verify`, {
    bootstrap_key: wrongKey,
  });

  if (goldenVerifyBad.status !== candidateVerifyBad.status) {
    throw new Error(
      `verify(bad) 状态码不一致: golden=${goldenVerifyBad.status} candidate=${candidateVerifyBad.status}`
    );
  }
  assertSameShape(goldenVerifyBad.json, candidateVerifyBad.json, "$", {});

  // 1.5) initialize：错误 key -> 400 + {error:"密钥无效"}
  const goldenInitBad = await postJson(`${goldenBase}/api/admin/bootstrap/initialize`, {
    bootstrap_key: wrongKey,
    admin_username: "admin",
    admin_password: "admin123",
    allow_register: false,
  });
  const candidateInitBad = await postJson(`${candidateBase}/api/admin/bootstrap/initialize`, {
    bootstrap_key: wrongKey,
    admin_username: "admin",
    admin_password: "admin123",
    allow_register: false,
  });
  if (goldenInitBad.status !== candidateInitBad.status) {
    throw new Error(
      `initialize(bad) 状态码不一致: golden=${goldenInitBad.status} candidate=${candidateInitBad.status}`
    );
  }
  assertSameShape(goldenInitBad.json, candidateInitBad.json, "$", {});

  const goldenConfigPath = process.env.GOLDEN_CONFIG_PATH;
  const candidateConfigPath = process.env.CANDIDATE_CONFIG_PATH;
  if (!goldenConfigPath || !candidateConfigPath) return;

  const goldenKey = await readBootstrapKey(goldenConfigPath);
  const candidateKey = await readBootstrapKey(candidateConfigPath);

  // 2) verify：正确 key -> 200 + {valid:true,message:"密钥验证成功"}
  const goldenVerifyOk = await postJson(`${goldenBase}/api/admin/bootstrap/verify`, {
    bootstrap_key: goldenKey,
  });
  const candidateVerifyOk = await postJson(`${candidateBase}/api/admin/bootstrap/verify`, {
    bootstrap_key: candidateKey,
  });
  if (goldenVerifyOk.status !== candidateVerifyOk.status) {
    throw new Error(
      `verify(ok) 状态码不一致: golden=${goldenVerifyOk.status} candidate=${candidateVerifyOk.status}`
    );
  }
  assertSameShape(goldenVerifyOk.json, candidateVerifyOk.json, "$", {});

  // 3) initialize：正确 key -> 200 + {message:"系统初始化成功",admin_created:true}
  const goldenInitOk = await postJson(`${goldenBase}/api/admin/bootstrap/initialize`, {
    bootstrap_key: goldenKey,
    admin_username: "admin",
    admin_password: "admin123",
    allow_register: true,
  });
  const candidateInitOk = await postJson(`${candidateBase}/api/admin/bootstrap/initialize`, {
    bootstrap_key: candidateKey,
    admin_username: "admin",
    admin_password: "admin123",
    allow_register: true,
  });
  if (goldenInitOk.status !== candidateInitOk.status) {
    throw new Error(
      `initialize(ok) 状态码不一致: golden=${goldenInitOk.status} candidate=${candidateInitOk.status}`
    );
  }
  assertSameShape(goldenInitOk.json, candidateInitOk.json, "$", {});

  // 4) verify：已初始化 -> 410 + {error:"System already initialized"}
  const goldenVerifyGone = await postJson(`${goldenBase}/api/admin/bootstrap/verify`, {
    bootstrap_key: goldenKey,
  });
  const candidateVerifyGone = await postJson(`${candidateBase}/api/admin/bootstrap/verify`, {
    bootstrap_key: candidateKey,
  });
  if (goldenVerifyGone.status !== candidateVerifyGone.status) {
    throw new Error(
      `verify(gone) 状态码不一致: golden=${goldenVerifyGone.status} candidate=${candidateVerifyGone.status}`
    );
  }
  assertSameShape(goldenVerifyGone.json, candidateVerifyGone.json, "$", {});
}

async function readBootstrapKey(configPath: string): Promise<string> {
  const raw = await readFile(configPath, "utf-8");
  const json = JSON.parse(raw) as any;
  const key = json?.bootstrap_key;
  if (typeof key !== "string" || key.length < 10) {
    throw new Error(`config.bootstrap_key 无效: path=${configPath}`);
  }
  return key;
}
