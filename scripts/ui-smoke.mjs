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
const ROUTES = ["/", "/projects", "/models", "/logs", "/settings"];

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
    usedBy: i % 3 === 0 ? [{ projectId: "demo-app", projectName: "demo-app", projectPath: "/path/to/projects/demo-app" }] : [],
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
  bridgeStatus: { state: "off", label: "Off" },
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
  if (Object.prototype.hasOwnProperty.call(body, "autostartApps")) {
    configState.autostartApps = Array.isArray(body.autostartApps) ? body.autostartApps : [];
  }
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
          {
            id: "demo-app/web",
            name: "web",
            path: "/path/to/projects/demo-app",
            role: "fe",
            kind: "node",
            framework: "sveltekit",
            version: "1.0.0",
            nodeModules: true,
            parent: null,
            relativePath: ".",
            git: { branch: "main", commit: "abc1234", dirty: false },
            build: { built: true, dir: "dist", stale: false, lastBuiltAt: 1_700_000_000_000 },
            scripts: [
              { name: "dev", kind: "dev", scope: "fe", command: "npm run dev", cwd: "/path/to/projects/demo-app" },
              { name: "build", kind: "build", scope: "fe", command: "npm run build", cwd: "/path/to/projects/demo-app" },
            ],
            ports: [],
          },
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
  "GET /api/runs": { runs: [], warnings: [] },
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
    projectFiles: [],
    totals: [{ id: "ollama", label: "Ollama", path: "/path/to/models", bytes: models.reduce((sum, item) => sum + item.sizeBytes, 0) }],
    grandTotalBytes: models.reduce((sum, item) => sum + item.sizeBytes, 0),
  },
  "GET /api/docker": [],
};

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
      if (msg.type() === "error") errors.push(`console: ${msg.text()}`);
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

    await page.goto(`${base}/models`, { waitUntil: "networkidle" });
    await page.waitForSelector(".model-list", { timeout: 10_000 });
    const before = await page.locator("[data-main-scroll]").evaluate((el) => el.scrollTop);
    await page.locator(".model-list").hover();
    await page.mouse.wheel(0, 800);
    const after = await page.locator("[data-main-scroll]").evaluate((el) => el.scrollTop);
    if (after <= before) {
      errors.push(`wheel over models list did not scroll .list-pane (scrollTop ${before} -> ${after})`);
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

const server = http.createServer(handler);
await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
const { port } = server.address();
const base = `http://127.0.0.1:${port}`;
console.log(`UI smoke server on ${base}`);

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
