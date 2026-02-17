export type Account = { id: string; parent: string | null; is_default?: boolean } & Record<string, any>;

export function pickDefaultParentAccountId(accounts: Account[]): string | null {
  if (!Array.isArray(accounts) || accounts.length === 0) return null;
  const parents = accounts.filter((a) => !a?.parent);
  if (parents.length === 0) return null;
  const def = parents.find((a) => a?.is_default === true);
  return (def?.id ?? parents[0]?.id) || null;
}

