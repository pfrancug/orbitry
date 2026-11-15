export type RecentScript = {
  partId: string;
  partName: string;
  groupName: string;
  script: string;
  at: number;
};

const RECENT_KEY = "orbitry.recent";
const FAVORITES_KEY = "orbitry.favorites";
const MAX_RECENT = 8;

function readJson<T>(key: string, fallback: T): T {
  try {
    const raw = localStorage.getItem(key);
    if (!raw) return fallback;
    return JSON.parse(raw) as T;
  } catch {
    return fallback;
  }
}

export function loadRecent(): RecentScript[] {
  const items = readJson<RecentScript[]>(RECENT_KEY, []);
  return Array.isArray(items) ? items : [];
}

export function rememberRecent(entry: Omit<RecentScript, "at">): RecentScript[] {
  const next = [
    { ...entry, at: Date.now() },
    ...loadRecent().filter((item) => item.partId !== entry.partId || item.script !== entry.script),
  ].slice(0, MAX_RECENT);
  try {
    localStorage.setItem(RECENT_KEY, JSON.stringify(next));
  } catch {
    // Private mode can block storage.
  }
  return next;
}

export function favoriteKey(partId: string, script: string): string {
  return `${partId}::${script}`;
}

export function loadFavorites(): string[] {
  const items = readJson<string[]>(FAVORITES_KEY, []);
  return Array.isArray(items) ? items : [];
}

export function toggleFavorite(partId: string, script: string): string[] {
  const key = favoriteKey(partId, script);
  const current = loadFavorites();
  const next = current.includes(key) ? current.filter((item) => item !== key) : [...current, key];
  try {
    localStorage.setItem(FAVORITES_KEY, JSON.stringify(next));
  } catch {
    // Private mode can block storage.
  }
  return next;
}
