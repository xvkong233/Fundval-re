export type PositionAccount = { id: string; parent: string | null } & Record<string, any>;

export function pickDefaultChildAccountId(
  accounts: PositionAccount[],
  preferredId?: string | null
): string | null {
  if (!Array.isArray(accounts) || accounts.length === 0) return null;
  const children = accounts.filter((a) => !!a?.parent);
  if (children.length === 0) return null;
  if (preferredId && children.some((c) => c.id === preferredId)) return preferredId;
  return children[0]?.id ?? null;
}

