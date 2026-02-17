export type SourceItem = { name: string } & Record<string, any>;

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

export const normalizeSourceAccuracy = (input: SourceAccuracyLike): NormalizedSourceAccuracy => {
  return {
    avg_error_rate: toNumber(input?.avg_error_rate),
    record_count: Number.isFinite(Number(input?.record_count)) ? Number(input?.record_count) : 0,
  };
};

export const formatErrorRatePercent = (avgErrorRate: number): string => {
  return `${(avgErrorRate * 100).toFixed(2)}%`;
};

