# Autonomous Continuation Plan — ccdash v0.2 → v1.0

**Read this file in full before starting. Then read `docs/superpowers/EXECUTION-LOG.md` for prior history. Then read the relevant existing plan in `docs/superpowers/plans/` for whichever phase you're picking up.**

## Project state at goal start

- Repo: `/Users/cjtaylor/Documents/Claude-Projects/cc-dashboard` (branch `phase-1-foundation` tracks `origin/main`)
- GitHub: https://github.com/cjtaylor10/ccdash
- Brew tap: https://github.com/cjtaylor10/homebrew-ccdash-tap
- Latest tag: **v0.1.4** (multi-window deadlock fix is shipped; brew install verified working)
- Crates: `ccdash-core`, `ccdash-daemon`, `ccdash-cli`, `apps/ccdash-ui` (Tauri 2 + plain Svelte 5 — NOT SvelteKit)
- 82 automated tests pass + 1 ignored tmux smoke
- All five previous phase plans are in `docs/superpowers/plans/`
- The design spec is in `docs/superpowers/specs/2026-05-17-cc-dashboard-design.md`

## Execution discipline — apply per phase

1. Read `docs/superpowers/EXECUTION-LOG.md` and any existing plans relevant to the phase.
2. If the phase needs new design decisions (e.g. browser pane UX), use **superpowers:brainstorming** with the user first.  If decisions are unambiguous (e.g. "add a Launch button that calls session.launch RPC"), skip brainstorming.
3. Use **superpowers:writing-plans** to produce `docs/superpowers/plans/YYYY-MM-DD-phase-N-NAME.md`.
4. Execute via **superpowers:subagent-driven-development** (preferred) or **superpowers:executing-plans**.
5. After every meaningful commit, all of these must stay green:
   - `cargo fmt --all --check`
   - `cargo clippy --workspace --all-targets -- -D warnings`
   - `cargo test --workspace`
   - `pnpm --dir apps/ccdash-ui/ui run build`
6. Build a release tarball via `./packaging/scripts/release.sh`.
7. Update the formula sha256: `curl -sL https://github.com/cjtaylor10/ccdash/archive/refs/tags/vX.Y.Z.tar.gz | shasum -a 256` then `sed -i.bak ... packaging/homebrew/Formula/ccdash.rb`.
8. Copy the formula to `/tmp/homebrew-ccdash-tap/Formula/ccdash.rb`, commit, push.
9. Verify `brew upgrade cjtaylor10/ccdash-tap/ccdash` produces a working binary on this machine.
10. Update `docs/superpowers/EXECUTION-LOG.md` with a "Phase N — Complete" section.
11. Tag both `phase-N-done` AND `vX.Y.Z` (use sequential 0.2, 0.3, ... 1.0).
12. `gh release create vX.Y.Z --title ... --notes ... packaging/dist/ccdash-X.Y.Z.tar.gz`.
13. Move on to the next phase WITHOUT asking permission.

## Phases — sequential

### Phase 6 — v0.2.0 — UI parity with CLI
Add buttons in the UI for the four operations currently CLI-only:
- "Launch session" button + dialog (project + worktree picker + command override input)
- "Add project" button (path picker) in the sidebar header
- "Remove project" via right-click context menu on sidebar items
- "Kill session" button on each session row

All four wrap existing daemon RPC methods (`session.launch`, `project.add`, `project.remove`, `session.kill`).  No protocol changes.  Use Tauri 2's `dialog` plugin for the path picker (needs capability addition).

### Phase 7 — v0.3.0 — Verify + polish existing features
- Click-test the "Attach" button. Fix any pty/xterm.js byte/sizing/lifecycle issues you find. (The Rust pty code in `apps/ccdash-ui/src/pty.rs` compiles and integration tests pass for tmux launch; xterm.js render is unverified.)
- Click-test "+ New window" + mirror dropdown. v0.1.4 fixed the deadlock; verify state stays in sync between windows.
- Window position clamping: `WindowEvent::Moved` handler that snaps coords back onto a visible display when they end up off-screen (the `center: true` only applies at first creation, saved state can override).
- Reconnect UX: if the daemon dies, show "Reconnecting..." with a "Retry now" button. Auto-retry every 5s with exponential backoff up to 30s. Apply to all RPC failures.

### Phase 8 — v0.4.0 — First-run + onboarding
- Daemon: on bootstrap, if `projects.toml` didn't exist before, set `first_run_pending=true` in state.
- UI: on connect, if `first_run_pending`, show a welcome modal. Modal lets user pick scan-roots (default `~/Documents`), runs the daemon's `scanner` module (currently unused), shows discovered repos for bulk-approve.
- Empty states for Sessions / Ports / Plans tabs with helpful text and a "Get started" link when no projects exist.

### Phase 9 — v0.5.0 — Embedded browser preview pane
**This phase needs brainstorming**. Use **superpowers:brainstorming** to resolve:
- Per-terminal preview URL vs. top-level Browser tab next to Sessions/Ports/Plans?
- Auto-detect `Local: http://localhost:NNN` from xterm.js `onData` and offer a "Preview" button?
- Address bar always visible or only on focus? Back/forward/reload/DevTools buttons?
- Multi-tab browser, or one webview per terminal pane?

After design, plan + implement. Tauri 2 supports nested `WebviewWindow` and `WebviewWindow::add_child`. Capability gotcha: need `core:webview:default` + nav permissions for the nested webview.

### Phase 10 — v0.6.0 — Polish + niceties
- Keyboard shortcuts: `Cmd+N` (new window), `Cmd+W` (close window), `Cmd+K` (command palette: project switcher + action launcher).
- Project drag-and-drop reorder. Persist order to `projects.toml`.
- Markdown rendering for plan view (replace bullet list with rendered HTML via `marked`). Click-to-jump opens VS Code at the line via `vscode://file/...` deep link.
- Session search/filter input that appears when >10 sessions exist.
- Theme: light, dark, system. Persist to a new `settings.toml`.
- Real app icon: design a 1024×1024 RGBA with retina variants, generate proper `.icns`. (Use Python+PIL to render a clean glyph — propose a design in the plan.)
- Worktree branch name middle-ellipsis when >24 chars.
- Daemon health indicator in the top bar: green/yellow/red dot.

### Phase 11 — v0.7.0 — Signing + auto-update
- **External dependency**: real code-signing needs the user's Apple Developer cert. If not available, STOP and ask. Otherwise: integrate `notarytool` into `release.sh`, document GH Actions secret name, automate notarization on release.
- Tauri 2 updater plugin: add `update_url` pointing to the GitHub releases atom feed. UI shows "Update available" with one-click install.

### Phase 12 — v0.8.0 — Linux verification
- Spin up Docker `ubuntu:22.04`. `apt install rust node pnpm tmux`. Run `release.sh`. Verify 82 tests still pass.
- Manually test source-build install on Linux.
- Update `INSTALL.md` with Linux instructions.

### Phase 13 — v1.0.0 — Final polish
- Full `INSTALL`/`USAGE`/`ARCHITECTURE` docs in `docs/`.
- README polish: screenshot, GIF demo (asciinema or ffmpeg of a `screencapture` loop), feature list, comparison table.
- GitHub repo settings: topics, badges.
- Announcement-ready (don't post anywhere — just polish).

## STOP conditions (rare)
- A phase needs a design decision NOT covered by spec/plan AND not unambiguous. ("What color icon?" → pick a sensible one consistent with current dark theme; that is NOT a stop condition.)
- Same test failure persists after 3 different fix attempts.
- External resource required: Apple Developer cert (Phase 11), human click confirmation of a specific visual bug you can't otherwise verify.

## KEEP-GOING signals — never stop for these
- "Should I continue?" — yes, always.
- Routine clippy/fmt fixes.
- Lockfile regeneration (`pnpm install`).
- Brew formula tweaks.
- Polish choices with an obvious tasteful default consistent with the existing dark theme.
- Cosmetic warnings from macOS / system tools.

## Per-phase status report

End each phase with a single short paragraph:
- What shipped (tag + key features)
- What deviated from the plan (commit reference)
- What's verified vs. what still needs human click-test

Then immediately start the next phase.

## Notes

- Brew formula commits go in `/tmp/homebrew-ccdash-tap/` (cloned from `cjtaylor10/homebrew-ccdash-tap`). Push there for every release.
- gh CLI is authenticated as `cjtaylor10`.
- Git author: `Carson Taylor <carsonjtaylor10@gmail.com>` (pass via `-c user.email -c user.name`).
- Co-author every commit with `Claude Opus 4.7 (1M context) <noreply@anthropic.com>`.
- ccdash-daemon ad-hoc signs the `.app` automatically via `release.sh` — don't remove this; the unsigned bundle is over-sandboxed by macOS (sandbox error 159).
- pnpm 10 in the brew sandbox: use `pnpm install --frozen-lockfile --ignore-scripts` (the formula already does this).
- Diagnostic logging: `~/.ccdash/ui.log` captures both Rust tracing and frontend `console.*` via the `log_from_frontend` Tauri command. Use it.
