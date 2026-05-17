use crate::commands::connect;
use anyhow::Result;
use clap::Subcommand;
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum Sub {
    /// Add a project by path.
    Add {
        path: PathBuf,
        #[arg(long)]
        name: Option<String>,
    },
    /// Remove a project by id.
    Rm { id: String },
    /// List projects.
    List,
    /// Scan a root directory for git repos (not yet wired via RPC).
    Scan {
        #[arg(long)]
        root: Option<PathBuf>,
    },
}

pub async fn run(socket: Option<PathBuf>, sub: Sub) -> Result<()> {
    let mut c = connect(socket).await?;
    match sub {
        Sub::Add { path, name } => {
            let resp = c
                .call(
                    "project.add",
                    serde_json::json!({"path": path, "name": name}),
                )
                .await?;
            if let Some(err) = resp.error {
                anyhow::bail!("project.add: {}", err.message);
            }
            let p = resp.result.unwrap();
            println!(
                "added: {}  ({})",
                p["name"].as_str().unwrap_or("?"),
                p["id"].as_str().unwrap_or("?")
            );
        }
        Sub::Rm { id } => {
            let resp = c
                .call("project.remove", serde_json::json!({"id": id}))
                .await?;
            if let Some(err) = resp.error {
                anyhow::bail!("project.remove: {}", err.message);
            }
            println!("removed: {}", id);
        }
        Sub::List => {
            let resp = c.call("project.list", serde_json::json!({})).await?;
            let projects = resp
                .result
                .unwrap()["projects"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            for p in projects {
                println!(
                    "{}  {}  {}",
                    p["id"].as_str().unwrap_or("?"),
                    p["name"].as_str().unwrap_or("?"),
                    p["path"].as_str().unwrap_or("?")
                );
            }
        }
        Sub::Scan { root: _ } => {
            println!("scan: not yet wired (Phase 2 daemon ships scanner module but no RPC). Use `project add <path>` for now.");
        }
    }
    Ok(())
}
