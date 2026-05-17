//! JSON-RPC 2.0 protocol types for the ccdash daemon socket.
//!
//! Wire format: each request and response is a single JSON object, one per line
//! (LF-terminated). This lets clients debug with `socat - UNIX-CONNECT:$SOCK`.

use crate::domain::{Project, ProjectId, Session};
use serde::{Deserialize, Serialize};

/// Top-level JSON-RPC 2.0 request envelope.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct Request {
    pub jsonrpc: String,       // always "2.0"
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
        Self {
            jsonrpc: "2.0".into(),
            id,
            result: Some(result),
            error: None,
        }
    }
    pub fn err(id: serde_json::Value, error: RpcError) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id,
            result: None,
            error: Some(error),
        }
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
        Self {
            jsonrpc: "2.0".into(),
            method: method.into(),
            params,
        }
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
pub struct ProjectReorderParams {
    pub ids: Vec<ProjectId>,
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
    /// One-shot token returned in a prior `PortConflictData`. When present,
    /// skips conflict gating for this launch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub force_token: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct SessionLaunchResult {
    pub session: Session,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct SessionKillParams {
    pub tmux_session_id: String,
}

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

// === First-run / onboarding ===

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct FirstRunStatusResult {
    pub pending: bool,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct ScanPathsParams {
    pub roots: Vec<std::path::PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DiscoveredRepo {
    pub path: std::path::PathBuf,
    pub suggested_name: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct ScanPathsResult {
    pub discovered: Vec<DiscoveredRepo>,
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
        assert!(
            !s.contains("\"error\""),
            "ok response should not serialize null error"
        );
    }

    #[test]
    fn response_err_omits_result_field() {
        let r = Response::err(
            serde_json::json!(1),
            RpcError {
                code: -32000,
                message: "fail".into(),
                data: None,
            },
        );
        let s = serde_json::to_string(&r).unwrap();
        assert!(
            !s.contains("\"result\""),
            "err response should not serialize null result"
        );
    }

    #[test]
    fn topic_serializes_lowercase() {
        let t = Topic::Sessions;
        assert_eq!(serde_json::to_string(&t).unwrap(), "\"sessions\"");
    }

    #[test]
    fn handshake_params_roundtrip() {
        let p = HandshakeParams {
            token: "deadbeef".into(),
            client: ClientKind::Ui,
        };
        let s = serde_json::to_string(&p).unwrap();
        let back: HandshakeParams = serde_json::from_str(&s).unwrap();
        assert_eq!(p, back);
    }
}
