/** Versioned, backward-compatible helpers for Orbitry's localStorage keys. */

export const STORAGE_VERSION = 5;
const VERSION_KEY = "orbitry.storageVersion";
const PREFIX = "orbitry.";

export function readJson<T>(key: string, fallback: T): T {
  try {
    const raw = localStorage.getItem(key);
    if (!raw) return fallback;
    return JSON.parse(raw) as T;
  } catch {
    return fallback;
  }
}

export function writeJson(key: string, value: unknown): void {
  try {
    localStorage.setItem(key, JSON.stringify(value));
  } catch {
    // Private mode can block storage.
  }
}

export function migrateClientStorage(): void {
  try {
    const raw = localStorage.getItem(VERSION_KEY);
    const from = raw && /^\d+$/.test(raw) ? Number(raw) : 0;
    if (from < 1) {
      const locale = localStorage.getItem("orbitry.locale");
      if (locale && locale !== "en" && locale !== "pl") {
        const lower = locale.toLowerCase();
        localStorage.setItem("orbitry.locale", lower.startsWith("pl") ? "pl" : "en");
      }
      const arrayKeys = new Set([
        "orbitry.recent",
        "orbitry.favorites",
        "orbitry.recentDoneDismissed",
        "orbitry.expanded",
      ]);
      const keys: string[] = [];
      for (let i = 0; i < localStorage.length; i++) {
        const key = localStorage.key(i);
        if (key && key.startsWith(PREFIX) && key !== VERSION_KEY) keys.push(key);
      }
      for (const key of keys) {
        const cur = localStorage.getItem(key);
        if (!cur) continue;
        const looksJson = cur.startsWith("{") || cur.startsWith("[");
        if (!looksJson) continue;
        try {
          const parsed = JSON.parse(cur);
          if (arrayKeys.has(key) && !Array.isArray(parsed)) localStorage.removeItem(key);
        } catch {
          localStorage.removeItem(key);
        }
      }
    }
    if (from < 2) {
      // 0.5.3: an open Changed project could crash Projects and stay saved as expanded.
      localStorage.removeItem("orbitry.expanded");
    }
    if (from < 3) {
      // 0.9.0: the details panel is session-only and must not restore stale content.
      localStorage.removeItem("orbitry.panelOpen");
      localStorage.removeItem("orbitry.selectedPart");
      localStorage.removeItem("orbitry.selectedScript");
      localStorage.removeItem("orbitry.alertsOpen");
    }
    if (from < 4) {
      // 0.11.0: Open-in visibility lives in server config (openIn), not localStorage.
      // Bump the client version so older clients re-run migrations cleanly.
    }
    if (from < 5) {
      // 0.13.0: favorites removed; recently-run capped at 10.
      localStorage.removeItem("orbitry.favorites");
      try {
        const recentRaw = localStorage.getItem("orbitry.recent");
        if (recentRaw) {
          const parsed = JSON.parse(recentRaw);
          if (Array.isArray(parsed) && parsed.length > 10) {
            localStorage.setItem("orbitry.recent", JSON.stringify(parsed.slice(0, 10)));
          }
        }
      } catch {
        localStorage.removeItem("orbitry.recent");
      }
    }
    if (from !== STORAGE_VERSION) {
      localStorage.setItem(VERSION_KEY, String(STORAGE_VERSION));
    }
  } catch {
    // Private mode can block storage.
  }
}

export function resetClientStorage(): void {
  try {
    const keys: string[] = [];
    for (let i = 0; i < localStorage.length; i++) {
      const key = localStorage.key(i);
      if (key && key.startsWith(PREFIX)) keys.push(key);
    }
    for (const key of keys) localStorage.removeItem(key);
  } catch {
    // Private mode can block storage.
  }
}
