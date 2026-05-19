# Installing ccdash

`ccdash` is a local desktop dashboard for Claude Code sessions. It runs on
macOS and Linux.

## Prerequisites

- `tmux` (`brew install tmux` / `apt install tmux`)
- `lsof` — pre-installed on macOS; on Linux: `apt install lsof` / `dnf install lsof`
- macOS 12+ or a recent Linux distro (Ubuntu 22.04+ verified)
- (Source build only) `rust` (1.83+), `node`, `pnpm`
- (Source build, UI bundle, Linux only) `libwebkit2gtk-4.1-dev`, `libgtk-3-dev`,
  `libayatana-appindicator3-dev`, `librsvg2-dev`, `libsoup-3.0-dev`

## Install via Homebrew (recommended)

```bash
brew tap cjtaylor10/ccdash-tap
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
git clone https://github.com/cjtaylor10/ccdash.git
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

## Linux notes

- The `ccdash` and `ccdash-daemon` Rust binaries build clean on Ubuntu
  22.04 LTS. Verified via `packaging/linux/Dockerfile.test` (the
  `daemon-only` build target runs full `cargo fmt --check`, `cargo
  clippy -D warnings`, and `cargo test` against the daemon + CLI + core
  crates).
- The full Tauri app bundle (`ccdash-ui`) requires `libwebkit2gtk-4.1`
  + GTK system dev libraries on Linux; see the prereqs above. The
  `full` Dockerfile target in `packaging/linux/Dockerfile.test`
  exercises that path.
- `lsof` is a hard runtime dep (used to detect listening TCP ports for
  conflict prevention). The Homebrew formula declares it as a
  `depends_on "lsof"` on Linux; macOS uses the system-shipped binary.
- `brew services start` is macOS-only. On Linux, use the systemd user
  unit installed by `packaging/scripts/install-service.sh`:
  `systemctl --user enable --now ccdash-daemon.service`.

## Notes

- ccdash is currently ad-hoc-signed on macOS. macOS may prompt
  "unidentified developer" on first launch — right-click → Open to
  accept. Real Apple Developer signing is planned for a later release.
- ccdash supports macOS + Linux. Windows is not supported.
