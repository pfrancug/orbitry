# Changelog

All notable changes to Orbitry are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.4.0] - 2026-11-16

### Added

- Per-app autostart. Each app has a "Uruchamiaj przy starcie Orbitry" toggle, stored in the Orbitry config by the app's stable id. The control is disabled, with a tooltip, while "Uruchamiaj Orbitry razem z systemem" is off (the real OS autostart state). When that system setting is on, selected apps start after Orbitry itself starts — from login or a manual launch — once the first fresh scan of the process completes, not from a cached inventory. Later rescans do not start them again. Apps that are already running, including those re-adopted from the run registry, are skipped. An app starts only when every assigned or expected port is free on both IPv4 and IPv6; if a port is taken, that app is left stopped and a warning names the occupying process. Starts are staggered slightly. Outcomes are logged, and an overlay toast summarizes the result.

## [0.3.1] - 2026-11-16

### Added

- A full-screen update overlay in the dashboard, with download progress, SHA-256 verification, install, and restart steps. The page reloads when the new version comes up. Failures and rollbacks are shown in place.
- Changelog-backed GitHub release notes: each tag publishes the matching section of this file instead of a compare link alone.
- Detection of a VS Code install, so Open in VS Code can be disabled with an explanation when it is missing.
- Clear labels and tooltips for the Terminal and VS Code actions.
- A web UI favicon based on the tray icon, with an SVG primary, PNG/ICO fallbacks, an Apple touch icon, and a theme-color meta tag.
- Overview resource gauges for system CPU and memory, plus per-app CPU and RSS (the whole process tree) on running rows and in the detail panel. Samples are taken in the background every two seconds.
- Managed apps now survive an Orbitry update or restart. Processes are started detached, logs go to files under the Orbitry config directory (`logs/`, on Windows `%APPDATA%\orbitry\logs`), and a run registry is re-adopted on startup when the PID and process start time still match.
- A Settings control, "Maks. rozmiar logów na aplikację", defaulting to 10 MB per app (5 MB plus one rotation, range 2–100 MB). The global cap scales with this value (at least 200 MB, or 20× the per-app limit). Changes apply without a restart.

- A Settings toggle, "Uruchamiaj Orbitry razem z systemem", off by default. When it is on, Orbitry starts in the tray at login and does not open the browser (`--autostart`). The real state is read from the operating system, not only from the app config: a Current Version Run key on Windows, a Launch Agent on macOS, and an autostart `.desktop` file on Linux. The registered executable path is updated if the app moves or is updated. The same option is available as a checkbox in the tray menu.

### Changed

- The control panel no longer offers port-name or health-URL overrides. Health checks use the detected port automatically. You can still ignore a false-positive port.
- Update steps are written to the Orbitry config directory (`update.log`, on Windows `%APPDATA%\orbitry\update.log`).
- The update notification is a dismissible overlay toast. It no longer pushes the page down. A manual Check now uses the same toast, and Settings show the current update status next to the button.
- Warnings no longer push the page down. The summary is a compact chip in the top bar, and the list opens as a scrollable popover.
- Tray Quit stops managed apps and then exits. An update restart leaves them running so the new process can re-adopt them. If shutdown hangs for about five seconds, Orbitry exits anyway.

### Fixed

- Linux and macOS builds compile again. The detached log pump now calls `setsid` on the real command, and Windows-only process-creation flags are no longer imported on Unix. The in-app updater only offers a GitHub release that is not a draft and that includes both the build for this system and `SHA256SUMS.txt`.

- Installing an update could leave Orbitry hung after the dashboard went down, then fail with "timed out waiting for process … to exit". The outgoing process now leaves the tray and exits, and the incoming process will stop a leftover instance before swapping binaries. If the swap fails, Orbitry restores the previous executable and launches it again.
- After a failed or timed-out update, the tray menu could stop responding, including Wyjdź. Quit is handled in every updater state, failed updates reset the updater so the menu stays usable, and a short exit watchdog covers a stuck shutdown.

## [0.3.0] - 2026-11-16

Not released — build failed; changes shipped in 0.3.1.

### Added

- A Settings toggle, "Uruchamiaj Orbitry razem z systemem", off by default. When it is on, Orbitry starts in the tray at login and does not open the browser (`--autostart`). The real state is read from the operating system, not only from the app config: a Current Version Run key on Windows, a Launch Agent on macOS, and an autostart `.desktop` file on Linux. The registered executable path is updated if the app moves or is updated. The same option is available as a checkbox in the tray menu.

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
- A Settings control, "Maks. rozmiar logów na aplikację", defaulting to 10 MB per app (5 MB plus one rotation, range 2–100 MB). The global cap scales with this value (at least 200 MB, or 20× the per-app limit). Changes apply without a restart.

### Changed

- The control panel no longer offers port-name or health-URL overrides. Health checks use the detected port automatically. You can still ignore a false-positive port.
- Update steps are written to the Orbitry config directory (`update.log`, on Windows `%APPDATA%\orbitry\update.log`).
- The update notification is a dismissible overlay toast. It no longer pushes the page down. A manual Check now uses the same toast, and Settings show the current update status next to the button.
- Warnings no longer push the page down. The summary is a compact chip in the top bar, and the list opens as a scrollable popover.
- Tray Quit stops managed apps and then exits. An update restart leaves them running so the new process can re-adopt them. If shutdown hangs for about five seconds, Orbitry exits anyway.

### Fixed

- Installing an update could leave Orbitry hung after the dashboard went down, then fail with "timed out waiting for process … to exit". The outgoing process now leaves the tray and exits, and the incoming process will stop a leftover instance before swapping binaries. If the swap fails, Orbitry restores the previous executable and launches it again.
- After a failed or timed-out update, the tray menu could stop responding, including Wyjdź. Quit is handled in every updater state, failed updates reset the updater so the menu stays usable, and a short exit watchdog covers a stuck shutdown.

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
