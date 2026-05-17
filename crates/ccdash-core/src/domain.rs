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
}
