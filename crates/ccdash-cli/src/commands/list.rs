use crate::commands::connect;
use anyhow::Result;
use std::path::PathBuf;

pub async fn run(socket: Option<PathBuf>) -> Result<()> {
    let mut c = connect(socket).await?;
    let resp = c.call("session.list", serde_json::json!({})).await?;
    if let Some(err) = resp.error {
        anyhow::bail!("session.list: {}", err.message);
    }
    let sessions = resp.result.unwrap()["sessions"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    if sessions.is_empty() {
        println!("(no sessions)");
        return Ok(());
    }
    for s in sessions {
        println!(
            "{}  {}  pid={}  cwd={}",
            s["tmux_session_id"].as_str().unwrap_or("?"),
            s["name"].as_str().unwrap_or("?"),
            s["pid"].as_i64().unwrap_or(-1),
            s["cwd"].as_str().unwrap_or("?")
        );
    }
    Ok(())
}
