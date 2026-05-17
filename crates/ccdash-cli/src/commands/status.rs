use crate::commands::connect;
use anyhow::Result;
use std::path::PathBuf;

pub async fn run(socket: Option<PathBuf>) -> Result<()> {
    let mut c = connect(socket).await?;

    let projects = c.call("project.list", serde_json::json!({})).await?;
    let plist = projects
        .result
        .as_ref()
        .and_then(|r| r["projects"].as_array().cloned())
        .unwrap_or_default();

    let sessions = c.call("session.list", serde_json::json!({})).await?;
    let slist = sessions
        .result
        .as_ref()
        .and_then(|r| r["sessions"].as_array().cloned())
        .unwrap_or_default();

    println!("daemon: ok");
    println!("projects: {}", plist.len());
    println!("sessions: {}", slist.len());
    Ok(())
}
