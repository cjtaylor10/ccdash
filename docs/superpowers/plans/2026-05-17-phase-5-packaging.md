# ccdash Phase 5 — Packaging Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship ccdash via `brew install cjtaylor/ccdash-tap/ccdash` on macOS and Linux: Homebrew formula, launchd plist (Mac), systemd user unit (Linux), release build script, and a brief INSTALL.md walking new users through first-run.

**Architecture:** All packaging artifacts live under `packaging/`. The brew formula installs three binaries (`ccdash`, `ccdash-daemon`, `ccdash-ui`) and the platform service file, then registers the service in post-install. No code-signing (deferred to v2 per spec §10). No first-run UI wizard (deferred — CLI's `ccdash project add` is the supported v1 onboarding).

**Tech Stack:**
- Homebrew formula (Ruby)
- launchd plist (XML)
- systemd user unit (INI-like)
- POSIX shell for the release build script

**Spec reference:** `docs/superpowers/specs/2026-05-17-cc-dashboard-design.md`
**Predecessor:** Phase 4 complete; tag `phase-4-done`.

**Design choices (confirmed by user):**
- Brew tap location: `github.com/cjtaylor/ccdash-tap`.
- No code-signing.
- macOS + Linux only.

---

## File Structure

```
ccdash/
├── packaging/
│   ├── homebrew/
│   │   └── Formula/
│   │       └── ccdash.rb                 # brew formula installed by `brew tap cjtaylor/ccdash-tap`
│   ├── launchd/
│   │   └── com.ccdash.daemon.plist       # ~/Library/LaunchAgents/ entry
│   ├── systemd/
│   │   └── ccdash-daemon.service         # ~/.config/systemd/user/ entry
│   └── scripts/
│       ├── release.sh                    # build all binaries + bundle frontend + tar
│       ├── install-service.sh            # post-install: load launchd / systemd
│       └── uninstall-service.sh
├── INSTALL.md                            # user-facing install instructions
└── README.md                             # top-level project intro
```

---

## Task A1: Write the launchd plist

**Files:**
- Create: `packaging/launchd/com.ccdash.daemon.plist`

- [ ] **Step 1: Write the plist**

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>com.ccdash.daemon</string>
  <key>ProgramArguments</key>
  <array>
    <string>BREW_PREFIX/bin/ccdash-daemon</string>
    <string>--log-level</string>
    <string>info</string>
  </array>
  <key>RunAtLoad</key>
  <true/>
  <key>KeepAlive</key>
  <dict>
    <key>SuccessfulExit</key>
    <false/>
  </dict>
  <key>StandardOutPath</key>
  <string>USER_LOG_DIR/ccdash/daemon.out.log</string>
  <key>StandardErrorPath</key>
  <string>USER_LOG_DIR/ccdash/daemon.err.log</string>
  <key>EnvironmentVariables</key>
  <dict>
    <key>PATH</key>
    <string>BREW_PREFIX/bin:/usr/local/bin:/opt/homebrew/bin:/usr/bin:/bin</string>
  </dict>
</dict>
</plist>
```

> Placeholders `BREW_PREFIX` and `USER_LOG_DIR` are substituted by `install-service.sh` at install time (sed replaces them with `$(brew --prefix)` and `$HOME/Library/Logs`).

- [ ] **Step 2: Commit**

```bash
mkdir -p packaging/launchd
# Write the file above to packaging/launchd/com.ccdash.daemon.plist
git add packaging/launchd
git commit -m "packaging: launchd plist template for ccdash-daemon"
```

---

## Task A2: Write the systemd user unit

**Files:**
- Create: `packaging/systemd/ccdash-daemon.service`

- [ ] **Step 1: Write the unit**

```ini
[Unit]
Description=ccdash daemon
After=default.target

[Service]
Type=simple
ExecStart=BREW_PREFIX/bin/ccdash-daemon --log-level info
Restart=on-failure
RestartSec=2

[Install]
WantedBy=default.target
```

> Same `BREW_PREFIX` substitution as the plist.

- [ ] **Step 2: Commit**

```bash
mkdir -p packaging/systemd
# Write the file above to packaging/systemd/ccdash-daemon.service
git add packaging/systemd
git commit -m "packaging: systemd user unit for ccdash-daemon"
```

---

## Task A3: Write install/uninstall scripts

**Files:**
- Create: `packaging/scripts/install-service.sh`
- Create: `packaging/scripts/uninstall-service.sh`

- [ ] **Step 1: Write `install-service.sh`**

```bash
#!/usr/bin/env bash
# Installs the ccdash-daemon service for the current user. Called by the brew
# formula's `post_install` step. Idempotent.
set -euo pipefail

BREW_PREFIX="${1:-$(brew --prefix)}"
USER_LOG_DIR="${HOME}/Library/Logs"

uname_s="$(uname -s)"
case "$uname_s" in
  Darwin)
    PLIST_SRC="${BREW_PREFIX}/share/ccdash/com.ccdash.daemon.plist"
    PLIST_DEST="${HOME}/Library/LaunchAgents/com.ccdash.daemon.plist"
    mkdir -p "${HOME}/Library/LaunchAgents" "${USER_LOG_DIR}/ccdash"
    sed -e "s|BREW_PREFIX|${BREW_PREFIX}|g" \
        -e "s|USER_LOG_DIR|${USER_LOG_DIR}|g" \
        "${PLIST_SRC}" > "${PLIST_DEST}"
    launchctl unload "${PLIST_DEST}" 2>/dev/null || true
    launchctl load "${PLIST_DEST}"
    echo "ccdash-daemon: launchd service installed and loaded"
    ;;
  Linux)
    UNIT_SRC="${BREW_PREFIX}/share/ccdash/ccdash-daemon.service"
    UNIT_DEST="${HOME}/.config/systemd/user/ccdash-daemon.service"
    mkdir -p "${HOME}/.config/systemd/user"
    sed -e "s|BREW_PREFIX|${BREW_PREFIX}|g" "${UNIT_SRC}" > "${UNIT_DEST}"
    systemctl --user daemon-reload
    systemctl --user enable --now ccdash-daemon.service
    echo "ccdash-daemon: systemd user service installed and started"
    ;;
  *)
    echo "ccdash-daemon: unsupported OS '$uname_s'. Start it manually with:"
    echo "  ${BREW_PREFIX}/bin/ccdash-daemon"
    ;;
esac
```

- [ ] **Step 2: Write `uninstall-service.sh`**

```bash
#!/usr/bin/env bash
# Removes the ccdash-daemon service for the current user.
set -euo pipefail

uname_s="$(uname -s)"
case "$uname_s" in
  Darwin)
    PLIST_DEST="${HOME}/Library/LaunchAgents/com.ccdash.daemon.plist"
    if [ -f "${PLIST_DEST}" ]; then
      launchctl unload "${PLIST_DEST}" 2>/dev/null || true
      rm -f "${PLIST_DEST}"
      echo "ccdash-daemon: launchd service removed"
    fi
    ;;
  Linux)
    systemctl --user disable --now ccdash-daemon.service 2>/dev/null || true
    rm -f "${HOME}/.config/systemd/user/ccdash-daemon.service"
    systemctl --user daemon-reload 2>/dev/null || true
    echo "ccdash-daemon: systemd user service removed"
    ;;
esac
```

- [ ] **Step 3: Make executable + commit**

```bash
mkdir -p packaging/scripts
# Write both files above.
chmod +x packaging/scripts/install-service.sh packaging/scripts/uninstall-service.sh
git add packaging/scripts
git commit -m "packaging: install/uninstall service scripts (macOS launchd + Linux systemd)"
```

---

## Task A4: Write the release build script

**Files:**
- Create: `packaging/scripts/release.sh`

- [ ] **Step 1: Write the script**

```bash
#!/usr/bin/env bash
# Build all release binaries + the SvelteKit frontend, then tar them into
# packaging/dist/. Intended to be run from the repo root.
set -euo pipefail

cd "$(dirname "$0")/../.."
ROOT="$(pwd)"
VERSION=$(grep '^version' Cargo.toml | head -1 | sed -E 's/version *= *"(.+)"/\1/')
DIST="${ROOT}/packaging/dist"

echo "==> Building Rust binaries (--release)"
cargo build --release -p ccdash-daemon -p ccdash-cli -p ccdash-ui

echo "==> Building frontend"
(cd apps/ccdash-ui/ui && pnpm install --frozen-lockfile && pnpm run build)

echo "==> Bundling tarball"
rm -rf "${DIST}" && mkdir -p "${DIST}/ccdash-${VERSION}/bin" "${DIST}/ccdash-${VERSION}/share/ccdash"
cp target/release/ccdash "${DIST}/ccdash-${VERSION}/bin/"
cp target/release/ccdash-daemon "${DIST}/ccdash-${VERSION}/bin/"
cp target/release/ccdash-ui "${DIST}/ccdash-${VERSION}/bin/"
cp packaging/launchd/com.ccdash.daemon.plist "${DIST}/ccdash-${VERSION}/share/ccdash/"
cp packaging/systemd/ccdash-daemon.service "${DIST}/ccdash-${VERSION}/share/ccdash/"
cp packaging/scripts/install-service.sh "${DIST}/ccdash-${VERSION}/share/ccdash/"
cp packaging/scripts/uninstall-service.sh "${DIST}/ccdash-${VERSION}/share/ccdash/"

tar -C "${DIST}" -czf "${DIST}/ccdash-${VERSION}.tar.gz" "ccdash-${VERSION}"
SHA=$(shasum -a 256 "${DIST}/ccdash-${VERSION}.tar.gz" | awk '{print $1}')
echo
echo "Built ${DIST}/ccdash-${VERSION}.tar.gz"
echo "  sha256 = ${SHA}"
echo
echo "Update Formula/ccdash.rb 'sha256' with the value above."
```

- [ ] **Step 2: Make executable + commit**

```bash
chmod +x packaging/scripts/release.sh
git add packaging/scripts/release.sh
git commit -m "packaging: release build script (binaries + frontend + tarball)"
```

---

## Task B1: Write the Homebrew formula

**Files:**
- Create: `packaging/homebrew/Formula/ccdash.rb`

- [ ] **Step 1: Write the formula**

```ruby
class Ccdash < Formula
  desc "Local desktop dashboard for managing Claude Code sessions, projects, and ports"
  homepage "https://github.com/cjtaylor/ccdash"
  version "0.1.0"

  # Source-build formula: clone the repo, build with cargo + pnpm.
  # When precompiled release artifacts are hosted, replace `url` and add `sha256`.
  url "https://github.com/cjtaylor/ccdash/archive/refs/tags/v#{version}.tar.gz"
  sha256 "0000000000000000000000000000000000000000000000000000000000000000"
  license "MIT"

  depends_on "rust" => :build
  depends_on "node" => :build
  depends_on "pnpm" => :build
  depends_on "tmux"

  def install
    # Build Rust binaries.
    system "cargo", "build", "--release",
           "-p", "ccdash-daemon",
           "-p", "ccdash-cli",
           "-p", "ccdash-ui"

    # Build SvelteKit frontend.
    cd "apps/ccdash-ui/ui" do
      system "pnpm", "install", "--frozen-lockfile"
      system "pnpm", "run", "build"
    end

    bin.install "target/release/ccdash"
    bin.install "target/release/ccdash-daemon"
    bin.install "target/release/ccdash-ui"

    pkgshare.install "packaging/launchd/com.ccdash.daemon.plist"
    pkgshare.install "packaging/systemd/ccdash-daemon.service"
    pkgshare.install "packaging/scripts/install-service.sh"
    pkgshare.install "packaging/scripts/uninstall-service.sh"
  end

  service do
    run [opt_bin/"ccdash-daemon", "--log-level", "info"]
    keep_alive true
    log_path var/"log/ccdash/daemon.out.log"
    error_log_path var/"log/ccdash/daemon.err.log"
  end

  def post_install
    system Formula["bash"].opt_bin/"bash",
           pkgshare/"install-service.sh",
           HOMEBREW_PREFIX.to_s
  rescue => e
    opoo "Could not auto-install service (#{e.message}). Run manually:"
    opoo "  #{pkgshare}/install-service.sh #{HOMEBREW_PREFIX}"
  end

  test do
    system "#{bin}/ccdash", "--version"
    system "#{bin}/ccdash-daemon", "--help"
    system "#{bin}/ccdash-ui", "--version" rescue true # GUI app may not accept --version
  end
end
```

> Note: the `sha256 "00...0"` is a placeholder for the source tarball. When you cut a release tag, GitHub generates `https://github.com/cjtaylor/ccdash/archive/refs/tags/v0.1.0.tar.gz`; compute its sha256 with `shasum -a 256 …` and paste the value. Until then, brew will reject installation — but the formula structure is what gets reviewed.

- [ ] **Step 2: Commit**

```bash
mkdir -p packaging/homebrew/Formula
# Write the file above.
git add packaging/homebrew
git commit -m "packaging: Homebrew formula for ccdash (source-build, service-managed)"
```

---

## Task C1: User-facing install instructions

**Files:**
- Create: `INSTALL.md`
- Modify: `README.md` (create if missing)

- [ ] **Step 1: Write `INSTALL.md`**

```markdown
# Installing ccdash

`ccdash` is a local desktop dashboard for Claude Code sessions. It runs on
macOS and Linux.

## Prerequisites

- `tmux` (`brew install tmux` / `apt install tmux`)
- macOS 12+ or a recent Linux distro
- (Source build only) `rust`, `node`, `pnpm`

## Install via Homebrew (recommended)

\`\`\`bash
brew tap cjtaylor/ccdash-tap
brew install ccdash
\`\`\`

This installs three binaries:
- `ccdash` — CLI client
- `ccdash-daemon` — background service (auto-started via launchd / systemd)
- `ccdash-ui` — desktop app

The post-install step registers the daemon as a user service that auto-starts
on login.

## Verify the install

\`\`\`bash
# Check daemon is running
ccdash status

# Register your first project
ccdash project add ~/Documents/MyProject

# List projects
ccdash project list

# Launch the desktop UI
ccdash-ui
\`\`\`

## Uninstall

\`\`\`bash
brew uninstall ccdash
~/.brew/share/ccdash/uninstall-service.sh   # path may vary by brew prefix
\`\`\`

## Build from source

\`\`\`bash
git clone https://github.com/cjtaylor/ccdash.git
cd ccdash
./packaging/scripts/release.sh
# Binaries land in target/release/.
# Run the service script manually:
./packaging/scripts/install-service.sh "$(pwd)"   # passes repo path as BREW_PREFIX shim
\`\`\`

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
```

- [ ] **Step 2: Write a minimal `README.md`**

```markdown
# ccdash

Local desktop dashboard for managing Claude Code sessions, projects, ports,
and plans. Built in Rust + Tauri + SvelteKit.

See [INSTALL.md](./INSTALL.md) for installation and [docs/superpowers/specs/](./docs/superpowers/specs/)
for the full design.

## Quick start

\`\`\`bash
brew tap cjtaylor/ccdash-tap
brew install ccdash
ccdash project add ~/path/to/your/repo
ccdash-ui
\`\`\`

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
```

- [ ] **Step 3: Commit**

```bash
git add INSTALL.md README.md
git commit -m "docs: README + INSTALL with brew tap + usage"
```

---

## Task D1: Phase 5 verification + final tag

- [ ] **Step 1: Smoke-build the release tarball locally**

```bash
./packaging/scripts/release.sh
ls -lh packaging/dist/
```

Expected: a `ccdash-0.1.0.tar.gz` file appears under `packaging/dist/`. Note the printed sha256.

- [ ] **Step 2: Update formula sha256 (if you're cutting a real release)**

If you've already pushed a `v0.1.0` git tag, fetch the GitHub-generated archive and update the formula:

```bash
TAG=v0.1.0
URL="https://github.com/cjtaylor/ccdash/archive/refs/tags/${TAG}.tar.gz"
curl -sL "$URL" -o /tmp/ccdash-src.tar.gz
SHA=$(shasum -a 256 /tmp/ccdash-src.tar.gz | awk '{print $1}')
sed -i.bak "s|sha256 \"0\{64\}\"|sha256 \"${SHA}\"|" packaging/homebrew/Formula/ccdash.rb
rm packaging/homebrew/Formula/ccdash.rb.bak
git add packaging/homebrew/Formula/ccdash.rb
git commit -m "packaging: update formula sha256 for v0.1.0"
```

(If you're not cutting the real release yet, skip this step — the placeholder sha is fine for plan-completion purposes.)

- [ ] **Step 3: fmt + clippy + tests**

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
tmux kill-server 2>/dev/null; sleep 0.2
cargo test --workspace
```

Expected: same as Phase 4 — clippy clean, all tests pass.

- [ ] **Step 4: Tag**

```bash
git tag phase-5-done
git tag v0.1.0
```

- [ ] **Step 5: Update execution log**

Append a "Phase 5 — Complete" section to `docs/superpowers/EXECUTION-LOG.md` summarizing what was built.

```bash
git add docs/superpowers/EXECUTION-LOG.md
git commit -m "docs: phase-5 complete — packaging done, v0.1.0 ready"
```

---

## What Phase 5 ships

- Homebrew formula at `packaging/homebrew/Formula/ccdash.rb` — installs all three binaries, ships the service files, runs `install-service.sh` in post-install.
- `packaging/launchd/com.ccdash.daemon.plist` — RunAtLoad + KeepAlive macOS service.
- `packaging/systemd/ccdash-daemon.service` — `WantedBy=default.target` user unit for Linux.
- `packaging/scripts/release.sh` — one-shot build that produces a tarball ready to upload.
- `packaging/scripts/install-service.sh` / `uninstall-service.sh` — register/deregister the platform service.
- Top-level `README.md` + `INSTALL.md`.

## What's NOT in Phase 5 (deferred per spec §10)

- Code-signing + notarization. v0.1.0 ships unsigned.
- `.deb`/`.pkg` native installers. Homebrew is the only supported install path; raw release tarball works for source builds.
- Windows support.
- First-run wizard inside the Tauri UI. Use `ccdash project add` from the CLI for v1 onboarding.

## Self-Review

**Spec coverage:**
- §7.12 Homebrew distribution + post-install service registration → B1 + A3
- §6.1 step 3 first-run launchctl/systemctl semantics → A3 install-service.sh
- §10 deferred: signing & Windows ✓ (deferred as planned)

**Placeholder scan:** the `sha256 "00..."` in the formula is an intentional placeholder for the source tarball — the plan documents this and Task D1 step 2 covers the real-release path. Not a plan-failure placeholder.

**Type consistency:** N/A (no Rust types in this phase).

---

**Plan complete and saved to `docs/superpowers/plans/2026-05-17-phase-5-packaging.md`.**
