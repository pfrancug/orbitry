# Changelog

All notable changes to Orbitry are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.15.0] - 2026-11-26

### Added

- Scanner recognises Swift / Apple projects via `Package.swift`, `*.xcodeproj` / `*.xcworkspace`, or an XcodeGen `project.yml` (`name:` + `targets:`). They show a Swift tech tag, usual git / DB / AI signals (including Package.swift / XcodeGen packages and `*.xcdatamodeld`), and no runnable scripts.
- Fallback: a folder with its own `.git` that matches no language manifest is still listed as a generic project (no scripts). Bare-git roots do not block scanning language children of a monorepo folder. Scan cache version bumped.

## [0.14.3] - 2026-11-26

### Fixed

- Overview running-row Restart and Stop are icon buttons again (same size and alignment as Logs and other row actions). Removing the overflow menu in 0.14.2 had left text labels inside icon-sized controls.

## [0.14.2] - 2026-11-26

### Changed

- Overview running-row actions no longer use the overflow ActionMenu; Restart and Stop are direct row buttons.

## [0.14.1] - 2026-11-19

### Changed

- Plugins are plain on/off toggles (no Auto). Legacy `configBridge: "auto"` migrates on load to on when Node and the bridge plugin are available, otherwise off, and is rewritten in config.
- AutoHotkey plugin card lists v1 and v2 separately as installed (with path) or not installed. Scripts that need a missing major show a clear block reason and cannot be started; the runner no longer falls back to another major.

## [0.14.0] - 2026-11-19

### Added

- AutoHotkey plugin (Windows): detects the interpreter via registry and Program Files, treats top-level `*.ahk` files as runnable scripts (v1/v2 from `#Requires`), and runs them with `CREATE_NO_WINDOW`. Listed as unavailable on non-Windows hosts. Toggle lives under Settings → Plugins (default on when usable). Scan cache version bumped.
- Plugin cards under Settings → Plugins: Node.js (former config bridge) and AutoHotkey, each with status, framework chips, and a short list of what they read.

### Changed

- Config bridge is presented as the Node.js plugin. The existing `configBridge` setting is unchanged for older configs. package.json scripts remain a core scan, not part of the Node.js plugin.

## [0.13.0] - 2026-11-19

### Added

- Settings uses a left-side category nav (General, Appearance, Open in, Scanning, Plugins, About). Config bridge lives under Plugins and documents that it reads Vite/Next/Astro config ports only (not package.json scripts).
- Setting to include documentation Markdown (`README*.md`, `docs/`) in the AI models attribution scan (off by default). Models scan cache version bumped.
- Closable log tabs, including tabs opened from recently stopped runs.
- Meaningful exit labels on the dashboard (stopped / finished / crashed with code), English and Polish.
- Unified recently-run and recently-stopped lists (preview 3 + show more, hard cap 10).

### Changed

- Tag hover popups no longer show a native browser tooltip on top, and stay open while the pointer moves into the popup.
- Projects page shows only a running indicator; crash/exit status belongs on the Overview.
- Favorites are removed. Older `orbitry.favorites` localStorage is ignored and cleared on migrate (storage version 5).
- Databases project group headers are visually distinct from indented database rows.
- Run payloads may include optional `stopRequested` for stopped-by-user labeling; older clients ignore it.

## [0.12.4] - 2026-11-19

### Fixed

- Per-app Stop and Restart from the Overview menu work again. ActionMenu was calling `stopPropagation` on clicks, which blocked Svelte 5's delegated `onclick` handlers so the stop API never ran (Stop all still worked). Single-app stop now also resolves the live run by part + script on the server the same way Stop all discovers targets.
- Overlay toasts and banners auto-dismiss after about 6 seconds (about 10 seconds for errors), stay closable with X, and pause while hovered.

## [0.12.3] - 2026-11-19

### Changed

- Releases from 0.12.3 on are licensed under [PolyForm Noncommercial 1.0.0](https://polyformproject.org/licenses/noncommercial/1.0.0/). Earlier releases keep their original license.

## [0.12.2] - 2026-11-19

### Fixed

- The AI tag shows again on projects that use local models or AI tooling. Attribution matches model `usedBy` and weight-file ownership with normalized paths (including Windows separators and case), and also marks projects that declare AI dependencies such as `openai`, `@anthropic-ai/sdk`, `ai`, `langchain`, `transformers`, `torch`, `diffusers`, `ollama`, or `comfyui`. The tag tooltip lists the models and dependencies that triggered it. A failed models inventory fetch no longer breaks the Projects page.
- Databases and Models tables use a shared CSS grid so project rows, indented database rows, and model columns stay aligned, with ellipsis truncation, sticky headers, and sorting. Layout is verified in the UI smoke path with mock data.

### Changed

- Project scan records optional `aiDependencies` on each part (scan cache version bumped). Older clients ignore the field.

## [0.12.1] - 2026-11-19

### Fixed

- The same SQLite (or other file) database no longer appears twice when a monorepo root and a nested app both detect it. Entries are deduped by canonical absolute path for files and by engine + host + port for server databases, keeping the richer entry. The same rules apply on the Databases page, project tags, and the details panel.
- Databases and Models pages use real tables with a shared column layout and a sticky header so headers and cells stay aligned, including indented database rows under each project.
- Models "Used by" shows the owning scanned project name only (deduped), not parent or nested folder path segments. Paths outside any scanned project still fall back to the nearest folder name. The models scan cache version is bumped so attribution refreshes.

### Changed

- The README describes the product and where to download it. Development and build instructions were removed.

## [0.12.0] - 2026-11-19

### Added

- A Databases page lists local databases grouped by project, with file size for on-disk databases and "—" for server databases (Orbitry never connects). Columns are sortable; default sort is project name. The scan cache version is bumped so previous snapshots pick up sizes.
- Project and app rows show an AI tag when models inventory attributes models to that project. Tag order is AI, database, deps, then warning last, each with a distinct color.
- Settings shows a Changelog link next to the app version that opens the GitHub CHANGELOG in the default browser.
- Hover tooltips for warning, deps, database, and AI tags render in a fixed overlay that flips and shifts to stay in the viewport, so they are no longer clipped by expanded project panels.

### Changed

- The "changed" markers on projects and apps are removed. The post-scan notice no longer lists change details; it only reports that the scan is complete.
- The Models table columns are sortable (click a header to toggle ascending/descending). Default sort is model name ascending.

## [0.11.0] - 2026-11-19

### Added

- The details panel can open a project folder in Cursor next to the existing VS Code action. Orbitry launches the Cursor editor window (not the Agents view) via `--classic`, detects whether Cursor is installed, and disables the action with a tooltip when it is missing. On Windows the launcher uses `CREATE_NO_WINDOW`. The tools API gains an optional `cursorInstalled` field so older clients keep working.
- Settings include an Open in checklist for Terminal, VS Code, Cursor, and the system file manager. Choices persist in config (with defaults that keep every action visible, including Cursor). Older configs load without changes. If every target is unchecked, the Open in footer hides. Client storage version is bumped for a clean migration.

## [0.10.0] - 2026-11-19

### Added

- Project scan detects local databases and shows them on project and app rows, with details in the side panel. Signals include SQLite files, Prisma schema providers, Docker Compose database services and host ports, local `DATABASE_URL` / similar env values, and common database client dependencies. Each entry shows the engine, where it was found, and a port or file path when known. Passwords in connection strings are never kept. Scanning stays file-based and does not open or connect to databases.
- Optional `databases` field on scanned project parts. Older clients and empty results stay compatible; the scan cache version is bumped so previous snapshots are refreshed.

## [0.9.4] - 2026-11-17

### Fixed

- Starting, restarting, or installing a script from the dashboard no longer opens the details panel. The panel still opens only from a list item, and it still closes when that item is clicked again or when the panel is closed. If the panel is already open, it stays open and follows the script that was just started. Port links, the logs action, and the overflow menu also leave the panel alone unless they are meant to open it.

## [0.9.3] - 2026-11-17

### Fixed

- Port labels prefer a name from the project's configuration over a name guessed from log output. Logs still confirm that a port is listening, but a line that only mentions another URL is no longer treated as a bound port and can no longer overwrite a config-derived name.
- Fallback port values in source, such as `process.env.PORT || …`, are recognized. If that variable is set in a project `.env` file or in the start script (for example `PORT=… npm run dev`), that value is used; otherwise the fallback after `||` or `??` is used. A framework default is only shown when neither is present.

## [0.9.2] - 2026-11-17

### Fixed

- Removing a project from the Settings ignore list now works and stays gone after a reload. Adding a project still works. Saving settings no longer aborts before the request is sent when the in-memory config cannot be cloned, so the updated ignore list is actually stored.

## [0.9.1] - 2026-11-16

### Fixed

- The left sidebar now animates when it collapses to the icon rail or expands again. Icons stay in a fixed column and no longer jump or recenter. Labels fade and clip out, and the collapse control stays in place.
- The details panel slides in from the right and fades when it opens or closes, including close from the X or by clicking the same list item again. It remains an overlay, so it does not push the page.
- Remaining top-bar and sidebar chrome strings (Refresh, Control plane, Collapse, scan status) are translated in Polish and English.

## [0.9.0] - 2026-11-16

### Changed

- The dashboard is a compact desktop layout: 13px base type, 12px secondary text, restrained 16px page titles, 20px KPI figures, 28px-tall controls, tighter spacing, and 6px radii. Inter replaces the previous display and body fonts. The same tokens apply across Overview, Projects, Models, Logs, Settings, the side panel, modals, and popovers.
- Expanding or collapsing a project is a smooth height and opacity animation. Chevrons rotate in place and no longer jump.
- The unused top-bar menu button is gone. The left sidebar stays available and can still collapse to an icon rail from its own control.
- The details panel is no longer opened from a top-bar icon. It opens from a list item and closes when that item is clicked again or when the panel is closed. Closed panel content is not kept or restored. Existing saved panel and popover state is cleared by a localStorage migration.
- The warnings popover is wider and simpler: a short header, a single scrolling list, minimal dividers, and an Open details action when the warning belongs to an app. On narrower desktop widths the panel overlays on the right instead of becoming a mobile bottom sheet.

## [0.8.0] - 2026-11-16

### Added

- The details side panel has Details and Logs tabs, and a footer with Open in Terminal, VS Code, and the system file manager (File Explorer on Windows, Finder on macOS, the file manager on Linux). Those open actions are user-initiated and use `CREATE_NO_WINDOW` on Windows.
- The Logs page uses a console-style tab strip instead of pill chips, with a dark monospace output area.
- On Overview, each running process keeps a logs link. Restart and Stop moved into a "…" menu. The port itself is the link to the app; the extra external-link icon is gone.

### Changed

- On the AI Models page, "Used by" shows unique project names, not a list of file paths. Paths stay in the expandable row details. The list has column headers, and the summary cards put the label under the number (models, total, sources).
- Project app rows are a single aligned row in the style of a process list: role, name and indicators, ports, resources, and a primary script action with a menu of every script from the app's `package.json`. Autostart was removed from the list and stays in the details panel. The separate Build button and section are gone; build is one of the scripts. A script's source is shown only when it is not the app's own package. Missing dependencies show as a "deps" indicator next to warnings, with the explanation and Install action on click or in the details panel.
- The details panel is wider. Warnings in the panel are listed immediately, not behind a collapsed disclosure.
- The "Ignore port" control was removed from the UI. Existing config may still contain an `ignore` field; it is accepted and ignored so older files keep loading.
- Settings saves no longer replace the whole dashboard snapshot or flash the form as busy. Only the changed config fields update, and the project list is refetched only when folders or script patterns change.
- Button hover and focus styles are consistent: a subtle background and border change, no layout shift, and a visible focus ring. Primary buttons keep their fill on hover instead of dropping back to a faint background.

### Detection

- The scanner now lists every script in an app's `package.json`. Known run and build names stay classified as before. Other scripts are available to run but are not used for autostart or as the default run action.

## [0.7.0] - 2026-11-16

### Added

- An ignore list in Settings. You can add a project from the current project list, and remove it later. Ignored projects stay in Projects, but they produce no notices or warnings in the alert strip, the warnings popover, scan-change notices, or desktop notifications. Existing config files stay valid; the list defaults to empty.
- Standing principles for the ignore list, labeled ports, absolute resource scales, and calmer app rows.

### Changed

- Overview CPU and RAM meters use a fixed scale from zero to maximum (CPU 0–100%, RAM 0–total memory), instead of auto-scaling to recent samples. The meters are larger.
- Every visible port has a label for what it is, such as dev, preview, api, or the port's source, not only a number.
- Project app rows are two lines, with the autostart control in a fixed position on the left. Run scripts stay in the row. Build actions moved to an advanced Build section in the side panel. The "app" / "apps" count on the project row is gone. Missing dependencies are a labeled status with an explanation and an install action, not a short orange badge.
- The side panel's command preview is split into labeled fields: Command in a monospace block, Working directory on its own line, and Source underneath.
- On the AI Models page, "Used by" is one chip per full path where the model is used. Long paths are truncated, with the full path in a tooltip.

## [0.6.0] - 2026-11-16

### Added

- Standing product principles in `docs/PRINCIPLES.md`, linked from the README. They record the rules this release is built around: overlays that never push layout, a single page scroller, no duplicate action buttons, aligned app-row columns, Polish and English, backward-compatible config, and generic changelog wording.
- CI UI smoke: after the production UI build, a headless Chromium pass opens every dashboard route against mock APIs, fails on page errors or console errors, checks that a scan notice does not change the main content's top offset, and checks that a wheel gesture over a models list scrolls the main page region. CI and Release Linux jobs are pinned to Ubuntu 24.04 so Playwright can install Chromium.
- Incremental AI model scanning with a persisted directory index, so repeated scans skip unchanged weight trees and still return cached Ollama and cache results first.
- Optional desktop notifications when an app exits unexpectedly or its health turns red. The toggle is in Settings and defaults to off. Existing config files stay valid without the new field.
- Docker Compose on the Projects page: compose files are detected next to a project, service and container state is shown when Docker is available, and Up, Down, Restart, and Logs are user-initiated. When Docker is missing, the services from the file are still listed and the actions stay disabled. Windows CLI calls use `CREATE_NO_WINDOW`.
- A single flat SVG mark for the tray icon, executable icon, favicon, and sidebar logo, derived from the previous exe/tray planet and moon, with fewer details so it stays crisp at 16px. `python scripts/generate-icons.py` regenerates the raster sizes from that SVG.

### Changed

- In-flow notices, including "Changes since last scan" and the error banner, are overlays anchored to the top bar. They can be dismissed and no longer push page content down.
- The app shell's main region is the only page scroller. Tables, cards, and section lists no longer use their own scroll wrappers. Logs, the warnings popover, the side panel, and modals may still scroll internally.
- The AI Models page is a top summary (count, total disk, sources) plus a simple list of name, size, location, and "Used by" project chips, sorted by size. Paths, IDs, and dates sit in expandable rows. The page is fully translated.
- Build actions no longer repeat the same command as several identical "Build" buttons. One button is shown when the commands match; distinct build scripts get distinct labels.
- App rows in an expanded project share a fixed column grid so roles, names, tags, ports, and buttons line up across apps.

### Fixed

- Historical changelog wording is generic. Personal paths, usernames, and private project names were removed from this file and from already-published GitHub release notes.

## [0.5.3] - 2026-11-16

### Fixed

- Expanding a project tagged Changed could freeze the Projects page. After that, the page could stay broken even after restarting Orbitry, because the open row was saved in localStorage. The crash came from two scripts on the same app sharing a list key (a package script and a workspace script with the same name). Those lists now use unique keys. A client storage migration resets saved expanded rows so an already-stuck session opens cleanly.

## [0.5.2] - 2026-11-16

### Fixed

- The dashboard showed "500 Internal Error" on every page in 0.5.1. The language store used a Svelte rune (`$state`) in a plain `i18n.ts` file, so the compiler left a literal `$state(...)` call in the production bundle. The browser then threw `ReferenceError: $state is not defined` while loading the root layout. Clearing localStorage did not help. The store now lives in `i18n.svelte.ts` so the rune is compiled. A new error page shows the real message and a button to reset Orbitry's browser cache without touching the on-disk config. Client storage is migrated in a versioned way and invalid values are dropped. API JSON is parsed defensively. A scan diff that omits empty `added` or `removed` arrays no longer throws while rendering the layout.

## [0.5.1] - 2026-11-16

### Added

- Polish and English for the whole dashboard, with a language switch in Settings. The default follows the browser locale (`pl` if the language starts with `pl`) and is stored in `localStorage` as `orbitry.locale`.
- A tooltip and explanation on the orange "deps" badge ("Dependencies are not installed — run npm install"), plus an Install dependencies action that runs the package manager install in the app folder. It is user-initiated, like Start.
- Detection of build scripts (`build`, `build:*`) as their own kind, with a separate Build button. Preview is disabled when there is no build output, and a warning is shown when sources are newer than the last build. Build detection now covers `.svelte-kit/output`, `.output`, and `.nuxt` as well as `dist`, `build`, `.next`, and `out`.
- Workspace-targeted root scripts (for example `npm run dev -w apps/frontend`) are attached to the target app, run with the workspace root as the working directory, and labeled with the real command and package they come from.
- "Clear all" and per-item clear for recent scripts and for Recently stopped on Overview. Dismissals are persisted.

### Changed

- The alerts popover is a flat list. Identical messages are deduped instead of being collapsed into expandable groups by warning code.
- Overview CPU and RAM sparklines are taller (40px) and scaled to the data range with padding, so small changes are visible.
- Settings are laid out as cards with headings, hints, and a consistent compact control scale.
- Project rows use a three-column layout (name | ports | actions) so ports and start buttons stay on one line. Long names truncate with a tooltip.
- The app detail panel is organized into sections (App, Autostart, Run, Status, Warnings, Ports, Logs). The script selector shows a human-readable label and a preview of the command that will run, from which package or workspace root, instead of a synthetic "dev · fe" name.
- Historical entries in this changelog use the English UI names.

### Fixed

- The AI Models scan could take a long time on a large project root and left the page looking empty for the whole time. The first request after startup returned an empty inventory in state "scanning" because results were published only when the walk and attribution finished. The scanner now publishes Ollama and cache results first, skips backup/archive/data trees when attributing "Used by", caps how many source files it reads, and falls back to `USERPROFILE` / `HOMEDRIVE`+`HOMEPATH` if the home directory is missing. The page shows last-scan time and scan errors.
- Updating 0.4.0 to 0.5.0 could finish in the background with no progress overlay. The overlay's status poller lived only inside the overlay component, and that component was mounted only after the UI already believed an update was in progress (`update.installing`, which the in-app Install button sets immediately). A tray-triggered update never set that flag in open tabs. The layout only polled `/api/version` every 60 seconds, and this update finished in about ten seconds, so tabs never saw `downloading` / `installing` / `restarting` and never started the post-restart reload poller. Every open tab now polls update status about once a second (every 300ms while an update is active) and shows the overlay for tray- or UI-triggered updates, then reloads when the new version comes up.

## [0.5.0] - 2026-11-16

### Added

- An "AI Models" page that inventories local AI models in the background. It lists Ollama models (install, version, and the service at 127.0.0.1:11434, or the on-disk manifests if the service is down), Hugging Face, torch hub, InsightFace, and Whisper caches, and weight files under configured project roots. A "Used by" column attributes models to projects by scanning source and config for known names. Totals are shown per source and overall. The scan never runs on the request path; results are cached in `models-cache.json` and refreshed with "Scan again".

## [0.4.0] - 2026-11-16

### Added

- Per-app autostart. Each app has a "Start with Orbitry" toggle, stored in the Orbitry config by the app's stable id. The control is disabled, with a tooltip, while "Start Orbitry with the system" is off (the real OS autostart state). When that system setting is on, selected apps start after Orbitry itself starts — from login or a manual launch — once the first fresh scan of the process completes, not from a cached inventory. Later rescans do not start them again. Apps that are already running, including those re-adopted from the run registry, are skipped. An app starts only when every assigned or expected port is free on both IPv4 and IPv6; if a port is taken, that app is left stopped and a warning names the occupying process. Starts are staggered slightly. Outcomes are logged, and an overlay toast summarizes the result.

## [0.3.1] - 2026-11-16

### Added

- A full-screen update overlay in the dashboard, with download progress, SHA-256 verification, install, and restart steps. The page reloads when the new version comes up. Failures and rollbacks are shown in place.
- Changelog-backed GitHub release notes: each tag publishes the matching section of this file instead of a compare link alone.
- Detection of a VS Code install, so Open in VS Code can be disabled with an explanation when it is missing.
- Clear labels and tooltips for the Terminal and VS Code actions.
- A web UI favicon based on the tray icon, with an SVG primary, PNG/ICO fallbacks, an Apple touch icon, and a theme-color meta tag.
- Overview resource gauges for system CPU and memory, plus per-app CPU and RSS (the whole process tree) on running rows and in the detail panel. Samples are taken in the background every two seconds.
- Managed apps now survive an Orbitry update or restart. Processes are started detached, logs go to files under the Orbitry config directory (`logs/`, on Windows `%APPDATA%\orbitry\logs`), and a run registry is re-adopted on startup when the PID and process start time still match.
- A Settings control, "Max log size per app", defaulting to 10 MB per app (5 MB plus one rotation, range 2–100 MB). The global cap scales with this value (at least 200 MB, or 20× the per-app limit). Changes apply without a restart.

- A Settings toggle, "Start Orbitry with the system", off by default. When it is on, Orbitry starts in the tray at login and does not open the browser (`--autostart`). The real state is read from the operating system, not only from the app config: a Current Version Run key on Windows, a Launch Agent on macOS, and an autostart `.desktop` file on Linux. The registered executable path is updated if the app moves or is updated. The same option is available as a checkbox in the tray menu.

### Changed

- The control panel no longer offers port-name or health-URL overrides. Health checks use the detected port automatically. You can still ignore a false-positive port.
- Update steps are written to the Orbitry config directory (`update.log`, on Windows `%APPDATA%\orbitry\update.log`).
- The update notification is a dismissible overlay toast. It no longer pushes the page down. A manual Check now uses the same toast, and Settings show the current update status next to the button.
- Warnings no longer push the page down. The summary is a compact chip in the top bar, and the list opens as a scrollable popover.
- Tray Quit stops managed apps and then exits. An update restart leaves them running so the new process can re-adopt them. If shutdown hangs for about five seconds, Orbitry exits anyway.

### Fixed

- Linux and macOS builds compile again. The detached log pump now calls `setsid` on the real command, and Windows-only process-creation flags are no longer imported on Unix. The in-app updater only offers a GitHub release that is not a draft and that includes both the build for this system and `SHA256SUMS.txt`.

- Installing an update could leave Orbitry hung after the dashboard went down, then fail with "timed out waiting for process … to exit". The outgoing process now leaves the tray and exits, and the incoming process will stop a leftover instance before swapping binaries. If the swap fails, Orbitry restores the previous executable and launches it again.
- After a failed or timed-out update, the tray menu could stop responding, including Quit. Quit is handled in every updater state, failed updates reset the updater so the menu stays usable, and a short exit watchdog covers a stuck shutdown.

## [0.3.0] - 2026-11-16

Not released — build failed; changes shipped in 0.3.1.

### Added

- A Settings toggle, "Start Orbitry with the system", off by default. When it is on, Orbitry starts in the tray at login and does not open the browser (`--autostart`). The real state is read from the operating system, not only from the app config: a Current Version Run key on Windows, a Launch Agent on macOS, and an autostart `.desktop` file on Linux. The registered executable path is updated if the app moves or is updated. The same option is available as a checkbox in the tray menu.

## [0.2.0] - 2026-11-16

Not released — build failed; changes shipped in 0.3.1.

### Added

- A full-screen update overlay in the dashboard, with download progress, SHA-256 verification, install, and restart steps. The page reloads when the new version comes up. Failures and rollbacks are shown in place.
- Changelog-backed GitHub release notes: each tag publishes the matching section of this file instead of a compare link alone.
- Detection of a VS Code install, so Open in VS Code can be disabled with an explanation when it is missing.
- Clear labels and tooltips for the Terminal and VS Code actions.
- A web UI favicon based on the tray icon, with an SVG primary, PNG/ICO fallbacks, an Apple touch icon, and a theme-color meta tag.
- Overview resource gauges for system CPU and memory, plus per-app CPU and RSS (the whole process tree) on running rows and in the detail panel. Samples are taken in the background every two seconds.
- Managed apps now survive an Orbitry update or restart. Processes are started detached, logs go to files under the Orbitry config directory (`logs/`, on Windows `%APPDATA%\orbitry\logs`), and a run registry is re-adopted on startup when the PID and process start time still match.
- A Settings control, "Max log size per app", defaulting to 10 MB per app (5 MB plus one rotation, range 2–100 MB). The global cap scales with this value (at least 200 MB, or 20× the per-app limit). Changes apply without a restart.

### Changed

- The control panel no longer offers port-name or health-URL overrides. Health checks use the detected port automatically. You can still ignore a false-positive port.
- Update steps are written to the Orbitry config directory (`update.log`, on Windows `%APPDATA%\orbitry\update.log`).
- The update notification is a dismissible overlay toast. It no longer pushes the page down. A manual Check now uses the same toast, and Settings show the current update status next to the button.
- Warnings no longer push the page down. The summary is a compact chip in the top bar, and the list opens as a scrollable popover.
- Tray Quit stops managed apps and then exits. An update restart leaves them running so the new process can re-adopt them. If shutdown hangs for about five seconds, Orbitry exits anyway.

### Fixed

- Installing an update could leave Orbitry hung after the dashboard went down, then fail with "timed out waiting for process … to exit". The outgoing process now leaves the tray and exits, and the incoming process will stop a leftover instance before swapping binaries. If the swap fails, Orbitry restores the previous executable and launches it again.
- After a failed or timed-out update, the tray menu could stop responding, including Quit. Quit is handled in every updater state, failed updates reset the updater so the menu stays usable, and a short exit watchdog covers a stuck shutdown.

## [0.1.3] - 2026-11-16

### Changed

- Global warnings collapse into a compact summary bar with counts, a scrollable list, and grouping by warning code. Project rows use a badge instead of a wall of text.
- The dashboard uses slim, app-style scrollbars and keeps scrolling inside the pane you are looking at.

### Fixed

- False port-conflict warnings in monorepos (for example a shared port 3190 claimed by a root `dev` script, a frontend proxy, a backend, and a library). Orbitry no longer treats Vite proxy targets, shared-package constants, composite root scripts, or a repo-root `.env` as bind ports for every workspace package. Library packages without a dev server are not assigned a listen port.

## [0.1.2] - 2026-11-16

### Changed

- Orbitry stays a browser dashboard opened from the tray on every OS. The embedded-webview plan was dropped.

### Fixed

- Left-clicking the tray icon opened the dashboard and the context menu at the same time. The menu now opens on right-click only.
- Saving settings could freeze the app. Rescans now run in the background, and only when roots or patterns change.
- Refreshing the Projects page showed a skeleton instead of the projects you already had. The cached list paints immediately.
- The config-bridge setting did not persist and always looked like Auto, with an unknown status. The mode is saved, and Settings show whether Node is installed and which version.

## [0.1.1] - 2026-11-16

### Fixed

- Release builds now stamp the version from the Git tag, so the app and the GitHub Release agree on the version number.

## [0.1.0] - 2026-11-16

### Added

- First tagged builds for Windows, macOS, and Linux, served from GitHub Releases, with in-app update checks from the tray.
