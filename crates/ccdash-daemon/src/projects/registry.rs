//! Persistent project registry backed by `projects.toml`.

use anyhow::{Context, Result};
use ccdash_core::domain::{Project, ProjectId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;
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
