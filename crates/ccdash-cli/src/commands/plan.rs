use crate::commands::{connect, resolve_project_id};
use anyhow::Result;
use std::path::PathBuf;

pub async fn run(socket: Option<PathBuf>, project: String) -> Result<()> {
    let mut c = connect(socket).await?;
    let project_id = resolve_project_id(&mut c, &project).await?;
    let resp = c
        .call("plans.get", serde_json::json!({"project_id": project_id}))
        .await?;
    if let Some(err) = resp.error {
        anyhow::bail!("plans.get: {}", err.message);
    }
    let plans = resp.result.unwrap()["plans"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    if plans.is_empty() {
        println!("(no plans found under docs/superpowers/{{specs,plans}}/)");
        return Ok(());
    }
    for p in plans {
        println!(
            "== {} ({}) ==",
            p["title"].as_str().unwrap_or("?"),
            p["path"].as_str().unwrap_or("?")
        );
        let phases = p["phases"].as_array().cloned().unwrap_or_default();
        for phase in phases {
            println!("  ## {}", phase["name"].as_str().unwrap_or("?"));
            let tasks = phase["tasks"].as_array().cloned().unwrap_or_default();
            let done = tasks
                .iter()
                .filter(|t| t["done"].as_bool().unwrap_or(false))
                .count();
            let total = tasks.len();
            for t in &tasks {
                let marker = if t["done"].as_bool().unwrap_or(false) {
                    "[x]"
                } else {
                    "[ ]"
                };
                println!("    {} {}", marker, t["title"].as_str().unwrap_or("?"));
            }
            println!("    ({}/{} done)", done, total);
        }
        println!();
    }
    Ok(())
}
