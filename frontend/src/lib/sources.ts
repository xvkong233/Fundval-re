export type SourceItem = { name: string } & Record<string, any>;

export type SourceHealthLike = {
  name?: string;
  ok?: boolean;
  latency_ms?: number | string | null;
  error?: string | null;
} & Record<string, any>;

export type NormalizedSourceHealth = {
  ok: boolean;
  latency_ms: number | null;
  error: string | null;
};

export type SourceAccuracyLike = {
  avg_error_rate?: string | number;
  record_count?: number;
} & Record<string, any>;

export type NormalizedSourceAccuracy = {
  avg_error_rate: number;
  record_count: number;
};

const toNumber = (value: unknown): number => {
  const n = Number(value);
  return Number.isFinite(n) ? n : 0;
};

export const normalizeSourceHealth = (input: SourceHealthLike): NormalizedSourceHealth => {
  const ok = Boolean(input?.ok);
  const latency =
    input?.latency_ms === null || typeof input?.latency_ms === "undefined"
      ? null
      : Number.isFinite(Number(input?.latency_ms))
        ? Number(input?.latency_ms)
        : null;
  const error =
    typeof input?.error === "string" && input.error.trim().length > 0 ? input.error.trim() : null;

  return { ok, latency_ms: latency, error };
};

export const normalizeSourceAccuracy = (input: SourceAccuracyLike): NormalizedSourceAccuracy => {
  return {
    avg_error_rate: toNumber(input?.avg_error_rate),
    record_count: Number.isFinite(Number(input?.record_count)) ? Number(input?.record_count) : 0,
  };
};

export const formatErrorRatePercent = (avgErrorRate: number): string => {
  return `${(avgErrorRate * 100).toFixed(2)}%`;
};

export const sourceDisplayName = (sourceName: string): string => {
  const name = String(sourceName ?? "").trim();
  switch (name) {
    case "tiantian":
      return "天天基金";
    case "danjuan":
      return "蛋卷";
    case "ths":
      return "同花顺";
    default:
      return name || "-";
  }
};

