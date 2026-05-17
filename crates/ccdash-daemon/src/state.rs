//! Composite daemon state. Cheap to clone (Arcs all the way down).

use crate::broadcast::Bus;
use crate::ports::Registry as PortsRegistry;
use crate::projects::Registry as ProjectsRegistry;
use crate::sessions::Manager;
use anyhow::Result;
use ccdash_core::paths;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct AppState {
    pub projects: Arc<ProjectsRegistry>,
    pub sessions: Arc<Manager>,
    pub ports: Arc<PortsRegistry>,
    pub plans: Arc<crate::plans::Manager>,
    pub bus: Bus,
    pub auth_token: Arc<String>,
    #[allow(dead_code)] // read by file watchers + ports module in Phase 2+
    pub data_dir: PathBuf,
    /// One-shot tokens issued in `PortConflictData`. A token in this set lets the
    /// next `session.launch` bypass conflict gating.
    pub conflict_tokens: Arc<Mutex<HashSet<String>>>,
    /// True iff projects.toml didn't exist when the daemon started. Cleared
    /// when the UI calls `daemon.first_run_complete` after the welcome flow.
    pub first_run_pending: Arc<AtomicBool>,
}

impl AppState {
    pub async fn bootstrap(data_dir: PathBuf) -> Result<Self> {
        let token = ccdash_core::auth::ensure_token(&data_dir.join("auth"))?;
        let projects = Arc::new(ProjectsRegistry::load(data_dir.join("projects.toml")).await?);
        let first_run = projects.was_new_on_disk();
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
            first_run_pending: Arc::new(AtomicBool::new(first_run)),
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

    #[tokio::test]
    async fn first_run_pending_when_no_projects_toml() {
        let dir = tempdir().unwrap();
        let state = AppState::bootstrap(dir.path().to_path_buf()).await.unwrap();
        assert!(state
            .first_run_pending
            .load(std::sync::atomic::Ordering::Relaxed));
    }

    #[tokio::test]
    async fn first_run_not_pending_when_projects_toml_exists() {
        let dir = tempdir().unwrap();
        // Touch an empty projects.toml so the registry treats it as existing.
        std::fs::write(dir.path().join("projects.toml"), "[projects]\n").unwrap();
        let state = AppState::bootstrap(dir.path().to_path_buf()).await.unwrap();
        assert!(!state
            .first_run_pending
            .load(std::sync::atomic::Ordering::Relaxed));
    }
}
