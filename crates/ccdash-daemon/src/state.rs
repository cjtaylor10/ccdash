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
