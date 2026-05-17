//! Session manager — joins live tmux state with `sessions.toml` metadata.
//! Keyed on tmux's stable `session_id` (e.g. "$3"). Sessions are considered
//! visible iff they have a pane running `claude`.

use crate::tmux::{self, PaneRow};
use anyhow::{Context, Result};
use ccdash_core::domain::{ProjectId, Session, SessionState};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;
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
            Err(e) => {
                return Err(anyhow::Error::new(e).context(format!("reading {}", file.display())))
            }
        };
        Ok(Self {
            file,
            meta: RwLock::new(meta),
            cache: RwLock::new(BTreeMap::new()),
        })
    }

    async fn write(&self) -> Result<()> {
        let meta = self.meta.read().await;
        let disk = OnDisk {
            sessions: meta.clone(),
        };
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

    #[allow(dead_code)] // wired into RPC + polling loop in Phase 2
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
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn load_missing_file_returns_empty() {
        let dir = tempdir().unwrap();
        let m = Manager::load(dir.path().join("sessions.toml"))
            .await
            .unwrap();
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
        meta.insert(
            "$3".into(),
            SessionMeta {
                project_id: Some(ProjectId("pid".into())),
                worktree: Some("main".into()),
                first_seen: 1_700_000_000,
            },
        );
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
