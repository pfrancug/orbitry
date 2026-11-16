<p align="center">
  <br />
  <strong>✦ ORBITRY</strong>
</p>

<h1 align="center">All your apps. One orbit.</h1>

<p align="center">
  A local command center for the projects that live on your machine.<br />
  See them. Launch them. Watch them — without the chaos.
</p>

<p align="center">
  <img alt="MIT License" src="https://img.shields.io/badge/license-MIT-6c8cff?style=for-the-badge&labelColor=0b0b12" />
  &nbsp;
  <img alt="Early, in the making" src="https://img.shields.io/badge/status-early%2C%20in%20the%20making-c4b5fd?style=for-the-badge&labelColor=0b0b12" />
</p>

<br />

## A quieter kind of control

You don't have one project. You have a constellation.

Frontends, APIs, tools, experiments — each with its own window, its own port, its own half-remembered command. Orbitry is the place they all come home to. Point it at a folder of projects and it becomes a living map of everything you are building: what's ready, what's running, and what's waiting to launch.

## What it does

**Choose your sky**  
Point Orbitry at any folder of projects — on any machine. Nothing is hardcoded. Your workspace, your map.

**See the whole constellation**  
Every app at a glance: version, branch, whether it's built, whether it's running, and on which ports.

**Launch without the clutter**  
Start and stop frontends, backends, and shared dev servers from one surface. Live logs stream in place — no forest of leftover terminals.

**It already knows how they start**  
Orbitry finds the commands that actually run your apps, and tells frontends from backends. When a project is special, you teach it new patterns right in the dashboard.

**Built to disappear into your day**  
Local-first. Yours. A command center that stays on your machine and stays out of your way.

## Vision

Orbitry should feel like a modern instrument panel, not a settings page. Calm, precise, a little bit magical. The kind of tool you open once and leave open — because it makes a busy desk feel like a single, quiet hangar.

It lives in your tray, always a glance away.

## Get Orbitry

Grab the latest build for Windows, macOS, or Linux from the [Releases](https://github.com/pfrancug/orbitry/releases) page. Once it's installed, Orbitry can check for a newer version from the tray and update itself when you ask it to.

## Develop

Build the UI once (`npm run build` in `ui/`), then run:

```
cargo run -p orbitry -- --root /path/to/projects
```

Standing rules for the project live in [docs/PRINCIPLES.md](docs/PRINCIPLES.md).

## Status


Early. In the making. The orbit is taking shape.

<p align="center">
  <sub>MIT License · © 2026 Piotr Francug</sub>
</p>