//! Per-project plan-file cache with refresh-on-read semantics.
//!
//! Phase 2: refresh-on-read. The `notify` upgrade for live watching is deferred
//! to a later phase — refresh-on-read is sufficient because `plans.get` is
//! issued by client UIs on demand, not as a continuous stream.

use crate::plans::parser;
use anyhow::Result;
use ccdash_core::domain::ProjectId;
use ccdash_core::protocol::Plan;
use std::collections::HashMap;
use std::path::Path;
use tokio::fs;
use tokio::sync::RwLock;

pub struct Manager {
    cache: RwLock<HashMap<ProjectId, Vec<Plan>>>,
}

impl Manager {
    pub fn new() -> Self {
        Self {
            cache: RwLock::new(HashMap::new()),
        }
    }

    /// Re-scan the plan/spec markdown files for one project and cache the result.
    pub async fn refresh(&self, project_id: &ProjectId, project_root: &Path) -> Result<Vec<Plan>> {
        let mut plans = Vec::new();
        for sub in ["docs/superpowers/specs", "docs/superpowers/plans"] {
            let dir = project_root.join(sub);
            plans.extend(scan_dir(&dir).await);
        }
        plans.sort_by(|a, b| a.path.cmp(&b.path));
        self.cache
            .write()
            .await
            .insert(project_id.clone(), plans.clone());
        Ok(plans)
    }

    #[allow(dead_code)]
    pub async fn get(&self, project_id: &ProjectId) -> Option<Vec<Plan>> {
        self.cache.read().await.get(project_id).cloned()
    }
}

impl Default for Manager {
    fn default() -> Self {
        Self::new()
    }
}

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
        std::fs::write(
            nested.join("plan.md"),
            "# Nested Plan\n## Phase 1: x\n- [ ] t\n",
        )
        .unwrap();
        let mgr = Manager::new();
        let pid = ProjectId("p1".into());
        let found = mgr.refresh(&pid, dir.path()).await.unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].title, "Nested Plan");
    }
}
