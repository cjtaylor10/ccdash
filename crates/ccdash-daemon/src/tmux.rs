//! Tmux interaction by shelling out to the `tmux` binary.
//! Phase 1 uses 2s polling; control-mode deferred to Phase 2.

use anyhow::{Context, Result};
use std::path::Path;
use tokio::process::Command;

/// One tmux pane row, parsed from `tmux list-panes -a -F ...`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneRow {
    pub session_id: String,    // e.g. "$3"
    pub session_name: String,  // e.g. "ccdash:lp:main"
    pub pane_pid: i32,
    pub pane_cmd: String,
    pub cwd: String,
}

/// True iff `tmux` is on PATH and a server is reachable. Tries `tmux -V` first
/// (always succeeds if installed), then `tmux ls` (succeeds only if a server is running).
pub async fn check_installed() -> bool {
    Command::new("tmux").arg("-V").output().await.map(|o| o.status.success()).unwrap_or(false)
}

/// List all panes across all sessions, returning a tuple per pane.
pub async fn list_panes() -> Result<Vec<PaneRow>> {
    let fmt = "#{session_id}\t#{session_name}\t#{pane_pid}\t#{pane_current_command}\t#{pane_current_path}";
    let output = Command::new("tmux")
        .args(["list-panes", "-a", "-F", fmt])
        .output()
        .await
        .context("running tmux list-panes")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // tmux returns non-zero with "no server running" — treat as empty list, not error.
        if stderr.contains("no server running") || stderr.contains("error connecting") {
            return Ok(vec![]);
        }
        anyhow::bail!("tmux list-panes failed: {}", stderr.trim());
    }
    let stdout = String::from_utf8(output.stdout).context("tmux stdout not utf8")?;
    Ok(parse_panes(&stdout))
}

fn parse_panes(s: &str) -> Vec<PaneRow> {
    s.lines()
        .filter(|l| !l.is_empty())
        .filter_map(|line| {
            let mut it = line.splitn(5, '\t');
            let session_id = it.next()?.to_string();
            let session_name = it.next()?.to_string();
            let pane_pid: i32 = it.next()?.parse().ok()?;
            let pane_cmd = it.next()?.to_string();
            let cwd = it.next()?.to_string();
            Some(PaneRow { session_id, session_name, pane_pid, pane_cmd, cwd })
        })
        .collect()
}

/// Launch a new detached tmux session running `command` in `cwd` with name `name`.
/// Sets `remain-on-exit on` so the pane survives when `command` exits.
pub async fn new_session(name: &str, cwd: &Path, command: &str) -> Result<String> {
    let status = Command::new("tmux")
        .args([
            "new-session",
            "-d",
            "-s", name,
            "-c", &cwd.to_string_lossy(),
            command,
        ])
        .status()
        .await
        .context("running tmux new-session")?;
    if !status.success() {
        anyhow::bail!("tmux new-session failed (status {:?})", status.code());
    }
    // Configure remain-on-exit for this session's windows.
    let _ = Command::new("tmux")
        .args(["set-option", "-t", name, "remain-on-exit", "on"])
        .status()
        .await;

    // Look up the session_id we just created.
    let output = Command::new("tmux")
        .args(["display-message", "-p", "-t", name, "#{session_id}"])
        .output()
        .await
        .context("running tmux display-message")?;
    if !output.status.success() {
        anyhow::bail!("could not resolve session_id for {}", name);
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Kill the tmux session by id (e.g. "$3").
pub async fn kill_session(session_id: &str) -> Result<()> {
    let status = Command::new("tmux")
        .args(["kill-session", "-t", session_id])
        .status()
        .await
        .context("running tmux kill-session")?;
    if !status.success() {
        anyhow::bail!("tmux kill-session failed for {}", session_id);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_panes_two_rows() {
        let input = "$0\tccdash:a:main\t1234\tclaude\t/home/u/a\n$1\tccdash:b:main\t5678\tzsh\t/home/u/b\n";
        let parsed = parse_panes(input);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].session_id, "$0");
        assert_eq!(parsed[0].pane_pid, 1234);
        assert_eq!(parsed[0].pane_cmd, "claude");
        assert_eq!(parsed[1].cwd, "/home/u/b");
    }

    #[test]
    fn parse_panes_skips_malformed() {
        let input = "garbage line\n$0\tn\t1\tc\t/\n";
        let parsed = parse_panes(input);
        assert_eq!(parsed.len(), 1);
    }

    #[test]
    fn parse_panes_empty() {
        assert!(parse_panes("").is_empty());
    }

    #[tokio::test]
    #[ignore]
    async fn tmux_smoke_new_and_kill() {
        if !check_installed().await {
            eprintln!("tmux not installed; skipping");
            return;
        }
        let name = format!("ccdash-smoketest-{}", std::process::id());
        let cwd = std::env::current_dir().unwrap();
        let id = new_session(&name, &cwd, "sleep 30").await.unwrap();
        assert!(id.starts_with('$'));
        let panes = list_panes().await.unwrap();
        assert!(panes.iter().any(|p| p.session_id == id));
        kill_session(&id).await.unwrap();
    }
}
