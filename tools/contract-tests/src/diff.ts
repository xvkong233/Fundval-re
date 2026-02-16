export type JsonValue =
  | null
  | boolean
  | number
  | string
  | JsonValue[]
  | { [key: string]: JsonValue };

export type DiffOptions = {
  allowValueDiffAtPaths?: Set<string>;
};

function jsonType(value: unknown): string {
  if (value === null) return "null";
  if (Array.isArray(value)) return "array";
  return typeof value;
}

function isPlainObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

export function assertSameShape(
  golden: JsonValue,
  candidate: JsonValue,
  path: string,
  options: DiffOptions
): void {
  const allow = options.allowValueDiffAtPaths?.has(path) ?? false;

  const goldenType = jsonType(golden);
  const candidateType = jsonType(candidate);
  if (goldenType !== candidateType) {
    throw new Error(
      `类型不一致 @ ${path}: golden=${goldenType}, candidate=${candidateType}`
    );
  }

  if (allow) return;

  if (isPlainObject(golden) && isPlainObject(candidate)) {
    const goldenKeys = Object.keys(golden).sort();
    const candidateKeys = Object.keys(candidate).sort();
    const goldenKeyStr = goldenKeys.join(",");
    const candidateKeyStr = candidateKeys.join(",");
    if (goldenKeyStr !== candidateKeyStr) {
      throw new Error(
        `对象键集合不一致 @ ${path}: golden=[${goldenKeyStr}] candidate=[${candidateKeyStr}]`
      );
    }
    for (const key of goldenKeys) {
      assertSameShape(
        golden[key] as JsonValue,
        candidate[key] as JsonValue,
        `${path}.${key}`,
        options
      );
    }
    return;
  }

  if (Array.isArray(golden) && Array.isArray(candidate)) {
    if (golden.length === 0 || candidate.length === 0) return;
    assertSameShape(golden[0] as JsonValue, candidate[0] as JsonValue, `${path}[0]`, options);
    return;
  }

  // 标量：默认要求值一致（后续用 allowValueDiffAtPaths 放开 token/timestamp 等）
  if (golden !== candidate) {
    throw new Error(`值不一致 @ ${path}: golden=${String(golden)} candidate=${String(candidate)}`);
  }
}

