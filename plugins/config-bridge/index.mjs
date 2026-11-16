#!/usr/bin/env node
/**
 * Orbitry config-bridge
 *
 * Reads a project's Vite or Next config and prints resolved ports as JSON.
 * Never starts a dev server. Uses the project's own vite/next when present.
 */

import { createRequire } from "node:module";
import { pathToFileURL } from "node:url";
import fs from "node:fs";
import path from "node:path";
import process from "node:process";

const TIMEOUT_MS = 8000;
const VITE_FILES = [
  "vite.config.ts",
  "vite.config.mts",
  "vite.config.js",
  "vite.config.mjs",
  "vite.config.cjs",
];
const NEXT_FILES = [
  "next.config.ts",
  "next.config.mts",
  "next.config.js",
  "next.config.mjs",
  "next.config.cjs",
];
const ASTRO_FILES = ["astro.config.ts", "astro.config.mjs", "astro.config.js"];

const timer = setTimeout(() => {
  emit({ ok: false, error: "timed out" });
  process.exit(0);
}, TIMEOUT_MS);
timer.unref?.();

function emit(payload) {
  try {
    clearTimeout(timer);
  } catch {
    // already cleared
  }
  process.stdout.write(JSON.stringify(payload) + "\n");
}

function fail(error) {
  emit({ ok: false, error: String(error && error.message ? error.message : error) });
}

function firstExisting(dir, names) {
  for (const name of names) {
    const full = path.join(dir, name);
    if (fs.existsSync(full) && fs.statSync(full).isFile()) return full;
  }
  return null;
}

function asNumber(value) {
  if (typeof value === "number" && Number.isFinite(value) && value > 0) return Math.round(value);
  if (typeof value === "string" && /^\d+$/.test(value.trim())) {
    const port = Number(value.trim());
    if (port > 0 && port <= 65535) return port;
  }
  return null;
}

function asHost(value) {
  if (value == null || value === false) return null;
  if (value === true) return "0.0.0.0";
  if (typeof value === "string" && value.trim()) return value.trim();
  return null;
}

function extractProxy(proxy) {
  if (!proxy || typeof proxy !== "object") return [];
  const entries = [];
  for (const [key, value] of Object.entries(proxy)) {
    if (typeof value === "string") {
      entries.push({ path: key, target: value });
      continue;
    }
    if (value && typeof value === "object" && typeof value.target === "string") {
      entries.push({ path: key, target: value.target });
    }
  }
  return entries;
}

function projectRequire(cwd) {
  const pkg = path.join(cwd, "package.json");
  const anchor = fs.existsSync(pkg) ? pkg : path.join(cwd, "__orbitry_bridge.js");
  return createRequire(pathToFileURL(anchor).href);
}

function resolveFromProject(cwd, id) {
  try {
    return projectRequire(cwd).resolve(id);
  } catch {
    return null;
  }
}

async function loadWithVite(cwd, configPath) {
  const id = resolveFromProject(cwd, "vite");
  if (!id) return null;
  const vite = projectRequire(cwd)("vite");
  if (typeof vite.loadConfigFromFile !== "function") return null;
  const loaded = await vite.loadConfigFromFile(
    { command: "serve", mode: "development" },
    configPath,
    cwd,
  );
  if (!loaded?.config) return null;
  const server = loaded.config.server ?? {};
  const preview = loaded.config.preview ?? {};
  return {
    ok: true,
    framework: "vite",
    configPath,
    port: asNumber(server.port),
    host: asHost(server.host),
    strictPort: Boolean(server.strictPort),
    preview: {
      port: asNumber(preview.port),
      host: asHost(preview.host),
    },
    proxy: extractProxy(server.proxy),
  };
}

async function loadWithNext(cwd, configPath) {
  if (!resolveFromProject(cwd, "next")) return null;
  const req = projectRequire(cwd);
  let loadConfig = null;
  let phase = "phase-development-server";
  try {
    const constants = req("next/constants");
    if (constants?.PHASE_DEVELOPMENT_SERVER) phase = constants.PHASE_DEVELOPMENT_SERVER;
  } catch {
    // keep default phase name
  }
  try {
    const mod = req("next/dist/server/config");
    loadConfig = mod.loadConfig ?? mod.default ?? mod;
  } catch {
    return null;
  }
  if (typeof loadConfig !== "function") return null;
  const config = await loadConfig(phase, cwd);
  if (!config || typeof config !== "object") return null;
  return {
    ok: true,
    framework: "next",
    configPath,
    port: asNumber(config.port ?? config.env?.PORT),
    host: asHost(config.hostname ?? config.host),
    strictPort: false,
    preview: { port: null, host: null },
    proxy: [],
  };
}

async function loadPlainModule(configPath, framework) {
  if (!/\.(m?js|cjs)$/i.test(configPath)) return null;
  const mod = await import(pathToFileURL(configPath).href);
  let config = mod.default ?? mod;
  if (typeof config === "function") {
    config = await config({ command: "serve", mode: "development" });
  }
  if (!config || typeof config !== "object") return null;
  const server = config.server ?? {};
  const preview = config.preview ?? {};
  return {
    ok: true,
    framework,
    configPath,
    port: asNumber(server.port ?? config.port),
    host: asHost(server.host ?? config.host ?? config.hostname),
    strictPort: Boolean(server.strictPort),
    preview: {
      port: asNumber(preview.port),
      host: asHost(preview.host),
    },
    proxy: extractProxy(server.proxy),
  };
}

async function readRequest() {
  const args = process.argv.slice(2);
  const out = { cwd: process.cwd(), kind: "auto" };
  for (let i = 0; i < args.length; i += 1) {
    const arg = args[i];
    if (arg === "--cwd" && args[i + 1]) {
      out.cwd = args[++i];
    } else if (arg === "--kind" && args[i + 1]) {
      out.kind = args[++i];
    } else if (arg === "--json" && args[i + 1]) {
      Object.assign(out, JSON.parse(args[++i]));
    }
  }
  if (!process.stdin.isTTY && !args.includes("--cwd") && !args.includes("--json")) {
    const chunks = [];
    for await (const chunk of process.stdin) chunks.push(chunk);
    const text = Buffer.concat(chunks).toString("utf8").trim();
    if (text) Object.assign(out, JSON.parse(text));
  }
  out.cwd = path.resolve(out.cwd);
  out.kind = String(out.kind || "auto").toLowerCase();
  return out;
}

async function main() {
  const request = await readRequest();
  if (!fs.existsSync(request.cwd) || !fs.statSync(request.cwd).isDirectory()) {
    fail("cwd not found");
    return;
  }

  const kind = request.kind;
  const wantVite = kind === "auto" || kind === "vite";
  const wantNext = kind === "auto" || kind === "next";
  const wantAstro = kind === "auto" || kind === "astro";

  const viteConfig = wantVite ? firstExisting(request.cwd, VITE_FILES) : null;
  const nextConfig = wantNext ? firstExisting(request.cwd, NEXT_FILES) : null;
  const astroConfig = wantAstro ? firstExisting(request.cwd, ASTRO_FILES) : null;

  if (!viteConfig && !nextConfig && !astroConfig) {
    fail("no supported config file");
    return;
  }

  if (viteConfig) {
    try {
      const loaded = (await loadWithVite(request.cwd, viteConfig)) ?? (await loadPlainModule(viteConfig, "vite"));
      if (loaded) {
        emit(loaded);
        return;
      }
    } catch (err) {
      if (!nextConfig && !astroConfig) {
        fail(err);
        return;
      }
    }
  }

  if (nextConfig) {
    try {
      const loaded = (await loadWithNext(request.cwd, nextConfig)) ?? (await loadPlainModule(nextConfig, "next"));
      if (loaded) {
        emit(loaded);
        return;
      }
    } catch (err) {
      if (!astroConfig) {
        fail(err);
        return;
      }
    }
  }

  if (astroConfig) {
    try {
      const loaded = (await loadWithVite(request.cwd, astroConfig)) ?? (await loadPlainModule(astroConfig, "astro"));
      if (loaded) {
        emit(loaded);
        return;
      }
    } catch (err) {
      fail(err);
      return;
    }
  }

  fail("could not resolve config");
}

main().catch((err) => {
  fail(err);
});
