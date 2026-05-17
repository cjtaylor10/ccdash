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
