import { assertSameSchema } from "../diff.js";
import { getJson } from "../http.js";

export async function runSourcesAccuracy(goldenBase: string, candidateBase: string): Promise<void> {
  const golden = await getJson(`${goldenBase}/api/sources/eastmoney/accuracy/`);
  const candidate = await getJson(`${candidateBase}/api/sources/eastmoney/accuracy/`);

  if (golden.status !== candidate.status) {
    throw new Error(
      `sources.accuracy 状态码不一致: golden=${golden.status} candidate=${candidate.status}`
    );
  }
  assertSameSchema(golden.json, candidate.json, "$");

  if (process.env.ENABLE_DB_SEED !== "true") return;

  // 值级对照（由 seed 数据保证稳定）
  const seeded = await getJson(`${goldenBase}/api/sources/eastmoney/accuracy/?days=100`);
  const seededCandidate = await getJson(`${candidateBase}/api/sources/eastmoney/accuracy/?days=100`);

  if (seeded.status !== 200 || seededCandidate.status !== 200) {
    throw new Error(
      `sources.accuracy(seeded) 状态码异常: golden=${seeded.status} candidate=${seededCandidate.status}`
    );
  }

  const gAvg = (seeded.json as any)?.avg_error_rate;
  const cAvg = (seededCandidate.json as any)?.avg_error_rate;
  const gCount = (seeded.json as any)?.record_count;
  const cCount = (seededCandidate.json as any)?.record_count;

  if (typeof gCount !== "number" || typeof cCount !== "number") {
    throw new Error(`sources.accuracy(seeded) record_count 类型错误: ${String(gCount)} / ${String(cCount)}`);
  }
  if (gCount !== 2 || cCount !== 2) {
    throw new Error(`sources.accuracy(seeded) record_count 不符合预期: golden=${gCount} candidate=${cCount}`);
  }

  const toNum = (v: unknown) => (typeof v === "number" ? v : Number(v));
  const gAvgNum = toNum(gAvg);
  const cAvgNum = toNum(cAvg);
  if (!Number.isFinite(gAvgNum) || !Number.isFinite(cAvgNum)) {
    throw new Error(`sources.accuracy(seeded) avg_error_rate 非数字: ${String(gAvg)} / ${String(cAvg)}`);
  }

  const expected = 0.0180665;
  const eps = 1e-6;
  if (Math.abs(gAvgNum - expected) > eps || Math.abs(cAvgNum - expected) > eps) {
    throw new Error(
      `sources.accuracy(seeded) avg_error_rate 不符合预期: golden=${gAvgNum} candidate=${cAvgNum} expected=${expected}`
    );
  }

  const diffEps = 1e-9;
  if (Math.abs(gAvgNum - cAvgNum) > diffEps) {
    throw new Error(
      `sources.accuracy(seeded) avg_error_rate golden/candidate 不一致: golden=${gAvgNum} candidate=${cAvgNum}`
    );
  }
}
