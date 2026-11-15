const EXPANDED_KEY = "orbitry.expanded";

export function loadExpanded(): string[] {
  try {
    const raw = localStorage.getItem(EXPANDED_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw) as unknown;
    return Array.isArray(parsed) ? parsed.filter((item) => typeof item === "string") : [];
  } catch {
    return [];
  }
}

export function saveExpanded(ids: string[]) {
  try {
    localStorage.setItem(EXPANDED_KEY, JSON.stringify(ids));
  } catch {
    // Private mode can block storage.
  }
}

export function toggleExpanded(ids: string[], groupId: string): string[] {
  const next = ids.includes(groupId) ? ids.filter((id) => id !== groupId) : [...ids, groupId];
  saveExpanded(next);
  return next;
}
