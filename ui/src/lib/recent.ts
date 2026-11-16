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
const DISMISSED_DONE_KEY = "orbitry.recentDoneDismissed";

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

export function removeRecent(partId: string, script: string): RecentScript[] {
  const next = loadRecent().filter((item) => item.partId !== partId || item.script !== script);
  try {
    localStorage.setItem(RECENT_KEY, JSON.stringify(next));
  } catch {
    // Private mode can block storage.
  }
  return next;
}

export function clearAllRecent(): RecentScript[] {
  try {
    localStorage.setItem(RECENT_KEY, JSON.stringify([]));
  } catch {
    // Private mode can block storage.
  }
  return [];
}

export function doneKey(partId: string, script: string, endedAt?: number | null, runId?: string | null): string {
  return runId || `${partId}::${script}::${endedAt ?? 0}`;
}

export function loadDismissedDone(): string[] {
  const items = readJson<string[]>(DISMISSED_DONE_KEY, []);
  return Array.isArray(items) ? items : [];
}

export function dismissDone(key: string): string[] {
  const next = Array.from(new Set([...loadDismissedDone(), key]));
  try {
    localStorage.setItem(DISMISSED_DONE_KEY, JSON.stringify(next.slice(-200)));
  } catch {
    // Private mode can block storage.
  }
  return next;
}

export function clearAllDone(keys: string[]): string[] {
  const next = Array.from(new Set([...loadDismissedDone(), ...keys]));
  try {
    localStorage.setItem(DISMISSED_DONE_KEY, JSON.stringify(next.slice(-200)));
  } catch {
    // Private mode can block storage.
  }
  return next;
}
