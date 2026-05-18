//! Tmux interaction by shelling out to the `tmux` binary.
//! Phase 1 uses 2s polling; control-mode deferred to Phase 2.

use anyhow::{Context, Result};
use std::path::Path;
use tokio::process::Command;

/// One tmux pane row, parsed from `tmux list-panes -a -F ...`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneRow {
    pub session_id: String,   // e.g. "$3"
    pub session_name: String, // e.g. "ccdash:lp:main"
    pub pane_pid: i32,
    pub pane_cmd: String,
    pub cwd: String,
    /// True iff the pane's child process has exited and remain-on-exit is
    /// keeping the pane alive. From tmux's `#{pane_dead}` format.
    pub pane_dead: bool,
}

/// True iff `tmux` is on PATH and a server is reachable. Tries `tmux -V` first
/// (always succeeds if installed), then `tmux ls` (succeeds only if a server is running).
#[allow(dead_code)] // wired into daemon.health RPC in Phase 2
pub async fn check_installed() -> bool {
    Command::new("tmux")
        .arg("-V")
        .output()
        .await
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// List all panes across all sessions, returning a tuple per pane.
pub async fn list_panes() -> Result<Vec<PaneRow>> {
    let fmt = "#{session_id}\t#{session_name}\t#{pane_pid}\t#{pane_current_command}\t#{pane_current_path}\t#{pane_dead}";
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
            let mut it = line.splitn(6, '\t');
            let session_id = it.next()?.to_string();
            let session_name = it.next()?.to_string();
            let pane_pid: i32 = it.next()?.parse().ok()?;
            let pane_cmd = it.next()?.to_string();
            let cwd = it.next()?.to_string();
            // pane_dead may be missing on older tmux builds; default to false.
            let pane_dead = it.next().is_some_and(|s| s == "1");
            Some(PaneRow {
                session_id,
                session_name,
                pane_pid,
                pane_cmd,
                cwd,
                pane_dead,
            })
        })
        .collect()
}

/// Wrap `command` so it runs through the user's login shell. This loads
/// the user's PATH and shell init (~/.zshrc, ~/.bashrc, NVM, pyenv,
/// pipx, ~/.local/bin, etc.) — without this, the daemon's minimal env
/// can't find user-installed binaries like `claude`, `pnpm`, or anything
/// in `~/.local/bin`.
fn wrap_in_login_shell(command: &str) -> (String, String) {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());
    // Use exec so the user's command BECOMES the shell process — that way
    // tmux's pane_current_command shows the actual command (e.g. "claude")
    // not the wrapper shell.
    let wrapped = format!("exec {}", command);
    (shell, wrapped)
}

/// Launch a new detached tmux session running `command` in `cwd` with name
/// `requested_name`. The command runs inside the user's login shell so the
/// daemon's stripped environment doesn't hide user-installed binaries. If a
/// tmux session with `requested_name` already exists, retries with
/// `requested_name_2`, `_3`, … up to `_99`. Sets `remain-on-exit on` so the
/// pane survives when `command` exits. Returns `(session_id, actual_name)`.
pub async fn new_session(
    requested_name: &str,
    cwd: &Path,
    command: &str,
) -> Result<(String, String)> {
    let (shell, wrapped_cmd) = wrap_in_login_shell(command);
    for suffix in 0..100 {
        let name = if suffix == 0 {
            requested_name.to_string()
        } else {
            format!("{}_{}", requested_name, suffix + 1)
        };
        let output = Command::new("tmux")
            .args([
                "new-session",
                "-d",
                "-s",
                &name,
                "-c",
                &cwd.to_string_lossy(),
                &shell,
                "-l",
                "-c",
                &wrapped_cmd,
            ])
            .output()
            .await
            .context("running tmux new-session")?;
        if output.status.success() {
            return finalize_new_session(&name).await;
        }
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("duplicate session") {
            // try the next suffix
            continue;
        }
        anyhow::bail!(
            "tmux new-session failed (status {:?}): {}",
            output.status.code(),
            stderr.trim()
        );
    }
    anyhow::bail!(
        "tmux new-session failed: 100 collisions on prefix {}",
        requested_name
    )
}

async fn finalize_new_session(name: &str) -> Result<(String, String)> {
    // Look up the stable session_id (e.g. "$3") via the session_name we just created.
    let output = Command::new("tmux")
        .args(["display-message", "-p", "-t", name, "#{session_id}"])
        .output()
        .await
        .context("running tmux display-message")?;
    if !output.status.success() {
        anyhow::bail!(
            "could not resolve session_id for {}: {}",
            name,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let session_id = String::from_utf8_lossy(&output.stdout).trim().to_string();

    // Configure remain-on-exit via the stable session_id (no colon-target ambiguity).
    let _ = Command::new("tmux")
        .args(["set-option", "-t", &session_id, "remain-on-exit", "on"])
        .output()
        .await;

    Ok((session_id, name.to_string()))
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
        let input =
            "$0\tccdash:a:main\t1234\tclaude\t/home/u/a\t0\n$1\tccdash:b:main\t5678\tzsh\t/home/u/b\t1\n";
        let parsed = parse_panes(input);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].session_id, "$0");
        assert_eq!(parsed[0].pane_pid, 1234);
        assert_eq!(parsed[0].pane_cmd, "claude");
        assert!(!parsed[0].pane_dead);
        assert_eq!(parsed[1].cwd, "/home/u/b");
        assert!(parsed[1].pane_dead);
    }

    #[test]
    fn parse_panes_legacy_rows_without_pane_dead() {
        // Older tmux output (no pane_dead field) must still parse cleanly.
        let input = "$0\tccdash:a:main\t1234\tclaude\t/home/u/a\n";
        let parsed = parse_panes(input);
        assert_eq!(parsed.len(), 1);
        assert!(!parsed[0].pane_dead);
    }

    #[test]
    fn parse_panes_skips_malformed() {
        let input = "garbage line\n$0\tn\t1\tc\t/\t0\n";
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
        let (id, _name) = new_session(&name, &cwd, "sleep 30").await.unwrap();
        assert!(id.starts_with('$'));
        let panes = list_panes().await.unwrap();
        assert!(panes.iter().any(|p| p.session_id == id));
        kill_session(&id).await.unwrap();
    }
}
