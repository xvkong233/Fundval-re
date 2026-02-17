export type Watchlist = { id: string; name?: string } & Record<string, any>;

export function pickDefaultWatchlistId(watchlists: Watchlist[]): string | null {
  if (!Array.isArray(watchlists) || watchlists.length === 0) return null;
  const id = watchlists[0]?.id;
  return typeof id === "string" && id.length > 0 ? id : null;
}

