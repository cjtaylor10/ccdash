pub mod kill;
pub mod launch;
pub mod list;
pub mod plan;
pub mod ports;
pub mod project;
pub mod status;

use anyhow::Result;
use ccdash_core::client::Client;
use ccdash_core::protocol::ClientKind;
use std::path::PathBuf;

/// Helper: connect to socket (default if None), handshake, return ready Client.
pub async fn connect(socket: Option<PathBuf>) -> Result<Client> {
    let mut c = match socket {
        Some(p) => Client::connect(&p).await?,
        None => Client::connect_default().await?,
    };
    let resp = c.handshake(ClientKind::Cli).await?;
    if let Some(e) = resp.error {
        anyhow::bail!("handshake failed: {}", e.message);
    }
    Ok(c)
}

/// Resolve a `name-or-id` string against the project list, returning the project id.
pub async fn resolve_project_id(c: &mut Client, name_or_id: &str) -> Result<String> {
    let resp = c.call("project.list", serde_json::json!({})).await?;
    let projects = resp
        .result
        .ok_or_else(|| anyhow::anyhow!("project.list returned no result"))?["projects"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("projects not an array"))?
        .clone();
    for p in &projects {
        let id = p["id"].as_str().unwrap_or("");
        let name = p["name"].as_str().unwrap_or("");
        if id == name_or_id || name == name_or_id {
            return Ok(id.to_string());
        }
    }
    anyhow::bail!("no project matches '{}'", name_or_id);
}
