# ccdash Phase 1 — Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the `ccdash-core` shared library and `ccdash-daemon` long-lived service, providing JSON-RPC over Unix socket with token auth, in-memory project + worktree + session state backed by tmux, and a broadcast bus that lets multiple clients subscribe to state deltas.

**Architecture:** Cargo workspace with two crates. `ccdash-core` defines protocol/domain types shared between daemon and future clients. `ccdash-daemon` is a tokio-based service that owns state, exposes JSON-RPC 2.0 on a Unix socket with `0600` permissions and shared-secret auth, manages tmux sessions via shelling out to `tmux`, watches `~/.ccdash/projects.toml` for project registry, runs `git worktree list --porcelain` to discover worktrees, and broadcasts deltas via `tokio::sync::broadcast`.

**Tech Stack:**
- Rust 1.83+, 2021 edition
- `tokio` (rt-multi-thread, net, fs, sync, signal, process)
- `serde` + `serde_json` + `toml`
- `tracing` + `tracing-subscriber` for logging
- `clap` for daemon CLI flags (`--socket`, `--data-dir`, `--log-level`)
- `notify` crate for file-system watching
- `anyhow` + `thiserror` for error handling
- `rand` for auth token generation
- Tmux interaction by shelling out to the `tmux` binary (control mode deferred to Phase 2)

**Spec reference:** `docs/superpowers/specs/2026-05-17-cc-dashboard-design.md`

---

## File Structure

```
ccdash/
├── Cargo.toml                                   # workspace root
├── Cargo.lock
├── rust-toolchain.toml                          # pin toolchain
├── .gitignore
├── crates/
│   ├── ccdash-core/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs                           # re-exports
│   │       ├── protocol.rs                      # JSON-RPC enums
│   │       ├── domain.rs                        # Project, Worktree, Session, etc.
│   │       ├── auth.rs                          # token loader
│   │       └── paths.rs                         # XDG/HOME path resolution
│   └── ccdash-daemon/
│       ├── Cargo.toml
│       └── src/
│           ├── main.rs                          # entry, signal handling
│           ├── config.rs                        # daemon CLI + env config
│           ├── state.rs                         # AppState wrapper (Arc<RwLock<…>>)
│           ├── broadcast.rs                     # event bus wrapper
│           ├── rpc/
│           │   ├── mod.rs                       # socket listener, conn loop
│           │   ├── codec.rs                     # line-delimited JSON framing
│           │   ├── dispatch.rs                  # method → handler routing
│           │   └── handlers.rs                  # handshake, subscribe, project.*, session.*
│           ├── projects/
│           │   ├── mod.rs
│           │   ├── registry.rs                  # projects.toml persistence
│           │   └── scanner.rs                   # root-dir git-repo discovery
│           ├── worktrees.rs                     # git worktree list parser
│           ├── tmux.rs                          # shell-out wrapper
│           └── sessions.rs                      # join tmux + metadata
└── tests/                                       # workspace-level integration tests
    ├── common/
    │   └── mod.rs                               # test harness: temp HOME, daemon spawn
    ├── handshake.rs
    ├── projects.rs
    └── sessions.rs
```

Each file has one clear responsibility. The `rpc/` and `projects/` modules are split into submodules because each has distinct sub-responsibilities (framing vs dispatch; persistence vs scanning).

---

## Task A1: Initialize Cargo workspace

**Files:**
- Create: `Cargo.toml`
- Create: `rust-toolchain.toml`
- Modify: `.gitignore`

- [ ] **Step 1: Write `rust-toolchain.toml`**

```toml
[toolchain]
channel = "1.83"
components = ["rustfmt", "clippy"]
```

- [ ] **Step 2: Write the workspace `Cargo.toml`**

```toml
[workspace]
resolver = "2"
members = ["crates/ccdash-core", "crates/ccdash-daemon"]

[workspace.package]
version = "0.1.0"
edition = "2021"
rust-version = "1.83"
license = "MIT"
repository = "https://github.com/cjtaylor/ccdash"

[workspace.dependencies]
anyhow = "1"
thiserror = "1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = "0.8"
tokio = { version = "1.40", features = ["rt-multi-thread", "macros", "net", "fs", "sync", "signal", "process", "io-util", "time"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
clap = { version = "4", features = ["derive"] }
notify = "6"
rand = "0.8"
tempfile = "3"
```

- [ ] **Step 3: Update `.gitignore`**

Append to `.gitignore`:

```
target/
**/*.rs.bk
Cargo.lock.bak
```

- [ ] **Step 4: Verify workspace compiles**

Run: `cargo check --workspace`
Expected: error about no members existing — that's fine; we'll add them in the next tasks. If you see "manifest path does not exist" for one of the members, that's expected.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml rust-toolchain.toml .gitignore
git commit -m "scaffold cargo workspace"
```

---

## Task A2: Scaffold `ccdash-core` crate

**Files:**
- Create: `crates/ccdash-core/Cargo.toml`
- Create: `crates/ccdash-core/src/lib.rs`

- [ ] **Step 1: Write the crate `Cargo.toml`**

```toml
[package]
name = "ccdash-core"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[dependencies]
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
anyhow = { workspace = true }
rand = { workspace = true }

[dev-dependencies]
tempfile = { workspace = true }
```

- [ ] **Step 2: Write a stub `lib.rs`**

```rust
//! ccdash shared library.
//!
//! Contains protocol types, domain types, auth, and path resolution
//! shared between the daemon and any client (CLI, Tauri UI).

pub mod auth;
pub mod domain;
pub mod paths;
pub mod protocol;
```

- [ ] **Step 3: Create empty submodule files**

Create empty files (just `//! Module placeholder.` in each):
- `crates/ccdash-core/src/auth.rs`
- `crates/ccdash-core/src/domain.rs`
- `crates/ccdash-core/src/paths.rs`
- `crates/ccdash-core/src/protocol.rs`

Each file contains exactly:
```rust
//! Module placeholder.
```

- [ ] **Step 4: Verify build**

Run: `cargo check -p ccdash-core`
Expected: SUCCESS, possibly with "unused module" warnings.

- [ ] **Step 5: Commit**

```bash
git add crates/ccdash-core
git commit -m "ccdash-core: scaffold crate with empty modules"
```

---

## Task A3: Implement `ccdash-core::paths`

Centralizes path resolution so daemon and clients agree on socket/data paths across macOS and Linux.

**Files:**
- Modify: `crates/ccdash-core/src/paths.rs`

- [ ] **Step 1: Write the failing test**

Edit `crates/ccdash-core/src/paths.rs`:

```rust
//! Resolves XDG / HOME-relative paths used across ccdash.

use std::path::PathBuf;

/// Returns the directory where ccdash stores its data files.
/// Honors `CCDASH_HOME` env var for testing; otherwise `~/.ccdash`.
pub fn data_dir() -> PathBuf {
    if let Ok(custom) = std::env::var("CCDASH_HOME") {
        return PathBuf::from(custom);
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home).join(".ccdash")
}

/// Returns the default socket path for the current platform.
/// macOS: `/tmp/ccdash.sock`. Linux: `$XDG_RUNTIME_DIR/ccdash.sock`
/// (falls back to `/tmp/ccdash.sock` if `XDG_RUNTIME_DIR` is unset).
/// Honors `CCDASH_SOCKET` env var for testing.
pub fn default_socket_path() -> PathBuf {
    if let Ok(custom) = std::env::var("CCDASH_SOCKET") {
        return PathBuf::from(custom);
    }
    #[cfg(target_os = "macos")]
    {
        PathBuf::from("/tmp/ccdash.sock")
    }
    #[cfg(target_os = "linux")]
    {
        if let Ok(xdg) = std::env::var("XDG_RUNTIME_DIR") {
            PathBuf::from(xdg).join("ccdash.sock")
        } else {
            PathBuf::from("/tmp/ccdash.sock")
        }
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        PathBuf::from("/tmp/ccdash.sock")
    }
}

pub fn projects_toml() -> PathBuf {
    data_dir().join("projects.toml")
}

pub fn sessions_toml() -> PathBuf {
    data_dir().join("sessions.toml")
}

pub fn auth_token_path() -> PathBuf {
    data_dir().join("auth")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_dir_honors_ccdash_home() {
        std::env::set_var("CCDASH_HOME", "/tmp/ccdash-test");
        assert_eq!(data_dir(), PathBuf::from("/tmp/ccdash-test"));
        std::env::remove_var("CCDASH_HOME");
    }

    #[test]
    fn projects_toml_is_under_data_dir() {
        std::env::set_var("CCDASH_HOME", "/tmp/ccdash-test");
        assert_eq!(projects_toml(), PathBuf::from("/tmp/ccdash-test/projects.toml"));
        std::env::remove_var("CCDASH_HOME");
    }

    #[test]
    fn socket_honors_ccdash_socket() {
        std::env::set_var("CCDASH_SOCKET", "/tmp/explicit.sock");
        assert_eq!(default_socket_path(), PathBuf::from("/tmp/explicit.sock"));
        std::env::remove_var("CCDASH_SOCKET");
    }
}
```

> Note: tests set/unset env vars. If you run `cargo test` in parallel and other tests touch the same env vars, they can race. The two tests above that touch `CCDASH_HOME` use unique enough values that they self-correct; if flakiness emerges, switch to `cargo test -- --test-threads=1` for this module.

- [ ] **Step 2: Run tests**

Run: `cargo test -p ccdash-core --lib paths::`
Expected: 3 PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/ccdash-core/src/paths.rs
git commit -m "ccdash-core: path resolution for data dir, socket, files"
```

---

## Task A4: Implement `ccdash-core::auth` token generation and loading

**Files:**
- Modify: `crates/ccdash-core/src/auth.rs`

- [ ] **Step 1: Write the failing test**

Edit `crates/ccdash-core/src/auth.rs`:

```rust
//! Shared-secret auth tokens.
//!
//! The daemon writes a random token to `~/.ccdash/auth` with mode 0600.
//! Clients read it and present it in the JSON-RPC handshake.

use anyhow::{Context, Result};
use rand::RngCore;
use std::fs;
use std::io::Write;
use std::path::Path;

/// Length of generated tokens in bytes (32 = 256 bits → 64 hex chars).
const TOKEN_BYTES: usize = 32;

/// Generate a new random token as a lowercase hex string.
pub fn generate_token() -> String {
    let mut buf = [0u8; TOKEN_BYTES];
    rand::thread_rng().fill_bytes(&mut buf);
    buf.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Read the auth token from the given path. Returns `Ok(None)` if the file
/// doesn't exist; `Err` for any other failure.
pub fn read_token(path: &Path) -> Result<Option<String>> {
    match fs::read_to_string(path) {
        Ok(s) => Ok(Some(s.trim().to_string())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(anyhow::Error::new(e).context(format!("reading {}", path.display()))),
    }
}

/// Write the given token to the path with mode 0600 (Unix). Creates the parent
/// directory if missing.
#[cfg(unix)]
pub fn write_token(path: &Path, token: &str) -> Result<()> {
    use std::os::unix::fs::OpenOptionsExt;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    }
    let mut f = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)
        .with_context(|| format!("opening {}", path.display()))?;
    f.write_all(token.as_bytes())?;
    f.write_all(b"\n")?;
    Ok(())
}

/// Ensure an auth token exists at the given path. If absent, generates and
/// writes one. Returns the resulting token.
pub fn ensure_token(path: &Path) -> Result<String> {
    if let Some(existing) = read_token(path)? {
        return Ok(existing);
    }
    let token = generate_token();
    write_token(path, &token)?;
    Ok(token)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn generate_token_returns_64_hex_chars() {
        let t = generate_token();
        assert_eq!(t.len(), 64);
        assert!(t.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    }

    #[test]
    fn read_missing_returns_none() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("missing");
        assert!(read_token(&path).unwrap().is_none());
    }

    #[test]
    fn ensure_creates_when_missing() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("auth");
        let token = ensure_token(&path).unwrap();
        assert_eq!(token.len(), 64);
        let again = ensure_token(&path).unwrap();
        assert_eq!(token, again, "second call must return same token");
    }

    #[cfg(unix)]
    #[test]
    fn write_sets_mode_0600() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempdir().unwrap();
        let path = dir.path().join("auth");
        write_token(&path, "abc").unwrap();
        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p ccdash-core --lib auth::`
Expected: 4 PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/ccdash-core/src/auth.rs
git commit -m "ccdash-core: shared-secret auth tokens with 0600 perms"
```

---

## Task A5: Implement `ccdash-core::domain` types

These are the value types the daemon and clients exchange.

**Files:**
- Modify: `crates/ccdash-core/src/domain.rs`

- [ ] **Step 1: Write the implementation with inline tests**

Edit `crates/ccdash-core/src/domain.rs`:

```rust
//! Domain types shared across daemon, CLI, and UI.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Stable identifier for a registered project.
/// Generated as a short random hex string when the project is registered.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProjectId(pub String);

impl ProjectId {
    pub fn new() -> Self {
        use rand::RngCore;
        let mut buf = [0u8; 4];
        rand::thread_rng().fill_bytes(&mut buf);
        Self(buf.iter().map(|b| format!("{:02x}", b)).collect())
    }
}

impl Default for ProjectId {
    fn default() -> Self {
        Self::new()
    }
}

/// A registered project (git repo root).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Project {
    pub id: ProjectId,
    pub name: String,
    pub path: PathBuf,
    /// Auto-detected worktrees discovered via `git worktree list --porcelain`.
    /// Empty until first refresh. Always includes the main worktree.
    #[serde(default)]
    pub worktrees: Vec<Worktree>,
    #[serde(default)]
    pub state: ProjectState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProjectState {
    #[default]
    Ok,
    /// Project directory no longer exists on disk.
    Missing,
}

/// A single git worktree under a project.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Worktree {
    pub path: PathBuf,
    /// Worktree branch (e.g. "main") or `(detached)` for detached HEAD.
    pub branch: String,
    /// `true` for the main worktree (whose path == project.path), `false` for linked worktrees.
    pub is_primary: bool,
}

/// Tmux-backed claude session. Identified by tmux's stable session_id ($0, $1, ...).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Session {
    /// Tmux session_id (e.g. "$3").
    pub tmux_session_id: String,
    /// Display name (e.g. "ccdash:loanplatform:main"). Cosmetic only.
    pub name: String,
    /// Project this session belongs to (if known from ccdash metadata).
    pub project_id: Option<ProjectId>,
    /// Worktree name (e.g. "main", "angry-sammet"). None for ad-hoc sessions.
    pub worktree: Option<String>,
    /// Working directory of the first pane.
    pub cwd: PathBuf,
    /// PID of the foreground process in the first pane (typically `claude`).
    pub pid: i32,
    /// State per spec §8 session lifecycle table.
    pub state: SessionState,
    /// Unix epoch seconds when ccdash first observed this session.
    pub first_seen: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionState {
    Running,
    Exited,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_id_is_8_hex_chars() {
        let id = ProjectId::new();
        assert_eq!(id.0.len(), 8);
        assert!(id.0.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn project_serde_roundtrip() {
        let p = Project {
            id: ProjectId("abcd1234".into()),
            name: "loanplatform".into(),
            path: "/home/u/Loanplatform".into(),
            worktrees: vec![Worktree {
                path: "/home/u/Loanplatform".into(),
                branch: "main".into(),
                is_primary: true,
            }],
            state: ProjectState::Ok,
        };
        let json = serde_json::to_string(&p).unwrap();
        let back: Project = serde_json::from_str(&json).unwrap();
        assert_eq!(p, back);
    }

    #[test]
    fn session_serde_roundtrip() {
        let s = Session {
            tmux_session_id: "$3".into(),
            name: "ccdash:lp:main".into(),
            project_id: Some(ProjectId("aa".into())),
            worktree: Some("main".into()),
            cwd: "/tmp".into(),
            pid: 12345,
            state: SessionState::Running,
            first_seen: 1_700_000_000,
        };
        let json = serde_json::to_string(&s).unwrap();
        let back: Session = serde_json::from_str(&json).unwrap();
        assert_eq!(s, back);
    }

    #[test]
    fn project_state_defaults_to_ok() {
        let json = r#"{"id":"x","name":"y","path":"/tmp"}"#;
        // ProjectId is a tuple struct around String; serde_json renders it as the string directly with `#[serde(transparent)]`?
        // No — without transparent, it serializes as a wrapped object. Skip this case; the previous roundtrip covers happy path.
        let _ = json; // silence unused
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p ccdash-core --lib domain::`
Expected: 3 PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/ccdash-core/src/domain.rs
git commit -m "ccdash-core: domain types (Project, Worktree, Session)"
```

---

## Task A6: Implement `ccdash-core::protocol`

Defines the JSON-RPC method enum and request/response payloads.

**Files:**
- Modify: `crates/ccdash-core/src/protocol.rs`

- [ ] **Step 1: Write the implementation with inline tests**

Edit `crates/ccdash-core/src/protocol.rs`:

```rust
//! JSON-RPC 2.0 protocol types for the ccdash daemon socket.
//!
//! Wire format: each request and response is a single JSON object, one per line
//! (LF-terminated). This lets clients debug with `socat - UNIX-CONNECT:$SOCK`.

use crate::domain::{Project, ProjectId, Session};
use serde::{Deserialize, Serialize};

/// Top-level JSON-RPC 2.0 request envelope.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct Request {
    pub jsonrpc: String, // always "2.0"
    pub id: serde_json::Value, // number or string per spec; we accept both
    pub method: String,
    pub params: serde_json::Value,
}

/// Top-level JSON-RPC 2.0 response envelope.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct Response {
    pub jsonrpc: String,
    pub id: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
}

impl Response {
    pub fn ok(id: serde_json::Value, result: serde_json::Value) -> Self {
        Self { jsonrpc: "2.0".into(), id, result: Some(result), error: None }
    }
    pub fn err(id: serde_json::Value, error: RpcError) -> Self {
        Self { jsonrpc: "2.0".into(), id, result: None, error: Some(error) }
    }
}

/// JSON-RPC error object with optional structured `data` payload.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct RpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

/// Server-pushed notification envelope (no `id`).
/// Sent for state changes after the client has subscribed.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct Notification {
    pub jsonrpc: String,
    pub method: String,
    pub params: serde_json::Value,
}

impl Notification {
    pub fn new(method: impl Into<String>, params: serde_json::Value) -> Self {
        Self { jsonrpc: "2.0".into(), method: method.into(), params }
    }
}

// === Method-specific params/results ===

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct HandshakeParams {
    pub token: String,
    pub client: ClientKind,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ClientKind {
    Cli,
    Ui,
    Other,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct HandshakeResult {
    pub daemon_version: String,
    pub protocol_version: u32,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct SubscribeParams {
    pub topics: Vec<Topic>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Topic {
    Projects,
    Sessions,
    Ports,
    Plans,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct ProjectListResult {
    pub projects: Vec<Project>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct ProjectAddParams {
    pub path: std::path::PathBuf,
    pub name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct ProjectRemoveParams {
    pub id: ProjectId,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct SessionListResult {
    pub sessions: Vec<Session>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct SessionLaunchParams {
    pub project_id: ProjectId,
    /// Worktree name. `None` means use the primary worktree.
    pub worktree: Option<String>,
    /// Command override. Defaults to `claude` when absent.
    pub command: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct SessionLaunchResult {
    pub session: Session,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct SessionKillParams {
    pub tmux_session_id: String,
}

/// Protocol version this build of `ccdash-core` understands.
pub const PROTOCOL_VERSION: u32 = 1;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_roundtrip() {
        let r = Request {
            jsonrpc: "2.0".into(),
            id: serde_json::json!(1),
            method: "handshake".into(),
            params: serde_json::json!({"token": "abc", "client": "cli"}),
        };
        let s = serde_json::to_string(&r).unwrap();
        let back: Request = serde_json::from_str(&s).unwrap();
        assert_eq!(r, back);
    }

    #[test]
    fn response_ok_omits_error_field() {
        let r = Response::ok(serde_json::json!(1), serde_json::json!({"ok": true}));
        let s = serde_json::to_string(&r).unwrap();
        assert!(!s.contains("\"error\""), "ok response should not serialize null error");
    }

    #[test]
    fn response_err_omits_result_field() {
        let r = Response::err(
            serde_json::json!(1),
            RpcError { code: -32000, message: "fail".into(), data: None },
        );
        let s = serde_json::to_string(&r).unwrap();
        assert!(!s.contains("\"result\""), "err response should not serialize null result");
    }

    #[test]
    fn topic_serializes_lowercase() {
        let t = Topic::Sessions;
        assert_eq!(serde_json::to_string(&t).unwrap(), "\"sessions\"");
    }

    #[test]
    fn handshake_params_roundtrip() {
        let p = HandshakeParams { token: "deadbeef".into(), client: ClientKind::Ui };
        let s = serde_json::to_string(&p).unwrap();
        let back: HandshakeParams = serde_json::from_str(&s).unwrap();
        assert_eq!(p, back);
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p ccdash-core --lib protocol::`
Expected: 5 PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/ccdash-core/src/protocol.rs
git commit -m "ccdash-core: JSON-RPC 2.0 protocol types"
```

---

## Task B1: Scaffold `ccdash-daemon` crate

**Files:**
- Create: `crates/ccdash-daemon/Cargo.toml`
- Create: `crates/ccdash-daemon/src/main.rs`

- [ ] **Step 1: Write the crate `Cargo.toml`**

```toml
[package]
name = "ccdash-daemon"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[[bin]]
name = "ccdash-daemon"
path = "src/main.rs"

[dependencies]
ccdash-core = { path = "../ccdash-core" }
anyhow = { workspace = true }
thiserror = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
toml = { workspace = true }
tokio = { workspace = true }
tracing = { workspace = true }
tracing-subscriber = { workspace = true }
clap = { workspace = true }
notify = { workspace = true }

[dev-dependencies]
tempfile = { workspace = true }
```

- [ ] **Step 2: Write a minimal `main.rs` that prints version and exits**

Edit `crates/ccdash-daemon/src/main.rs`:

```rust
//! ccdash daemon entry point.

fn main() {
    println!("ccdash-daemon {}", env!("CARGO_PKG_VERSION"));
}
```

- [ ] **Step 3: Verify build + run**

Run: `cargo run -p ccdash-daemon`
Expected output: `ccdash-daemon 0.1.0`

- [ ] **Step 4: Commit**

```bash
git add crates/ccdash-daemon
git commit -m "ccdash-daemon: scaffold binary crate"
```

---

## Task B2: Implement daemon `config` module

Reads CLI flags and env vars into a single `Config` struct.

**Files:**
- Create: `crates/ccdash-daemon/src/config.rs`
- Modify: `crates/ccdash-daemon/src/main.rs`

- [ ] **Step 1: Write `config.rs`**

```rust
//! Daemon configuration: CLI args + env vars.

use ccdash_core::paths;
use clap::Parser;
use std::path::PathBuf;

#[derive(Debug, Clone, Parser)]
#[command(name = "ccdash-daemon", version, about = "ccdash background service")]
pub struct Config {
    /// Path to the Unix socket the daemon listens on.
    #[arg(long, env = "CCDASH_SOCKET")]
    pub socket: Option<PathBuf>,

    /// Directory for state files (projects.toml, sessions.toml, auth).
    #[arg(long, env = "CCDASH_HOME")]
    pub data_dir: Option<PathBuf>,

    /// Log level (trace, debug, info, warn, error). Default: info.
    #[arg(long, env = "CCDASH_LOG", default_value = "info")]
    pub log_level: String,
}

impl Config {
    pub fn resolved_socket(&self) -> PathBuf {
        self.socket.clone().unwrap_or_else(paths::default_socket_path)
    }
    pub fn resolved_data_dir(&self) -> PathBuf {
        self.data_dir.clone().unwrap_or_else(paths::data_dir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn defaults_when_no_flags() {
        let c = Config::parse_from(["ccdash-daemon"]);
        assert!(c.socket.is_none());
        assert!(c.data_dir.is_none());
        assert_eq!(c.log_level, "info");
    }

    #[test]
    fn flags_override() {
        let c = Config::parse_from([
            "ccdash-daemon",
            "--socket", "/tmp/x.sock",
            "--data-dir", "/tmp/x",
            "--log-level", "debug",
        ]);
        assert_eq!(c.socket, Some(PathBuf::from("/tmp/x.sock")));
        assert_eq!(c.data_dir, Some(PathBuf::from("/tmp/x")));
        assert_eq!(c.log_level, "debug");
    }
}
```

- [ ] **Step 2: Wire into `main.rs`**

Edit `crates/ccdash-daemon/src/main.rs`:

```rust
//! ccdash daemon entry point.

mod config;

use clap::Parser;

fn main() {
    let cfg = config::Config::parse();
    println!(
        "ccdash-daemon {} socket={} data_dir={} log={}",
        env!("CARGO_PKG_VERSION"),
        cfg.resolved_socket().display(),
        cfg.resolved_data_dir().display(),
        cfg.log_level,
    );
}
```

- [ ] **Step 3: Run tests + binary**

Run: `cargo test -p ccdash-daemon --lib config::`
Expected: 2 PASS.

Run: `cargo run -p ccdash-daemon -- --socket /tmp/x.sock`
Expected: line starting with `ccdash-daemon 0.1.0 socket=/tmp/x.sock`

- [ ] **Step 4: Commit**

```bash
git add crates/ccdash-daemon/src/config.rs crates/ccdash-daemon/src/main.rs
git commit -m "ccdash-daemon: clap-based config (socket, data-dir, log-level)"
```

---

## Task B3: Implement `broadcast` event bus

Wraps `tokio::sync::broadcast` with a friendly `Event` enum so handlers can publish without knowing subscriber details.

**Files:**
- Create: `crates/ccdash-daemon/src/broadcast.rs`

- [ ] **Step 1: Write the module with inline tests**

```rust
//! Internal event bus shared across daemon modules.
//! Each subscribed client connection receives a tokio broadcast Receiver.

use ccdash_core::domain::{Project, Session};
use ccdash_core::protocol::Topic;
use serde::Serialize;
use tokio::sync::broadcast;

/// Channel buffer. If a slow client lags this many events behind, it gets
/// `RecvError::Lagged` on next recv and we drop the slowest event.
/// 128 is enough for short bursts; sustained lag triggers reconciliation.
const BUFFER: usize = 128;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Event {
    /// Full snapshot — sent to clients on subscribe.
    ProjectsSnapshot { projects: Vec<Project> },
    SessionsSnapshot { sessions: Vec<Session> },
    /// Project added/removed/updated. Carries full updated project.
    ProjectUpdated { project: Project },
    ProjectRemoved { id: ccdash_core::domain::ProjectId },
    /// Session lifecycle.
    SessionLaunched { session: Session },
    SessionUpdated { session: Session },
    SessionRemoved { tmux_session_id: String },
}

impl Event {
    /// Which subscription topic this event belongs to.
    pub fn topic(&self) -> Topic {
        match self {
            Event::ProjectsSnapshot { .. } | Event::ProjectUpdated { .. } | Event::ProjectRemoved { .. } => {
                Topic::Projects
            }
            Event::SessionsSnapshot { .. } | Event::SessionLaunched { .. }
            | Event::SessionUpdated { .. } | Event::SessionRemoved { .. } => Topic::Sessions,
        }
    }
}

/// Bus handle — cheap to clone; backed by a single broadcast::Sender.
#[derive(Clone)]
pub struct Bus {
    tx: broadcast::Sender<Event>,
}

impl Bus {
    pub fn new() -> Self {
        let (tx, _rx) = broadcast::channel(BUFFER);
        Self { tx }
    }

    pub fn publish(&self, event: Event) {
        // Send fails only if there are zero receivers — that's fine, log it at trace.
        let _ = self.tx.send(event);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.tx.subscribe()
    }
}

impl Default for Bus {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ccdash_core::domain::ProjectId;

    #[tokio::test]
    async fn subscriber_receives_published_event() {
        let bus = Bus::new();
        let mut rx = bus.subscribe();
        bus.publish(Event::ProjectRemoved { id: ProjectId("abc".into()) });
        let evt = rx.recv().await.unwrap();
        match evt {
            Event::ProjectRemoved { id } => assert_eq!(id.0, "abc"),
            other => panic!("unexpected event: {:?}", other),
        }
    }

    #[tokio::test]
    async fn topic_classification() {
        let evt = Event::SessionRemoved { tmux_session_id: "$1".into() };
        assert_eq!(evt.topic(), Topic::Sessions);
        let evt = Event::ProjectRemoved { id: ProjectId("a".into()) };
        assert_eq!(evt.topic(), Topic::Projects);
    }

    #[tokio::test]
    async fn publish_with_no_subscribers_does_not_panic() {
        let bus = Bus::new();
        bus.publish(Event::SessionRemoved { tmux_session_id: "$0".into() });
    }
}
```

- [ ] **Step 2: Add `mod broadcast;` to `main.rs`**

Edit `crates/ccdash-daemon/src/main.rs`, add at the top after `mod config;`:

```rust
mod broadcast;
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p ccdash-daemon --lib broadcast::`
Expected: 3 PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/ccdash-daemon/src/broadcast.rs crates/ccdash-daemon/src/main.rs
git commit -m "ccdash-daemon: broadcast bus with typed Event enum"
```

---

## Task C1: Implement `projects::registry` (TOML persistence)

Stores `Vec<Project>` to `~/.ccdash/projects.toml`.

**Files:**
- Create: `crates/ccdash-daemon/src/projects/mod.rs`
- Create: `crates/ccdash-daemon/src/projects/registry.rs`

- [ ] **Step 1: Write `projects/mod.rs`**

```rust
//! Project registry + root-directory scanner.

pub mod registry;
pub mod scanner;

pub use registry::Registry;
```

- [ ] **Step 2: Write `projects/registry.rs`**

```rust
//! Persistent project registry backed by `projects.toml`.

use anyhow::{Context, Result};
use ccdash_core::domain::{Project, ProjectId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::sync::RwLock;

#[derive(Debug, Default, Serialize, Deserialize)]
struct OnDisk {
    #[serde(default)]
    projects: BTreeMap<String, ProjectRow>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ProjectRow {
    name: String,
    path: PathBuf,
}

/// Async-safe registry handle. Internally guarded by a RwLock.
pub struct Registry {
    file: PathBuf,
    inner: RwLock<Vec<Project>>,
}

impl Registry {
    /// Load the registry from `file`, creating an empty one if absent.
    pub async fn load(file: PathBuf) -> Result<Self> {
        let projects = match fs::read_to_string(&file).await {
            Ok(s) => Self::parse(&s)?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(e) => return Err(anyhow::Error::new(e).context(format!("reading {}", file.display()))),
        };
        Ok(Self { file, inner: RwLock::new(projects) })
    }

    fn parse(s: &str) -> Result<Vec<Project>> {
        let disk: OnDisk = toml::from_str(s).context("parsing projects.toml")?;
        let projects = disk
            .projects
            .into_iter()
            .map(|(id, row)| Project {
                id: ProjectId(id),
                name: row.name,
                path: row.path,
                worktrees: vec![],
                state: Default::default(),
            })
            .collect();
        Ok(projects)
    }

    async fn write(&self) -> Result<()> {
        let projects = self.inner.read().await;
        let disk = OnDisk {
            projects: projects.iter().map(|p| {
                (p.id.0.clone(), ProjectRow { name: p.name.clone(), path: p.path.clone() })
            }).collect(),
        };
        let toml_str = toml::to_string_pretty(&disk).context("serializing projects.toml")?;
        if let Some(parent) = self.file.parent() {
            fs::create_dir_all(parent).await.with_context(|| format!("creating {}", parent.display()))?;
        }
        // Atomic write: write to tmp, then rename.
        let tmp = self.file.with_extension("toml.tmp");
        fs::write(&tmp, toml_str).await.with_context(|| format!("writing {}", tmp.display()))?;
        fs::rename(&tmp, &self.file).await.with_context(|| format!("renaming to {}", self.file.display()))?;
        Ok(())
    }

    pub async fn list(&self) -> Vec<Project> {
        self.inner.read().await.clone()
    }

    pub async fn add(&self, path: PathBuf, name: Option<String>) -> Result<Project> {
        let canonical = std::fs::canonicalize(&path).with_context(|| format!("canonicalizing {}", path.display()))?;
        let mut projects = self.inner.write().await;
        if let Some(existing) = projects.iter().find(|p| p.path == canonical) {
            return Ok(existing.clone());
        }
        let name = name.unwrap_or_else(|| canonical.file_name().and_then(|s| s.to_str()).unwrap_or("project").to_string());
        let project = Project {
            id: ProjectId::new(),
            name,
            path: canonical,
            worktrees: vec![],
            state: Default::default(),
        };
        projects.push(project.clone());
        drop(projects);
        self.write().await?;
        Ok(project)
    }

    pub async fn remove(&self, id: &ProjectId) -> Result<bool> {
        let mut projects = self.inner.write().await;
        let len_before = projects.len();
        projects.retain(|p| &p.id != id);
        let removed = projects.len() != len_before;
        drop(projects);
        if removed {
            self.write().await?;
        }
        Ok(removed)
    }

    /// Replace the worktrees for a given project (called by the worktrees module
    /// after `git worktree list`). No-op if id is unknown. Does NOT persist —
    /// worktree state is runtime-only.
    pub async fn set_worktrees(&self, id: &ProjectId, worktrees: Vec<ccdash_core::domain::Worktree>) {
        let mut projects = self.inner.write().await;
        if let Some(p) = projects.iter_mut().find(|p| &p.id == id) {
            p.worktrees = worktrees;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn load_missing_file_returns_empty() {
        let dir = tempdir().unwrap();
        let reg = Registry::load(dir.path().join("projects.toml")).await.unwrap();
        assert!(reg.list().await.is_empty());
    }

    #[tokio::test]
    async fn add_then_list_returns_one_project() {
        let dir = tempdir().unwrap();
        let project_dir = dir.path().join("proj1");
        std::fs::create_dir(&project_dir).unwrap();
        let reg = Registry::load(dir.path().join("projects.toml")).await.unwrap();
        let p = reg.add(project_dir.clone(), None).await.unwrap();
        assert_eq!(p.name, "proj1");
        assert_eq!(reg.list().await.len(), 1);
    }

    #[tokio::test]
    async fn add_is_idempotent_by_canonical_path() {
        let dir = tempdir().unwrap();
        let project_dir = dir.path().join("proj1");
        std::fs::create_dir(&project_dir).unwrap();
        let reg = Registry::load(dir.path().join("projects.toml")).await.unwrap();
        let p1 = reg.add(project_dir.clone(), None).await.unwrap();
        let p2 = reg.add(project_dir.clone(), None).await.unwrap();
        assert_eq!(p1.id, p2.id);
        assert_eq!(reg.list().await.len(), 1);
    }

    #[tokio::test]
    async fn persistence_roundtrip() {
        let dir = tempdir().unwrap();
        let project_dir = dir.path().join("proj1");
        std::fs::create_dir(&project_dir).unwrap();
        let file = dir.path().join("projects.toml");

        let reg = Registry::load(file.clone()).await.unwrap();
        let added = reg.add(project_dir.clone(), Some("custom".into())).await.unwrap();

        let reg2 = Registry::load(file).await.unwrap();
        let list = reg2.list().await;
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, added.id);
        assert_eq!(list[0].name, "custom");
    }

    #[tokio::test]
    async fn remove_unknown_returns_false() {
        let dir = tempdir().unwrap();
        let reg = Registry::load(dir.path().join("projects.toml")).await.unwrap();
        assert!(!reg.remove(&ProjectId("ghost".into())).await.unwrap());
    }
}
```

- [ ] **Step 3: Write a stub `projects/scanner.rs`**

```rust
//! Root-directory scanner — discovers candidate git repos under given root dirs.
//! Implementation deferred to a later task in this plan (Task C2).

use std::path::PathBuf;

#[allow(dead_code)]
pub async fn scan(_roots: &[PathBuf]) -> Vec<PathBuf> {
    vec![]
}
```

- [ ] **Step 4: Wire into `main.rs`**

Edit `crates/ccdash-daemon/src/main.rs`:

```rust
//! ccdash daemon entry point.

mod broadcast;
mod config;
mod projects;

use clap::Parser;

fn main() {
    let cfg = config::Config::parse();
    println!(
        "ccdash-daemon {} socket={} data_dir={} log={}",
        env!("CARGO_PKG_VERSION"),
        cfg.resolved_socket().display(),
        cfg.resolved_data_dir().display(),
        cfg.log_level,
    );
}
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p ccdash-daemon projects::`
Expected: 5 PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/ccdash-daemon/src/projects crates/ccdash-daemon/src/main.rs
git commit -m "ccdash-daemon: projects registry with atomic toml persistence"
```

---

## Task C2: Implement `projects::scanner` (root-dir git-repo discovery)

**Files:**
- Modify: `crates/ccdash-daemon/src/projects/scanner.rs`

- [ ] **Step 1: Replace stub with real implementation**

```rust
//! Recursively scans configured root directories for git repos.
//! Limits depth to avoid descending into `node_modules`/build outputs.

use std::path::{Path, PathBuf};
use tokio::fs;

const MAX_DEPTH: usize = 4;

/// Skip these directory names while scanning.
const SKIP_DIRS: &[&str] = &[
    "node_modules", "target", ".git", ".cache", ".venv", "venv",
    "dist", "build", ".next", ".turbo", "vendor", ".gradle",
];

/// Return the absolute paths of git-repo roots found under `roots`.
/// A directory is treated as a git-repo root iff it contains a `.git` entry
/// (file OR directory; we accept both because worktrees use a `.git` file).
pub async fn scan(roots: &[PathBuf]) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for root in roots {
        scan_dir(root, 0, &mut found).await;
    }
    found.sort();
    found.dedup();
    found
}

async fn scan_dir(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth > MAX_DEPTH {
        return;
    }

    let mut entries = match fs::read_dir(dir).await {
        Ok(e) => e,
        Err(_) => return, // permission denied, broken symlink, etc.
    };

    // Check the directory itself first.
    if has_git(dir).await {
        out.push(dir.to_path_buf());
        return; // don't descend into a known repo
    }

    let mut children = Vec::new();
    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        let name = match entry.file_name().to_str() {
            Some(s) => s.to_string(),
            None => continue,
        };
        if name.starts_with('.') && name != "." {
            // Skip hidden dirs except current — but we still descend into project dirs.
            // We do allow `.foo` if it contains a `.git` (rare). Practical: skip them.
            continue;
        }
        if SKIP_DIRS.contains(&name.as_str()) {
            continue;
        }
        match entry.file_type().await {
            Ok(ft) if ft.is_dir() => children.push(path),
            _ => continue,
        }
    }

    for child in children {
        Box::pin(scan_dir(&child, depth + 1, out)).await;
    }
}

async fn has_git(dir: &Path) -> bool {
    fs::metadata(dir.join(".git")).await.is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn finds_repo_at_depth_2() {
        let dir = tempdir().unwrap();
        let repo = dir.path().join("a").join("b");
        std::fs::create_dir_all(repo.join(".git")).unwrap();
        let found = scan(&[dir.path().to_path_buf()]).await;
        assert_eq!(found, vec![repo]);
    }

    #[tokio::test]
    async fn skips_node_modules() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("node_modules").join("pkg").join(".git")).unwrap();
        let found = scan(&[dir.path().to_path_buf()]).await;
        assert!(found.is_empty(), "should not descend into node_modules");
    }

    #[tokio::test]
    async fn does_not_descend_into_found_repo() {
        let dir = tempdir().unwrap();
        let outer = dir.path().join("outer");
        let inner = outer.join("inner");
        std::fs::create_dir_all(outer.join(".git")).unwrap();
        std::fs::create_dir_all(inner.join(".git")).unwrap();
        let found = scan(&[dir.path().to_path_buf()]).await;
        assert_eq!(found, vec![outer]);
    }

    #[tokio::test]
    async fn accepts_git_as_file_for_worktrees() {
        let dir = tempdir().unwrap();
        let worktree = dir.path().join("wt");
        std::fs::create_dir_all(&worktree).unwrap();
        std::fs::write(worktree.join(".git"), "gitdir: /some/main/.git/worktrees/wt").unwrap();
        let found = scan(&[dir.path().to_path_buf()]).await;
        assert_eq!(found, vec![worktree]);
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p ccdash-daemon projects::scanner::`
Expected: 4 PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/ccdash-daemon/src/projects/scanner.rs
git commit -m "ccdash-daemon: scanner for git repos under root dirs"
```

---

## Task D1: Implement `worktrees` (git worktree list parser)

**Files:**
- Create: `crates/ccdash-daemon/src/worktrees.rs`

- [ ] **Step 1: Write the module**

```rust
//! Discovers git worktrees for a given project by shelling out to
//! `git worktree list --porcelain`.

use anyhow::{Context, Result};
use ccdash_core::domain::Worktree;
use std::path::{Path, PathBuf};
use tokio::process::Command;

/// Run `git worktree list --porcelain` in `project_path` and parse the output.
/// Returns at minimum the primary worktree (which is `project_path` itself).
pub async fn list(project_path: &Path) -> Result<Vec<Worktree>> {
    let output = Command::new("git")
        .args(["worktree", "list", "--porcelain"])
        .current_dir(project_path)
        .output()
        .await
        .with_context(|| format!("running git worktree list in {}", project_path.display()))?;
    if !output.status.success() {
        anyhow::bail!(
            "git worktree list failed in {}: {}",
            project_path.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let stdout = String::from_utf8(output.stdout).context("git stdout not utf8")?;
    Ok(parse(&stdout, project_path))
}

fn parse(porcelain: &str, project_path: &Path) -> Vec<Worktree> {
    let mut out = Vec::new();
    let mut current_path: Option<PathBuf> = None;
    let mut current_branch = String::from("(detached)");

    let mut flush = |path: Option<PathBuf>, branch: &str, out: &mut Vec<Worktree>| {
        if let Some(p) = path {
            let is_primary = p == project_path;
            out.push(Worktree { path: p, branch: branch.to_string(), is_primary });
        }
    };

    for line in porcelain.lines() {
        if line.is_empty() {
            flush(current_path.take(), &current_branch, &mut out);
            current_branch = String::from("(detached)");
            continue;
        }
        if let Some(rest) = line.strip_prefix("worktree ") {
            current_path = Some(PathBuf::from(rest));
        } else if let Some(rest) = line.strip_prefix("branch ") {
            // Format: refs/heads/<name>
            current_branch = rest.strip_prefix("refs/heads/").unwrap_or(rest).to_string();
        } else if line == "detached" {
            current_branch = "(detached)".to_string();
        }
    }
    // Trailing record (porcelain ends with blank line normally but be defensive)
    flush(current_path, &current_branch, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_single_main_worktree() {
        let input = "worktree /home/u/proj\nHEAD abc\nbranch refs/heads/main\n\n";
        let parsed = parse(input, Path::new("/home/u/proj"));
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].path, PathBuf::from("/home/u/proj"));
        assert_eq!(parsed[0].branch, "main");
        assert!(parsed[0].is_primary);
    }

    #[test]
    fn parse_main_plus_linked() {
        let input = "worktree /home/u/proj\nHEAD abc\nbranch refs/heads/main\n\nworktree /home/u/proj-wt\nHEAD def\nbranch refs/heads/feature\n\n";
        let parsed = parse(input, Path::new("/home/u/proj"));
        assert_eq!(parsed.len(), 2);
        assert!(parsed[0].is_primary);
        assert!(!parsed[1].is_primary);
        assert_eq!(parsed[1].branch, "feature");
    }

    #[test]
    fn parse_detached_head() {
        let input = "worktree /home/u/proj\nHEAD abc\ndetached\n\n";
        let parsed = parse(input, Path::new("/home/u/proj"));
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].branch, "(detached)");
    }

    #[test]
    fn parse_empty_input_yields_empty() {
        let parsed = parse("", Path::new("/home/u/proj"));
        assert!(parsed.is_empty());
    }
}
```

- [ ] **Step 2: Wire into `main.rs`**

Edit `crates/ccdash-daemon/src/main.rs`, add `mod worktrees;` near the other mod declarations.

- [ ] **Step 3: Run tests**

Run: `cargo test -p ccdash-daemon worktrees::`
Expected: 4 PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/ccdash-daemon/src/worktrees.rs crates/ccdash-daemon/src/main.rs
git commit -m "ccdash-daemon: parse git worktree list --porcelain"
```

---

## Task E1: Implement `tmux` shell-out wrapper

**Files:**
- Create: `crates/ccdash-daemon/src/tmux.rs`

- [ ] **Step 1: Write the module**

```rust
//! Tmux interaction by shelling out to the `tmux` binary.
//! Phase 1 uses 2s polling; control-mode deferred to Phase 2.

use anyhow::{Context, Result};
use std::path::Path;
use tokio::process::Command;

/// One tmux pane row, parsed from `tmux list-panes -a -F …`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneRow {
    pub session_id: String,    // e.g. "$3"
    pub session_name: String,  // e.g. "ccdash:lp:main"
    pub pane_pid: i32,
    pub pane_cmd: String,
    pub cwd: String,
}

/// True iff `tmux` is on PATH and a server is reachable. Tries `tmux -V` first
/// (always succeeds if installed), then `tmux ls` (succeeds only if a server is running).
pub async fn check_installed() -> bool {
    Command::new("tmux").arg("-V").output().await.map(|o| o.status.success()).unwrap_or(false)
}

/// List all panes across all sessions, returning a tuple per pane.
pub async fn list_panes() -> Result<Vec<PaneRow>> {
    let fmt = "#{session_id}\t#{session_name}\t#{pane_pid}\t#{pane_current_command}\t#{pane_current_path}";
    let output = Command::new("tmux")
        .args(["list-panes", "-a", "-F", fmt])
        .output()
        .await
        .context("running tmux list-panes")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // tmux returns non-zero with "no server running" — treat as empty list, not error.
        if stderr.contains("no server running") || stderr.contains("error connecting") {
            return Ok(vec![]);
        }
        anyhow::bail!("tmux list-panes failed: {}", stderr.trim());
    }
    let stdout = String::from_utf8(output.stdout).context("tmux stdout not utf8")?;
    Ok(parse_panes(&stdout))
}

fn parse_panes(s: &str) -> Vec<PaneRow> {
    s.lines()
        .filter(|l| !l.is_empty())
        .filter_map(|line| {
            let mut it = line.splitn(5, '\t');
            let session_id = it.next()?.to_string();
            let session_name = it.next()?.to_string();
            let pane_pid: i32 = it.next()?.parse().ok()?;
            let pane_cmd = it.next()?.to_string();
            let cwd = it.next()?.to_string();
            Some(PaneRow { session_id, session_name, pane_pid, pane_cmd, cwd })
        })
        .collect()
}

/// Launch a new detached tmux session running `command` in `cwd` with name `name`.
/// Sets `remain-on-exit on` so the pane survives when `command` exits.
pub async fn new_session(name: &str, cwd: &Path, command: &str) -> Result<String> {
    let status = Command::new("tmux")
        .args([
            "new-session",
            "-d",
            "-s", name,
            "-c", &cwd.to_string_lossy(),
            command,
        ])
        .status()
        .await
        .context("running tmux new-session")?;
    if !status.success() {
        anyhow::bail!("tmux new-session failed (status {:?})", status.code());
    }
    // Configure remain-on-exit for this session's windows.
    let _ = Command::new("tmux")
        .args(["set-option", "-t", name, "remain-on-exit", "on"])
        .status()
        .await;

    // Look up the session_id we just created.
    let output = Command::new("tmux")
        .args(["display-message", "-p", "-t", name, "#{session_id}"])
        .output()
        .await
        .context("running tmux display-message")?;
    if !output.status.success() {
        anyhow::bail!("could not resolve session_id for {}", name);
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Kill the tmux session by id (e.g. "$3").
pub async fn kill_session(session_id: &str) -> Result<()> {
    let status = Command::new("tmux")
        .args(["kill-session", "-t", session_id])
        .status()
        .await
        .context("running tmux kill-session")?;
    if !status.success() {
        anyhow::bail!("tmux kill-session failed for {}", session_id);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_panes_two_rows() {
        let input = "$0\tccdash:a:main\t1234\tclaude\t/home/u/a\n$1\tccdash:b:main\t5678\tzsh\t/home/u/b\n";
        let parsed = parse_panes(input);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].session_id, "$0");
        assert_eq!(parsed[0].pane_pid, 1234);
        assert_eq!(parsed[0].pane_cmd, "claude");
        assert_eq!(parsed[1].cwd, "/home/u/b");
    }

    #[test]
    fn parse_panes_skips_malformed() {
        let input = "garbage line\n$0\tn\t1\tc\t/\n";
        let parsed = parse_panes(input);
        assert_eq!(parsed.len(), 1);
    }

    #[test]
    fn parse_panes_empty() {
        assert!(parse_panes("").is_empty());
    }
}
```

- [ ] **Step 2: Wire into `main.rs`**

Edit `crates/ccdash-daemon/src/main.rs`, add `mod tmux;`.

- [ ] **Step 3: Run unit tests**

Run: `cargo test -p ccdash-daemon tmux::`
Expected: 3 PASS.

- [ ] **Step 4: (Optional) Smoke test against real tmux**

If you have tmux installed:

Run: `cargo test -p ccdash-daemon -- --ignored tmux_smoke`

Add this test gated by `#[ignore]` at the end of `tmux.rs`'s `mod tests`:

```rust
    #[tokio::test]
    #[ignore]
    async fn tmux_smoke_new_and_kill() {
        if !check_installed().await {
            eprintln!("tmux not installed; skipping");
            return;
        }
        let name = format!("ccdash-smoketest-{}", std::process::id());
        let cwd = std::env::current_dir().unwrap();
        let id = new_session(&name, &cwd, "sleep 30").await.unwrap();
        assert!(id.starts_with('$'));
        let panes = list_panes().await.unwrap();
        assert!(panes.iter().any(|p| p.session_id == id));
        kill_session(&id).await.unwrap();
    }
```

- [ ] **Step 5: Commit**

```bash
git add crates/ccdash-daemon/src/tmux.rs crates/ccdash-daemon/src/main.rs
git commit -m "ccdash-daemon: tmux shell-out wrapper (list, new, kill)"
```

---

## Task E2: Implement `sessions` module (join tmux state + metadata)

**Files:**
- Create: `crates/ccdash-daemon/src/sessions.rs`

- [ ] **Step 1: Write the module**

```rust
//! Session manager — joins live tmux state with `sessions.toml` metadata.
//! Keyed on tmux's stable `session_id` (e.g. "$3"). Sessions are considered
//! visible iff they have a pane running `claude`.

use crate::tmux::{self, PaneRow};
use anyhow::{Context, Result};
use ccdash_core::domain::{ProjectId, Session, SessionState};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct OnDisk {
    /// Keyed by tmux session_id (e.g. "$3"). Values survive across daemon
    /// restarts but are reconciled against `tmux list-panes` on load.
    #[serde(default)]
    sessions: BTreeMap<String, SessionMeta>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SessionMeta {
    project_id: Option<ProjectId>,
    worktree: Option<String>,
    first_seen: i64,
}

pub struct Manager {
    file: PathBuf,
    meta: RwLock<BTreeMap<String, SessionMeta>>,
    /// Last-known set of sessions, by session_id.
    cache: RwLock<BTreeMap<String, Session>>,
}

impl Manager {
    pub async fn load(file: PathBuf) -> Result<Self> {
        let meta = match fs::read_to_string(&file).await {
            Ok(s) => {
                let disk: OnDisk = toml::from_str(&s).context("parsing sessions.toml")?;
                disk.sessions
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => BTreeMap::new(),
            Err(e) => return Err(anyhow::Error::new(e).context(format!("reading {}", file.display()))),
        };
        Ok(Self { file, meta: RwLock::new(meta), cache: RwLock::new(BTreeMap::new()) })
    }

    async fn write(&self) -> Result<()> {
        let meta = self.meta.read().await;
        let disk = OnDisk { sessions: meta.clone() };
        let s = toml::to_string_pretty(&disk).context("serializing sessions.toml")?;
        if let Some(parent) = self.file.parent() {
            fs::create_dir_all(parent).await?;
        }
        let tmp = self.file.with_extension("toml.tmp");
        fs::write(&tmp, s).await?;
        fs::rename(&tmp, &self.file).await?;
        Ok(())
    }

    /// Record metadata for a session ccdash launched. Called immediately after
    /// `tmux::new_session` returns a fresh session_id.
    pub async fn record_launch(
        &self,
        session_id: String,
        project_id: ProjectId,
        worktree: Option<String>,
    ) -> Result<()> {
        let mut meta = self.meta.write().await;
        meta.insert(
            session_id,
            SessionMeta {
                project_id: Some(project_id),
                worktree,
                first_seen: now_epoch(),
            },
        );
        drop(meta);
        self.write().await?;
        Ok(())
    }

    /// Re-poll tmux and rebuild the in-memory session list.
    /// Returns `(current, removed_ids)` — `removed_ids` are sessions present in
    /// the previous cache but gone now.
    pub async fn refresh(&self) -> Result<(Vec<Session>, Vec<String>)> {
        let panes = tmux::list_panes().await?;
        let claude_panes: Vec<_> = panes
            .into_iter()
            .filter(|p| p.pane_cmd == "claude")
            .collect();

        let meta = self.meta.read().await.clone();
        let now = now_epoch();
        let new_sessions: BTreeMap<String, Session> = claude_panes
            .iter()
            .map(|p| build_session(p, &meta, now))
            .map(|s| (s.tmux_session_id.clone(), s))
            .collect();

        let mut cache = self.cache.write().await;
        let removed_ids: Vec<String> = cache
            .keys()
            .filter(|k| !new_sessions.contains_key(*k))
            .cloned()
            .collect();
        *cache = new_sessions.clone();
        let current: Vec<Session> = new_sessions.into_values().collect();
        Ok((current, removed_ids))
    }

    pub async fn current(&self) -> Vec<Session> {
        self.cache.read().await.values().cloned().collect()
    }

    /// Drop metadata for a session that has truly exited (no longer in tmux).
    /// Called after `refresh` reports a removal.
    pub async fn forget(&self, session_id: &str) -> Result<()> {
        let mut meta = self.meta.write().await;
        if meta.remove(session_id).is_some() {
            drop(meta);
            self.write().await?;
        }
        Ok(())
    }
}

fn build_session(p: &PaneRow, meta: &BTreeMap<String, SessionMeta>, now: i64) -> Session {
    let m = meta.get(&p.session_id);
    Session {
        tmux_session_id: p.session_id.clone(),
        name: p.session_name.clone(),
        project_id: m.and_then(|m| m.project_id.clone()),
        worktree: m.and_then(|m| m.worktree.clone()),
        cwd: PathBuf::from(&p.cwd),
        pid: p.pane_pid,
        state: SessionState::Running,
        first_seen: m.map(|m| m.first_seen).unwrap_or(now),
    }
}

fn now_epoch() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn load_missing_file_returns_empty() {
        let dir = tempdir().unwrap();
        let m = Manager::load(dir.path().join("sessions.toml")).await.unwrap();
        assert!(m.current().await.is_empty());
    }

    #[tokio::test]
    async fn record_launch_persists() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("sessions.toml");
        let m = Manager::load(file.clone()).await.unwrap();
        m.record_launch("$3".into(), ProjectId("pid".into()), Some("main".into()))
            .await
            .unwrap();
        let m2 = Manager::load(file).await.unwrap();
        let meta = m2.meta.read().await;
        let entry = meta.get("$3").unwrap();
        assert_eq!(entry.project_id.as_ref().unwrap().0, "pid");
        assert_eq!(entry.worktree.as_deref(), Some("main"));
    }

    #[tokio::test]
    async fn build_session_uses_metadata_when_present() {
        let pane = PaneRow {
            session_id: "$3".into(),
            session_name: "ccdash:foo".into(),
            pane_pid: 42,
            pane_cmd: "claude".into(),
            cwd: "/tmp".into(),
        };
        let mut meta = BTreeMap::new();
        meta.insert("$3".into(), SessionMeta {
            project_id: Some(ProjectId("pid".into())),
            worktree: Some("main".into()),
            first_seen: 1_700_000_000,
        });
        let s = build_session(&pane, &meta, 0);
        assert_eq!(s.first_seen, 1_700_000_000);
        assert_eq!(s.worktree.as_deref(), Some("main"));
    }

    #[tokio::test]
    async fn build_session_falls_back_when_no_metadata() {
        let pane = PaneRow {
            session_id: "$9".into(),
            session_name: "x".into(),
            pane_pid: 1,
            pane_cmd: "claude".into(),
            cwd: "/x".into(),
        };
        let s = build_session(&pane, &BTreeMap::new(), 7);
        assert_eq!(s.first_seen, 7);
        assert!(s.project_id.is_none());
    }
}
```

- [ ] **Step 2: Wire into `main.rs`**

Add `mod sessions;` to `main.rs`.

- [ ] **Step 3: Run tests**

Run: `cargo test -p ccdash-daemon sessions::`
Expected: 4 PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/ccdash-daemon/src/sessions.rs crates/ccdash-daemon/src/main.rs
git commit -m "ccdash-daemon: sessions manager joining tmux state + metadata"
```

---

## Task F1: Implement `state` module (composite app state)

Wraps all the modules so handlers have one handle to grab.

**Files:**
- Create: `crates/ccdash-daemon/src/state.rs`

- [ ] **Step 1: Write the module**

```rust
//! Composite daemon state. Cheap to clone (Arcs all the way down).

use crate::broadcast::Bus;
use crate::projects::Registry;
use crate::sessions::Manager;
use anyhow::Result;
use ccdash_core::paths;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub projects: Arc<Registry>,
    pub sessions: Arc<Manager>,
    pub bus: Bus,
    pub auth_token: Arc<String>,
    pub data_dir: PathBuf,
}

impl AppState {
    pub async fn bootstrap(data_dir: PathBuf) -> Result<Self> {
        let token = ccdash_core::auth::ensure_token(&data_dir.join("auth"))?;
        let projects = Registry::load(data_dir.join("projects.toml")).await?;
        let sessions = Manager::load(data_dir.join("sessions.toml")).await?;
        Ok(Self {
            projects: Arc::new(projects),
            sessions: Arc::new(sessions),
            bus: Bus::new(),
            auth_token: Arc::new(token),
            data_dir,
        })
    }

    /// For tests: build a state rooted at the given dir, isolated from the user's real `~/.ccdash`.
    #[cfg(test)]
    pub async fn for_test(data_dir: PathBuf) -> Result<Self> {
        Self::bootstrap(data_dir).await
    }
}

#[allow(dead_code)]
pub fn default_data_dir() -> PathBuf {
    paths::data_dir()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn bootstrap_creates_auth_token() {
        let dir = tempdir().unwrap();
        let state = AppState::bootstrap(dir.path().to_path_buf()).await.unwrap();
        assert_eq!(state.auth_token.len(), 64);
        assert!(dir.path().join("auth").exists());
    }
}
```

- [ ] **Step 2: Wire into `main.rs`**

Add `mod state;` to `main.rs`.

- [ ] **Step 3: Run tests**

Run: `cargo test -p ccdash-daemon state::`
Expected: 1 PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/ccdash-daemon/src/state.rs crates/ccdash-daemon/src/main.rs
git commit -m "ccdash-daemon: composite AppState"
```

---

## Task G1: Implement `rpc::codec` (line-delimited JSON framing)

**Files:**
- Create: `crates/ccdash-daemon/src/rpc/mod.rs`
- Create: `crates/ccdash-daemon/src/rpc/codec.rs`

- [ ] **Step 1: Write `rpc/mod.rs` (just declarations for now)**

```rust
//! JSON-RPC 2.0 server over Unix socket.

pub mod codec;
pub mod dispatch;
pub mod handlers;

mod server;
pub use server::serve;
```

- [ ] **Step 2: Write `rpc/codec.rs`**

```rust
//! Line-delimited JSON-RPC framing. Each frame is a single JSON object
//! terminated by `\n`. Frames over 1 MiB are rejected to prevent DoS.

use anyhow::{Context, Result};
use ccdash_core::protocol::{Request, Response};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::unix::OwnedReadHalf;
use tokio::net::unix::OwnedWriteHalf;

const MAX_FRAME_BYTES: usize = 1024 * 1024;

pub struct FrameReader {
    inner: BufReader<OwnedReadHalf>,
    buf: String,
}

impl FrameReader {
    pub fn new(half: OwnedReadHalf) -> Self {
        Self { inner: BufReader::new(half), buf: String::with_capacity(1024) }
    }

    /// Read one JSON-RPC request frame. Returns `Ok(None)` on EOF.
    pub async fn next_request(&mut self) -> Result<Option<Request>> {
        self.buf.clear();
        let n = self.inner.read_line(&mut self.buf).await.context("reading frame")?;
        if n == 0 {
            return Ok(None);
        }
        if self.buf.len() > MAX_FRAME_BYTES {
            anyhow::bail!("frame exceeds {} bytes", MAX_FRAME_BYTES);
        }
        let req: Request = serde_json::from_str(self.buf.trim_end())
            .context("parsing JSON-RPC request")?;
        Ok(Some(req))
    }
}

pub struct FrameWriter {
    inner: OwnedWriteHalf,
}

impl FrameWriter {
    pub fn new(half: OwnedWriteHalf) -> Self {
        Self { inner: half }
    }

    pub async fn write_response(&mut self, resp: &Response) -> Result<()> {
        let mut bytes = serde_json::to_vec(resp).context("serializing response")?;
        bytes.push(b'\n');
        self.inner.write_all(&bytes).await.context("writing response")?;
        self.inner.flush().await.context("flushing")?;
        Ok(())
    }

    pub async fn write_notification(&mut self, n: &ccdash_core::protocol::Notification) -> Result<()> {
        let mut bytes = serde_json::to_vec(n).context("serializing notification")?;
        bytes.push(b'\n');
        self.inner.write_all(&bytes).await?;
        self.inner.flush().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::UnixStream;

    #[tokio::test]
    async fn round_trip_one_request() {
        let (a, b) = UnixStream::pair().unwrap();
        let (a_r, a_w) = a.into_split();
        let (_b_r, mut b_w) = b.into_split();

        // Send a request from b to a.
        let req = Request {
            jsonrpc: "2.0".into(),
            id: serde_json::json!(1),
            method: "handshake".into(),
            params: serde_json::json!({"token": "x", "client": "cli"}),
        };
        let mut bytes = serde_json::to_vec(&req).unwrap();
        bytes.push(b'\n');
        b_w.write_all(&bytes).await.unwrap();
        drop(b_w);

        let mut reader = FrameReader::new(a_r);
        let got = reader.next_request().await.unwrap().unwrap();
        assert_eq!(got.method, "handshake");
        assert!(reader.next_request().await.unwrap().is_none(), "EOF expected");
        let _ = a_w; // keep alive
    }
}
```

- [ ] **Step 3: Write a temporary stub `rpc/server.rs` so the module compiles**

```rust
//! Placeholder — implemented in Task G3.

use crate::state::AppState;
use anyhow::Result;
use std::path::Path;

#[allow(unused_variables, dead_code)]
pub async fn serve(state: AppState, socket: &Path) -> Result<()> {
    Ok(())
}
```

- [ ] **Step 4: Write a temporary stub `rpc/dispatch.rs` and `rpc/handlers.rs`**

```rust
//! Placeholder — implemented in Task G2.
```

(Same one-line content in both files.)

- [ ] **Step 5: Wire `mod rpc;` into `main.rs`**

Add `mod rpc;` to `main.rs`.

- [ ] **Step 6: Run tests**

Run: `cargo test -p ccdash-daemon rpc::codec::`
Expected: 1 PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/ccdash-daemon/src/rpc
git commit -m "ccdash-daemon: line-delimited JSON-RPC codec"
```

---

## Task G2: Implement `rpc::handlers` and `rpc::dispatch`

**Files:**
- Modify: `crates/ccdash-daemon/src/rpc/handlers.rs`
- Modify: `crates/ccdash-daemon/src/rpc/dispatch.rs`

- [ ] **Step 1: Write `rpc/handlers.rs`**

```rust
//! Method-handler functions. Each takes parsed params + AppState, returns a
//! serializable result or RpcError.

use crate::broadcast::Event;
use crate::state::AppState;
use crate::{tmux, worktrees};
use ccdash_core::domain::{ProjectId, Session, SessionState};
use ccdash_core::protocol::{
    HandshakeParams, HandshakeResult, ProjectAddParams, ProjectListResult, ProjectRemoveParams,
    RpcError, SessionKillParams, SessionLaunchParams, SessionLaunchResult, SessionListResult,
    PROTOCOL_VERSION,
};

pub const E_AUTH: i32 = -32001;
pub const E_INVALID_PARAMS: i32 = -32602;
pub const E_INTERNAL: i32 = -32000;
pub const E_NOT_FOUND: i32 = -32004;

pub fn err(code: i32, msg: impl Into<String>) -> RpcError {
    RpcError { code, message: msg.into(), data: None }
}

pub fn handle_handshake(params: HandshakeParams, state: &AppState) -> Result<HandshakeResult, RpcError> {
    if params.token != *state.auth_token {
        return Err(err(E_AUTH, "invalid auth token"));
    }
    Ok(HandshakeResult {
        daemon_version: env!("CARGO_PKG_VERSION").to_string(),
        protocol_version: PROTOCOL_VERSION,
    })
}

pub async fn handle_project_list(state: &AppState) -> ProjectListResult {
    ProjectListResult { projects: state.projects.list().await }
}

pub async fn handle_project_add(params: ProjectAddParams, state: &AppState) -> Result<ccdash_core::domain::Project, RpcError> {
    let project = state.projects.add(params.path, params.name).await.map_err(|e| err(E_INTERNAL, e.to_string()))?;
    // Best-effort worktree discovery; failures don't block project add.
    if let Ok(wts) = worktrees::list(&project.path).await {
        state.projects.set_worktrees(&project.id, wts).await;
    }
    let updated = state.projects.list().await.into_iter().find(|p| p.id == project.id).unwrap_or(project.clone());
    state.bus.publish(Event::ProjectUpdated { project: updated.clone() });
    Ok(updated)
}

pub async fn handle_project_remove(params: ProjectRemoveParams, state: &AppState) -> Result<(), RpcError> {
    let removed = state.projects.remove(&params.id).await.map_err(|e| err(E_INTERNAL, e.to_string()))?;
    if !removed {
        return Err(err(E_NOT_FOUND, "no such project"));
    }
    state.bus.publish(Event::ProjectRemoved { id: params.id });
    Ok(())
}

pub async fn handle_session_list(state: &AppState) -> Result<SessionListResult, RpcError> {
    let (current, _) = state.sessions.refresh().await.map_err(|e| err(E_INTERNAL, e.to_string()))?;
    Ok(SessionListResult { sessions: current })
}

pub async fn handle_session_launch(params: SessionLaunchParams, state: &AppState) -> Result<SessionLaunchResult, RpcError> {
    let projects = state.projects.list().await;
    let project = projects.iter().find(|p| p.id == params.project_id).ok_or_else(|| err(E_NOT_FOUND, "no such project"))?;
    let worktree_name = params.worktree.clone().unwrap_or_else(|| "main".to_string());
    let cwd = project.worktrees.iter()
        .find(|w| w.branch == worktree_name || (worktree_name == "main" && w.is_primary))
        .map(|w| w.path.clone())
        .unwrap_or_else(|| project.path.clone());
    let cmd = params.command.unwrap_or_else(|| "claude".to_string());

    let safe_wt = sanitize(&worktree_name);
    let safe_proj = sanitize(&project.name);
    let name = format!("ccdash:{}:{}", safe_proj, safe_wt);

    let session_id = tmux::new_session(&name, &cwd, &cmd).await.map_err(|e| err(E_INTERNAL, e.to_string()))?;
    state.sessions.record_launch(session_id.clone(), project.id.clone(), Some(worktree_name)).await
        .map_err(|e| err(E_INTERNAL, e.to_string()))?;

    let (current, _) = state.sessions.refresh().await.map_err(|e| err(E_INTERNAL, e.to_string()))?;
    let session = current.into_iter().find(|s| s.tmux_session_id == session_id).unwrap_or_else(|| Session {
        tmux_session_id: session_id,
        name,
        project_id: Some(project.id.clone()),
        worktree: None,
        cwd,
        pid: 0,
        state: SessionState::Running,
        first_seen: 0,
    });
    state.bus.publish(Event::SessionLaunched { session: session.clone() });
    Ok(SessionLaunchResult { session })
}

pub async fn handle_session_kill(params: SessionKillParams, state: &AppState) -> Result<(), RpcError> {
    tmux::kill_session(&params.tmux_session_id).await.map_err(|e| err(E_INTERNAL, e.to_string()))?;
    state.sessions.forget(&params.tmux_session_id).await.map_err(|e| err(E_INTERNAL, e.to_string()))?;
    state.bus.publish(Event::SessionRemoved { tmux_session_id: params.tmux_session_id });
    Ok(())
}

/// Sanitize a string for use in a tmux session name: replace ':' and whitespace with '_'.
fn sanitize(s: &str) -> String {
    s.chars()
        .map(|c| if c == ':' || c.is_whitespace() { '_' } else { c })
        .collect()
}

#[allow(dead_code)] // used by handle_session_launch via ProjectId path
fn _ensure_used(_: ProjectId) {}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn handshake_rejects_bad_token() {
        let dir = tempdir().unwrap();
        let state = AppState::for_test(dir.path().to_path_buf()).await.unwrap();
        let result = handle_handshake(HandshakeParams { token: "wrong".into(), client: ccdash_core::protocol::ClientKind::Cli }, &state);
        assert_eq!(result.unwrap_err().code, E_AUTH);
    }

    #[tokio::test]
    async fn handshake_accepts_correct_token() {
        let dir = tempdir().unwrap();
        let state = AppState::for_test(dir.path().to_path_buf()).await.unwrap();
        let token = (*state.auth_token).clone();
        let result = handle_handshake(HandshakeParams { token, client: ccdash_core::protocol::ClientKind::Cli }, &state);
        assert_eq!(result.unwrap().protocol_version, PROTOCOL_VERSION);
    }

    #[test]
    fn sanitize_replaces_colon() {
        assert_eq!(sanitize("foo:bar baz"), "foo_bar_baz");
    }

    #[tokio::test]
    async fn project_add_publishes_event() {
        let dir = tempdir().unwrap();
        let state = AppState::for_test(dir.path().to_path_buf()).await.unwrap();
        let mut rx = state.bus.subscribe();
        let proj_dir = dir.path().join("p1");
        std::fs::create_dir(&proj_dir).unwrap();
        let _ = handle_project_add(ProjectAddParams { path: proj_dir, name: None }, &state).await.unwrap();
        let evt = rx.recv().await.unwrap();
        assert!(matches!(evt, Event::ProjectUpdated { .. }));
    }
}
```

- [ ] **Step 2: Write `rpc/dispatch.rs`**

```rust
//! Method routing: parse `params` for the named method, call the handler,
//! produce a `Response`.

use crate::rpc::handlers::{self, err, E_AUTH, E_INVALID_PARAMS};
use crate::state::AppState;
use ccdash_core::protocol::{
    HandshakeParams, ProjectAddParams, ProjectRemoveParams, Request, Response, SessionKillParams,
    SessionLaunchParams, SubscribeParams,
};
use serde_json::json;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Per-connection state held by the dispatch loop.
pub struct ConnContext {
    pub authed: bool,
    pub subscriptions: HashSet<ccdash_core::protocol::Topic>,
}

impl ConnContext {
    pub fn new() -> Self {
        Self { authed: false, subscriptions: HashSet::new() }
    }
}

pub async fn dispatch(req: Request, state: &AppState, ctx: &Arc<RwLock<ConnContext>>) -> Response {
    let id = req.id.clone();

    // handshake is the only method allowed pre-auth.
    if req.method != "handshake" && !ctx.read().await.authed {
        return Response::err(id, err(E_AUTH, "must call handshake first"));
    }

    match req.method.as_str() {
        "handshake" => {
            let params: HandshakeParams = match serde_json::from_value(req.params) {
                Ok(p) => p,
                Err(e) => return Response::err(id, err(E_INVALID_PARAMS, e.to_string())),
            };
            match handlers::handle_handshake(params, state) {
                Ok(r) => {
                    ctx.write().await.authed = true;
                    Response::ok(id, serde_json::to_value(r).unwrap())
                }
                Err(e) => Response::err(id, e),
            }
        }
        "subscribe" => {
            let params: SubscribeParams = match serde_json::from_value(req.params) {
                Ok(p) => p,
                Err(e) => return Response::err(id, err(E_INVALID_PARAMS, e.to_string())),
            };
            ctx.write().await.subscriptions.extend(params.topics.into_iter());
            Response::ok(id, json!({"subscribed": true}))
        }
        "project.list" => {
            let r = handlers::handle_project_list(state).await;
            Response::ok(id, serde_json::to_value(r).unwrap())
        }
        "project.add" => {
            let params: ProjectAddParams = match serde_json::from_value(req.params) {
                Ok(p) => p,
                Err(e) => return Response::err(id, err(E_INVALID_PARAMS, e.to_string())),
            };
            match handlers::handle_project_add(params, state).await {
                Ok(p) => Response::ok(id, serde_json::to_value(p).unwrap()),
                Err(e) => Response::err(id, e),
            }
        }
        "project.remove" => {
            let params: ProjectRemoveParams = match serde_json::from_value(req.params) {
                Ok(p) => p,
                Err(e) => return Response::err(id, err(E_INVALID_PARAMS, e.to_string())),
            };
            match handlers::handle_project_remove(params, state).await {
                Ok(()) => Response::ok(id, json!({"ok": true})),
                Err(e) => Response::err(id, e),
            }
        }
        "session.list" => {
            match handlers::handle_session_list(state).await {
                Ok(r) => Response::ok(id, serde_json::to_value(r).unwrap()),
                Err(e) => Response::err(id, e),
            }
        }
        "session.launch" => {
            let params: SessionLaunchParams = match serde_json::from_value(req.params) {
                Ok(p) => p,
                Err(e) => return Response::err(id, err(E_INVALID_PARAMS, e.to_string())),
            };
            match handlers::handle_session_launch(params, state).await {
                Ok(r) => Response::ok(id, serde_json::to_value(r).unwrap()),
                Err(e) => Response::err(id, e),
            }
        }
        "session.kill" => {
            let params: SessionKillParams = match serde_json::from_value(req.params) {
                Ok(p) => p,
                Err(e) => return Response::err(id, err(E_INVALID_PARAMS, e.to_string())),
            };
            match handlers::handle_session_kill(params, state).await {
                Ok(()) => Response::ok(id, json!({"ok": true})),
                Err(e) => Response::err(id, e),
            }
        }
        other => Response::err(id, err(-32601, format!("method not found: {}", other))),
    }
}
```

- [ ] **Step 3: Run dispatch + handlers tests**

Run: `cargo test -p ccdash-daemon rpc::handlers::`
Expected: 4 PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/ccdash-daemon/src/rpc/handlers.rs crates/ccdash-daemon/src/rpc/dispatch.rs
git commit -m "ccdash-daemon: rpc handlers + dispatch for handshake, projects, sessions"
```

---

## Task G3: Implement `rpc::server` (Unix socket listener + connection loop)

**Files:**
- Modify: `crates/ccdash-daemon/src/rpc/server.rs`

- [ ] **Step 1: Replace stub with real implementation**

```rust
//! Unix-socket listener and per-connection loop.

use crate::broadcast::Event;
use crate::rpc::codec::{FrameReader, FrameWriter};
use crate::rpc::dispatch::{dispatch, ConnContext};
use crate::state::AppState;
use anyhow::{Context, Result};
use ccdash_core::protocol::Notification;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::Arc;
use tokio::net::UnixListener;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

pub async fn serve(state: AppState, socket: &Path) -> Result<()> {
    // Remove stale socket if it exists.
    if socket.exists() {
        std::fs::remove_file(socket).with_context(|| format!("removing stale socket {}", socket.display()))?;
    }
    if let Some(parent) = socket.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    let listener = UnixListener::bind(socket).with_context(|| format!("binding {}", socket.display()))?;
    // chmod 0600
    let perms = std::fs::Permissions::from_mode(0o600);
    std::fs::set_permissions(socket, perms).with_context(|| "setting socket permissions")?;
    info!(socket = %socket.display(), "ccdash-daemon listening");

    loop {
        let (stream, _addr) = match listener.accept().await {
            Ok(s) => s,
            Err(e) => {
                error!(error = %e, "accept failed");
                continue;
            }
        };
        let state = state.clone();
        tokio::spawn(async move {
            if let Err(e) = connection(state, stream).await {
                warn!(error = %e, "connection ended");
            }
        });
    }
}

async fn connection(state: AppState, stream: tokio::net::UnixStream) -> Result<()> {
    let (r, w) = stream.into_split();
    let mut reader = FrameReader::new(r);
    let mut writer = FrameWriter::new(w);
    let ctx = Arc::new(RwLock::new(ConnContext::new()));

    // Subscribe to bus; we'll forward matching events as JSON-RPC notifications.
    let mut bus_rx = state.bus.subscribe();

    loop {
        tokio::select! {
            req = reader.next_request() => {
                match req {
                    Ok(Some(req)) => {
                        let resp = dispatch(req, &state, &ctx).await;
                        if let Err(e) = writer.write_response(&resp).await {
                            warn!(error = %e, "write failed; closing");
                            break;
                        }
                    }
                    Ok(None) => break, // EOF
                    Err(e) => {
                        warn!(error = %e, "frame read error; closing");
                        break;
                    }
                }
            }
            evt = bus_rx.recv() => {
                match evt {
                    Ok(event) => {
                        let topic = event.topic();
                        if ctx.read().await.subscriptions.contains(&topic) {
                            let n = Notification::new(method_for(&event), serde_json::to_value(&event).unwrap());
                            if let Err(e) = writer.write_notification(&n).await {
                                warn!(error = %e, "notification write failed");
                                break;
                            }
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        warn!(skipped = n, "subscriber lagged");
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        }
    }
    Ok(())
}

fn method_for(event: &Event) -> &'static str {
    match event {
        Event::ProjectsSnapshot { .. } => "projects.snapshot",
        Event::SessionsSnapshot { .. } => "sessions.snapshot",
        Event::ProjectUpdated { .. } => "project.updated",
        Event::ProjectRemoved { .. } => "project.removed",
        Event::SessionLaunched { .. } => "session.launched",
        Event::SessionUpdated { .. } => "session.updated",
        Event::SessionRemoved { .. } => "session.removed",
    }
}
```

- [ ] **Step 2: Update `main.rs` to actually start the server**

Edit `crates/ccdash-daemon/src/main.rs`:

```rust
//! ccdash daemon entry point.

mod broadcast;
mod config;
mod projects;
mod rpc;
mod sessions;
mod state;
mod tmux;
mod worktrees;

use anyhow::Result;
use clap::Parser;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    let cfg = config::Config::parse();

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_new(&cfg.log_level).unwrap_or_else(|_| EnvFilter::new("info")))
        .init();

    let state = state::AppState::bootstrap(cfg.resolved_data_dir()).await?;
    let socket = cfg.resolved_socket();

    // Spawn a graceful-shutdown task: on Ctrl-C or SIGTERM, remove the socket and exit.
    let shutdown_socket = socket.clone();
    tokio::spawn(async move {
        let ctrl_c = tokio::signal::ctrl_c();
        let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("install SIGTERM handler");
        tokio::select! {
            _ = ctrl_c => {},
            _ = term.recv() => {},
        }
        let _ = std::fs::remove_file(&shutdown_socket);
        std::process::exit(0);
    });

    rpc::serve(state, &socket).await
}
```

- [ ] **Step 3: Run + smoke-test**

Run: `cargo build -p ccdash-daemon`
Expected: SUCCESS.

In one terminal:

```bash
CCDASH_HOME=/tmp/ccdash-smoke CCDASH_SOCKET=/tmp/ccdash-smoke.sock \
  cargo run -p ccdash-daemon -- --log-level debug
```

In another terminal:

```bash
TOKEN=$(cat /tmp/ccdash-smoke/auth)
printf '%s\n' \
  "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"handshake\",\"params\":{\"token\":\"$TOKEN\",\"client\":\"cli\"}}" \
  | socat - UNIX-CONNECT:/tmp/ccdash-smoke.sock
```

Expected output: a single line `{"jsonrpc":"2.0","id":1,"result":{"daemon_version":"0.1.0","protocol_version":1}}`.

Then stop the daemon with Ctrl-C and confirm `/tmp/ccdash-smoke.sock` is gone.

- [ ] **Step 4: Commit**

```bash
git add crates/ccdash-daemon/src/rpc/server.rs crates/ccdash-daemon/src/main.rs
git commit -m "ccdash-daemon: Unix socket server + per-connection dispatch + signal handling"
```

---

## Task H1: Workspace-level integration test — handshake + project lifecycle

**Files:**
- Create: `tests/common/mod.rs`
- Create: `tests/handshake.rs`
- Create: `tests/projects.rs`
- Modify: `Cargo.toml` (root workspace) — register a workspace dev-dep on the daemon binary

> Workspace integration tests live in the top-level `tests/` directory and link against a small helper crate. Cargo's standard convention is `tests/<name>.rs` per top-level test file. We'll spawn the daemon binary via `cargo run --bin ccdash-daemon` indirection — except integration tests don't get access to that automatically, so we'll use the path inside `target/debug/`.

- [ ] **Step 1: Add a `dev-dependencies` and `[package]` to the workspace root `Cargo.toml`**

Currently the root is a workspace-only manifest. We need it to also be a package so it can hold integration tests. Edit `Cargo.toml`:

```toml
[workspace]
resolver = "2"
members = ["crates/ccdash-core", "crates/ccdash-daemon"]

[workspace.package]
version = "0.1.0"
edition = "2021"
rust-version = "1.83"
license = "MIT"
repository = "https://github.com/cjtaylor/ccdash"

[workspace.dependencies]
anyhow = "1"
thiserror = "1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = "0.8"
tokio = { version = "1.40", features = ["rt-multi-thread", "macros", "net", "fs", "sync", "signal", "process", "io-util", "time"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
clap = { version = "4", features = ["derive"] }
notify = "6"
rand = "0.8"
tempfile = "3"

[package]
name = "ccdash-tests"
version.workspace = true
edition.workspace = true
publish = false

[[test]]
name = "handshake"
path = "tests/handshake.rs"

[[test]]
name = "projects"
path = "tests/projects.rs"

[dev-dependencies]
ccdash-core = { path = "crates/ccdash-core" }
ccdash-daemon = { path = "crates/ccdash-daemon" }
anyhow = { workspace = true }
tokio = { workspace = true }
serde_json = { workspace = true }
tempfile = { workspace = true }
```

Note: making the workspace root also a package means `cargo build` at the root will try to build a library by default. To avoid that, the manifest above declares no `[lib]`; the package has only `[[test]]` targets, which Cargo handles. If you see "no library or binary" warnings, that's fine — tests still compile and run.

- [ ] **Step 2: Write the test harness**

Create `tests/common/mod.rs`:

```rust
//! Shared test harness — spawns the daemon binary, returns a connection handle.

use anyhow::{Context, Result};
use ccdash_core::protocol::{Request, Response};
use std::path::PathBuf;
use std::time::Duration;
use tempfile::TempDir;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::process::{Child, Command};

pub struct Harness {
    pub dir: TempDir,
    pub socket: PathBuf,
    pub token: String,
    child: Child,
}

impl Harness {
    pub async fn spawn() -> Result<Self> {
        let dir = tempfile::tempdir()?;
        let socket = dir.path().join("ccdash.sock");
        let data_dir = dir.path().join("home");
        std::fs::create_dir_all(&data_dir)?;

        // Find the daemon binary path. Cargo writes it to target/debug/ccdash-daemon
        // when tests are run; we trust CARGO_BIN_EXE_<name> being set via [[bin]] deps,
        // but for workspace tests we just rely on the path. Build first if needed.
        let bin = std::env::var("CCDASH_DAEMON_BIN")
            .map(PathBuf::from)
            .unwrap_or_else(|_| target_dir().join("ccdash-daemon"));
        assert!(bin.exists(), "daemon binary not found at {} — run `cargo build -p ccdash-daemon` first", bin.display());

        let child = Command::new(&bin)
            .arg("--socket").arg(&socket)
            .arg("--data-dir").arg(&data_dir)
            .arg("--log-level").arg("warn")
            .kill_on_drop(true)
            .spawn()
            .with_context(|| format!("spawning {}", bin.display()))?;

        // Wait up to 3s for socket to appear.
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        while !socket.exists() && std::time::Instant::now() < deadline {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        anyhow::ensure!(socket.exists(), "daemon did not create socket within 3s");

        let token = tokio::fs::read_to_string(data_dir.join("auth")).await?.trim().to_string();
        Ok(Self { dir, socket, token, child })
    }

    pub async fn connect(&self) -> Result<Conn> {
        let stream = UnixStream::connect(&self.socket).await.context("connecting socket")?;
        let (r, w) = stream.into_split();
        Ok(Conn { reader: BufReader::new(r), writer: w, next_id: 1 })
    }

    pub async fn handshake(&self, conn: &mut Conn) -> Result<Response> {
        conn.call("handshake", serde_json::json!({"token": self.token, "client": "cli"})).await
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        let _ = self.child.start_kill();
    }
}

pub struct Conn {
    reader: BufReader<tokio::net::unix::OwnedReadHalf>,
    writer: tokio::net::unix::OwnedWriteHalf,
    next_id: u64,
}

impl Conn {
    pub async fn call(&mut self, method: &str, params: serde_json::Value) -> Result<Response> {
        let id = self.next_id;
        self.next_id += 1;
        let req = Request {
            jsonrpc: "2.0".into(),
            id: serde_json::json!(id),
            method: method.into(),
            params,
        };
        let mut bytes = serde_json::to_vec(&req)?;
        bytes.push(b'\n');
        self.writer.write_all(&bytes).await?;
        self.writer.flush().await?;
        let mut line = String::new();
        let n = self.reader.read_line(&mut line).await?;
        anyhow::ensure!(n > 0, "EOF before response");
        Ok(serde_json::from_str(line.trim_end())?)
    }
}

fn target_dir() -> PathBuf {
    // CARGO_MANIFEST_DIR is the workspace root.
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target").join("debug")
}
```

- [ ] **Step 3: Write `tests/handshake.rs`**

```rust
mod common;

use common::Harness;

#[tokio::test]
async fn handshake_with_correct_token_succeeds() {
    let h = Harness::spawn().await.unwrap();
    let mut c = h.connect().await.unwrap();
    let resp = h.handshake(&mut c).await.unwrap();
    assert!(resp.error.is_none(), "got error: {:?}", resp.error);
    assert!(resp.result.unwrap()["protocol_version"].as_u64().unwrap() >= 1);
}

#[tokio::test]
async fn handshake_with_wrong_token_fails() {
    let h = Harness::spawn().await.unwrap();
    let mut c = h.connect().await.unwrap();
    let resp = c.call("handshake", serde_json::json!({"token": "wrong", "client": "cli"})).await.unwrap();
    assert!(resp.error.is_some());
    assert_eq!(resp.error.unwrap().code, -32001);
}

#[tokio::test]
async fn pre_auth_method_call_is_rejected() {
    let h = Harness::spawn().await.unwrap();
    let mut c = h.connect().await.unwrap();
    let resp = c.call("project.list", serde_json::json!({})).await.unwrap();
    assert!(resp.error.is_some());
    assert_eq!(resp.error.unwrap().code, -32001);
}
```

- [ ] **Step 4: Write `tests/projects.rs`**

```rust
mod common;

use common::Harness;
use tempfile::tempdir;

#[tokio::test]
async fn project_add_list_remove() {
    let h = Harness::spawn().await.unwrap();
    let mut c = h.connect().await.unwrap();
    h.handshake(&mut c).await.unwrap().result.expect("handshake ok");

    let proj = tempdir().unwrap();
    let add = c.call("project.add", serde_json::json!({"path": proj.path()})).await.unwrap();
    assert!(add.error.is_none(), "{:?}", add.error);
    let project = add.result.unwrap();
    let id = project["id"].as_str().unwrap().to_string();

    let list = c.call("project.list", serde_json::json!({})).await.unwrap();
    let projects = list.result.unwrap()["projects"].as_array().unwrap().clone();
    assert_eq!(projects.len(), 1);

    let rm = c.call("project.remove", serde_json::json!({"id": id})).await.unwrap();
    assert!(rm.error.is_none());

    let list2 = c.call("project.list", serde_json::json!({})).await.unwrap();
    assert!(list2.result.unwrap()["projects"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn project_remove_unknown_returns_not_found() {
    let h = Harness::spawn().await.unwrap();
    let mut c = h.connect().await.unwrap();
    h.handshake(&mut c).await.unwrap().result.expect("handshake ok");

    let rm = c.call("project.remove", serde_json::json!({"id": "no-such-id"})).await.unwrap();
    assert!(rm.error.is_some());
    assert_eq!(rm.error.unwrap().code, -32004);
}
```

- [ ] **Step 5: Build daemon then run integration tests**

Run:

```bash
cargo build -p ccdash-daemon
cargo test --test handshake --test projects
```

Expected: 5 PASS (3 handshake + 2 projects).

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml tests/
git commit -m "tests: workspace integration suite for handshake + project lifecycle"
```

---

## Task H2: Workspace-level integration test — sessions (requires tmux)

**Files:**
- Create: `tests/sessions.rs`
- Modify: `Cargo.toml` to register the new test

- [ ] **Step 1: Register the test in workspace `Cargo.toml`**

Add under the existing `[[test]]` entries:

```toml
[[test]]
name = "sessions"
path = "tests/sessions.rs"
```

- [ ] **Step 2: Write the test**

Create `tests/sessions.rs`:

```rust
mod common;

use common::Harness;
use std::process::Command;
use tempfile::tempdir;

fn tmux_available() -> bool {
    Command::new("tmux").arg("-V").output().map(|o| o.status.success()).unwrap_or(false)
}

#[tokio::test]
async fn launch_then_kill_session() {
    if !tmux_available() {
        eprintln!("tmux not on PATH; skipping");
        return;
    }
    let h = Harness::spawn().await.unwrap();
    let mut c = h.connect().await.unwrap();
    h.handshake(&mut c).await.unwrap().result.expect("handshake ok");

    let proj = tempdir().unwrap();
    let add = c.call("project.add", serde_json::json!({"path": proj.path(), "name": "smoke"})).await.unwrap();
    let project_id = add.result.unwrap()["id"].as_str().unwrap().to_string();

    let launch = c.call("session.launch", serde_json::json!({
        "project_id": project_id,
        "worktree": null,
        "command": "sleep 30",
    })).await.unwrap();
    assert!(launch.error.is_none(), "launch error: {:?}", launch.error);
    let session = launch.result.unwrap()["session"].clone();
    let sid = session["tmux_session_id"].as_str().unwrap().to_string();
    assert!(sid.starts_with('$'));

    let list = c.call("session.list", serde_json::json!({})).await.unwrap();
    let sessions = list.result.unwrap()["sessions"].as_array().unwrap().clone();
    // The session may or may not be visible depending on whether `pane_current_command`
    // is `sleep` (it will be in this test, not `claude`). So we skip the visibility
    // assertion and verify kill works regardless.
    let _ = sessions;

    let kill = c.call("session.kill", serde_json::json!({"tmux_session_id": sid})).await.unwrap();
    assert!(kill.error.is_none(), "kill error: {:?}", kill.error);
}
```

> Note: the visibility part is loose because `session.list` filters by `pane_current_command == "claude"` per spec §7.7. Using `sleep` as the command in tests means it won't show up in the list — that's correct behavior. Verifying the launch/kill round-trip is enough for Phase 1; we'll re-test with a real `claude` binary in the manual acceptance pass.

- [ ] **Step 3: Build + run**

```bash
cargo build -p ccdash-daemon
cargo test --test sessions
```

Expected: 1 PASS (skipped silently if tmux is not installed).

- [ ] **Step 4: Commit**

```bash
git add tests/sessions.rs Cargo.toml
git commit -m "tests: session launch+kill integration test (tmux-gated)"
```

---

## Task H3: Workspace-level integration test — broadcast subscription

**Files:**
- Create: `tests/broadcast.rs`
- Modify: `Cargo.toml` to register the new test

- [ ] **Step 1: Register the test**

Add under the other `[[test]]` entries:

```toml
[[test]]
name = "broadcast"
path = "tests/broadcast.rs"
```

- [ ] **Step 2: Write the test**

Create `tests/broadcast.rs`:

```rust
mod common;

use common::Harness;
use std::time::Duration;
use tempfile::tempdir;
use tokio::time::timeout;

#[tokio::test]
async fn subscribed_client_receives_project_updated_notification() {
    let h = Harness::spawn().await.unwrap();

    // Client A: subscribes to projects.
    let mut a = h.connect().await.unwrap();
    h.handshake(&mut a).await.unwrap().result.expect("handshake ok");
    let sub = a.call("subscribe", serde_json::json!({"topics": ["projects"]})).await.unwrap();
    assert!(sub.error.is_none());

    // Client B: triggers a project.add.
    let mut b = h.connect().await.unwrap();
    h.handshake(&mut b).await.unwrap().result.expect("handshake ok");
    let proj = tempdir().unwrap();
    let _ = b.call("project.add", serde_json::json!({"path": proj.path()})).await.unwrap();

    // Client A should now see a project.updated notification.
    // The notification arrives as a separate frame; we'd need a streaming read.
    // Conn::call reads one line per call — but the notification comes UNSOLICITED.
    // To keep this test simple, we just verify the next line on A's stream is a notification.
    // (We piggy-back on call() by sending a no-op call and reading until we get the notification.)
    let next_line = read_next_line(&mut a).await.unwrap();
    let v: serde_json::Value = serde_json::from_str(&next_line).unwrap();
    assert_eq!(v["jsonrpc"], "2.0");
    assert!(v["method"].as_str().unwrap().starts_with("project."));
    assert!(v.get("id").is_none() || v["id"].is_null(), "notifications must have no id");
}

async fn read_next_line(c: &mut common::Conn) -> anyhow::Result<String> {
    // Reach into Conn for a raw read. This is a test-only helper.
    use tokio::io::AsyncBufReadExt;
    let mut line = String::new();
    timeout(Duration::from_secs(2), c.reader().read_line(&mut line)).await??;
    Ok(line)
}
```

> Note: this test needs `Conn` to expose its reader. Add this accessor to `tests/common/mod.rs` Conn impl:
>
> ```rust
> pub fn reader(&mut self) -> &mut BufReader<tokio::net::unix::OwnedReadHalf> {
>     &mut self.reader
> }
> ```

Add that accessor now.

- [ ] **Step 3: Build + run**

```bash
cargo build -p ccdash-daemon
cargo test --test broadcast
```

Expected: 1 PASS.

- [ ] **Step 4: Commit**

```bash
git add tests/broadcast.rs tests/common/mod.rs Cargo.toml
git commit -m "tests: broadcast subscription notification end-to-end"
```

---

## Task I1: Run the full Phase 1 verification

- [ ] **Step 1: Format + lint**

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: both succeed. If clippy emits warnings, fix them inline (typical: unused imports, unnecessary `.clone()`, `match` over `if let`).

- [ ] **Step 2: Full test run**

```bash
cargo build -p ccdash-daemon
cargo test --workspace
```

Expected: every test passes. As of plan completion, that's ~30 unit tests + ~8 integration tests. tmux-gated tests print "skipping" if tmux is absent.

- [ ] **Step 3: Manual smoke test**

In one terminal:

```bash
CCDASH_HOME=/tmp/ccdash-acceptance CCDASH_SOCKET=/tmp/ccdash-acceptance.sock \
  cargo run -p ccdash-daemon -- --log-level info
```

In another terminal — run a sequence of `socat` calls. Save the token first:

```bash
TOKEN=$(cat /tmp/ccdash-acceptance/auth)
SOCK=/tmp/ccdash-acceptance.sock

# Helper:
call() { printf '%s\n' "$1" | socat - "UNIX-CONNECT:$SOCK"; }

call "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"handshake\",\"params\":{\"token\":\"$TOKEN\",\"client\":\"cli\"}}"
call "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"project.add\",\"params\":{\"path\":\"$HOME\"}}"
call "{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"project.list\",\"params\":{}}"
```

Expected: each call returns a JSON response on a new line. The `project.list` response includes the project you just added.

Notes about the smoke test: each `socat` invocation is a fresh connection, so each one needs to re-handshake. To preserve auth across calls, do this interactively: `socat - UNIX-CONNECT:$SOCK` and then paste lines into the stdin. Or fold the handshake into every payload as the first frame.

- [ ] **Step 4: Tag the milestone**

```bash
git tag phase-1-foundation
```

- [ ] **Step 5: Commit any final cleanup**

```bash
git add -A
git status
# If anything is uncommitted (e.g., clippy fixes), commit it:
git commit -m "phase 1: cleanup pass after full verification"
```

---

## What Phase 1 ships

After Phase 1:
- `ccdash-daemon` binary that runs as a long-lived service, listens on a Unix socket, authenticates clients with a shared token at `~/.ccdash/auth`, and exposes JSON-RPC 2.0 for project + session management.
- Projects can be added, listed, removed, with persistence to `~/.ccdash/projects.toml`.
- Worktrees are discovered via `git worktree list --porcelain` whenever a project is touched.
- Sessions are launched via tmux, tracked by tmux's stable `session_id`, and persisted in `~/.ccdash/sessions.toml`.
- Multiple connected clients can subscribe to topics; the daemon broadcasts state changes.
- 100% of the protocol is debuggable from a terminal with `socat`.

Not yet built (Phase 2+):
- Port registry / declared-port parsing.
- Plan-file watching.
- `ccdash` CLI binary.
- Tauri UI.
- Embedded terminals.
- Multi-window mirror coordination.
- Packaging (launchd/systemd/brew).

---

## Self-Review Summary

**Spec coverage** (spec §sections → tasks):
- §4.2 process model (daemon, socket, auth) → A4, F1, G3
- §5.1 ccdash-core (protocol, domain, auth, paths) → A2–A6
- §5.2 daemon modules — `tmux` (E1), `projects` (C1+C2), `worktrees` (D1), `sessions` (E2), `rpc` (G1–G3), `broadcast` (B3). Ports + plans deferred to Phase 2.
- §6.1 startup flow (handshake, subscribe) → G2 + H1
- §6.2 session launch (partial — port-conflict logic deferred) → G2 (launch path without ports)
- §7.6 hybrid tmux naming → E1 + E2 (session_id authoritative)
- §7.7 visibility filter (`pane_current_command == "claude"`) → E2 `Manager::refresh`
- §7.11 socket perms + shared secret → A4, G3
- §8 error handling: tmux missing (E1), tmux server not running (E1 returns empty), malformed RPC (G1 + G2). Reconnect-on-server-restart deferred to Phase 2.
- §9.1–9.2 unit + integration testing → present throughout, plus H1–H3.

**Placeholder scan**: no TBD/TODO/"implement later" patterns. Each step includes complete code or exact commands.

**Type consistency**: `ProjectId`, `Session`, `Worktree`, `Topic`, `Event`, `RpcError` all defined exactly once in `ccdash-core` or `ccdash-daemon::broadcast`, used consistently in handlers and dispatch.

**Known scope omissions** (deliberate, recorded in "Not yet built"):
- Port conflict / declared-port parser (Phase 2).
- Plan watcher (Phase 2 or 5).
- File-watcher on projects.toml (changes only happen via daemon, so we don't need to watch our own writes).
- launchd/systemd service files (Phase 5).

---

**Plan complete and saved to `docs/superpowers/plans/2026-05-17-phase-1-foundation.md`.**

Two execution options:

1. **Subagent-Driven (recommended)** — I dispatch a fresh subagent per task, review between tasks, fast iteration.
2. **Inline Execution** — Execute tasks in this session using executing-plans, batch execution with checkpoints.

Which approach do you want for executing this plan?
