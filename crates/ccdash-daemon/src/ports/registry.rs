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
    pub async fn conflicts_for(
        &self,
        project_id: &ccdash_core::domain::ProjectId,
    ) -> Vec<(u16, PortBinding)> {
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
        let projects = Arc::new(
            ProjectsRegistry::load(tempdir().unwrap().path().join("p.toml"))
                .await
                .unwrap(),
        );
        let reg = Registry::new(projects);
        assert!(reg.running().await.is_empty());
        assert!(reg.declared().await.is_empty());
    }

    #[tokio::test]
    async fn conflicts_for_returns_running_holders() {
        let dir = tempdir().unwrap();
        let projects = Arc::new(ProjectsRegistry::load(dir.path().join("p.toml")).await.unwrap());
        let reg = Registry::new(projects);

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
            project_id: Some(pid.clone()),
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
