//! Recursively scans configured root directories for git repos.
//! Limits depth to avoid descending into `node_modules`/build outputs.
//!
//! Phase 1 includes the implementation but no consumer; Phase 2 wires it
//! into the RPC layer for the auto-detection / scan-with-confirm flow.

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use tokio::fs;

const MAX_DEPTH: usize = 4;

/// Skip these directory names while scanning.
const SKIP_DIRS: &[&str] = &[
    "node_modules",
    "target",
    ".git",
    ".cache",
    ".venv",
    "venv",
    "dist",
    "build",
    ".next",
    ".turbo",
    "vendor",
    ".gradle",
];

/// Return the absolute paths of git-repo roots found under `roots`.
/// A directory is treated as a git-repo root iff it contains a `.git` entry
/// (file OR directory; we accept both because worktrees use a `.git` file).
pub async fn scan(roots: &[PathBuf]) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for root in roots {
        scan_dir(root, 0, &mut found).await;
    }
    found.sort();
    found.dedup();
    found
}

async fn scan_dir(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth > MAX_DEPTH {
        return;
    }

    let mut entries = match fs::read_dir(dir).await {
        Ok(e) => e,
        Err(_) => return, // permission denied, broken symlink, etc.
    };

    // Check the directory itself first.
    if has_git(dir).await {
        out.push(dir.to_path_buf());
        return; // don't descend into a known repo
    }

    let mut children = Vec::new();
    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        let name = match entry.file_name().to_str() {
            Some(s) => s.to_string(),
            None => continue,
        };
        if name.starts_with('.') && name != "." {
            // Skip hidden dirs except current — but we still descend into project dirs.
            // We do allow `.foo` if it contains a `.git` (rare). Practical: skip them.
            continue;
        }
        if SKIP_DIRS.contains(&name.as_str()) {
            continue;
        }
        match entry.file_type().await {
            Ok(ft) if ft.is_dir() => children.push(path),
            _ => continue,
        }
    }

    for child in children {
        Box::pin(scan_dir(&child, depth + 1, out)).await;
    }
}

async fn has_git(dir: &Path) -> bool {
    fs::metadata(dir.join(".git")).await.is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn finds_repo_at_depth_2() {
        let dir = tempdir().unwrap();
        let repo = dir.path().join("a").join("b");
        std::fs::create_dir_all(repo.join(".git")).unwrap();
        let found = scan(&[dir.path().to_path_buf()]).await;
        assert_eq!(found, vec![repo]);
    }

    #[tokio::test]
    async fn skips_node_modules() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("node_modules").join("pkg").join(".git")).unwrap();
        let found = scan(&[dir.path().to_path_buf()]).await;
        assert!(found.is_empty(), "should not descend into node_modules");
    }

    #[tokio::test]
    async fn does_not_descend_into_found_repo() {
        let dir = tempdir().unwrap();
        let outer = dir.path().join("outer");
        let inner = outer.join("inner");
        std::fs::create_dir_all(outer.join(".git")).unwrap();
        std::fs::create_dir_all(inner.join(".git")).unwrap();
        let found = scan(&[dir.path().to_path_buf()]).await;
        assert_eq!(found, vec![outer]);
    }

    #[tokio::test]
    async fn accepts_git_as_file_for_worktrees() {
        let dir = tempdir().unwrap();
        let worktree = dir.path().join("wt");
        std::fs::create_dir_all(&worktree).unwrap();
        std::fs::write(
            worktree.join(".git"),
            "gitdir: /some/main/.git/worktrees/wt",
        )
        .unwrap();
        let found = scan(&[dir.path().to_path_buf()]).await;
        assert_eq!(found, vec![worktree]);
    }
}
