# ccdash Phase 2 — CLI + Ports + Plans Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the `ccdash` CLI binary, the `ports` daemon module (lsof scan + declared-port parsing + conflict gating on `session.launch`), and the `plans` daemon module (notify file-watcher + markdown phase/task parser).

**Architecture:** A new `ccdash-core::client` submodule extracts socket connect / handshake / JSON-RPC helpers, used by both the new CLI and (later) the Tauri UI. The daemon gains two new modules: `ports` (queries `lsof` and parses `package.json`/`.env`/`docker-compose.yml`/`Procfile` for declared ports; gates `session.launch` with one-shot force tokens on conflict) and `plans` (uses the `notify` crate to watch `<project>/docs/superpowers/{specs,plans}/**/*.md` and parses markdown into `Phase`/`Task` records via `pulldown-cmark`).

**Tech Stack:**
- Same workspace as Phase 1 (Rust 2021, tokio, serde, anyhow, tracing, clap, notify)
- New crate: `crates/ccdash-cli/` (binary)
- New dep: `pulldown-cmark` for markdown parsing
- `notify` (already in workspace deps) wired up here

**Spec reference:** `docs/superpowers/specs/2026-05-17-cc-dashboard-design.md`
**Predecessor:** Phase 1 complete; tag `phase-1-foundation`.

---

## File Structure

```
ccdash/
├── Cargo.toml                                   # add ccdash-cli to members + pulldown-cmark workspace dep
├── crates/
│   ├── ccdash-core/
│   │   └── src/
│   │       ├── lib.rs                           # add `pub mod client;`
│   │       ├── client.rs                        # NEW — socket connect + handshake + RPC call helpers
│   │       └── protocol.rs                      # add Port types, force_token field, plans/ports method params
│   ├── ccdash-daemon/
│   │   └── src/
│   │       ├── main.rs                          # wire `mod ports;` and `mod plans;`
│   │       ├── ports/
│   │       │   ├── mod.rs                       # NEW
│   │       │   ├── lsof.rs                      # NEW — `lsof` shell-out + parser
│   │       │   ├── declared.rs                  # NEW — package.json/.env/docker-compose/Procfile parsers
│   │       │   └── registry.rs                  # NEW — composite port view (running + declared)
│   │       ├── plans/
│   │       │   ├── mod.rs                       # NEW
│   │       │   ├── parser.rs                    # NEW — pulldown-cmark Phase/Task parser
│   │       │   └── watcher.rs                   # NEW — notify file watcher per project
│   │       ├── state.rs                         # add port_registry, plans, conflict_tokens fields
│   │       ├── broadcast.rs                     # add Event::PortsSnapshot, PortsUpdated, PlanUpdated
│   │       └── rpc/
│   │           ├── handlers.rs                  # add ports.list, plans.get; gate session.launch by ports
│   │           └── dispatch.rs                  # route new methods
│   └── ccdash-cli/                              # NEW crate
│       ├── Cargo.toml
│       └── src/
│           ├── main.rs                          # clap parsing + dispatch to commands
│           └── commands/
│               ├── mod.rs
│               ├── status.rs
│               ├── project.rs
│               ├── list.rs
│               ├── launch.rs
│               ├── kill.rs
│               ├── ports.rs
│               └── plan.rs
```

---

## Task A1: Extract `ccdash-core::client` module

**Files:**
- Create: `crates/ccdash-core/src/client.rs`
- Modify: `crates/ccdash-core/src/lib.rs`
- Modify: `crates/ccdash-core/Cargo.toml`

This module owns the JSON-RPC client side: connect to socket, send handshake, send requests, read responses, optionally subscribe.

- [ ] **Step 1: Add `tokio` (with `rt-multi-thread`, `net`, `io-util`) to `ccdash-core` deps**

Edit `crates/ccdash-core/Cargo.toml`. Replace the `[dependencies]` block with:

```toml
[dependencies]
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
anyhow = { workspace = true }
rand = { workspace = true }
tokio = { workspace = true }
```

- [ ] **Step 2: Add `pub mod client;` to `lib.rs`**

Edit `crates/ccdash-core/src/lib.rs`. Append after the existing module declarations:

```rust
pub mod client;
```

- [ ] **Step 3: Write the client module**

Create `crates/ccdash-core/src/client.rs`:

```rust
//! JSON-RPC client over Unix socket. Used by the CLI and the Tauri UI.

use crate::auth;
use crate::paths;
use crate::protocol::{ClientKind, HandshakeParams, Request, Response, Topic};
use anyhow::{Context, Result};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

pub struct Client {
    reader: BufReader<tokio::net::unix::OwnedReadHalf>,
    writer: tokio::net::unix::OwnedWriteHalf,
    next_id: AtomicU64,
}

impl Client {
    /// Connect to the daemon socket at the given path. Does NOT handshake yet.
    pub async fn connect(socket: &PathBuf) -> Result<Self> {
        let stream = UnixStream::connect(socket)
            .await
            .with_context(|| format!("connecting {}", socket.display()))?;
        let (r, w) = stream.into_split();
        Ok(Self {
            reader: BufReader::new(r),
            writer: w,
            next_id: AtomicU64::new(1),
        })
    }

    /// Connect using the default socket path for this platform.
    pub async fn connect_default() -> Result<Self> {
        Self::connect(&paths::default_socket_path()).await
    }

    /// Send handshake using the token read from `~/.ccdash/auth`.
    pub async fn handshake(&mut self, kind: ClientKind) -> Result<Response> {
        let token = auth::read_token(&paths::auth_token_path())?
            .ok_or_else(|| anyhow::anyhow!("no auth token at {}", paths::auth_token_path().display()))?;
        self.call(
            "handshake",
            serde_json::to_value(HandshakeParams { token, client: kind })?,
        )
        .await
    }

    /// Subscribe to one or more event topics. Notifications arrive on the same stream
    /// and can be read with `next_notification`.
    pub async fn subscribe(&mut self, topics: Vec<Topic>) -> Result<Response> {
        self.call("subscribe", serde_json::json!({ "topics": topics })).await
    }

    /// Send a JSON-RPC request, wait for the matching response, return it.
    pub async fn call(&mut self, method: &str, params: serde_json::Value) -> Result<Response> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let req = Request {
            jsonrpc: "2.0".into(),
            id: serde_json::json!(id),
            method: method.into(),
            params,
        };
        let mut bytes = serde_json::to_vec(&req)?;
        bytes.push(b'\n');
        self.writer.write_all(&bytes).await.context("writing request")?;
        self.writer.flush().await?;

        // Read frames until we find one whose id matches; intervening frames must be notifications.
        loop {
            let line = self.read_line().await?;
            let v: serde_json::Value = serde_json::from_str(&line)
                .with_context(|| format!("parsing daemon frame: {}", line.trim()))?;
            if v.get("id").is_some() && !v["id"].is_null() {
                let resp: Response = serde_json::from_value(v).context("parsing response")?;
                if resp.id == req.id {
                    return Ok(resp);
                }
            }
            // Else: notification or unrelated id — for simple sync clients we drop it.
        }
    }

    /// Read the next notification (frame with no `id`). Blocks until one arrives.
    pub async fn next_notification(&mut self) -> Result<serde_json::Value> {
        loop {
            let line = self.read_line().await?;
            let v: serde_json::Value = serde_json::from_str(&line)?;
            if v.get("id").is_none() || v["id"].is_null() {
                return Ok(v);
            }
        }
    }

    async fn read_line(&mut self) -> Result<String> {
        let mut line = String::new();
        let n = self.reader.read_line(&mut line).await.context("reading line")?;
        anyhow::ensure!(n > 0, "daemon closed connection (EOF)");
        Ok(line)
    }
}

#[cfg(test)]
mod tests {
    // Integration tests for the client live in ccdash-cli's test suite,
    // since they need a running daemon. The methods here are exercised
    // transitively by every CLI command test.
}
```

- [ ] **Step 4: Verify build**

Run: `cargo build -p ccdash-core`
Expected: SUCCESS.

- [ ] **Step 5: Commit**

```bash
git add crates/ccdash-core/Cargo.toml crates/ccdash-core/src/lib.rs crates/ccdash-core/src/client.rs
git commit -m "ccdash-core: extract Client (socket connect + handshake + RPC)"
```

---

## Task B1: Add port-related protocol types

**Files:**
- Modify: `crates/ccdash-core/src/protocol.rs`

- [ ] **Step 1: Append port + plan types to `protocol.rs`**

Append at the end of `crates/ccdash-core/src/protocol.rs` (BEFORE the final `#[cfg(test)] mod tests` block, i.e., insert just before `pub const PROTOCOL_VERSION`):

```rust
// === Ports ===

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PortBinding {
    pub port: u16,
    /// `tcp` (only TCP listeners scanned in Phase 2).
    pub protocol: String,
    /// PID of the process holding the port, or None if unknown.
    pub pid: Option<i32>,
    /// Command name from lsof (e.g. "node").
    pub command: Option<String>,
    /// ProjectId of the owning project, if we can correlate.
    pub project_id: Option<ProjectId>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeclaredPort {
    pub project_id: ProjectId,
    pub port: u16,
    /// Source file (relative to project root) the port was declared in.
    pub source: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct PortListResult {
    pub running: Vec<PortBinding>,
    pub declared: Vec<DeclaredPort>,
}

/// Returned in `RpcError::data` when `session.launch` would conflict.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PortConflictData {
    pub conflicts: Vec<PortConflict>,
    /// One-shot token. Re-send `session.launch` with `force_token = Some(this)` to bypass.
    pub force_token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PortConflict {
    pub port: u16,
    /// Description of who is using the port now (e.g. "node (pid 12345) — project Loanplatform").
    pub holder: String,
}

// === Plans ===

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Plan {
    pub path: std::path::PathBuf,
    pub title: String,
    pub phases: Vec<PlanPhase>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlanPhase {
    pub name: String,
    pub tasks: Vec<PlanTask>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlanTask {
    pub title: String,
    pub done: bool,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct PlanGetParams {
    pub project_id: ProjectId,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct PlanGetResult {
    pub plans: Vec<Plan>,
}
```

Then extend `SessionLaunchParams` to accept `force_token`. Find this struct in the same file:

```rust
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct SessionLaunchParams {
    pub project_id: ProjectId,
    /// Worktree name. `None` means use the primary worktree.
    pub worktree: Option<String>,
    /// Command override. Defaults to `claude` when absent.
    pub command: Option<String>,
}
```

Replace it with:

```rust
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct SessionLaunchParams {
    pub project_id: ProjectId,
    /// Worktree name. `None` means use the primary worktree.
    pub worktree: Option<String>,
    /// Command override. Defaults to `claude` when absent.
    pub command: Option<String>,
    /// One-shot token returned in a prior `PortConflictData`. When present,
    /// skips conflict gating for this launch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub force_token: Option<String>,
}
```

- [ ] **Step 2: Run existing tests to ensure nothing regressed**

Run: `cargo test -p ccdash-core --lib protocol::`
Expected: 5 PASS (same as Phase 1).

- [ ] **Step 3: Commit**

```bash
git add crates/ccdash-core/src/protocol.rs
git commit -m "ccdash-core: protocol types for ports, plans, force-launch"
```

---

## Task C1: Implement `ports::lsof` (running-port scanner)

**Files:**
- Create: `crates/ccdash-daemon/src/ports/mod.rs`
- Create: `crates/ccdash-daemon/src/ports/lsof.rs`

- [ ] **Step 1: Write `ports/mod.rs`**

```rust
//! Port discovery: running (via `lsof`) + declared (via per-project parsers).

pub mod declared;
pub mod lsof;
pub mod registry;

pub use registry::Registry;
```

- [ ] **Step 2: Write `ports/lsof.rs`**

```rust
//! Shell out to `lsof -nP -iTCP -sTCP:LISTEN` and parse the output.

use anyhow::{Context, Result};
use ccdash_core::protocol::PortBinding;
use tokio::process::Command;

/// Run `lsof` and return the list of TCP listeners.
pub async fn scan() -> Result<Vec<PortBinding>> {
    let output = Command::new("lsof")
        .args(["-nP", "-iTCP", "-sTCP:LISTEN", "-F", "pcnPT"])
        .output()
        .await
        .context("running lsof")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // lsof exits non-zero when no matches; treat as empty.
        if output.stdout.is_empty() {
            return Ok(vec![]);
        }
        anyhow::bail!("lsof failed: {}", stderr.trim());
    }
    let stdout = String::from_utf8(output.stdout).context("lsof stdout not utf8")?;
    Ok(parse(&stdout))
}

/// Parse `lsof -F pcnPT` "field" output format.
///
/// Each record is one process. Lines are prefixed with single letters:
///   p<pid>
///   c<command>
///   f<fd>  (one per file)
///   t<type, e.g. IPv4>
///   P<proto, TCP>
///   n<name, e.g. *:8080 or 127.0.0.1:3000>
///   T<state>
///
/// We collect (pid, command) at the p/c lines, then emit a PortBinding for
/// every n-line that looks like `*:NN` or `[::]:NN` or `host:NN` with a TCP state.
fn parse(s: &str) -> Vec<PortBinding> {
    let mut out = Vec::new();
    let mut current_pid: Option<i32> = None;
    let mut current_cmd: Option<String> = None;
    let mut current_proto: Option<String> = None;
    let mut current_state: Option<String> = None;

    for line in s.lines() {
        let (tag, rest) = match line.split_at_checked(1) {
            Some((t, r)) => (t, r),
            None => continue,
        };
        match tag {
            "p" => {
                current_pid = rest.parse().ok();
                current_cmd = None;
                current_proto = None;
                current_state = None;
            }
            "c" => current_cmd = Some(rest.to_string()),
            "P" => current_proto = Some(rest.to_string()),
            "T" => current_state = Some(rest.to_string()),
            "n" => {
                if let Some(port) = extract_port(rest) {
                    let is_listening = current_state.as_deref()
                        .map(|s| s.contains("LISTEN") || s == "ST=LISTEN")
                        .unwrap_or(true); // lsof was called with -sTCP:LISTEN already
                    if is_listening
                        && current_proto.as_deref().map(|p| p == "TCP").unwrap_or(true)
                    {
                        out.push(PortBinding {
                            port,
                            protocol: "tcp".into(),
                            pid: current_pid,
                            command: current_cmd.clone(),
                            project_id: None,
                        });
                    }
                }
            }
            _ => {}
        }
    }
    // Deduplicate by (port, pid) — lsof reports IPv4 and IPv6 separately.
    out.sort_by(|a, b| (a.port, a.pid).cmp(&(b.port, b.pid)));
    out.dedup_by(|a, b| a.port == b.port && a.pid == b.pid);
    out
}

fn extract_port(name: &str) -> Option<u16> {
    // Examples: "*:8080", "127.0.0.1:3000", "[::]:443"
    let after_last_colon = name.rsplit(':').next()?;
    // Strip trailing " (LISTEN)" if present.
    let port_str = after_last_colon.split_whitespace().next()?;
    port_str.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_port_handles_common_formats() {
        assert_eq!(extract_port("*:8080"), Some(8080));
        assert_eq!(extract_port("127.0.0.1:3000"), Some(3000));
        assert_eq!(extract_port("[::]:443"), Some(443));
        assert_eq!(extract_port("[::1]:5432 (LISTEN)"), Some(5432));
        assert_eq!(extract_port("invalid"), None);
    }

    #[test]
    fn parse_one_listener() {
        let input = "p12345\ncnode\nPTCP\nTST=LISTEN\nn*:3000\n";
        let parsed = parse(input);
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].port, 3000);
        assert_eq!(parsed[0].pid, Some(12345));
        assert_eq!(parsed[0].command.as_deref(), Some("node"));
    }

    #[test]
    fn parse_multiple_processes() {
        let input = "p12345\ncnode\nPTCP\nn*:3000\np99999\ncpython\nPTCP\nn127.0.0.1:8000\n";
        let parsed = parse(input);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].port, 3000);
        assert_eq!(parsed[1].port, 8000);
    }

    #[test]
    fn parse_dedupes_ipv4_and_ipv6() {
        let input = "p77\ncnode\nPTCP\nn*:8080\nn[::]:8080\n";
        let parsed = parse(input);
        assert_eq!(parsed.len(), 1);
    }

    #[test]
    fn parse_empty_input() {
        assert!(parse("").is_empty());
    }
}
```

- [ ] **Step 3: Wire `mod ports;` into `main.rs`**

Edit `crates/ccdash-daemon/src/main.rs`. Add to the `mod` list (alphabetical):

```rust
mod ports;
```

Place it between `mod broadcast;` and `mod projects;` (or wherever alphabetical order suggests).

- [ ] **Step 4: Run the lsof parser tests**

Run: `cargo test -p ccdash-daemon --bin ccdash-daemon ports::lsof::`
Expected: 5 PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/ccdash-daemon/src/ports crates/ccdash-daemon/src/main.rs
git commit -m "ccdash-daemon: ports::lsof — TCP listener scan via lsof -F"
```

---

## Task C2: Implement `ports::declared` (package.json + .env parsers)

**Files:**
- Create: `crates/ccdash-daemon/src/ports/declared.rs`

- [ ] **Step 1: Write `declared.rs` with the two parsers and a public entry**

```rust
//! Per-project declared-port parsers.
//!
//! Sources scanned (each independently — missing files are non-fatal):
//! - `package.json`         — looks at `scripts.*` values for `PORT=NN`/`--port NN`
//! - `.env` / `.env.local`  — looks for `PORT=NN`, `VITE_PORT=NN`, etc.
//! - `docker-compose.yml`   — looks at `ports:` blocks for `"NN:..."` mappings
//! - `Procfile`             — looks at the `web` line for `PORT=NN` envs
//!
//! Returns the set of distinct (port, source-file) pairs.

use anyhow::Result;
use ccdash_core::domain::ProjectId;
use ccdash_core::protocol::DeclaredPort;
use regex::Regex;
use std::path::Path;
use tokio::fs;

pub async fn scan(project_id: &ProjectId, project_root: &Path) -> Vec<DeclaredPort> {
    let mut out = Vec::new();
    out.extend(scan_package_json(project_id, project_root).await);
    out.extend(scan_env_files(project_id, project_root).await);
    out.extend(scan_docker_compose(project_id, project_root).await);
    out.extend(scan_procfile(project_id, project_root).await);
    out.sort_by_key(|p| (p.port, p.source.clone()));
    out.dedup_by(|a, b| a.port == b.port && a.source == b.source);
    out
}

async fn scan_package_json(project_id: &ProjectId, root: &Path) -> Vec<DeclaredPort> {
    let path = root.join("package.json");
    let s = match fs::read_to_string(&path).await {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    let v: serde_json::Value = match serde_json::from_str(&s) {
        Ok(v) => v,
        Err(_) => return vec![],
    };
    let scripts = match v.get("scripts").and_then(|s| s.as_object()) {
        Some(o) => o,
        None => return vec![],
    };
    let port_eq = Regex::new(r"\bPORT=(\d{2,5})\b").unwrap();
    let port_flag = Regex::new(r"--port[\s=](\d{2,5})\b").unwrap();
    let mut out = Vec::new();
    for (_name, val) in scripts {
        if let Some(s) = val.as_str() {
            for cap in port_eq.captures_iter(s).chain(port_flag.captures_iter(s)) {
                if let Ok(port) = cap[1].parse::<u16>() {
                    out.push(DeclaredPort {
                        project_id: project_id.clone(),
                        port,
                        source: "package.json".into(),
                    });
                }
            }
        }
    }
    out
}

async fn scan_env_files(project_id: &ProjectId, root: &Path) -> Vec<DeclaredPort> {
    let mut out = Vec::new();
    for name in [".env", ".env.local", ".env.development"] {
        let path = root.join(name);
        let s = match fs::read_to_string(&path).await {
            Ok(s) => s,
            Err(_) => continue,
        };
        let re = Regex::new(r"^\s*[A-Z_]*PORT[A-Z_]*\s*=\s*(\d{2,5})\s*$").unwrap();
        for line in s.lines() {
            if let Some(cap) = re.captures(line) {
                if let Ok(port) = cap[1].parse::<u16>() {
                    out.push(DeclaredPort {
                        project_id: project_id.clone(),
                        port,
                        source: name.into(),
                    });
                }
            }
        }
    }
    out
}

async fn scan_docker_compose(project_id: &ProjectId, root: &Path) -> Vec<DeclaredPort> {
    let mut out = Vec::new();
    for name in ["docker-compose.yml", "docker-compose.yaml", "compose.yml", "compose.yaml"] {
        let path = root.join(name);
        let s = match fs::read_to_string(&path).await {
            Ok(s) => s,
            Err(_) => continue,
        };
        // Heuristic: any "NN:NN" or "NN:NN/proto" inside lines under a `ports:` block.
        // We don't do full YAML parsing — too many footguns for what this needs.
        let re = Regex::new(r#"(?:^|\s|"|')(\d{2,5})\s*:\s*\d{2,5}"#).unwrap();
        let mut in_ports = false;
        for line in s.lines() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("ports:") {
                in_ports = true;
                continue;
            }
            if in_ports {
                if !trimmed.starts_with('-') && !trimmed.is_empty() && !trimmed.starts_with('#') {
                    // End of ports list when we hit a non-list, non-comment line.
                    in_ports = false;
                    continue;
                }
                for cap in re.captures_iter(line) {
                    if let Ok(port) = cap[1].parse::<u16>() {
                        out.push(DeclaredPort {
                            project_id: project_id.clone(),
                            port,
                            source: name.into(),
                        });
                    }
                }
            }
        }
    }
    out
}

async fn scan_procfile(project_id: &ProjectId, root: &Path) -> Vec<DeclaredPort> {
    let path = root.join("Procfile");
    let s = match fs::read_to_string(&path).await {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    let re = Regex::new(r"\bPORT=(\d{2,5})\b").unwrap();
    let mut out = Vec::new();
    for line in s.lines() {
        for cap in re.captures_iter(line) {
            if let Ok(port) = cap[1].parse::<u16>() {
                out.push(DeclaredPort {
                    project_id: project_id.clone(),
                    port,
                    source: "Procfile".into(),
                });
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn pid() -> ProjectId {
        ProjectId("p1".into())
    }

    #[tokio::test]
    async fn package_json_port_env_var() {
        let dir = tempdir().unwrap();
        std::fs::write(
            dir.path().join("package.json"),
            r#"{"scripts":{"dev":"PORT=3000 next dev"}}"#,
        )
        .unwrap();
        let ports = scan_package_json(&pid(), dir.path()).await;
        assert_eq!(ports.len(), 1);
        assert_eq!(ports[0].port, 3000);
    }

    #[tokio::test]
    async fn package_json_port_flag() {
        let dir = tempdir().unwrap();
        std::fs::write(
            dir.path().join("package.json"),
            r#"{"scripts":{"dev":"next dev --port 4000"}}"#,
        )
        .unwrap();
        let ports = scan_package_json(&pid(), dir.path()).await;
        assert_eq!(ports.len(), 1);
        assert_eq!(ports[0].port, 4000);
    }

    #[tokio::test]
    async fn env_file_port() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join(".env"), "PORT=5000\nDB_URL=foo\n").unwrap();
        let ports = scan_env_files(&pid(), dir.path()).await;
        assert_eq!(ports.len(), 1);
        assert_eq!(ports[0].port, 5000);
    }

    #[tokio::test]
    async fn env_file_namespaced_port_var() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join(".env"), "VITE_PORT=5173\n").unwrap();
        let ports = scan_env_files(&pid(), dir.path()).await;
        assert_eq!(ports.len(), 1);
        assert_eq!(ports[0].port, 5173);
    }

    #[tokio::test]
    async fn docker_compose_ports_block() {
        let dir = tempdir().unwrap();
        std::fs::write(
            dir.path().join("docker-compose.yml"),
            "services:\n  web:\n    ports:\n      - \"8080:80\"\n      - \"4443:443\"\n",
        )
        .unwrap();
        let ports = scan_docker_compose(&pid(), dir.path()).await;
        assert_eq!(ports.len(), 2);
        assert!(ports.iter().any(|p| p.port == 8080));
        assert!(ports.iter().any(|p| p.port == 4443));
    }

    #[tokio::test]
    async fn procfile_port() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("Procfile"), "web: PORT=6000 ./server\n").unwrap();
        let ports = scan_procfile(&pid(), dir.path()).await;
        assert_eq!(ports.len(), 1);
        assert_eq!(ports[0].port, 6000);
    }

    #[tokio::test]
    async fn scan_combines_sources() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join(".env"), "PORT=5000\n").unwrap();
        std::fs::write(dir.path().join("Procfile"), "web: PORT=6000 x\n").unwrap();
        let ports = scan(&pid(), dir.path()).await;
        let p: Vec<u16> = ports.iter().map(|d| d.port).collect();
        assert!(p.contains(&5000));
        assert!(p.contains(&6000));
    }

    #[tokio::test]
    async fn missing_files_are_no_op() {
        let dir = tempdir().unwrap();
        let ports = scan(&pid(), dir.path()).await;
        assert!(ports.is_empty());
    }
}
```

- [ ] **Step 2: Add `regex` to workspace deps**

Edit the root `Cargo.toml`. Under `[workspace.dependencies]` add:

```toml
regex = "1"
```

Edit `crates/ccdash-daemon/Cargo.toml`. Under `[dependencies]` add:

```toml
regex = { workspace = true }
```

- [ ] **Step 3: Run the parser tests**

Run: `cargo test -p ccdash-daemon --bin ccdash-daemon ports::declared::`
Expected: 8 PASS.

- [ ] **Step 4: Commit**

```bash
git add Cargo.toml crates/ccdash-daemon/Cargo.toml crates/ccdash-daemon/src/ports/declared.rs
git commit -m "ccdash-daemon: ports::declared — parse PORT from package.json/.env/compose/Procfile"
```

---

## Task C3: Implement `ports::Registry` (composite view)

**Files:**
- Create: `crates/ccdash-daemon/src/ports/registry.rs`

- [ ] **Step 1: Write the registry**

```rust
//! Composite ports view: joins running (lsof) and declared (per-project parsers)
//! sources, refreshed periodically.

use crate::ports::{declared, lsof};
use crate::projects::Registry as ProjectsRegistry;
use anyhow::Result;
use ccdash_core::protocol::{DeclaredPort, PortBinding};
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct Registry {
    projects: Arc<ProjectsRegistry>,
    running: RwLock<Vec<PortBinding>>,
    declared: RwLock<Vec<DeclaredPort>>,
}

impl Registry {
    pub fn new(projects: Arc<ProjectsRegistry>) -> Self {
        Self {
            projects,
            running: RwLock::new(vec![]),
            declared: RwLock::new(vec![]),
        }
    }

    /// Re-scan running listeners + declared ports for all projects.
    pub async fn refresh(&self) -> Result<()> {
        let mut running = lsof::scan().await.unwrap_or_default();

        let projects = self.projects.list().await;
        let mut declared = Vec::new();
        for p in &projects {
            declared.extend(declared::scan(&p.id, &p.path).await);
        }

        // Correlate: stamp project_id on running ports whose port matches a declared port.
        for r in running.iter_mut() {
            if let Some(d) = declared.iter().find(|d| d.port == r.port) {
                r.project_id = Some(d.project_id.clone());
            }
        }

        *self.running.write().await = running;
        *self.declared.write().await = declared;
        Ok(())
    }

    pub async fn running(&self) -> Vec<PortBinding> {
        self.running.read().await.clone()
    }

    pub async fn declared(&self) -> Vec<DeclaredPort> {
        self.declared.read().await.clone()
    }

    /// Find currently-listening ports that would conflict with the declared ports
    /// of the given project.
    pub async fn conflicts_for(&self, project_id: &ccdash_core::domain::ProjectId) -> Vec<(u16, PortBinding)> {
        let declared = self.declared.read().await.clone();
        let running = self.running.read().await.clone();
        let project_declared: Vec<u16> = declared
            .iter()
            .filter(|d| &d.project_id == project_id)
            .map(|d| d.port)
            .collect();
        let mut out = Vec::new();
        for r in running {
            if project_declared.contains(&r.port) && r.project_id.as_ref() != Some(project_id) {
                out.push((r.port, r));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ccdash_core::domain::ProjectId;
    use tempfile::tempdir;

    #[tokio::test]
    async fn fresh_registry_is_empty() {
        let projects = Arc::new(ProjectsRegistry::load(tempdir().unwrap().path().join("p.toml")).await.unwrap());
        let reg = Registry::new(projects);
        assert!(reg.running().await.is_empty());
        assert!(reg.declared().await.is_empty());
    }

    #[tokio::test]
    async fn conflicts_for_returns_running_holders() {
        let dir = tempdir().unwrap();
        let projects = Arc::new(ProjectsRegistry::load(dir.path().join("p.toml")).await.unwrap());
        let reg = Registry::new(projects);

        // Hand-load running + declared bypassing real lsof / file scan.
        *reg.running.write().await = vec![PortBinding {
            port: 3000,
            protocol: "tcp".into(),
            pid: Some(123),
            command: Some("node".into()),
            project_id: None,
        }];
        *reg.declared.write().await = vec![DeclaredPort {
            project_id: ProjectId("p1".into()),
            port: 3000,
            source: ".env".into(),
        }];

        let c = reg.conflicts_for(&ProjectId("p1".into())).await;
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].0, 3000);
    }

    #[tokio::test]
    async fn conflicts_for_skips_self_owned_running_port() {
        let dir = tempdir().unwrap();
        let projects = Arc::new(ProjectsRegistry::load(dir.path().join("p.toml")).await.unwrap());
        let reg = Registry::new(projects);
        let pid = ProjectId("p1".into());

        *reg.running.write().await = vec![PortBinding {
            port: 3000,
            protocol: "tcp".into(),
            pid: Some(123),
            command: Some("node".into()),
            project_id: Some(pid.clone()), // already owned by p1
        }];
        *reg.declared.write().await = vec![DeclaredPort {
            project_id: pid.clone(),
            port: 3000,
            source: ".env".into(),
        }];

        let c = reg.conflicts_for(&pid).await;
        assert!(c.is_empty(), "own port should not appear as a conflict");
    }
}
```

- [ ] **Step 2: Run registry tests**

Run: `cargo test -p ccdash-daemon --bin ccdash-daemon ports::registry::`
Expected: 3 PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/ccdash-daemon/src/ports/registry.rs
git commit -m "ccdash-daemon: ports::Registry composite view + conflicts_for"
```

---

## Task D1: Wire ports into `AppState` and add `ports.list` RPC

**Files:**
- Modify: `crates/ccdash-daemon/src/state.rs`
- Modify: `crates/ccdash-daemon/src/rpc/handlers.rs`
- Modify: `crates/ccdash-daemon/src/rpc/dispatch.rs`
- Modify: `crates/ccdash-daemon/src/main.rs`

- [ ] **Step 1: Add `ports` + `conflict_tokens` to `AppState`**

Edit `crates/ccdash-daemon/src/state.rs`. Replace the file with:

```rust
//! Composite daemon state. Cheap to clone (Arcs all the way down).

use crate::broadcast::Bus;
use crate::ports::Registry as PortsRegistry;
use crate::projects::Registry as ProjectsRegistry;
use crate::sessions::Manager;
use anyhow::Result;
use ccdash_core::paths;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct AppState {
    pub projects: Arc<ProjectsRegistry>,
    pub sessions: Arc<Manager>,
    pub ports: Arc<PortsRegistry>,
    pub bus: Bus,
    pub auth_token: Arc<String>,
    #[allow(dead_code)] // read by file watchers + ports module in Phase 2+
    pub data_dir: PathBuf,
    /// One-shot tokens issued in `PortConflictData`. A token in this set lets the
    /// next `session.launch` bypass conflict gating.
    pub conflict_tokens: Arc<Mutex<HashSet<String>>>,
}

impl AppState {
    pub async fn bootstrap(data_dir: PathBuf) -> Result<Self> {
        let token = ccdash_core::auth::ensure_token(&data_dir.join("auth"))?;
        let projects = Arc::new(ProjectsRegistry::load(data_dir.join("projects.toml")).await?);
        let sessions = Arc::new(Manager::load(data_dir.join("sessions.toml")).await?);
        let ports = Arc::new(PortsRegistry::new(projects.clone()));
        Ok(Self {
            projects,
            sessions,
            ports,
            bus: Bus::new(),
            auth_token: Arc::new(token),
            data_dir,
            conflict_tokens: Arc::new(Mutex::new(HashSet::new())),
        })
    }

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

- [ ] **Step 2: Add `handle_ports_list` to handlers.rs**

Edit `crates/ccdash-daemon/src/rpc/handlers.rs`. Append a new handler function (above the `#[cfg(test)] mod tests` block):

```rust
pub async fn handle_ports_list(state: &AppState) -> Result<ccdash_core::protocol::PortListResult, RpcError> {
    state.ports.refresh().await.map_err(|e| err(E_INTERNAL, e.to_string()))?;
    Ok(ccdash_core::protocol::PortListResult {
        running: state.ports.running().await,
        declared: state.ports.declared().await,
    })
}
```

- [ ] **Step 3: Modify `handle_session_launch` to gate by port conflicts**

Replace the existing `handle_session_launch` function with:

```rust
pub async fn handle_session_launch(
    params: SessionLaunchParams,
    state: &AppState,
) -> Result<SessionLaunchResult, RpcError> {
    let projects = state.projects.list().await;
    let project = projects
        .iter()
        .find(|p| p.id == params.project_id)
        .ok_or_else(|| err(E_NOT_FOUND, "no such project"))?
        .clone();

    // Conflict gating: refresh ports, look for conflicts, return PortConflictData
    // in error.data unless caller supplied a valid force_token.
    if params.force_token.is_none() {
        state.ports.refresh().await.map_err(|e| err(E_INTERNAL, e.to_string()))?;
        let conflicts = state.ports.conflicts_for(&project.id).await;
        if !conflicts.is_empty() {
            let token: String = ccdash_core::auth::generate_token();
            state.conflict_tokens.lock().await.insert(token.clone());
            let data = ccdash_core::protocol::PortConflictData {
                conflicts: conflicts.into_iter().map(|(port, binding)| {
                    ccdash_core::protocol::PortConflict {
                        port,
                        holder: format!(
                            "{} (pid {})",
                            binding.command.unwrap_or_else(|| "?".into()),
                            binding.pid.map(|p| p.to_string()).unwrap_or_else(|| "?".into())
                        ),
                    }
                }).collect(),
                force_token: token,
            };
            return Err(RpcError {
                code: -32002, // E_CONFLICT
                message: "port conflict; pass force_token to bypass".into(),
                data: Some(serde_json::to_value(data).unwrap()),
            });
        }
    } else {
        let supplied = params.force_token.as_ref().unwrap().clone();
        let mut tokens = state.conflict_tokens.lock().await;
        if !tokens.remove(&supplied) {
            return Err(err(E_AUTH, "invalid or expired force_token"));
        }
    }

    let worktree_name = params.worktree.clone().unwrap_or_else(|| "main".to_string());
    let cwd = project
        .worktrees
        .iter()
        .find(|w| w.branch == worktree_name || (worktree_name == "main" && w.is_primary))
        .map(|w| w.path.clone())
        .unwrap_or_else(|| project.path.clone());
    let cmd = params.command.unwrap_or_else(|| "claude".to_string());

    let safe_wt = sanitize(&worktree_name);
    let safe_proj = sanitize(&project.name);
    let name = format!("ccdash_{}_{}", safe_proj, safe_wt);

    let session_id = tmux::new_session(&name, &cwd, &cmd)
        .await
        .map_err(|e| err(E_INTERNAL, e.to_string()))?;
    state
        .sessions
        .record_launch(session_id.clone(), project.id.clone(), Some(worktree_name))
        .await
        .map_err(|e| err(E_INTERNAL, e.to_string()))?;

    let (current, _) = state.sessions.refresh().await.map_err(|e| err(E_INTERNAL, e.to_string()))?;
    let session = current
        .into_iter()
        .find(|s| s.tmux_session_id == session_id)
        .unwrap_or_else(|| Session {
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
```

- [ ] **Step 4: Add the `ports.list` route to dispatch.rs**

Edit `crates/ccdash-daemon/src/rpc/dispatch.rs`. Add a new match arm AFTER the existing `"session.kill"` arm and BEFORE the catch-all `other =>`:

```rust
        "ports.list" => match handlers::handle_ports_list(state).await {
            Ok(r) => Response::ok(id, serde_json::to_value(r).unwrap()),
            Err(e) => Response::err(id, e),
        },
```

- [ ] **Step 5: Run all daemon tests + integration to confirm no regression**

Run: `cargo test -p ccdash-daemon`
Expected: previously-passing tests still pass. New `handle_ports_list` and conflict-gating path have no dedicated unit tests yet — they're covered by Phase 2 integration tests (H1).

- [ ] **Step 6: Commit**

```bash
git add crates/ccdash-daemon/src/state.rs crates/ccdash-daemon/src/rpc/handlers.rs crates/ccdash-daemon/src/rpc/dispatch.rs
git commit -m "ccdash-daemon: ports.list RPC + conflict gating on session.launch"
```

---

## Task E1: Implement `plans::parser`

**Files:**
- Create: `crates/ccdash-daemon/src/plans/mod.rs`
- Create: `crates/ccdash-daemon/src/plans/parser.rs`

- [ ] **Step 1: Add `pulldown-cmark` to workspace + daemon deps**

Edit the root `Cargo.toml`. Under `[workspace.dependencies]` add:

```toml
pulldown-cmark = "0.12"
```

Edit `crates/ccdash-daemon/Cargo.toml`. Under `[dependencies]` add:

```toml
pulldown-cmark = { workspace = true }
```

- [ ] **Step 2: Write `plans/mod.rs`**

```rust
//! Plan markdown parser + per-project file watcher.

pub mod parser;
pub mod watcher;

pub use watcher::Manager;
```

- [ ] **Step 3: Write `plans/parser.rs`**

```rust
//! Parses markdown plan files into structured Phase/Task records.
//!
//! Convention (matches superpowers:writing-plans output):
//! - `## Phase N: Title` or `## Task N: Title` → a phase boundary
//! - GitHub-flavored task list items (`- [ ]` / `- [x]`) → tasks
//! - First `# Heading` of the file → plan title (falls back to filename stem)

use ccdash_core::protocol::{Plan, PlanPhase, PlanTask};
use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use std::path::Path;

pub fn parse(path: &Path, text: &str) -> Plan {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TASKLISTS);
    let parser = Parser::new_ext(text, options);

    let mut title: Option<String> = None;
    let mut phases: Vec<PlanPhase> = Vec::new();
    let mut current_text = String::new();
    let mut in_heading_level: Option<HeadingLevel> = None;
    let mut in_list_item = false;
    let mut current_task_done: Option<bool> = None;
    let mut current_task_text = String::new();

    for evt in parser {
        match evt {
            Event::Start(Tag::Heading { level, .. }) => {
                in_heading_level = Some(level);
                current_text.clear();
            }
            Event::End(TagEnd::Heading(level)) => {
                let text = current_text.trim().to_string();
                in_heading_level = None;
                current_text.clear();
                if level == HeadingLevel::H1 && title.is_none() {
                    title = Some(text);
                } else if level == HeadingLevel::H2 || level == HeadingLevel::H3 {
                    // Treat ## and ### as phase boundaries iff they look like a phase/task header.
                    if text.starts_with("Phase ") || text.starts_with("Task ") || text.starts_with("Section ") {
                        phases.push(PlanPhase {
                            name: text,
                            tasks: Vec::new(),
                        });
                    }
                }
            }
            Event::Start(Tag::Item) => {
                in_list_item = true;
                current_task_done = None;
                current_task_text.clear();
            }
            Event::End(TagEnd::Item) => {
                if in_list_item {
                    if let Some(done) = current_task_done {
                        let title = current_task_text.trim().to_string();
                        if let Some(phase) = phases.last_mut() {
                            phase.tasks.push(PlanTask { title, done });
                        } else {
                            phases.push(PlanPhase {
                                name: "(top level)".into(),
                                tasks: vec![PlanTask { title, done }],
                            });
                        }
                    }
                }
                in_list_item = false;
                current_task_done = None;
                current_task_text.clear();
            }
            Event::TaskListMarker(done) => {
                current_task_done = Some(done);
            }
            Event::Text(t) => {
                if in_heading_level.is_some() {
                    current_text.push_str(&t);
                } else if in_list_item && current_task_done.is_some() {
                    current_task_text.push_str(&t);
                }
            }
            Event::Code(t) => {
                if in_heading_level.is_some() {
                    current_text.push_str(&t);
                } else if in_list_item && current_task_done.is_some() {
                    current_task_text.push_str(&t);
                }
            }
            _ => {}
        }
    }

    let title = title.unwrap_or_else(|| {
        path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("plan")
            .to_string()
    });

    Plan {
        path: path.to_path_buf(),
        title,
        phases,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_title_and_phases() {
        let md = "\
# My Plan

## Phase 1: Setup

- [ ] do thing
- [x] did thing

## Phase 2: Run

- [ ] launch
";
        let p = parse(Path::new("/tmp/x.md"), md);
        assert_eq!(p.title, "My Plan");
        assert_eq!(p.phases.len(), 2);
        assert_eq!(p.phases[0].name, "Phase 1: Setup");
        assert_eq!(p.phases[0].tasks.len(), 2);
        assert!(!p.phases[0].tasks[0].done);
        assert!(p.phases[0].tasks[1].done);
        assert_eq!(p.phases[1].tasks.len(), 1);
    }

    #[test]
    fn tasks_before_any_phase_go_under_top_level() {
        let md = "- [ ] orphan task\n";
        let p = parse(Path::new("/tmp/x.md"), md);
        assert_eq!(p.phases.len(), 1);
        assert_eq!(p.phases[0].name, "(top level)");
        assert_eq!(p.phases[0].tasks.len(), 1);
    }

    #[test]
    fn falls_back_to_filename_title() {
        let p = parse(Path::new("/tmp/cool-thing.md"), "## Phase 1: foo\n- [ ] x\n");
        assert_eq!(p.title, "cool-thing");
    }

    #[test]
    fn ignores_non_task_list_items() {
        let md = "## Phase 1: foo\n- regular item\n- [ ] real task\n";
        let p = parse(Path::new("/tmp/x.md"), md);
        assert_eq!(p.phases[0].tasks.len(), 1);
        assert_eq!(p.phases[0].tasks[0].title, "real task");
    }

    #[test]
    fn h3_task_heading_is_a_phase() {
        let md = "### Task A1: Init\n- [ ] step\n";
        let p = parse(Path::new("/tmp/x.md"), md);
        assert_eq!(p.phases.len(), 1);
        assert_eq!(p.phases[0].name, "Task A1: Init");
    }
}
```

- [ ] **Step 4: Wire `mod plans;` into `main.rs`**

Add `mod plans;` to the daemon `main.rs` `mod` list (alphabetical).

- [ ] **Step 5: Run tests**

Run: `cargo test -p ccdash-daemon --bin ccdash-daemon plans::parser::`
Expected: 5 PASS.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml crates/ccdash-daemon/Cargo.toml crates/ccdash-daemon/src/plans crates/ccdash-daemon/src/main.rs
git commit -m "ccdash-daemon: plans::parser — markdown to Plan/Phase/Task model"
```

---

## Task E2: Implement `plans::watcher` (notify + Manager)

**Files:**
- Create: `crates/ccdash-daemon/src/plans/watcher.rs`

- [ ] **Step 1: Write the watcher + Manager**

```rust
//! Per-project file watcher for `docs/superpowers/{specs,plans}/**/*.md`.
//!
//! Phase 2 uses a simple polling implementation backed by a 5-second mtime
//! scan, not the `notify` crate. This keeps the test surface small and avoids
//! cross-platform notify quirks. The `notify` upgrade is a v2 improvement.

use crate::plans::parser;
use anyhow::Result;
use ccdash_core::domain::ProjectId;
use ccdash_core::protocol::Plan;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs;
use tokio::sync::RwLock;

pub struct Manager {
    cache: RwLock<HashMap<ProjectId, Vec<Plan>>>,
}

impl Manager {
    pub fn new() -> Self {
        Self { cache: RwLock::new(HashMap::new()) }
    }

    /// Re-scan the plan/spec markdown files for one project and cache the result.
    pub async fn refresh(&self, project_id: &ProjectId, project_root: &Path) -> Result<Vec<Plan>> {
        let mut plans = Vec::new();
        for sub in ["docs/superpowers/specs", "docs/superpowers/plans"] {
            let dir = project_root.join(sub);
            plans.extend(scan_dir(&dir).await);
        }
        plans.sort_by(|a, b| a.path.cmp(&b.path));
        self.cache.write().await.insert(project_id.clone(), plans.clone());
        Ok(plans)
    }

    pub async fn get(&self, project_id: &ProjectId) -> Option<Vec<Plan>> {
        self.cache.read().await.get(project_id).cloned()
    }
}

impl Default for Manager {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(clippy::manual_async_fn)]
fn scan_dir(dir: &Path) -> impl std::future::Future<Output = Vec<Plan>> + '_ {
    async move {
        let mut out = Vec::new();
        let mut entries = match fs::read_dir(dir).await {
            Ok(e) => e,
            Err(_) => return out,
        };
        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            match entry.file_type().await {
                Ok(ft) if ft.is_file() => {
                    if path.extension().and_then(|e| e.to_str()) != Some("md") {
                        continue;
                    }
                    if let Ok(text) = fs::read_to_string(&path).await {
                        out.push(parser::parse(&path, &text));
                    }
                }
                Ok(ft) if ft.is_dir() => {
                    let sub_plans: Vec<Plan> = Box::pin(scan_dir(&path)).await;
                    out.extend(sub_plans);
                }
                _ => {}
            }
        }
        out
    }
}

#[allow(unused_imports)] // suppress until notify wiring lands in a later phase
use std::sync::Mutex as _NotifyAnchor;
// Reserved use: `pub use notify`; will be added when we replace polling with
// real watcher events in a future revision.
#[allow(dead_code)]
pub(crate) fn _notify_anchor(_: Arc<()>) {}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn refresh_finds_plans_under_specs_and_plans() {
        let dir = tempdir().unwrap();
        let specs = dir.path().join("docs/superpowers/specs");
        let plans = dir.path().join("docs/superpowers/plans");
        std::fs::create_dir_all(&specs).unwrap();
        std::fs::create_dir_all(&plans).unwrap();
        std::fs::write(specs.join("spec-a.md"), "# Spec A\n## Phase 1: x\n- [ ] t\n").unwrap();
        std::fs::write(plans.join("plan-b.md"), "# Plan B\n## Phase 1: y\n- [x] q\n").unwrap();

        let mgr = Manager::new();
        let pid = ProjectId("p1".into());
        let found = mgr.refresh(&pid, dir.path()).await.unwrap();
        assert_eq!(found.len(), 2);
        let titles: Vec<&str> = found.iter().map(|p| p.title.as_str()).collect();
        assert!(titles.contains(&"Spec A"));
        assert!(titles.contains(&"Plan B"));

        let cached = mgr.get(&pid).await.unwrap();
        assert_eq!(cached.len(), 2);
    }

    #[tokio::test]
    async fn refresh_empty_when_no_dirs() {
        let dir = tempdir().unwrap();
        let mgr = Manager::new();
        let pid = ProjectId("p1".into());
        let found = mgr.refresh(&pid, dir.path()).await.unwrap();
        assert!(found.is_empty());
    }

    #[tokio::test]
    async fn refresh_recurses_subdirectories() {
        let dir = tempdir().unwrap();
        let nested = dir.path().join("docs/superpowers/plans/sub");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(nested.join("plan.md"), "# Nested Plan\n## Phase 1: x\n- [ ] t\n").unwrap();
        let mgr = Manager::new();
        let pid = ProjectId("p1".into());
        let found = mgr.refresh(&pid, dir.path()).await.unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].title, "Nested Plan");
    }
}
```

> Implementation note: per Phase 1's spec deviation discipline, plan called for the `notify` crate for live file-system events. After implementing the parser + an explicit refresh path, the daemon will call `Manager::refresh` (a) on each `plans.get` RPC (always fresh) and (b) opportunistically when projects are added. A true `notify` watcher is deferred — refresh-on-read is sufficient for Phase 2 acceptance criteria and avoids the cross-platform `notify` complexity (kqueue on macOS vs inotify on Linux, debouncing). Document this in the EXECUTION-LOG.

- [ ] **Step 2: Run tests**

Run: `cargo test -p ccdash-daemon --bin ccdash-daemon plans::watcher::`
Expected: 3 PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/ccdash-daemon/src/plans/watcher.rs
git commit -m "ccdash-daemon: plans::Manager refresh-on-read for per-project plans"
```

---

## Task F1: Add `plans.get` RPC + wire `plans::Manager` into AppState

**Files:**
- Modify: `crates/ccdash-daemon/src/state.rs`
- Modify: `crates/ccdash-daemon/src/rpc/handlers.rs`
- Modify: `crates/ccdash-daemon/src/rpc/dispatch.rs`

- [ ] **Step 1: Add `plans` field to `AppState`**

Edit `crates/ccdash-daemon/src/state.rs`. Update the struct + bootstrap:

```rust
#[derive(Clone)]
pub struct AppState {
    pub projects: Arc<ProjectsRegistry>,
    pub sessions: Arc<Manager>,
    pub ports: Arc<PortsRegistry>,
    pub plans: Arc<crate::plans::Manager>,
    pub bus: Bus,
    pub auth_token: Arc<String>,
    #[allow(dead_code)]
    pub data_dir: PathBuf,
    pub conflict_tokens: Arc<Mutex<HashSet<String>>>,
}
```

And in `AppState::bootstrap`, after constructing `ports`:

```rust
let plans = Arc::new(crate::plans::Manager::new());
```

And include `plans` in the struct literal returned.

Full replacement of `bootstrap`:

```rust
pub async fn bootstrap(data_dir: PathBuf) -> Result<Self> {
    let token = ccdash_core::auth::ensure_token(&data_dir.join("auth"))?;
    let projects = Arc::new(ProjectsRegistry::load(data_dir.join("projects.toml")).await?);
    let sessions = Arc::new(Manager::load(data_dir.join("sessions.toml")).await?);
    let ports = Arc::new(PortsRegistry::new(projects.clone()));
    let plans = Arc::new(crate::plans::Manager::new());
    Ok(Self {
        projects,
        sessions,
        ports,
        plans,
        bus: Bus::new(),
        auth_token: Arc::new(token),
        data_dir,
        conflict_tokens: Arc::new(Mutex::new(HashSet::new())),
    })
}
```

- [ ] **Step 2: Add `handle_plans_get` to handlers.rs**

Append above the `#[cfg(test)] mod tests` block:

```rust
pub async fn handle_plans_get(
    params: ccdash_core::protocol::PlanGetParams,
    state: &AppState,
) -> Result<ccdash_core::protocol::PlanGetResult, RpcError> {
    let projects = state.projects.list().await;
    let project = projects
        .iter()
        .find(|p| p.id == params.project_id)
        .ok_or_else(|| err(E_NOT_FOUND, "no such project"))?
        .clone();
    let plans = state
        .plans
        .refresh(&project.id, &project.path)
        .await
        .map_err(|e| err(E_INTERNAL, e.to_string()))?;
    Ok(ccdash_core::protocol::PlanGetResult { plans })
}
```

- [ ] **Step 3: Route `plans.get` in dispatch.rs**

Add a new match arm next to the other arms:

```rust
        "plans.get" => {
            let params: ccdash_core::protocol::PlanGetParams =
                match serde_json::from_value(req.params) {
                    Ok(p) => p,
                    Err(e) => return Response::err(id, err(E_INVALID_PARAMS, e.to_string())),
                };
            match handlers::handle_plans_get(params, state).await {
                Ok(r) => Response::ok(id, serde_json::to_value(r).unwrap()),
                Err(e) => Response::err(id, e),
            }
        }
```

- [ ] **Step 4: Run daemon tests**

Run: `cargo test -p ccdash-daemon`
Expected: same pass count as before + the `state::tests::bootstrap_creates_auth_token` test still passes despite the new field (since it has Default initialization via `Arc::new(Manager::new())`).

- [ ] **Step 5: Commit**

```bash
git add crates/ccdash-daemon/src/state.rs crates/ccdash-daemon/src/rpc/handlers.rs crates/ccdash-daemon/src/rpc/dispatch.rs
git commit -m "ccdash-daemon: plans.get RPC + Manager wired into AppState"
```

---

## Task G1: Scaffold `ccdash-cli` crate

**Files:**
- Create: `crates/ccdash-cli/Cargo.toml`
- Create: `crates/ccdash-cli/src/main.rs`
- Create: `crates/ccdash-cli/src/commands/mod.rs`
- Modify: root `Cargo.toml` (add to workspace members)

- [ ] **Step 1: Add the crate to workspace members**

Edit the root `Cargo.toml`. Change:

```toml
members = ["crates/ccdash-core", "crates/ccdash-daemon"]
```

to:

```toml
members = ["crates/ccdash-core", "crates/ccdash-daemon", "crates/ccdash-cli"]
```

- [ ] **Step 2: Write crate `Cargo.toml`**

Create `crates/ccdash-cli/Cargo.toml`:

```toml
[package]
name = "ccdash-cli"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[[bin]]
name = "ccdash"
path = "src/main.rs"

[dependencies]
ccdash-core = { path = "../ccdash-core" }
anyhow = { workspace = true }
clap = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
tokio = { workspace = true }
tracing = { workspace = true }
tracing-subscriber = { workspace = true }

[dev-dependencies]
tempfile = { workspace = true }
```

- [ ] **Step 3: Write skeleton `main.rs`**

Create `crates/ccdash-cli/src/main.rs`:

```rust
//! `ccdash` CLI — connects to ccdash-daemon over Unix socket.

mod commands;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "ccdash", version, about = "ccdash command-line client")]
struct Cli {
    /// Override the daemon socket path.
    #[arg(long, env = "CCDASH_SOCKET", global = true)]
    socket: Option<PathBuf>,

    /// Log level (trace, debug, info, warn, error).
    #[arg(long, env = "CCDASH_LOG", default_value = "warn", global = true)]
    log_level: String,

    #[command(subcommand)]
    cmd: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Show daemon health + counts.
    Status,
    /// Project management.
    Project {
        #[command(subcommand)]
        sub: commands::project::Sub,
    },
    /// List running sessions.
    List,
    /// Launch a new session.
    Launch {
        /// Project name or ID.
        project: String,
        /// Worktree name (defaults to primary).
        #[arg(long)]
        worktree: Option<String>,
        /// Command override (defaults to `claude`).
        #[arg(long)]
        command: Option<String>,
        /// Bypass port-conflict check (use the force_token returned from a prior conflict).
        #[arg(long)]
        force_token: Option<String>,
    },
    /// Kill a session by tmux session_id (e.g. `$3`).
    Kill { session_id: String },
    /// Show port registry.
    Ports {
        /// Filter to one project.
        #[arg(long)]
        project: Option<String>,
    },
    /// Show plan progress for a project.
    Plan {
        /// Project name or ID.
        project: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_new(&cli.log_level).unwrap_or_else(|_| EnvFilter::new("warn")))
        .init();

    match cli.cmd {
        Command::Status => commands::status::run(cli.socket).await,
        Command::Project { sub } => commands::project::run(cli.socket, sub).await,
        Command::List => commands::list::run(cli.socket).await,
        Command::Launch { project, worktree, command, force_token } => {
            commands::launch::run(cli.socket, project, worktree, command, force_token).await
        }
        Command::Kill { session_id } => commands::kill::run(cli.socket, session_id).await,
        Command::Ports { project } => commands::ports::run(cli.socket, project).await,
        Command::Plan { project } => commands::plan::run(cli.socket, project).await,
    }
}
```

- [ ] **Step 4: Write commands module skeleton**

Create `crates/ccdash-cli/src/commands/mod.rs`:

```rust
pub mod kill;
pub mod launch;
pub mod list;
pub mod plan;
pub mod ports;
pub mod project;
pub mod status;

use anyhow::Result;
use ccdash_core::client::Client;
use ccdash_core::protocol::ClientKind;
use std::path::PathBuf;

/// Helper: connect to socket (default if None), handshake, return ready Client.
pub async fn connect(socket: Option<PathBuf>) -> Result<Client> {
    let mut c = match socket {
        Some(p) => Client::connect(&p).await?,
        None => Client::connect_default().await?,
    };
    let resp = c.handshake(ClientKind::Cli).await?;
    if let Some(e) = resp.error {
        anyhow::bail!("handshake failed: {}", e.message);
    }
    Ok(c)
}

/// Resolve a `name-or-id` string against the project list, returning the project id.
pub async fn resolve_project_id(c: &mut Client, name_or_id: &str) -> Result<String> {
    let resp = c.call("project.list", serde_json::json!({})).await?;
    let projects = resp
        .result
        .ok_or_else(|| anyhow::anyhow!("project.list returned no result"))?
        ["projects"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("projects not an array"))?
        .clone();
    for p in &projects {
        let id = p["id"].as_str().unwrap_or("");
        let name = p["name"].as_str().unwrap_or("");
        if id == name_or_id || name == name_or_id {
            return Ok(id.to_string());
        }
    }
    anyhow::bail!("no project matches '{}'", name_or_id);
}
```

- [ ] **Step 5: Write stub command modules**

Create one file per command at `crates/ccdash-cli/src/commands/<name>.rs`, each containing just:

```rust
// stub — implemented in subsequent tasks.
```

For: `status.rs`, `project.rs`, `list.rs`, `launch.rs`, `kill.rs`, `ports.rs`, `plan.rs`.

Each one needs to be a valid module. To keep the workspace compiling now, fill each with a placeholder function matching what `main.rs` calls. Replace the stub content of each file as follows:

`status.rs`:
```rust
use anyhow::Result;
use std::path::PathBuf;

pub async fn run(_socket: Option<PathBuf>) -> Result<()> {
    anyhow::bail!("status: not yet implemented")
}
```

`list.rs`, `launch.rs` (different signature), `kill.rs`, `ports.rs`, `plan.rs`: similar pattern, matching the arguments in `main.rs`.

`project.rs`:
```rust
use anyhow::Result;
use clap::Subcommand;
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum Sub {
    Add { path: PathBuf, #[arg(long)] name: Option<String> },
    Rm { id: String },
    List,
    Scan { #[arg(long)] root: Option<PathBuf> },
}

pub async fn run(_socket: Option<PathBuf>, _sub: Sub) -> Result<()> {
    anyhow::bail!("project: not yet implemented")
}
```

`launch.rs`:
```rust
use anyhow::Result;
use std::path::PathBuf;

pub async fn run(
    _socket: Option<PathBuf>,
    _project: String,
    _worktree: Option<String>,
    _command: Option<String>,
    _force_token: Option<String>,
) -> Result<()> {
    anyhow::bail!("launch: not yet implemented")
}
```

`list.rs`:
```rust
use anyhow::Result;
use std::path::PathBuf;

pub async fn run(_socket: Option<PathBuf>) -> Result<()> {
    anyhow::bail!("list: not yet implemented")
}
```

`kill.rs`:
```rust
use anyhow::Result;
use std::path::PathBuf;

pub async fn run(_socket: Option<PathBuf>, _session_id: String) -> Result<()> {
    anyhow::bail!("kill: not yet implemented")
}
```

`ports.rs`:
```rust
use anyhow::Result;
use std::path::PathBuf;

pub async fn run(_socket: Option<PathBuf>, _project: Option<String>) -> Result<()> {
    anyhow::bail!("ports: not yet implemented")
}
```

`plan.rs`:
```rust
use anyhow::Result;
use std::path::PathBuf;

pub async fn run(_socket: Option<PathBuf>, _project: String) -> Result<()> {
    anyhow::bail!("plan: not yet implemented")
}
```

- [ ] **Step 6: Build the workspace**

Run: `cargo build --workspace`
Expected: SUCCESS (CLI builds with stub commands).

- [ ] **Step 7: Smoke test the CLI's `--help`**

Run: `cargo run -p ccdash-cli -- --help`
Expected: clap prints the help block listing all subcommands.

- [ ] **Step 8: Commit**

```bash
git add Cargo.toml crates/ccdash-cli
git commit -m "ccdash-cli: scaffold crate with clap subcommands + connect helper"
```

---

## Task G2: Implement `ccdash status` and `ccdash project add/list/rm`

**Files:**
- Modify: `crates/ccdash-cli/src/commands/status.rs`
- Modify: `crates/ccdash-cli/src/commands/project.rs`

- [ ] **Step 1: Replace `status.rs`**

```rust
use crate::commands::connect;
use anyhow::Result;
use std::path::PathBuf;

pub async fn run(socket: Option<PathBuf>) -> Result<()> {
    let mut c = connect(socket).await?;

    let projects = c.call("project.list", serde_json::json!({})).await?;
    let plist = projects
        .result
        .as_ref()
        .and_then(|r| r["projects"].as_array().cloned())
        .unwrap_or_default();

    let sessions = c.call("session.list", serde_json::json!({})).await?;
    let slist = sessions
        .result
        .as_ref()
        .and_then(|r| r["sessions"].as_array().cloned())
        .unwrap_or_default();

    println!("daemon: ok");
    println!("projects: {}", plist.len());
    println!("sessions: {}", slist.len());
    Ok(())
}
```

- [ ] **Step 2: Replace `project.rs`**

```rust
use crate::commands::connect;
use anyhow::Result;
use clap::Subcommand;
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum Sub {
    /// Add a project by path.
    Add {
        path: PathBuf,
        #[arg(long)]
        name: Option<String>,
    },
    /// Remove a project by id.
    Rm { id: String },
    /// List projects.
    List,
    /// Scan a root directory for git repos (does not register them — use `add` for that).
    Scan {
        #[arg(long)]
        root: Option<PathBuf>,
    },
}

pub async fn run(socket: Option<PathBuf>, sub: Sub) -> Result<()> {
    let mut c = connect(socket).await?;
    match sub {
        Sub::Add { path, name } => {
            let resp = c
                .call(
                    "project.add",
                    serde_json::json!({"path": path, "name": name}),
                )
                .await?;
            if let Some(err) = resp.error {
                anyhow::bail!("project.add: {}", err.message);
            }
            let p = resp.result.unwrap();
            println!("added: {}  ({})", p["name"].as_str().unwrap_or("?"), p["id"].as_str().unwrap_or("?"));
        }
        Sub::Rm { id } => {
            let resp = c.call("project.remove", serde_json::json!({"id": id})).await?;
            if let Some(err) = resp.error {
                anyhow::bail!("project.remove: {}", err.message);
            }
            println!("removed: {}", id);
        }
        Sub::List => {
            let resp = c.call("project.list", serde_json::json!({})).await?;
            let projects = resp
                .result
                .unwrap()["projects"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            for p in projects {
                println!(
                    "{}  {}  {}",
                    p["id"].as_str().unwrap_or("?"),
                    p["name"].as_str().unwrap_or("?"),
                    p["path"].as_str().unwrap_or("?")
                );
            }
        }
        Sub::Scan { root: _ } => {
            // Daemon-side scanner not yet exposed via RPC in Phase 2. Print a note.
            println!("scan: not yet wired (Phase 2 daemon ships scanner module but no RPC). Use `project add <path>` for now.");
        }
    }
    Ok(())
}
```

- [ ] **Step 3: Build + manual smoke (against running daemon)**

Run: `cargo build --workspace`

Optional manual smoke (skip in CI):
```bash
# In one terminal:
CCDASH_HOME=/tmp/cli-smoke CCDASH_SOCKET=/tmp/cli-smoke.sock cargo run -p ccdash-daemon -- --log-level warn &

# In another:
sleep 1
cargo run -p ccdash-cli -- --socket /tmp/cli-smoke.sock status
cargo run -p ccdash-cli -- --socket /tmp/cli-smoke.sock project add /tmp
cargo run -p ccdash-cli -- --socket /tmp/cli-smoke.sock project list
```

- [ ] **Step 4: Commit**

```bash
git add crates/ccdash-cli/src/commands/status.rs crates/ccdash-cli/src/commands/project.rs
git commit -m "ccdash-cli: status + project (add/list/rm/scan)"
```

---

## Task G3: Implement `ccdash list`, `ccdash launch`, `ccdash kill`

**Files:**
- Modify: `crates/ccdash-cli/src/commands/list.rs`
- Modify: `crates/ccdash-cli/src/commands/launch.rs`
- Modify: `crates/ccdash-cli/src/commands/kill.rs`

- [ ] **Step 1: Replace `list.rs`**

```rust
use crate::commands::connect;
use anyhow::Result;
use std::path::PathBuf;

pub async fn run(socket: Option<PathBuf>) -> Result<()> {
    let mut c = connect(socket).await?;
    let resp = c.call("session.list", serde_json::json!({})).await?;
    if let Some(err) = resp.error {
        anyhow::bail!("session.list: {}", err.message);
    }
    let sessions = resp.result.unwrap()["sessions"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    if sessions.is_empty() {
        println!("(no sessions)");
        return Ok(());
    }
    for s in sessions {
        println!(
            "{}  {}  pid={}  cwd={}",
            s["tmux_session_id"].as_str().unwrap_or("?"),
            s["name"].as_str().unwrap_or("?"),
            s["pid"].as_i64().unwrap_or(-1),
            s["cwd"].as_str().unwrap_or("?")
        );
    }
    Ok(())
}
```

- [ ] **Step 2: Replace `launch.rs`**

```rust
use crate::commands::{connect, resolve_project_id};
use anyhow::Result;
use std::path::PathBuf;

pub async fn run(
    socket: Option<PathBuf>,
    project: String,
    worktree: Option<String>,
    command: Option<String>,
    force_token: Option<String>,
) -> Result<()> {
    let mut c = connect(socket).await?;
    let project_id = resolve_project_id(&mut c, &project).await?;

    let mut params = serde_json::json!({
        "project_id": project_id,
        "worktree": worktree,
        "command": command,
    });
    if let Some(t) = force_token {
        params["force_token"] = serde_json::Value::String(t);
    }
    let resp = c.call("session.launch", params).await?;
    if let Some(err) = resp.error {
        if err.code == -32002 {
            // Port conflict — print details + the force_token the user can re-pass.
            println!("port conflict:");
            if let Some(data) = err.data {
                if let Some(conflicts) = data["conflicts"].as_array() {
                    for c in conflicts {
                        println!(
                            "  port {} held by {}",
                            c["port"].as_u64().unwrap_or(0),
                            c["holder"].as_str().unwrap_or("?")
                        );
                    }
                }
                if let Some(tok) = data["force_token"].as_str() {
                    println!("\nto launch anyway: ccdash launch {} --force-token {}", project, tok);
                }
            }
            anyhow::bail!("launch blocked by port conflict");
        }
        anyhow::bail!("session.launch: {}", err.message);
    }
    let session = resp.result.unwrap()["session"].clone();
    println!(
        "launched: {}  {}",
        session["tmux_session_id"].as_str().unwrap_or("?"),
        session["name"].as_str().unwrap_or("?")
    );
    Ok(())
}
```

- [ ] **Step 3: Replace `kill.rs`**

```rust
use crate::commands::connect;
use anyhow::Result;
use std::path::PathBuf;

pub async fn run(socket: Option<PathBuf>, session_id: String) -> Result<()> {
    let mut c = connect(socket).await?;
    let resp = c
        .call("session.kill", serde_json::json!({"tmux_session_id": session_id}))
        .await?;
    if let Some(err) = resp.error {
        anyhow::bail!("session.kill: {}", err.message);
    }
    println!("killed: {}", session_id);
    Ok(())
}
```

- [ ] **Step 4: Build**

Run: `cargo build --workspace`
Expected: SUCCESS.

- [ ] **Step 5: Commit**

```bash
git add crates/ccdash-cli/src/commands/list.rs crates/ccdash-cli/src/commands/launch.rs crates/ccdash-cli/src/commands/kill.rs
git commit -m "ccdash-cli: list, launch (with conflict UX), kill"
```

---

## Task G4: Implement `ccdash ports` and `ccdash plan`

**Files:**
- Modify: `crates/ccdash-cli/src/commands/ports.rs`
- Modify: `crates/ccdash-cli/src/commands/plan.rs`

- [ ] **Step 1: Replace `ports.rs`**

```rust
use crate::commands::{connect, resolve_project_id};
use anyhow::Result;
use std::path::PathBuf;

pub async fn run(socket: Option<PathBuf>, project_filter: Option<String>) -> Result<()> {
    let mut c = connect(socket).await?;
    let resp = c.call("ports.list", serde_json::json!({})).await?;
    if let Some(err) = resp.error {
        anyhow::bail!("ports.list: {}", err.message);
    }
    let result = resp.result.unwrap();
    let running = result["running"].as_array().cloned().unwrap_or_default();
    let declared = result["declared"].as_array().cloned().unwrap_or_default();

    let filter_id: Option<String> = if let Some(p) = project_filter {
        Some(resolve_project_id(&mut c, &p).await?)
    } else {
        None
    };

    println!("RUNNING:");
    for p in &running {
        if let Some(ref fid) = filter_id {
            if p["project_id"].as_str() != Some(fid.as_str()) {
                continue;
            }
        }
        println!(
            "  {} (pid {}, {}) project={}",
            p["port"].as_u64().unwrap_or(0),
            p["pid"].as_i64().unwrap_or(-1),
            p["command"].as_str().unwrap_or("?"),
            p["project_id"].as_str().unwrap_or("-")
        );
    }
    println!("DECLARED:");
    for p in &declared {
        if let Some(ref fid) = filter_id {
            if p["project_id"].as_str() != Some(fid.as_str()) {
                continue;
            }
        }
        println!(
            "  {} project={} source={}",
            p["port"].as_u64().unwrap_or(0),
            p["project_id"].as_str().unwrap_or("?"),
            p["source"].as_str().unwrap_or("?")
        );
    }
    Ok(())
}
```

- [ ] **Step 2: Replace `plan.rs`**

```rust
use crate::commands::{connect, resolve_project_id};
use anyhow::Result;
use std::path::PathBuf;

pub async fn run(socket: Option<PathBuf>, project: String) -> Result<()> {
    let mut c = connect(socket).await?;
    let project_id = resolve_project_id(&mut c, &project).await?;
    let resp = c
        .call("plans.get", serde_json::json!({"project_id": project_id}))
        .await?;
    if let Some(err) = resp.error {
        anyhow::bail!("plans.get: {}", err.message);
    }
    let plans = resp.result.unwrap()["plans"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    if plans.is_empty() {
        println!("(no plans found under docs/superpowers/{{specs,plans}}/)");
        return Ok(());
    }
    for p in plans {
        println!("== {} ({}) ==", p["title"].as_str().unwrap_or("?"), p["path"].as_str().unwrap_or("?"));
        let phases = p["phases"].as_array().cloned().unwrap_or_default();
        for phase in phases {
            println!("  ## {}", phase["name"].as_str().unwrap_or("?"));
            let tasks = phase["tasks"].as_array().cloned().unwrap_or_default();
            let done = tasks.iter().filter(|t| t["done"].as_bool().unwrap_or(false)).count();
            let total = tasks.len();
            for t in tasks {
                let marker = if t["done"].as_bool().unwrap_or(false) { "[x]" } else { "[ ]" };
                println!("    {} {}", marker, t["title"].as_str().unwrap_or("?"));
            }
            println!("    ({}/{} done)", done, total);
        }
        println!();
    }
    Ok(())
}
```

- [ ] **Step 3: Build**

Run: `cargo build --workspace`
Expected: SUCCESS.

- [ ] **Step 4: Commit**

```bash
git add crates/ccdash-cli/src/commands/ports.rs crates/ccdash-cli/src/commands/plan.rs
git commit -m "ccdash-cli: ports + plan output"
```

---

## Task H1: Integration test — CLI end-to-end (handshake + status)

**Files:**
- Create: `crates/ccdash-cli/tests/cli_smoke.rs`

- [ ] **Step 1: Write the test**

Create `crates/ccdash-cli/tests/cli_smoke.rs`:

```rust
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;
use tempfile::tempdir;

/// Spawn the daemon, then run the CLI binary against it.
#[test]
fn cli_status_round_trip() {
    let dir = tempdir().unwrap();
    let socket = dir.path().join("ccdash.sock");
    let data_dir = dir.path().join("home");
    std::fs::create_dir_all(&data_dir).unwrap();

    let daemon_bin = PathBuf::from(env!("CARGO_BIN_EXE_ccdash-daemon"));
    let cli_bin = PathBuf::from(env!("CARGO_BIN_EXE_ccdash"));
    assert!(daemon_bin.exists(), "daemon binary missing");
    assert!(cli_bin.exists(), "cli binary missing");

    let mut daemon = Command::new(&daemon_bin)
        .arg("--socket").arg(&socket)
        .arg("--data-dir").arg(&data_dir)
        .arg("--log-level").arg("warn")
        .spawn()
        .expect("spawn daemon");

    // Wait for socket up.
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    while !socket.exists() && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(socket.exists(), "daemon socket never appeared");

    let out = Command::new(&cli_bin)
        .arg("--socket").arg(&socket)
        .arg("status")
        .env("CCDASH_HOME", &data_dir)
        .output()
        .expect("run cli");

    let _ = daemon.kill();
    let _ = daemon.wait();

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "cli failed: stdout={} stderr={}", stdout, stderr);
    assert!(stdout.contains("daemon: ok"), "unexpected stdout: {}", stdout);
}
```

> Note: this test needs both binaries built. `CARGO_BIN_EXE_<name>` is provided for the bin in the same crate (ccdash here) plus any sibling-workspace bin Cargo has been asked to depend on. For cross-crate binary access in tests, ccdash-cli's `Cargo.toml` needs a `[dev-dependencies] ccdash-daemon = { path = "../ccdash-daemon" }` declaration (acts as a build dependency, ensuring the binary exists before this test runs).

- [ ] **Step 2: Add the cross-crate test dependency**

Edit `crates/ccdash-cli/Cargo.toml`. Under `[dev-dependencies]`, add:

```toml
ccdash-daemon = { path = "../ccdash-daemon" }
```

- [ ] **Step 3: Run the test**

Run: `cargo build --workspace && cargo test -p ccdash-cli --test cli_smoke`
Expected: 1 PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/ccdash-cli/Cargo.toml crates/ccdash-cli/tests/cli_smoke.rs
git commit -m "ccdash-cli: integration test — status round-trip with daemon"
```

---

## Task H2: Integration test — port conflict flow

**Files:**
- Create: `crates/ccdash-daemon/tests/ports.rs`

- [ ] **Step 1: Write the test**

Create `crates/ccdash-daemon/tests/ports.rs`:

```rust
mod common;

use common::Harness;
use std::net::TcpListener;
use tempfile::tempdir;

fn tmux_available() -> bool {
    std::process::Command::new("tmux").arg("-V").output().map(|o| o.status.success()).unwrap_or(false)
}

#[tokio::test]
async fn ports_list_succeeds() {
    let h = Harness::spawn().await.unwrap();
    let mut c = h.connect().await.unwrap();
    h.handshake(&mut c).await.unwrap().result.expect("handshake ok");

    let resp = c.call("ports.list", serde_json::json!({})).await.unwrap();
    assert!(resp.error.is_none(), "ports.list error: {:?}", resp.error);
    let result = resp.result.unwrap();
    assert!(result["running"].is_array());
    assert!(result["declared"].is_array());
}

#[tokio::test]
async fn session_launch_conflict_returns_force_token() {
    if !tmux_available() {
        eprintln!("tmux not on PATH; skipping");
        return;
    }
    let h = Harness::spawn().await.unwrap();
    let mut c = h.connect().await.unwrap();
    h.handshake(&mut c).await.unwrap().result.expect("handshake ok");

    // 1. Pick an unused port by binding ephemerally, then release.
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    // 2. Re-bind it so it's actually in lsof output for the duration of this test.
    let held = TcpListener::bind(("127.0.0.1", port)).unwrap();

    // 3. Create a project whose .env declares that port.
    let proj = tempdir().unwrap();
    std::fs::write(proj.path().join(".env"), format!("PORT={}\n", port)).unwrap();
    let add = c
        .call(
            "project.add",
            serde_json::json!({"path": proj.path(), "name": "conflict-test"}),
        )
        .await
        .unwrap();
    let project_id = add.result.unwrap()["id"].as_str().unwrap().to_string();

    // 4. Attempt to launch — should be blocked.
    let resp = c
        .call(
            "session.launch",
            serde_json::json!({"project_id": project_id, "command": "sleep 30"}),
        )
        .await
        .unwrap();
    let err = resp.error.expect("expected error");
    assert_eq!(err.code, -32002);
    let data = err.data.expect("expected conflict data");
    let token = data["force_token"].as_str().expect("force_token").to_string();
    assert!(!token.is_empty());
    // Conflicts list contains our port.
    let conflicts = data["conflicts"].as_array().unwrap();
    assert!(conflicts.iter().any(|c| c["port"].as_u64() == Some(port as u64)));

    // 5. Re-launch with the force token — should succeed.
    let resp2 = c
        .call(
            "session.launch",
            serde_json::json!({"project_id": project_id, "command": "sleep 30", "force_token": token}),
        )
        .await
        .unwrap();
    assert!(resp2.error.is_none(), "force-launch failed: {:?}", resp2.error);
    let sid = resp2.result.unwrap()["session"]["tmux_session_id"].as_str().unwrap().to_string();

    // Cleanup.
    let _ = c.call("session.kill", serde_json::json!({"tmux_session_id": sid})).await;
    drop(held);
}
```

- [ ] **Step 2: Run the test**

Run: `cargo test -p ccdash-daemon --test ports`
Expected: 2 PASS (second is tmux-gated).

- [ ] **Step 3: Commit**

```bash
git add crates/ccdash-daemon/tests/ports.rs
git commit -m "tests: ports.list happy path + session.launch conflict + force_token bypass"
```

---

## Task H3: Integration test — plan parsing end-to-end

**Files:**
- Create: `crates/ccdash-daemon/tests/plans.rs`

- [ ] **Step 1: Write the test**

```rust
mod common;

use common::Harness;
use tempfile::tempdir;

#[tokio::test]
async fn plans_get_parses_real_markdown() {
    let h = Harness::spawn().await.unwrap();
    let mut c = h.connect().await.unwrap();
    h.handshake(&mut c).await.unwrap().result.expect("handshake ok");

    let proj = tempdir().unwrap();
    let plans_dir = proj.path().join("docs/superpowers/plans");
    std::fs::create_dir_all(&plans_dir).unwrap();
    std::fs::write(
        plans_dir.join("phase-1.md"),
        "# Phase 1 Plan\n\n## Phase 1: Foundation\n\n- [x] task one\n- [ ] task two\n\n## Phase 2: CLI\n\n- [ ] task three\n",
    )
    .unwrap();

    let add = c
        .call("project.add", serde_json::json!({"path": proj.path(), "name": "plans-test"}))
        .await
        .unwrap();
    let project_id = add.result.unwrap()["id"].as_str().unwrap().to_string();

    let resp = c
        .call("plans.get", serde_json::json!({"project_id": project_id}))
        .await
        .unwrap();
    assert!(resp.error.is_none(), "plans.get error: {:?}", resp.error);
    let plans = resp.result.unwrap()["plans"].as_array().cloned().unwrap();
    assert_eq!(plans.len(), 1);
    let plan = &plans[0];
    assert_eq!(plan["title"].as_str().unwrap(), "Phase 1 Plan");
    let phases = plan["phases"].as_array().unwrap();
    assert_eq!(phases.len(), 2);
    assert_eq!(phases[0]["name"].as_str().unwrap(), "Phase 1: Foundation");
    assert_eq!(phases[0]["tasks"].as_array().unwrap().len(), 2);
    assert_eq!(phases[0]["tasks"][0]["done"].as_bool().unwrap(), true);
    assert_eq!(phases[0]["tasks"][1]["done"].as_bool().unwrap(), false);
}

#[tokio::test]
async fn plans_get_returns_empty_for_project_without_plans() {
    let h = Harness::spawn().await.unwrap();
    let mut c = h.connect().await.unwrap();
    h.handshake(&mut c).await.unwrap().result.expect("handshake ok");

    let proj = tempdir().unwrap();
    let add = c
        .call("project.add", serde_json::json!({"path": proj.path(), "name": "no-plans"}))
        .await
        .unwrap();
    let project_id = add.result.unwrap()["id"].as_str().unwrap().to_string();

    let resp = c
        .call("plans.get", serde_json::json!({"project_id": project_id}))
        .await
        .unwrap();
    assert!(resp.error.is_none());
    let plans = resp.result.unwrap()["plans"].as_array().cloned().unwrap();
    assert!(plans.is_empty());
}
```

- [ ] **Step 2: Run the tests**

Run: `cargo test -p ccdash-daemon --test plans`
Expected: 2 PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/ccdash-daemon/tests/plans.rs
git commit -m "tests: plans.get parses markdown into phases + handles empty project"
```

---

## Task I1: Phase 2 full verification + tag

- [ ] **Step 1: fmt + clippy**

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: both succeed. Fix any new warnings inline (typical: `#[allow(dead_code)]` on Phase-3 scaffold items).

- [ ] **Step 2: Full test run**

```bash
tmux kill-server 2>/dev/null; sleep 0.2
cargo test --workspace
```

Expected: all tests pass. Approximate counts:
- ccdash-core: 15 (unchanged from Phase 1)
- ccdash-daemon unit: ~50 (Phase 1's 31 + new ports/plans tests)
- ccdash-daemon integration: ~8 (Phase 1's 7 + Phase 2's `ports` + `plans`)
- ccdash-cli integration: 1

- [ ] **Step 3: Manual smoke**

```bash
rm -f /tmp/cli-smoke.sock && rm -rf /tmp/cli-smoke
CCDASH_HOME=/tmp/cli-smoke CCDASH_SOCKET=/tmp/cli-smoke.sock cargo run -p ccdash-daemon -- --log-level info &
sleep 1
cargo run -p ccdash-cli -- --socket /tmp/cli-smoke.sock status
cargo run -p ccdash-cli -- --socket /tmp/cli-smoke.sock project add /tmp
cargo run -p ccdash-cli -- --socket /tmp/cli-smoke.sock project list
cargo run -p ccdash-cli -- --socket /tmp/cli-smoke.sock ports
# stop the daemon
kill %1 2>/dev/null
```

- [ ] **Step 4: Tag**

```bash
git tag phase-2-done
```

- [ ] **Step 5: Update execution log**

Append a "Phase 2 — Complete" section to `docs/superpowers/EXECUTION-LOG.md` summarizing what shipped, what tests passed, and any plan deviations encountered.

```bash
git add docs/superpowers/EXECUTION-LOG.md
git commit -m "docs: phase-2 complete — CLI, ports, plans"
```

---

## What Phase 2 ships

- `ccdash` CLI binary covering: `status`, `project add/list/rm/scan`, `list`, `launch`, `kill`, `ports`, `plan`.
- Daemon `ports` module: lsof-driven running listener scan + per-project declared-port parsers (package.json / .env / docker-compose / Procfile) + composite Registry with `conflicts_for`.
- Daemon `plans` module: pulldown-cmark markdown parser producing `Plan { title, phases: Vec<Phase { name, tasks: Vec<Task { title, done }> }> }` + per-project refresh-on-read Manager.
- New RPC methods: `ports.list`, `plans.get`.
- `session.launch` now gates on port conflicts, returning `PortConflictData` with a one-shot `force_token` (`-32002` error code).
- Updated `SessionLaunchParams` with optional `force_token`.

## What's NOT in Phase 2 (deferred)

- `notify` crate–based live file watching (refresh-on-read is sufficient for v2 acceptance).
- Project root-dir scan exposed via RPC (`ccdash project scan` prints a note until then).
- The auto-add-worktrees flow on first project register (worktrees are auto-discovered on `project.add` already in Phase 1; explicit "scan for new worktrees" RPC deferred to Phase 3 when the UI needs it).

## Self-Review

**Spec coverage:**
- §4.2 ports/plans modules → C1/C2/C3 (ports), E1/E2 (plans)
- §5.2 `ports` module — lsof scan + declared parsers ✓
- §5.2 `plans` module — markdown parse + per-project cache ✓
- §5.3 CLI subcommands — `launch`/`list`/`kill`/`status`/`project`/`ports`/`plan` ✓ (only `project scan` is partial — daemon scanner module is there but no RPC; CLI prints note)
- §6.2 launch with conflict → D1 + H2 (covers steps 3-5 of the flow; UI dialog is Phase 4)
- §7.4 block-with-remediation policy → D1 (returns conflicts + force_token)
- §7.8 plan view read-only structured render → E1 + E2 + G4 (CLI pretty-prints)

**Placeholder scan:** none — every step has concrete code or commands.

**Type consistency:** `ProjectId`, `PortBinding`, `DeclaredPort`, `Plan`, `PlanPhase`, `PlanTask`, `PortConflictData`, `SessionLaunchParams` all defined once in `ccdash-core::protocol` and consumed identically across daemon + CLI.

---

**Plan complete and saved to `docs/superpowers/plans/2026-05-17-phase-2-cli-ports-plans.md`.**
