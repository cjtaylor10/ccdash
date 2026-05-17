use crate::commands::connect;
use anyhow::Result;
use std::path::PathBuf;

pub async fn run(socket: Option<PathBuf>, session_id: String) -> Result<()> {
    let mut c = connect(socket).await?;
    let resp = c
        .call(
            "session.kill",
            serde_json::json!({"tmux_session_id": session_id}),
        )
        .await?;
    if let Some(err) = resp.error {
        anyhow::bail!("session.kill: {}", err.message);
    }
    println!("killed: {}", session_id);
    Ok(())
}
