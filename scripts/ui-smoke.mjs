#!/usr/bin/env node
/**
 * CI UI smoke: serve ui/dist with mock APIs, open every dashboard route in
 * headless Chromium, and assert overlay + scroll behavior.
 *
 * Run from the repo root after `npm run build` in ui/:
 *   node scripts/ui-smoke.mjs
 */
import http from "node:http";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const DIST = path.join(ROOT, "ui", "dist");
const ROUTES = ["/", "/projects", "/databases", "/models", "/logs", "/settings"];

const MIME = {
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".css": "text/css; charset=utf-8",
  ".svg": "image/svg+xml",
  ".png": "image/png",
  ".ico": "image/x-icon",
  ".json": "application/json",
  ".woff2": "font/woff2",
  ".map": "application/json",
};

const scanDiff = {
  added: [
    {
      kind: "part",
      project: "demo-app",
      projectId: "demo-app",
      part: "web",
      partId: "demo-app/web",
      detail: "new app",
    },
  ],
  removed: [],
  changed: [],
};

const models = [];
for (let i = 0; i < 24; i += 1) {
  models.push({
    name: `demo-model-${String(i + 1).padStart(2, "0")}`,
    id: `sha256:${"ab".repeat(8)}${i}`,
    sizeBytes: (24 - i) * 1024 * 1024 * 80,
    modifiedAt: 1_700_000_000_000 - i * 86_400_000,
    usedBy:
      i % 4 === 0
        ? [
            {
              projectId: "demo-app",
              projectName: "demo-app",
              projectPath: "/path/to/projects/demo-app",
              files: ["/path/to/projects/demo-app/src/lib/models.ts"],
            },
          ]
        : i % 4 === 1
          ? [
              {
                projectId: "analytics-pipeline-service",
                projectName: "analytics-pipeline-service-with-a-very-long-name",
                projectPath: "/path/to/projects/analytics-pipeline-service",
              },
            ]
          : [],
  });
}


const configState = {
  roots: ["/path/to/projects"],
  scriptPatterns: [],
  defaultPatterns: [{ kind: "dev", pattern: "dev" }],
  portOverrides: [],
  configBridge: "off",
  autoCheckUpdates: false,
  logLimitMb: 10,
  autostartApps: [],
  desktopNotifications: false,
  ignoredProjects: [],
  ahkPlugin: true,
  bridgeStatus: { state: "disabled", label: "disabled", available: false, node: false, plugin: true },
  ahkStatus: {
    enabled: true,
    available: false,
    windows: false,
    installed: false,
    label: "Windows only",
    state: "windows-only",
    reason: "AutoHotkey runs on Windows only.",
    interpreters: [
      { version: "v1", installed: false, path: null },
      { version: "v2", installed: false, path: null },
    ],
  },
  autostart: { enabled: false, stale: false },
};

function serializeConfig() {
  const body = { ...configState };
  if (!body.ignoredProjects.length) {
    const { ignoredProjects, ...rest } = body;
    return rest;
  }
  return body;
}

function applyConfigPut(body) {
  if (Array.isArray(body.roots)) configState.roots = body.roots;
  if (Array.isArray(body.scriptPatterns)) configState.scriptPatterns = body.scriptPatterns;
  if (Object.prototype.hasOwnProperty.call(body, "ignoredProjects")) {
    configState.ignoredProjects = Array.isArray(body.ignoredProjects) ? body.ignoredProjects : [];
  }
  if (body.configBridge != null) configState.configBridge = body.configBridge;
  if (body.autoCheckUpdates != null) configState.autoCheckUpdates = body.autoCheckUpdates;
  if (body.logLimitMb != null) configState.logLimitMb = body.logLimitMb;
  if (body.desktopNotifications != null) configState.desktopNotifications = body.desktopNotifications;
  if (body.ahkPlugin != null) {
    configState.ahkPlugin = body.ahkPlugin;
    configState.ahkStatus = {
      ...configState.ahkStatus,
      enabled: Boolean(body.ahkPlugin),
      label: body.ahkPlugin ? configState.ahkStatus.label : "disabled",
      state: body.ahkPlugin ? configState.ahkStatus.state : "disabled",
    };
  }
  if (Object.prototype.hasOwnProperty.call(body, "autostartApps")) {
    configState.autostartApps = Array.isArray(body.autostartApps) ? body.autostartApps : [];
  }
}

function part(base) {
  return {
    version: "1.0.0",
    nodeModules: true,
    parent: null,
    relativePath: ".",
    git: { branch: "main", commit: "abc1234", dirty: false },
    build: { built: true, dir: "dist", stale: false, lastBuiltAt: 1_700_000_000_000 },
    scripts: [
      { name: "dev", kind: "dev", scope: "fe", command: "npm run dev", cwd: base.path },
    ],
    ports: [],
    databases: [],
    ...base,
  };
}

const mocks = {
  "GET /api/projects": {
    groups: [
      {
        id: "demo-app",
        path: "/path/to/projects/demo-app",
        name: "demo-app",
        folder: "demo-app",
        version: "1.0.0",
        kind: "node",
        framework: "sveltekit",
        git: { branch: "main", commit: "abc1234", dirty: false },
        build: { built: true, dir: "dist", stale: false, lastBuiltAt: 1_700_000_000_000 },
        nodeModules: true,
        parts: [
          part({
            id: "demo-app/web",
            name: "web",
            path: "/path/to/projects/demo-app",
            role: "fe",
            kind: "node",
            framework: "sveltekit",
            aiDependencies: ["ai", "@ai-sdk/openai", "openai"],
            databases: [
              { engine: "sqlite", source: "file", path: "prisma/dev.db", sizeBytes: 2_458_624 },
              { engine: "postgres", source: "compose", host: "localhost", port: 5432, detail: "compose.yml:db" },
              { engine: "redis", source: "env", host: "localhost", port: 6379 },
            ],
          }),
        ],
      },
      {
        id: "analytics-pipeline-service",
        path: "/path/to/projects/analytics-pipeline-service",
        name: "analytics-pipeline-service-with-a-very-long-name",
        folder: "analytics-pipeline-service",
        version: "2.4.1",
        kind: "node",
        framework: "next",
        git: { branch: "main", commit: "def5678", dirty: true },
        build: { built: true, dir: ".next", stale: false, lastBuiltAt: 1_700_000_000_000 },
        nodeModules: true,
        parts: [
          part({
            id: "analytics-pipeline-service",
            name: "analytics-pipeline-service-with-a-very-long-name",
            path: "/path/to/projects/analytics-pipeline-service",
            role: "root",
            kind: "node",
            framework: "next",
            aiDependencies: ["langchain", "@anthropic-ai/sdk"],
            databases: [
              {
                engine: "sqlite",
                source: "file",
                path: "server/data/tracker.db",
                sizeBytes: 21 * 1024 * 1024,
              },
              {
                engine: "postgres",
                source: "env",
                host: "localhost",
                port: 5433,
                detail: ".env",
              },
              {
                engine: "mongodb",
                source: "compose",
                host: "localhost",
                port: 27017,
                detail: "docker-compose.yml:mongo",
              },
            ],
          }),
          part({
            id: "analytics-pipeline-service/server",
            name: "server",
            path: "/path/to/projects/analytics-pipeline-service/server",
            role: "be",
            kind: "node",
            framework: null,
            relativePath: "server",
            databases: [
              {
                engine: "sqlite",
                source: "file",
                path: "data/tracker.db",
                sizeBytes: 21 * 1024 * 1024,
              },
            ],
          }),
        ],
      },
      {
        id: "inventory-api",
        path: "/path/to/projects/inventory-api",
        name: "inventory-api",
        folder: "inventory-api",
        version: "0.9.0",
        kind: "rust",
        framework: "rust",
        git: { branch: "develop", commit: "cafe001", dirty: false },
        build: { built: true, dir: "target", stale: false, lastBuiltAt: 1_700_000_000_000 },
        nodeModules: false,
        parts: [
          part({
            id: "inventory-api",
            name: "inventory-api",
            path: "/path/to/projects/inventory-api",
            role: "be",
            kind: "rust",
            framework: "rust",
            databases: [
              {
                engine: "sqlite",
                source: "file",
                path: "storage/very/deeply/nested/path/to/production-mirror.sqlite3",
                sizeBytes: 180 * 1024 * 1024,
              },
              { engine: "mysql", source: "env", host: "localhost", port: 3306 },
            ],
          }),
        ],
      },
      {
        id: "notes-desktop",
        path: "/path/to/projects/notes-desktop",
        name: "notes-desktop",
        folder: "notes-desktop",
        version: "3.1.0",
        kind: "node",
        framework: "electron",
        git: { branch: "main", commit: "bead222", dirty: false },
        build: { built: false, dir: "dist", stale: true, lastBuiltAt: null },
        nodeModules: true,
        parts: [
          part({
            id: "notes-desktop",
            name: "notes-desktop",
            path: "/path/to/projects/notes-desktop",
            role: "root",
            kind: "node",
            framework: "electron",
            databases: [
              { engine: "sqlite", source: "file", path: "app.db", sizeBytes: 512_000 },
            ],
          }),
        ],
      },
      {
        id: "ComfyUI",
        path: "C:\\Users\\Piotr\\Projects\\ComfyUI",
        name: "ComfyUI",
        folder: "ComfyUI",
        version: null,
        kind: "python",
        framework: null,
        git: { branch: "master", commit: "c0ffee1", dirty: true },
        build: { built: false, dir: null, stale: false, lastBuiltAt: null },
        nodeModules: false,
        parts: [
          part({
            id: "ComfyUI",
            name: "ComfyUI",
            path: "C:\\Users\\Piotr\\Projects\\ComfyUI",
            role: "root",
            kind: "python",
            framework: null,
            nodeModules: false,
            aiDependencies: ["comfyui", "torch", "diffusers", "transformers"],
            databases: [],
            scripts: [{ name: "main", kind: "dev", scope: "be", command: "python main.py", cwd: "C:\\Users\\Piotr\\Projects\\ComfyUI" }],
          }),
        ],
      },
    ],
    warnings: [],
    scan: {
      state: "fresh",
      stale: false,
      scannedAt: Date.now(),
      startedAt: Date.now() - 1000,
      diff: scanDiff,
      error: null,
    },
    compose: [
      {
        projectId: "demo-app",
        projectName: "demo-app",
        workdir: "/path/to/projects/demo-app",
        file: "/path/to/projects/demo-app/compose.yaml",
        dockerAvailable: false,
        services: [{ name: "web", state: "exited", ports: ["3000:3000"] }],
      },
    ],
  },
  "GET /api/config": null,
  "GET /api/runs": {
    runs: [
      {
        id: "run-demo-dev",
        partId: "demo-app/web",
        partName: "web",
        partPath: "/path/to/projects/demo-app/web",
        script: "dev",
        command: "npm run dev",
        cwd: "/path/to/projects/demo-app/web",
        pid: 4242,
        status: "running",
        exitCode: null,
        startedAt: Date.now() - 60_000,
        endedAt: null,
        ports: [{ port: 5173, source: "default", url: "http://127.0.0.1:5173" }],
        health: { status: "ok", url: "http://127.0.0.1:5173", checkedAt: Date.now() },
      },
      {
        id: "run-demo-build",
        partId: "demo-app/web",
        partName: "web",
        partPath: "/path/to/projects/demo-app/web",
        script: "build",
        command: "npm run build",
        cwd: "/path/to/projects/demo-app/web",
        pid: null,
        status: "exited",
        exitCode: 0,
        stopRequested: true,
        startedAt: Date.now() - 120_000,
        endedAt: Date.now() - 30_000,
        ports: [],
        health: { status: "unknown", url: null, checkedAt: null },
      },
    ],
    warnings: [],
  },
  "GET /api/version": { current: "0.6.0", latest: "0.6.0", updateAvailable: false, dev: true },
  "GET /api/update/status": { current: "0.6.0", latest: "0.6.0", updateAvailable: false, dev: true },
  "GET /api/tools": { vscodeInstalled: false },
  "GET /api/metrics": {
    sampledAt: Date.now(),
    system: { cpuPct: 4, memUsed: 4 * 1024 * 1024 * 1024, memTotal: 16 * 1024 * 1024 * 1024 },
    runs: {},
    history: { cpu: [2, 3, 4, 3], memPct: [20, 21, 22, 21] },
  },
  "GET /api/models": {
    scan: { state: "fresh", scannedAt: Date.now(), stale: false },
    ollama: {
      installed: true,
      version: "0.0.0",
      serviceUrl: "http://127.0.0.1:11434",
      serviceReachable: false,
      modelsDir: "/path/to/models",
      modelsDirBytes: models.reduce((sum, item) => sum + item.sizeBytes, 0),
      source: "mock",
      models,
    },
    caches: [],
    projectFiles: [
      {
        projectId: "ComfyUI",
        projectName: "ComfyUI",
        projectPath: "C:\\Users\\Piotr\\Projects\\ComfyUI",
        totalBytes: 4_200_000_000,
        files: [
          {
            name: "sd_xl_base_1.0.safetensors",
            path: "C:\\Users\\Piotr\\Projects\\ComfyUI\\models\\checkpoints\\sd_xl_base_1.0.safetensors",
            sizeBytes: 4_200_000_000,
            usedBy: [],
          },
        ],
      },
    ],
    totals: [
      { id: "ollama", label: "Ollama", path: "/path/to/models", bytes: models.reduce((sum, item) => sum + item.sizeBytes, 0) },
      {
        id: "project-weights",
        label: "Project weights",
        path: "C:\\Users\\Piotr\\Projects\\ComfyUI",
        bytes: 4_200_000_000,
      },
    ],
    grandTotalBytes: models.reduce((sum, item) => sum + item.sizeBytes, 0) + 4_200_000_000,
  },
  "GET /api/docker": [],
};

/** When set, GET /api/models returns 500 so smoke can assert Projects still works. */
let modelsFailOnce = false;
/** Suppress expected console noise while probing models failure. */
let expectingModelsFailure = false;

function json(res, body, status = 200) {
  const data = JSON.stringify(body);
  res.writeHead(status, {
    "content-type": "application/json; charset=utf-8",
    "cache-control": "no-store",
    "content-length": Buffer.byteLength(data),
  });
  res.end(data);
}

function serveStatic(req, res) {
  const url = new URL(req.url ?? "/", "http://127.0.0.1");
  let rel = decodeURIComponent(url.pathname);
  if (rel === "/") rel = "/index.html";
  const filePath = path.join(DIST, rel);
  if (!filePath.startsWith(DIST)) {
    res.writeHead(403).end("forbidden");
    return;
  }
  fs.readFile(filePath, (err, data) => {
    if (err) {
      fs.readFile(path.join(DIST, "index.html"), (fallbackErr, html) => {
        if (fallbackErr) {
          res.writeHead(404).end("not found");
          return;
        }
        res.writeHead(200, { "content-type": "text/html; charset=utf-8" });
        res.end(html);
      });
      return;
    }
    const type = MIME[path.extname(filePath).toLowerCase()] ?? "application/octet-stream";
    res.writeHead(200, { "content-type": type });
    res.end(data);
  });
}

function handler(req, res) {
  const url = new URL(req.url ?? "/", "http://127.0.0.1");
  const key = `${req.method} ${url.pathname}`;
  if (url.pathname === "/api/config") {
    if (req.method === "GET") {
      json(res, serializeConfig());
      return;
    }
    if (req.method === "PUT") {
      const chunks = [];
      req.on("data", (chunk) => chunks.push(chunk));
      req.on("end", () => {
        let body = {};
        const raw = Buffer.concat(chunks).toString("utf8");
        if (raw) {
          try {
            body = JSON.parse(raw);
          } catch {
            json(res, { error: "invalid json" }, 400);
            return;
          }
        }
        applyConfigPut(body);
        json(res, serializeConfig());
      });
      return;
    }
  }
  if (url.pathname.startsWith("/api/")) {
    if (url.pathname === "/api/models" && req.method === "GET" && modelsFailOnce) {
      modelsFailOnce = false;
      json(res, { error: "models unavailable" }, 500);
      return;
    }
    if (mocks[key] !== undefined && mocks[key] !== null) {
      json(res, mocks[key]);
      return;
    }
    if (req.method === "POST" || req.method === "PUT") {
      json(res, mocks[`GET ${url.pathname}`] ?? {});
      return;
    }
    json(res, {});
    return;
  }
  serveStatic(req, res);
}

async function loadPlaywright() {
  const candidates = [
    "playwright",
    pathToFileURL(path.join(ROOT, "ui", "node_modules", "playwright", "index.js")).href,
    pathToFileURL(path.join(ROOT, "node_modules", "playwright", "index.js")).href,
  ];
  let last;
  for (const spec of candidates) {
    try {
      const mod = await import(spec);
      const chromium = mod.chromium ?? mod.default?.chromium;
      if (chromium) return { chromium };
    } catch (err) {
      last = err;
    }
  }
  throw last ?? new Error("playwright is not installed");
}

async function runSmoke(base) {
  const { chromium } = await loadPlaywright();
  const executablePath = process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH || undefined;
  const browser = await chromium.launch({ headless: true, executablePath });
  const errors = [];
  try {
    const page = await browser.newPage({ viewport: { width: 1280, height: 720 } });
    page.on("pageerror", (err) => errors.push(`pageerror: ${err.message}`));
    page.on("console", (msg) => {
      if (msg.type() !== "error") return;
      const text = msg.text();
      // Expected during the models-failure resilience check.
      if (expectingModelsFailure && /500|Failed to load resource/i.test(text)) return;
      // Smoke HTTP server has no log WebSocket; visiting /logs with a mock run is expected noise.
      if (/WebSocket connection to .*\/api\/runs\/.+\/logs/i.test(text)) return;
      errors.push(`console: ${text}`);
    });

    for (const route of ROUTES) {
      const response = await page.goto(`${base}${route}`, { waitUntil: "networkidle" });
      if (!response || response.status() >= 400) {
        errors.push(`${route} HTTP ${response ? response.status() : "no response"}`);
      }
      const errorPage = await page.locator("text=Internal Error").count().catch(() => 0);
      if (errorPage) errors.push(`${route} rendered an error page`);
      await page.waitForSelector("[data-main-scroll]", { timeout: 10_000 });
    }

    await page.goto(`${base}/`, { waitUntil: "networkidle" });
    await page.waitForSelector(".change-banner", { timeout: 10_000 });
    const withNotice = await page.locator("[data-main-scroll]").evaluate((el) => el.getBoundingClientRect().top);
    await page.locator(".change-banner button").click();
    await page.waitForSelector(".change-banner", { state: "detached", timeout: 10_000 });
    const withoutNotice = await page.locator("[data-main-scroll]").evaluate((el) => el.getBoundingClientRect().top);
    if (withNotice !== withoutNotice) {
      errors.push(`main content top offset changed with notices: ${withNotice} vs ${withoutNotice}`);
    }

    // Overview running row: icon-only Logs / Restart / Stop (no ActionMenu, no text labels).
    await page.waitForSelector(".run-table .row-actions", { timeout: 10_000 });
    const runningActions = await page.locator(".run-table tbody tr").first().locator(".row-actions button").evaluateAll((buttons) =>
      buttons.map((btn) => {
        const rect = btn.getBoundingClientRect();
        return {
          title: btn.getAttribute("title") || "",
          aria: btn.getAttribute("aria-label") || "",
          text: (btn.textContent || "").replace(/\s+/g, " ").trim(),
          hasSvg: Boolean(btn.querySelector("svg.icon")),
          width: Math.round(rect.width),
          height: Math.round(rect.height),
          classes: btn.className,
        };
      }),
    );
    if (runningActions.length < 3) {
      errors.push(`overview running row expected 3 action buttons, got ${runningActions.length}`);
    } else {
      const [logsBtn, restartBtn, stopBtn] = runningActions;
      if (!/log/i.test(logsBtn.title || logsBtn.aria) || !logsBtn.hasSvg || logsBtn.text) {
        errors.push(`overview logs button broken: ${JSON.stringify(logsBtn)}`);
      }
      if (!/restart|ponownie/i.test(restartBtn.title || restartBtn.aria) || !restartBtn.hasSvg || restartBtn.text) {
        errors.push(`overview restart button broken: ${JSON.stringify(restartBtn)}`);
      }
      if (!/stop|zatrzymaj/i.test(stopBtn.title || stopBtn.aria) || !stopBtn.hasSvg || stopBtn.text) {
        errors.push(`overview stop button broken: ${JSON.stringify(stopBtn)}`);
      }
      if (!stopBtn.classes.includes("danger")) {
        errors.push(`overview stop button missing danger class: ${stopBtn.classes}`);
      }
      const sizes = runningActions.map((btn) => `${btn.width}x${btn.height}`);
      if (new Set(sizes).size !== 1) {
        errors.push(`overview running action buttons unequal size: ${sizes.join(", ")}`);
      }
    }
    // Stopped/recently-done row should still expose icon actions.
    const stoppedPlay = await page.locator(".section .run-table .row-actions button[title*='Start'], .section .run-table .row-actions button[title*='again'], .section .run-table .row-actions button[aria-label*='Start']").count().catch(() => 0);
    const stoppedIcons = await page.locator(".section .run-table .row-actions svg.icon").count().catch(() => 0);
    if (!stoppedIcons) {
      errors.push("overview stopped section missing icon actions");
    }
    void stoppedPlay;

    await page.goto(`${base}/models`, { waitUntil: "networkidle" });
    await page.waitForSelector(".data-table .grid-row", { timeout: 10_000 });
    const before = await page.locator("[data-main-scroll]").evaluate((el) => el.scrollTop);
    await page.locator(".data-table").hover();
    await page.mouse.wheel(0, 800);
    const after = await page.locator("[data-main-scroll]").evaluate((el) => el.scrollTop);
    if (after <= before) {
      errors.push(`wheel over models table did not scroll .list-pane (scrollTop ${before} -> ${after})`);
    }

    // AI tag: appears right after the project name for deps + model attribution.
    await page.goto(`${base}/projects`, { waitUntil: "networkidle" });
    await page.waitForSelector(".project-row .title-row", { timeout: 10_000 });
    const aiPlacement = await page.evaluate(() => {
      const rows = [...document.querySelectorAll(".project-row .title-row")];
      const findings = [];
      for (const row of rows) {
        const h2 = row.querySelector("h2");
        const badge = row.querySelector(".ai-badge");
        if (!h2 || !badge) continue;
        const kids = [...row.children];
        const nameIdx = kids.findIndex((el) => el === h2 || el.contains(h2));
        const badgeWrap = badge.closest("[role='presentation']") ?? badge;
        const badgeIdx = kids.findIndex((el) => el === badgeWrap || el.contains(badge));
        findings.push({
          name: h2.textContent?.trim() ?? "",
          nameIdx,
          badgeIdx,
          // Custom HoverPopover replaces the native title tooltip; aria-label keeps the summary.
          label: badge.getAttribute("aria-label") ?? "",
          nativeTitle: badge.getAttribute("title") ?? "",
        });
      }
      return findings;
    });
    if (!aiPlacement.length) {
      errors.push("no AI badges rendered next to project names");
    } else {
      for (const item of aiPlacement) {
        // Pulse/run-count may sit between the name and the AI tag; require badge after name.
        if (item.nameIdx < 0 || item.badgeIdx < 0 || item.badgeIdx <= item.nameIdx) {
          errors.push(
            `AI badge for ${item.name} is not after the project name (nameIdx=${item.nameIdx} badgeIdx=${item.badgeIdx})`,
          );
        }
        if (!item.label) {
          errors.push(`AI badge for ${item.name} has empty aria-label`);
        }
        if (item.nativeTitle) {
          errors.push(`AI badge for ${item.name} still has a native title tooltip`);
        }
      }
      const demo = aiPlacement.find((item) => item.name === "demo-app");
      if (!demo) errors.push("demo-app missing AI badge (expected model + dependency signals)");
      else if (!/openai|ai|demo-model/i.test(demo.label)) {
        errors.push(`demo-app AI tooltip missing expected reasons: ${demo.label}`);
      }
      const comfy = aiPlacement.find((item) => item.name === "ComfyUI");
      if (!comfy) errors.push("ComfyUI missing AI badge (expected deps and/or local weights)");
      else if (!/comfyui|torch|diffusers|safetensors/i.test(comfy.label)) {
        errors.push(`ComfyUI AI tooltip missing expected reasons: ${comfy.label}`);
      }
    }

    // Projects must not show crash/fail dots — only running indicators.
    const failDots = await page.locator(".project-row .dot.fail, .app-row .status-dot.fail").count();
    if (failDots) errors.push(`Projects page still shows ${failDots} fail indicator(s)`);

    // Settings categories + Plugins / Appearance.
    await page.goto(`${base}/settings`, { waitUntil: "networkidle" });
    await page.waitForSelector(".settings-nav", { timeout: 10_000 });
    const settingsNav = await page.locator(".settings-nav-btn").allTextContents();
    for (const label of ["General", "Appearance", "Open in", "Scanning", "Plugins", "About"]) {
      if (!settingsNav.some((text) => text.trim() === label)) {
        errors.push(`Settings missing category nav button: ${label}`);
      }
    }
    await page.locator(".settings-nav-btn", { hasText: "Plugins" }).click();
    await page.waitForSelector('[data-plugin="nodejs"]', { timeout: 5_000 });
    const nodeCard = await page.locator('[data-plugin="nodejs"]').textContent().catch(() => "");
    if (!/Node\.js|Vite|Next|Astro|Ports/i.test(nodeCard || "")) {
      errors.push(`Node.js plugin card missing expected content: ${nodeCard}`);
    }
    if (/\bAuto\b/.test(nodeCard || "") && /segmented/.test(await page.locator('[data-plugin="nodejs"]').innerHTML().catch(() => ""))) {
      errors.push(`Node.js plugin card still exposes Auto mode: ${nodeCard}`);
    }
    const nodeToggle = await page.locator('[data-plugin="nodejs"] input[type="checkbox"]').count();
    if (!nodeToggle) errors.push("Node.js plugin card missing on/off toggle");
    const ahkCard = await page.locator('[data-plugin="ahk"]').textContent().catch(() => "");
    if (!/AutoHotkey/i.test(ahkCard || "")) {
      errors.push(`AutoHotkey plugin card missing: ${ahkCard}`);
    }
    if (!/Windows only|available|not installed|disabled|Not installed|Nie zainstalowany/i.test(ahkCard || "")) {
      errors.push(`AutoHotkey plugin card missing status: ${ahkCard}`);
    }
    if (!/AutoHotkey v1/i.test(ahkCard || "") || !/AutoHotkey v2/i.test(ahkCard || "")) {
      errors.push(`AutoHotkey plugin card missing per-version rows: ${ahkCard}`);
    }
    const ahkToggle = page.locator('[data-plugin="ahk"] input[type="checkbox"]');
    if (!(await ahkToggle.count())) errors.push("AutoHotkey plugin card missing on/off toggle");
    else if (!(await ahkToggle.isDisabled())) {
      errors.push("AutoHotkey toggle should be disabled on non-Windows smoke host");
    }

    // Databases project headers should be distinct from child rows.
    await page.goto(`${base}/databases`, { waitUntil: "networkidle" });
    await page.waitForSelector(".grid-row.project", { timeout: 10_000 });
    const dbHeaders = await page.locator(".grid-row.project .project-name strong").count();
    const dbChildren = await page.locator(".grid-row.child").count();
    if (!dbHeaders) errors.push("Databases page missing project header rows");
    if (!dbChildren) errors.push("Databases page missing indented database rows");

    // Failed models fetch must not break the projects page.
    expectingModelsFailure = true;
    modelsFailOnce = true;
    try {
      await page.goto(`${base}/projects`, { waitUntil: "networkidle" });
      await page.waitForSelector(".project-row", { timeout: 10_000 });
      const projectsOk = await page.locator(".project-row").count();
      const crashed = await page.locator("text=Internal Error").count().catch(() => 0);
      if (!projectsOk) errors.push("projects page empty after models fetch failure");
      if (crashed) errors.push("projects page crashed after models fetch failure");
      // Still expect AI badges from aiDependencies even without models inventory.
      const depOnly = await page.locator(".project-row .ai-badge").count();
      if (!depOnly) {
        errors.push("AI badges from aiDependencies missing after models fetch failure");
      }
    } finally {
      expectingModelsFailure = false;
      modelsFailOnce = false;
    }

    await page.goto(`${base}/`, { waitUntil: "networkidle" });
    await page.waitForSelector(".sidebar-nav .nav-item .nav-icon", { timeout: 10_000 });
    const boxesOf = (selector) =>
      page.$$eval(selector, (els) =>
        els.map((el) => {
          const r = el.getBoundingClientRect();
          return { x: r.x, y: r.y };
        }),
      );
    const iconSel = ".sidebar-nav .nav-item .nav-icon";
    const beforeIcons = await boxesOf(iconSel);
    const beforeToggle = await page.locator(".sidebar-toggle .nav-icon").evaluate((el) => {
      const r = el.getBoundingClientRect();
      return { x: r.x, y: r.y };
    });
    await page.locator(".sidebar-toggle").click();
    await page.locator(".sidebar").evaluate((el) => {
      const shell = el.closest(".app-shell");
      return new Promise((resolve) => {
        const done = () => resolve();
        const target = shell ?? el;
        const timeout = setTimeout(done, 400);
        target.addEventListener(
          "transitionend",
          (event) => {
            if (event.propertyName === "grid-template-columns" || event.propertyName === "width") {
              clearTimeout(timeout);
              done();
            }
          },
          { once: true },
        );
      });
    });
    await page.waitForTimeout(50);
    const afterIcons = await boxesOf(iconSel);
    const afterToggle = await page.locator(".sidebar-toggle .nav-icon").evaluate((el) => {
      const r = el.getBoundingClientRect();
      return { x: r.x, y: r.y };
    });
    if (!beforeIcons.length) errors.push("no sidebar nav icons found");
    if (beforeIcons.length !== afterIcons.length) {
      errors.push(`sidebar icon count changed: ${beforeIcons.length} -> ${afterIcons.length}`);
    }
    for (let i = 0; i < Math.min(beforeIcons.length, afterIcons.length); i += 1) {
      const dx = Math.abs(beforeIcons[i].x - afterIcons[i].x);
      const dy = Math.abs(beforeIcons[i].y - afterIcons[i].y);
      if (dx > 1 || dy > 1) {
        errors.push(`sidebar icon ${i} jumped by dx=${dx.toFixed(2)} dy=${dy.toFixed(2)}`);
      }
    }
    if (Math.abs(beforeToggle.x - afterToggle.x) > 1 || Math.abs(beforeToggle.y - afterToggle.y) > 1) {
      errors.push(
        `sidebar toggle icon jumped by dx=${Math.abs(beforeToggle.x - afterToggle.x).toFixed(2)} dy=${Math.abs(beforeToggle.y - afterToggle.y).toFixed(2)}`,
      );
    }

    await page.goto(`${base}/settings`, { waitUntil: "networkidle" });
    await page.waitForSelector("#ignore-project", { timeout: 10_000 });
    const ignoreValues = await page.locator("#ignore-project option").evaluateAll((opts) =>
      opts.map((opt) => opt.value).filter(Boolean),
    );
    if (!ignoreValues.length) {
      errors.push("settings ignore list has no projects to add");
    } else {
      await page.locator("#ignore-project").selectOption(ignoreValues[0]);
      await page.locator("[data-ignore-add]").click();
      try {
        await page.waitForSelector("[data-ignore-chip]", { timeout: 10_000 });
      } catch {
        errors.push("adding a project to the ignore list did not show a chip");
      }
      const added = await page.locator("[data-ignore-chip]").count();
      if (added !== 1) {
        errors.push(`expected 1 ignore chip after add, got ${added}`);
      }
      if (added > 0) {
        await page.locator("[data-ignore-chip]").first().click();
        try {
          await page.waitForFunction(() => document.querySelectorAll("[data-ignore-chip]").length === 0, {
            timeout: 10_000,
          });
        } catch {
          errors.push("removing a project from the ignore list did not clear the chip");
        }
      }
      await page.goto(`${base}/settings`, { waitUntil: "networkidle" });
      await page.waitForSelector("#ignore-project", { timeout: 10_000 });
      const afterReload = await page.locator("[data-ignore-chip]").count();
      if (afterReload !== 0) {
        errors.push(`ignore removal did not persist after reload (chips=${afterReload})`);
      }
      const ignoredAfter = await page.evaluate(async () => {
        const response = await fetch("/api/config");
        const body = await response.json();
        return body.ignoredProjects ?? [];
      });
      if (!Array.isArray(ignoredAfter) || ignoredAfter.length !== 0) {
        errors.push(`GET /api/config ignoredProjects after remove+reload was ${JSON.stringify(ignoredAfter)}`);
      }
    }
  } finally {
    await browser.close();
  }
  return errors;
}

if (!fs.existsSync(path.join(DIST, "index.html"))) {
  console.error(`Missing UI build at ${DIST}. Run npm run build in ui/ first.`);
  process.exit(1);
}

const serveOnly = process.env.ORBITRY_SMOKE_SERVE_ONLY === "1";
const preferredPort = Number(process.env.ORBITRY_SMOKE_PORT || "0") || 0;
const server = http.createServer(handler);
await new Promise((resolve) => server.listen(preferredPort, "127.0.0.1", resolve));
const { port } = server.address();
const base = `http://127.0.0.1:${port}`;
console.log(`UI smoke server on ${base}`);

if (serveOnly) {
  await new Promise(() => {
    // Keep serving until the parent kills this process.
  });
}

let errors;
try {
  errors = await runSmoke(base);
} catch (err) {
  errors = [`smoke crashed: ${err instanceof Error ? err.stack ?? err.message : String(err)}`];
} finally {
  server.close();
}

if (errors.length) {
  console.error("UI smoke failed:");
  for (const line of errors) console.error(`- ${line}`);
  process.exit(1);
}
console.log("UI smoke passed.");
