//! Shell out to `lsof -nP -iTCP -sTCP:LISTEN` and parse the output.

use anyhow::{Context, Result};
use ccdash_core::protocol::PortBinding;
use tokio::process::Command;

/// Run `lsof` and return the list of TCP listeners.
pub async fn scan() -> Result<Vec<PortBinding>> {
    let output = Command::new("lsof")
        .args(["-nP", "-iTCP", "-sTCP:LISTEN", "-F", "pcnPT"])
        .output()
        .await
        .context("running lsof")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // lsof exits non-zero when no matches; treat as empty.
        if output.stdout.is_empty() {
            return Ok(vec![]);
        }
        anyhow::bail!("lsof failed: {}", stderr.trim());
    }
    let stdout = String::from_utf8(output.stdout).context("lsof stdout not utf8")?;
    Ok(parse(&stdout))
}

/// Parse `lsof -F pcnPT` "field" output format.
fn parse(s: &str) -> Vec<PortBinding> {
    let mut out = Vec::new();
    let mut current_pid: Option<i32> = None;
    let mut current_cmd: Option<String> = None;
    let mut current_proto: Option<String> = None;
    let mut current_state: Option<String> = None;

    for line in s.lines() {
        if line.is_empty() {
            continue;
        }
        let (tag, rest) = line.split_at(1);
        match tag {
            "p" => {
                current_pid = rest.parse().ok();
                current_cmd = None;
                current_proto = None;
                current_state = None;
            }
            "c" => current_cmd = Some(rest.to_string()),
            "P" => current_proto = Some(rest.to_string()),
            "T" => current_state = Some(rest.to_string()),
            "n" => {
                if let Some(port) = extract_port(rest) {
                    let is_listening = current_state
                        .as_deref()
                        .map(|s| s.contains("LISTEN") || s == "ST=LISTEN")
                        .unwrap_or(true); // lsof was called with -sTCP:LISTEN already
                    if is_listening && current_proto.as_deref().map(|p| p == "TCP").unwrap_or(true)
                    {
                        out.push(PortBinding {
                            port,
                            protocol: "tcp".into(),
                            pid: current_pid,
                            command: current_cmd.clone(),
                            project_id: None,
                        });
                    }
                }
            }
            _ => {}
        }
    }
    // Deduplicate by (port, pid).
    out.sort_by_key(|p| (p.port, p.pid));
    out.dedup_by(|a, b| a.port == b.port && a.pid == b.pid);
    out
}

fn extract_port(name: &str) -> Option<u16> {
    // Examples: "*:8080", "127.0.0.1:3000", "[::]:443"
    let after_last_colon = name.rsplit(':').next()?;
    let port_str = after_last_colon.split_whitespace().next()?;
    port_str.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_port_handles_common_formats() {
        assert_eq!(extract_port("*:8080"), Some(8080));
        assert_eq!(extract_port("127.0.0.1:3000"), Some(3000));
        assert_eq!(extract_port("[::]:443"), Some(443));
        assert_eq!(extract_port("[::1]:5432 (LISTEN)"), Some(5432));
        assert_eq!(extract_port("invalid"), None);
    }

    #[test]
    fn parse_one_listener() {
        let input = "p12345\ncnode\nPTCP\nTST=LISTEN\nn*:3000\n";
        let parsed = parse(input);
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].port, 3000);
        assert_eq!(parsed[0].pid, Some(12345));
        assert_eq!(parsed[0].command.as_deref(), Some("node"));
    }

    #[test]
    fn parse_multiple_processes() {
        let input = "p12345\ncnode\nPTCP\nn*:3000\np99999\ncpython\nPTCP\nn127.0.0.1:8000\n";
        let parsed = parse(input);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].port, 3000);
        assert_eq!(parsed[1].port, 8000);
    }

    #[test]
    fn parse_dedupes_ipv4_and_ipv6() {
        let input = "p77\ncnode\nPTCP\nn*:8080\nn[::]:8080\n";
        let parsed = parse(input);
        assert_eq!(parsed.len(), 1);
    }

    #[test]
    fn parse_empty_input() {
        assert!(parse("").is_empty());
    }
}
