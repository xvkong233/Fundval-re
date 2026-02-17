export type Watchlist = { id: string; name?: string } & Record<string, any>;

export function pickDefaultWatchlistId(watchlists: Watchlist[]): string | null {
  if (!Array.isArray(watchlists) || watchlists.length === 0) return null;
  const id = watchlists[0]?.id;
  return typeof id === "string" && id.length > 0 ? id : null;
}

export function clampIndex(index: number, length: number): number {
  if (!Number.isFinite(index) || !Number.isFinite(length) || length <= 0) return 0;
  if (index < 0) return 0;
  if (index >= length) return length - 1;
  return index;
}

export function moveInArray<T>(arr: T[], fromIndex: number, toIndex: number): T[] {
  if (!Array.isArray(arr) || arr.length === 0) return [];
  const from = clampIndex(fromIndex, arr.length);
  const to = clampIndex(toIndex, arr.length);
  if (from === to) return [...arr];

  const copy = [...arr];
  const [item] = copy.splice(from, 1);
  copy.splice(to, 0, item);
  return copy;
}

export function getFundCodes(rows: Array<{ fund_code?: string }>): string[] {
  if (!Array.isArray(rows)) return [];
  return rows
    .map((r) => r?.fund_code)
    .filter((v): v is string => typeof v === "string" && v.length > 0);
}

