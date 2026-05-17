# ccdash

Local desktop dashboard for managing Claude Code sessions, projects, ports,
and plans. Built in Rust + Tauri + SvelteKit.

See [INSTALL.md](./INSTALL.md) for installation and
[docs/superpowers/specs/](./docs/superpowers/specs/) for the full design.

## Quick start

```bash
brew tap cjtaylor/ccdash-tap
brew install ccdash
ccdash project add ~/path/to/your/repo
ccdash-ui
```

## Architecture

Three crates + one Tauri app:
- `ccdash-core` — shared protocol + client library
- `ccdash-daemon` — long-lived service over Unix socket (JSON-RPC 2.0)
- `ccdash-cli` — `ccdash` command-line client
- `apps/ccdash-ui` — Tauri 2.x desktop app

See `docs/superpowers/specs/2026-05-17-cc-dashboard-design.md` for the full
design rationale.

## Status

v0.1.0 (preview):
- ✅ Phase 1: daemon foundation (project/session/worktree registry)
- ✅ Phase 2: CLI + ports module + plans module
- ✅ Phase 3: Tauri UI shell + static views
- ✅ Phase 4: embedded terminals + multi-window mirror
- ✅ Phase 5: packaging (Homebrew, launchd, systemd)

Deferred to v0.2:
- Code-signing + notarization (current builds are unsigned)
- Windows support
- File browser inside the UI
- Live edit of plan markdown from the dashboard

## License

MIT
