use crate::commands::{connect, resolve_project_id};
use anyhow::Result;
use std::path::PathBuf;

pub async fn run(socket: Option<PathBuf>, project_filter: Option<String>) -> Result<()> {
    let mut c = connect(socket).await?;
    let resp = c.call("ports.list", serde_json::json!({})).await?;
    if let Some(err) = resp.error {
        anyhow::bail!("ports.list: {}", err.message);
    }
    let result = resp.result.unwrap();
    let running = result["running"].as_array().cloned().unwrap_or_default();
    let declared = result["declared"].as_array().cloned().unwrap_or_default();

    let filter_id: Option<String> = if let Some(p) = project_filter {
        Some(resolve_project_id(&mut c, &p).await?)
    } else {
        None
    };

    println!("RUNNING:");
    for p in &running {
        if let Some(ref fid) = filter_id {
            if p["project_id"].as_str() != Some(fid.as_str()) {
                continue;
            }
        }
        println!(
            "  {} (pid {}, {}) project={}",
            p["port"].as_u64().unwrap_or(0),
            p["pid"].as_i64().unwrap_or(-1),
            p["command"].as_str().unwrap_or("?"),
            p["project_id"].as_str().unwrap_or("-")
        );
    }
    println!("DECLARED:");
    for p in &declared {
        if let Some(ref fid) = filter_id {
            if p["project_id"].as_str() != Some(fid.as_str()) {
                continue;
            }
        }
        println!(
            "  {} project={} source={}",
            p["port"].as_u64().unwrap_or(0),
            p["project_id"].as_str().unwrap_or("?"),
            p["source"].as_str().unwrap_or("?")
        );
    }
    Ok(())
}
