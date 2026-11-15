# Orbitry roadmap

## Guiding principles

- **The whole app is Rust.** The scanner (including `package.json` via `serde_json`), script detection, process manager (`portable-pty`), port detection, git/build/version info, and the HTTP/WebSocket server are all Rust. The Rust server serves the UI. Later, the Tauri tray app reuses the same Rust core.
- **One exception: an optional Node config-bridge plugin.** It acts as an intermediary that evaluates code-based configs (`vite.config.ts`, `next.config.js`, and similar) and returns resolved ports and settings to the Rust core as JSON. When the plugin is not available, the core falls back to regex, framework default ports, and live PID/log detection. The plugin is not a foundation of the app — Orbitry must work fully without it.
- **The UI stays React + Vite**, served by the Rust server. `package.json` exists only for the UI and the optional plugin, not for the application itself.
- **Run scripts only.** Test, lint, and similar scripts are out of scope and must not be scanned. Detect only what actually runs an app: **dev** and **preview/start**.
- **Patterns live in the dashboard.** Defaults ship with the scanner. Extra script patterns per category (shared / FE / BE, dev / preview) are added in the UI — no hand-written config required. The scanner uses defaults plus whatever the user added.
- **Environment-independent.** The user picks one or more project root folders in the dashboard. That choice is stored in user config, never hardcoded to a machine.

## How to run (first slice)

Single command, after the UI has been built once (`npm run build` in `ui/`):

```
cargo run -p orbitry -- --root C:\Projects\private
```

Then open http://127.0.0.1:4280 (the process opens the browser unless you pass `--no-open`). `--root` can be repeated and is saved to user config (`%APPDATA%\orbitry\config.json` on Windows, `~/Library/Application Support/orbitry` or `~/.config/orbitry` elsewhere). `--port` defaults to `4280`.

Hot-reload UI during development (API + Vite):

```
powershell -File scripts/dev.ps1 -Root C:\Projects\private
```

or, in two terminals:

```
cargo run -p orbitry -- --root C:\Projects\private --no-open
cd ui
npm run dev
```

The Vite dev server is http://127.0.0.1:4281 and proxies `/api` to the Rust server.

## Phases

### 1. Scaffold the Rust workspace + UI

- [x] Grow the Cargo workspace: `orbitry-core` plus the `orbitry` server binary
- [x] Minimal Rust HTTP server that serves a React + Vite UI
- [x] Local-first: one binary should eventually open the dashboard in the browser

### 2. Project root picker and config

- [x] First-run (and later) folder picker for one or more project roots
- [x] Persist roots and preferences in user config (e.g. `~/.orbitry/config.json`), not in the repo
- [x] Paths and process spawning work the same on Windows, macOS, and Linux
- [ ] Native folder dialog (text input is enough for now)

### 3. Project scanner (Rust core)

- [x] Grow `crates/core` (`orbitry-core`) into the scanner
- [x] Read `package.json` with `serde_json`, including workspaces and `apps/*`
- [x] Also discover `pyproject.toml`, `Cargo.toml`, `go.mod`, and `docker-compose`
- [x] Return a structured inventory the UI can render

### 4. Script detection, classification, and custom patterns UI

- [x] Auto-detect only **dev** and **preview/start** scripts (never tests)
- [x] Classify each as shared, FE, or BE — from dependencies (vite / next / react = FE; hono / express / fastapi = BE), script names (`dev:web`, `dev:api`, `dev:all`), and folders (`web`, `server`, `api`)
- [x] Dashboard UI to add extra script patterns per category; scanner uses defaults + those patterns

### 5. Git, build, and version info (Rust core)

- [x] Per app: version, git branch / commit / dirty state
- [x] Built or not, plus last build time from `dist` / `build` / `.next` mtime
- [x] Whether `node_modules` is installed

### 6. Process manager and live logs (Rust server)

- [ ] Start and stop scripts as child processes in the project's own cwd, via `portable-pty` — no new console windows
- [ ] Kill the full process tree on stop (important on Windows)
- [ ] Stream live logs per process to the UI over WebSocket from the Rust server
- [ ] Optional actions: open in the system terminal, open in VS Code

### 7. Port detection (Rust core + optional plugin)

- [ ] Static: `.env`, CLI flags, framework defaults, and regex on code-based configs
- [ ] Live: ports bound by the process PID tree (`netstat` / `lsof`)
- [ ] From log lines such as `Local: http://localhost:5173`
- [ ] If the config-bridge plugin is present, use it for resolved ports/settings from `vite.config.ts`, `next.config.js`, and similar
- [ ] If the plugin is missing, fall back to regex + framework defaults + live PID/log detection

### 8. Optional Node config-bridge plugin

- [ ] Small, optional package under `plugins/config-bridge` — Orbitry runs without it
- [ ] Evaluates code-based configs and returns resolved ports and settings to the Rust core as JSON
- [ ] The core never depends on Node being installed

### 9. Health

- [ ] Running or not, plus a health check when a project exposes one (e.g. `/api/health`)

### 10. Dashboard UI polish

- [x] First visible slice: custom dark, iOS-like cards (not a generic component kit), search, root picker, settings sheet
- [ ] Further polish and denser / grouped layouts as the app grows
- [ ] Clear per-app cards: version, git, build, running state, ports, FE/BE split
- [ ] Calm, precise, instrument-panel feel
- [x] The Rust server serves the built UI

### 11. Packaging

- [ ] Ship Orbitry as a Rust binary (not a Node app)
- [ ] Optional later: a thin npm wrapper only if `npx` convenience is worth it
- [ ] `package.json` remains only for the UI and the optional config-bridge plugin

### 12. Later: Tauri tray app

- [ ] Menu-bar / tray companion with a Rust backend
- [ ] Reuse `orbitry-core`; the React UI stays