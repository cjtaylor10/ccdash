use crate::commands::{connect, resolve_project_id};
use anyhow::Result;
use std::path::PathBuf;

pub async fn run(
    socket: Option<PathBuf>,
    project: String,
    worktree: Option<String>,
    command: Option<String>,
    force_token: Option<String>,
) -> Result<()> {
    let mut c = connect(socket).await?;
    let project_id = resolve_project_id(&mut c, &project).await?;

    let mut params = serde_json::json!({
        "project_id": project_id,
        "worktree": worktree,
        "command": command,
    });
    if let Some(t) = force_token {
        params["force_token"] = serde_json::Value::String(t);
    }
    let resp = c.call("session.launch", params).await?;
    if let Some(err) = resp.error {
        if err.code == -32002 {
            println!("port conflict:");
            if let Some(data) = err.data {
                if let Some(conflicts) = data["conflicts"].as_array() {
                    for c in conflicts {
                        println!(
                            "  port {} held by {}",
                            c["port"].as_u64().unwrap_or(0),
                            c["holder"].as_str().unwrap_or("?")
                        );
                    }
                }
                if let Some(tok) = data["force_token"].as_str() {
                    println!(
                        "\nto launch anyway: ccdash launch {} --force-token {}",
                        project, tok
                    );
                }
            }
            anyhow::bail!("launch blocked by port conflict");
        }
        anyhow::bail!("session.launch: {}", err.message);
    }
    let session = resp.result.unwrap()["session"].clone();
    println!(
        "launched: {}  {}",
        session["tmux_session_id"].as_str().unwrap_or("?"),
        session["name"].as_str().unwrap_or("?")
    );
    Ok(())
}
