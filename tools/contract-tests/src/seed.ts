import pg from "pg";

const SEED_FUND_ID = "11111111-1111-1111-1111-111111111111";
const SEED_FUND_CODE = "000001";
const SEED_FUND_TYPE = "SEED";
const SEED_SOURCE_NAME = "eastmoney";
const SEED_SOURCE_NAME_2 = "seed-source-2";

type DbTarget = { label: string; url: string };

export async function seedDatabases(): Promise<void> {
  if (process.env.ENABLE_DB_SEED !== "true") return;
  if (process.env.ENABLE_DB_CASES !== "true") return;

  const goldenUrl = process.env.GOLDEN_DB_URL;
  const candidateUrl = process.env.CANDIDATE_DB_URL;
  if (!goldenUrl || !candidateUrl) {
    throw new Error(
      `ENABLE_DB_SEED=true 但缺少 DB 连接串：GOLDEN_DB_URL=${String(goldenUrl)} CANDIDATE_DB_URL=${String(
        candidateUrl
      )}`
    );
  }

  await seedOne({ label: "golden", url: goldenUrl });
  await seedOne({ label: "candidate", url: candidateUrl });
}

async function seedOne(target: DbTarget): Promise<void> {
  const client = new pg.Client({ connectionString: target.url });
  await client.connect();
  try {
    await client.query("BEGIN");

    // 让合同测试具备可重复性：清空关键业务表
    // - auth_user：避免 bootstrap.initialize 因 admin 重复而失败（DB volume 可复用但每次测试必须回到“未初始化”状态）
    // - fund：CASCADE 会同时清空 estimate_accuracy 等依赖表
    await client.query(`TRUNCATE TABLE auth_user CASCADE`);
    await client.query(`TRUNCATE TABLE fund CASCADE`);

    const fundId = await upsertSeedFund(client);

    const rows = [
      { id: "22222222-2222-2222-2222-222222222222", sourceName: SEED_SOURCE_NAME, date: "2026-02-10", errorRate: "0.018066" },
      { id: "33333333-3333-3333-3333-333333333333", sourceName: SEED_SOURCE_NAME, date: "2026-02-11", errorRate: "0.018067" },
      { id: "44444444-4444-4444-4444-444444444444", sourceName: SEED_SOURCE_NAME_2, date: "2026-02-11", errorRate: "0.028067" },
    ] as const;

    for (const r of rows) {
      await client.query(
        `
        INSERT INTO estimate_accuracy
          (id, source_name, fund_id, estimate_date, estimate_nav, actual_nav, error_rate, created_at)
        VALUES
          ($1, $2, $3, $4, $5, $6, $7, NOW())
        ON CONFLICT (source_name, fund_id, estimate_date) DO UPDATE SET
          estimate_nav = EXCLUDED.estimate_nav,
          actual_nav = EXCLUDED.actual_nav,
          error_rate = EXCLUDED.error_rate
        `,
        [r.id, r.sourceName, fundId, r.date, "1.0000", "1.0000", r.errorRate]
      );
    }

    const navRows = [
      { id: "77777777-7777-7777-7777-777777777777", navDate: "2026-02-10", unitNav: "1.0800", accNav: "1.0800", dailyGrowth: "0.0100" },
      { id: "88888888-8888-8888-8888-888888888888", navDate: "2026-02-11", unitNav: "1.1000", accNav: "1.1000", dailyGrowth: "0.0185" },
      { id: "55555555-5555-5555-5555-555555555555", navDate: "2026-03-10", unitNav: "1.1000", accNav: "1.1000", dailyGrowth: "0.0100" },
      { id: "66666666-6666-6666-6666-666666666666", navDate: "2026-03-11", unitNav: "1.1200", accNav: "1.1200", dailyGrowth: "0.0182" },
    ] as const;

    for (const r of navRows) {
      await client.query(
        `
        INSERT INTO fund_nav_history
          (id, fund_id, nav_date, unit_nav, accumulated_nav, daily_growth, created_at, updated_at)
        VALUES
          ($1, $2, $3, $4, $5, $6, NOW(), NOW())
        ON CONFLICT (fund_id, nav_date) DO UPDATE SET
          unit_nav = EXCLUDED.unit_nav,
          accumulated_nav = EXCLUDED.accumulated_nav,
          daily_growth = EXCLUDED.daily_growth,
          updated_at = NOW()
        `,
        [r.id, fundId, r.navDate, r.unitNav, r.accNav, r.dailyGrowth]
      );
    }

    await client.query("COMMIT");
  } catch (error) {
    try {
      await client.query("ROLLBACK");
    } catch {
      // ignore rollback errors
    }
    throw new Error(`[seed:${target.label}] ${String((error as any)?.message ?? error)}`);
  } finally {
    await client.end();
  }
}

async function upsertSeedFund(client: pg.Client): Promise<string> {
  const res = await client.query(
    `
    INSERT INTO fund (id, fund_code, fund_name, fund_type, created_at, updated_at)
    VALUES ($1, $2, $3, $4, NOW(), NOW())
    ON CONFLICT (fund_code) DO UPDATE SET
      fund_name = EXCLUDED.fund_name,
      fund_type = EXCLUDED.fund_type,
      updated_at = NOW()
    RETURNING id
    `,
    [SEED_FUND_ID, SEED_FUND_CODE, "Seed Fund (contract-tests)", SEED_FUND_TYPE]
  );

  const id = res.rows?.[0]?.id as string | undefined;
  if (!id) throw new Error("seed fund upsert 未返回 id");
  return id;
}
