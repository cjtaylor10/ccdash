//! Discovers git worktrees for a given project by shelling out to
//! `git worktree list --porcelain`.

use anyhow::{Context, Result};
use ccdash_core::domain::Worktree;
use std::path::{Path, PathBuf};
use tokio::process::Command;

/// Run `git worktree list --porcelain` in `project_path` and parse the output.
/// Returns at minimum the primary worktree (which is `project_path` itself).
pub async fn list(project_path: &Path) -> Result<Vec<Worktree>> {
    let output = Command::new("git")
        .args(["worktree", "list", "--porcelain"])
        .current_dir(project_path)
        .output()
        .await
        .with_context(|| format!("running git worktree list in {}", project_path.display()))?;
    if !output.status.success() {
        anyhow::bail!(
            "git worktree list failed in {}: {}",
            project_path.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let stdout = String::from_utf8(output.stdout).context("git stdout not utf8")?;
    Ok(parse(&stdout, project_path))
}

fn parse(porcelain: &str, project_path: &Path) -> Vec<Worktree> {
    let mut out = Vec::new();
    let mut current_path: Option<PathBuf> = None;
    let mut current_branch = String::from("(detached)");

    let flush = |path: Option<PathBuf>, branch: &str, out: &mut Vec<Worktree>| {
        if let Some(p) = path {
            let is_primary = p == project_path;
            out.push(Worktree {
                path: p,
                branch: branch.to_string(),
                is_primary,
            });
        }
    };

    for line in porcelain.lines() {
        if line.is_empty() {
            flush(current_path.take(), &current_branch, &mut out);
            current_branch = String::from("(detached)");
            continue;
        }
        if let Some(rest) = line.strip_prefix("worktree ") {
            current_path = Some(PathBuf::from(rest));
        } else if let Some(rest) = line.strip_prefix("branch ") {
            // Format: refs/heads/<name>
            current_branch = rest.strip_prefix("refs/heads/").unwrap_or(rest).to_string();
        } else if line == "detached" {
            current_branch = "(detached)".to_string();
        }
    }
    // Trailing record (porcelain ends with blank line normally but be defensive)
    flush(current_path, &current_branch, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_single_main_worktree() {
        let input = "worktree /home/u/proj\nHEAD abc\nbranch refs/heads/main\n\n";
        let parsed = parse(input, Path::new("/home/u/proj"));
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].path, PathBuf::from("/home/u/proj"));
        assert_eq!(parsed[0].branch, "main");
        assert!(parsed[0].is_primary);
    }

    #[test]
    fn parse_main_plus_linked() {
        let input = "worktree /home/u/proj\nHEAD abc\nbranch refs/heads/main\n\nworktree /home/u/proj-wt\nHEAD def\nbranch refs/heads/feature\n\n";
        let parsed = parse(input, Path::new("/home/u/proj"));
        assert_eq!(parsed.len(), 2);
        assert!(parsed[0].is_primary);
        assert!(!parsed[1].is_primary);
        assert_eq!(parsed[1].branch, "feature");
    }

    #[test]
    fn parse_detached_head() {
        let input = "worktree /home/u/proj\nHEAD abc\ndetached\n\n";
        let parsed = parse(input, Path::new("/home/u/proj"));
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].branch, "(detached)");
    }

    #[test]
    fn parse_empty_input_yields_empty() {
        let parsed = parse("", Path::new("/home/u/proj"));
        assert!(parsed.is_empty());
    }
}
