# Installing ccdash

`ccdash` is a local desktop dashboard for Claude Code sessions. It runs on
macOS and Linux.

## Prerequisites

- `tmux` (`brew install tmux` / `apt install tmux`)
- macOS 12+ or a recent Linux distro
- (Source build only) `rust`, `node`, `pnpm`

## Install via Homebrew (recommended)

```bash
brew tap cjtaylor/ccdash-tap
brew install ccdash
```

This installs three binaries:
- `ccdash` — CLI client
- `ccdash-daemon` — background service (auto-started via launchd / systemd)
- `ccdash-ui` — desktop app

The post-install step registers the daemon as a user service that auto-starts
on login.

## Verify the install

```bash
# Check daemon is running
ccdash status

# Register your first project
ccdash project add ~/Documents/MyProject

# List projects
ccdash project list

# Launch the desktop UI
ccdash-ui
```

## Uninstall

```bash
brew uninstall ccdash
"$(brew --prefix)"/share/ccdash/uninstall-service.sh
```

## Build from source

```bash
# Prereqs: rust, node, pnpm, tmux, cargo install tauri-cli (if missing)
git clone https://github.com/cjtaylor/ccdash.git
cd ccdash

# Build all binaries + Tauri .app, then ad-hoc sign the .app:
./packaging/scripts/release.sh

# Binaries land in target/release/.
# The Tauri app bundle is at target/release/bundle/macos/ccdash.app (macOS).

# Run the service install script manually (replace REPO_ROOT with the repo path):
./packaging/scripts/install-service.sh REPO_ROOT
```

The release script automatically runs `codesign --force --deep --sign -` on
`ccdash.app` so the unsigned WebKit subprocesses aren't over-sandboxed by
macOS. Without ad-hoc signing, the GUI launches but the webview never
initializes (sandbox error 159).

## Configuration

- `~/.ccdash/projects.toml` — registered projects
- `~/.ccdash/sessions.toml` — tmux session metadata
- `~/.ccdash/auth` — daemon auth token (mode 0600; do not share)
- macOS logs: `~/Library/Logs/ccdash/`
- Linux logs: `journalctl --user -u ccdash-daemon`

## Notes

- v0.1.0 is unsigned. macOS may prompt "unidentified developer" on first launch —
  right-click → Open to accept.
- v0.1.0 supports macOS + Linux. Windows is not supported in this release.
