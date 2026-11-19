export type RecentScript = {
  partId: string;
  partName: string;
  groupName: string;
  script: string;
  at: number;
};

const RECENT_KEY = "orbitry.recent";
const FAVORITES_KEY = "orbitry.favorites";
const DISMISSED_DONE_KEY = "orbitry.recentDoneDismissed";

/** Hard cap for recently-run and recently-stopped lists. */
export const MAX_RECENT = 10;
/** Rows shown before "Show more". */
export const RECENT_PREVIEW = 3;

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
  if (!Array.isArray(items)) return [];
  return items.slice(0, MAX_RECENT);
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

/** Drop legacy favorites key; older clients may still write it until they upgrade. */
export function clearLegacyFavorites(): void {
  try {
    localStorage.removeItem(FAVORITES_KEY);
  } catch {
    // Private mode can block storage.
  }
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
