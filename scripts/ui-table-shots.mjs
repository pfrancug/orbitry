#!/usr/bin/env node
/**
 * Visual capture for Databases + Models tables (EN/PL × 1280/1600/1920).
 * Requires a built ui/dist. Starts ui-smoke in ORBITRY_SMOKE_SERVE_ONLY mode.
 */
import path from "node:path";
import fs from "node:fs";
import { fileURLToPath } from "node:url";
import { spawn } from "node:child_process";
import { createRequire } from "node:module";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const OUT = "/opt/cursor/artifacts";
const WIDTHS = [1280, 1600, 1920];
const LOCALES = ["en", "pl"];
const PORT = "35359";

async function loadPlaywright() {
  const require = createRequire(import.meta.url);
  try {
    return require(path.join(ROOT, "ui", "node_modules", "playwright"));
  } catch {
    return require("playwright");
  }
}

async function main() {
  fs.mkdirSync(OUT, { recursive: true });
  const smokePath = path.join(ROOT, "scripts", "ui-smoke.mjs");
  const child = spawn(process.execPath, [smokePath], {
    cwd: ROOT,
    env: { ...process.env, ORBITRY_SMOKE_SERVE_ONLY: "1", ORBITRY_SMOKE_PORT: PORT },
    stdio: ["ignore", "pipe", "pipe"],
  });
  let base = "";
  await new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error("server start timeout")), 20000);
    const onData = (buf) => {
      const text = buf.toString();
      process.stderr.write(text);
      const m = text.match(/http:\/\/127\.0\.0\.1:\d+/);
      if (m) {
        base = m[0];
        clearTimeout(timer);
        resolve();
      }
    };
    child.stdout.on("data", onData);
    child.stderr.on("data", onData);
    child.on("exit", (code) => {
      if (!base) reject(new Error(`server exited ${code}`));
    });
  });

  const playwright = await loadPlaywright();
  const browser = await playwright.chromium.launch({
    headless: true,
    executablePath: process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH || undefined,
  });

  try {
    for (const locale of LOCALES) {
      for (const width of WIDTHS) {
        const page = await browser.newPage({ viewport: { width, height: 900 } });
        await page.addInitScript((loc) => {
          localStorage.setItem("orbitry.locale", loc);
          localStorage.setItem("orbitry.storageVersion", "4");
        }, locale);
        for (const route of ["databases", "models", "projects"]) {
          await page.goto(`${base}/${route}`, { waitUntil: "networkidle" });
          if (route === "projects") {
            await page.waitForSelector(".ai-badge", { timeout: 10000 });
          } else {
            await page.waitForSelector(".data-table", { timeout: 10000 });
          }
          await page.waitForTimeout(250);
          const file = path.join(OUT, `table-${route}-${locale}-${width}.png`);
          await page.screenshot({ path: file, fullPage: false });
          console.log("wrote", file);
        }
        await page.close();
      }
    }
  } finally {
    await browser.close();
    child.kill("SIGTERM");
  }
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
