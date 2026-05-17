//! Per-project declared-port parsers.
//!
//! Sources scanned (each independently — missing files are non-fatal):
//! - `package.json`         — looks at `scripts.*` values for `PORT=NN`/`--port NN`
//! - `.env` / `.env.local`  — looks for `PORT=NN`, `VITE_PORT=NN`, etc.
//! - `docker-compose.yml`   — looks at `ports:` blocks for `"NN:..."` mappings
//! - `Procfile`             — looks at the `web` line for `PORT=NN` envs

use ccdash_core::domain::ProjectId;
use ccdash_core::protocol::DeclaredPort;
use regex::Regex;
use std::path::Path;
use std::sync::LazyLock;
use tokio::fs;

static PORT_EQ_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\bPORT=(\d{2,5})\b").unwrap());
static PORT_FLAG_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"--port[\s=](\d{2,5})\b").unwrap());
static ENV_PORT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*[A-Z_]*PORT[A-Z_]*\s*=\s*(\d{2,5})\s*$").unwrap());
static COMPOSE_PORT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?:^|\s|"|')(\d{2,5})\s*:\s*\d{2,5}"#).unwrap());

pub async fn scan(project_id: &ProjectId, project_root: &Path) -> Vec<DeclaredPort> {
    let mut out = Vec::new();
    out.extend(scan_package_json(project_id, project_root).await);
    out.extend(scan_env_files(project_id, project_root).await);
    out.extend(scan_docker_compose(project_id, project_root).await);
    out.extend(scan_procfile(project_id, project_root).await);
    out.sort_by_key(|p| (p.port, p.source.clone()));
    out.dedup_by(|a, b| a.port == b.port && a.source == b.source);
    out
}

async fn scan_package_json(project_id: &ProjectId, root: &Path) -> Vec<DeclaredPort> {
    let path = root.join("package.json");
    let s = match fs::read_to_string(&path).await {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    let v: serde_json::Value = match serde_json::from_str(&s) {
        Ok(v) => v,
        Err(_) => return vec![],
    };
    let scripts = match v.get("scripts").and_then(|s| s.as_object()) {
        Some(o) => o,
        None => return vec![],
    };
    let mut out = Vec::new();
    for (_name, val) in scripts {
        if let Some(s) = val.as_str() {
            for cap in PORT_EQ_RE
                .captures_iter(s)
                .chain(PORT_FLAG_RE.captures_iter(s))
            {
                if let Ok(port) = cap[1].parse::<u16>() {
                    out.push(DeclaredPort {
                        project_id: project_id.clone(),
                        port,
                        source: "package.json".into(),
                    });
                }
            }
        }
    }
    out
}

async fn scan_env_files(project_id: &ProjectId, root: &Path) -> Vec<DeclaredPort> {
    let mut out = Vec::new();
    for name in [".env", ".env.local", ".env.development"] {
        let path = root.join(name);
        let s = match fs::read_to_string(&path).await {
            Ok(s) => s,
            Err(_) => continue,
        };
        for line in s.lines() {
            if let Some(cap) = ENV_PORT_RE.captures(line) {
                if let Ok(port) = cap[1].parse::<u16>() {
                    out.push(DeclaredPort {
                        project_id: project_id.clone(),
                        port,
                        source: name.into(),
                    });
                }
            }
        }
    }
    out
}

async fn scan_docker_compose(project_id: &ProjectId, root: &Path) -> Vec<DeclaredPort> {
    let mut out = Vec::new();
    for name in [
        "docker-compose.yml",
        "docker-compose.yaml",
        "compose.yml",
        "compose.yaml",
    ] {
        let path = root.join(name);
        let s = match fs::read_to_string(&path).await {
            Ok(s) => s,
            Err(_) => continue,
        };
        let mut in_ports = false;
        for line in s.lines() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("ports:") {
                in_ports = true;
                continue;
            }
            if in_ports {
                if !trimmed.starts_with('-') && !trimmed.is_empty() && !trimmed.starts_with('#') {
                    in_ports = false;
                    continue;
                }
                for cap in COMPOSE_PORT_RE.captures_iter(line) {
                    if let Ok(port) = cap[1].parse::<u16>() {
                        out.push(DeclaredPort {
                            project_id: project_id.clone(),
                            port,
                            source: name.into(),
                        });
                    }
                }
            }
        }
    }
    out
}

async fn scan_procfile(project_id: &ProjectId, root: &Path) -> Vec<DeclaredPort> {
    let path = root.join("Procfile");
    let s = match fs::read_to_string(&path).await {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    let mut out = Vec::new();
    for line in s.lines() {
        for cap in PORT_EQ_RE.captures_iter(line) {
            if let Ok(port) = cap[1].parse::<u16>() {
                out.push(DeclaredPort {
                    project_id: project_id.clone(),
                    port,
                    source: "Procfile".into(),
                });
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn pid() -> ProjectId {
        ProjectId("p1".into())
    }

    #[tokio::test]
    async fn package_json_port_env_var() {
        let dir = tempdir().unwrap();
        std::fs::write(
            dir.path().join("package.json"),
            r#"{"scripts":{"dev":"PORT=3000 next dev"}}"#,
        )
        .unwrap();
        let ports = scan_package_json(&pid(), dir.path()).await;
        assert_eq!(ports.len(), 1);
        assert_eq!(ports[0].port, 3000);
    }

    #[tokio::test]
    async fn package_json_port_flag() {
        let dir = tempdir().unwrap();
        std::fs::write(
            dir.path().join("package.json"),
            r#"{"scripts":{"dev":"next dev --port 4000"}}"#,
        )
        .unwrap();
        let ports = scan_package_json(&pid(), dir.path()).await;
        assert_eq!(ports.len(), 1);
        assert_eq!(ports[0].port, 4000);
    }

    #[tokio::test]
    async fn env_file_port() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join(".env"), "PORT=5000\nDB_URL=foo\n").unwrap();
        let ports = scan_env_files(&pid(), dir.path()).await;
        assert_eq!(ports.len(), 1);
        assert_eq!(ports[0].port, 5000);
    }

    #[tokio::test]
    async fn env_file_namespaced_port_var() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join(".env"), "VITE_PORT=5173\n").unwrap();
        let ports = scan_env_files(&pid(), dir.path()).await;
        assert_eq!(ports.len(), 1);
        assert_eq!(ports[0].port, 5173);
    }

    #[tokio::test]
    async fn docker_compose_ports_block() {
        let dir = tempdir().unwrap();
        std::fs::write(
            dir.path().join("docker-compose.yml"),
            "services:\n  web:\n    ports:\n      - \"8080:80\"\n      - \"4443:443\"\n",
        )
        .unwrap();
        let ports = scan_docker_compose(&pid(), dir.path()).await;
        assert_eq!(ports.len(), 2);
        assert!(ports.iter().any(|p| p.port == 8080));
        assert!(ports.iter().any(|p| p.port == 4443));
    }

    #[tokio::test]
    async fn procfile_port() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("Procfile"), "web: PORT=6000 ./server\n").unwrap();
        let ports = scan_procfile(&pid(), dir.path()).await;
        assert_eq!(ports.len(), 1);
        assert_eq!(ports[0].port, 6000);
    }

    #[tokio::test]
    async fn scan_combines_sources() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join(".env"), "PORT=5000\n").unwrap();
        std::fs::write(dir.path().join("Procfile"), "web: PORT=6000 x\n").unwrap();
        let ports = scan(&pid(), dir.path()).await;
        let p: Vec<u16> = ports.iter().map(|d| d.port).collect();
        assert!(p.contains(&5000));
        assert!(p.contains(&6000));
    }

    #[tokio::test]
    async fn missing_files_are_no_op() {
        let dir = tempdir().unwrap();
        let ports = scan(&pid(), dir.path()).await;
        assert!(ports.is_empty());
    }
}
