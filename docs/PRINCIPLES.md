# Orbitry principles

These are standing rules for the project. Follow them in every change. They replace the old roadmap checklist.

## Shape of the app

- **Rust first.** The scanner, process manager, port detection, health, and the HTTP/WebSocket server are Rust. Orbitry ships as a single native binary.
- **The dashboard opens in the system browser**, from a native tray icon, on every OS. There is no embedded webview.
- **The UI is Svelte 5 + SvelteKit** (static SPA, `adapter-static`), served by the Rust server. `package.json` exists only for the UI and the optional config-bridge plugin, not for the application itself.
- **Optional Node config-bridge.** The plugin may evaluate code-based configs (`vite.config.ts`, `next.config.js`, and similar) and return resolved ports as JSON. Orbitry must work fully when the plugin or Node is missing, falling back to regex, framework defaults, and live PID/log detection.
- **Environment-independent.** The user picks project root folders in Settings. Nothing is hardcoded to a machine or a username.

## Processes

- **`CREATE_NO_WINDOW`.** Child processes on Windows must not open a new console window. This applies to app scripts, helpers, Docker commands, and any other process Orbitry starts.
- **Detached apps.** Managed apps outlive an Orbitry update or restart. Logs go to files under the Orbitry config directory. A run registry is re-adopted on startup when the PID and process start time still match.
- **User-initiated only.** Orbitry must not auto-run apps, builds, or Docker Compose on a developer machine except when the user has clearly turned on a setting (for example per-app autostart, and only after a fresh scan). Agents and scripts must not launch, restart, or kill Orbitry or user projects on a developer machine.

## Dashboard UI

- **Overlays only for notices.** Banners, toasts, alerts, and scan summaries must be `position: fixed` or `absolute`. They must never push page content down or change the main content's top offset.
- **Single page scroller.** The app shell's main region (`.list-pane`) is the only page scroller. Tables, cards, sections, and lists must not have their own `overflow` / `max-height` scroll wrappers. The only inner scrollers allowed are logs, the warnings popover, the side panel, and modals.
- **No duplicate action buttons.** If two scripts on the same app are the same command, show one button. If they are distinct, give each a clear distinct label.
- **Aligned columns.** In an expanded project, app rows use a shared, fixed column grid so roles, names, tags, ports, and buttons line up across apps.
- **Polish and English** for the whole dashboard. New UI strings need both locales. The default follows the browser locale and is stored as `orbitry.locale`.
- **Failed optional fetches never break pages.** A missing optional API, a partial payload, or invalid `localStorage` must degrade in place, not crash the layout.
- **One mark.** The tray icon, executable icon, favicon, and sidebar logo come from a single flat SVG (`crates/server/assets/orbitry.svg`). Regenerate the raster sizes with `python scripts/generate-icons.py`.
- **Ignore list.** Projects added to the Settings ignore list stay visible, but they produce no notices or warnings: not in the alert strip, the warnings popover, scan-change notices, or desktop notifications.
- **Labeled ports.** Every port shown in the dashboard has a visible label for what it is (a name such as dev, preview, or api, or the port's source). A number alone is not enough.
- **Absolute resource scales.** Overview CPU and RAM meters use a fixed scale from zero to maximum (CPU 0–100%, RAM 0–total), not a range auto-fitted to recent samples.
- **Calm app rows.** In an expanded project, each app is one aligned row in a shared column grid: role, name and indicators, ports, live resources, and a primary script action. Autostart does not belong in the list; it stays in the details panel. All scripts from the app's `package.json` are available from a primary run action and a script menu. Build is just one of those scripts, not a separate button. Do not show an "app" / "apps" count. Missing dependencies use a warning-like "deps" indicator, with the explanation and Install action on click or in the details panel.
- **Details panel.** The side panel is wide enough to read commands and paths. It has two tabs, Details and Logs. Warnings are listed immediately, not behind a collapsed disclosure. Open in Terminal, VS Code, and the system file manager sit at the bottom of the panel. Mention a script's source only when it does not come from the app's own package.
- **No port-ignore UI.** The dashboard does not offer an "ignore this port" control. An existing `ignore` field in config is still accepted so older files load, but it is not applied.
- **Fine-grained updates.** Saving a setting must update only the state that changed. Do not replace the whole config object, remount the page, refetch the project list, or flash a loading/disabled state on unrelated controls.


## Compatibility and releases

- **Backward compatible, with versioned migrations.** Config, caches, `localStorage`, and API shapes must keep older data loading. When a shape changes, bump a version and migrate. Never require users to delete their config.
- **Semver tags.** Releases are Git tags (`vMAJOR.MINOR.PATCH`). Changelog entries are in English and become the GitHub release body.
- **Keep changelog and docs generic.** Do not put personal paths, usernames, real machine names, private project names, or local ports into `CHANGELOG.md`, README, principles, or release notes. Use generic wording and example paths such as `/path/to/projects`. Do not rewrite git history or force-push to clean this up; edit the files and, for already-published releases, update the release body only.
- **Pre-tag checks.** Before tagging: the working tree is intentional, the changelog section for that version is complete, `npm run build` in `ui/` succeeds, `cargo check --workspace` succeeds, and `cargo check -p orbitry-core --target x86_64-unknown-linux-gnu` succeeds on a Windows dev machine. Then push `main`, wait for CI to go green, tag, and confirm the Release workflow built all four targets and published. Do not add a unit-test suite unless that policy changes.

## Scope of detection

- Detect every script in an app's `package.json`. Known names (`dev`, `preview`/`start`, `build`) keep their run or build kind. Other scripts (test, lint, and similar) are listed so the user can run them, but they are not used for autostart or as the default run action.
- Default script patterns ship with the scanner. Extra patterns are added in the dashboard, not by hand-editing config.
