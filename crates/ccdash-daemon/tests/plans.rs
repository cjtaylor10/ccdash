mod common;

use common::Harness;
use tempfile::tempdir;

#[tokio::test]
async fn plans_get_parses_real_markdown() {
    let h = Harness::spawn().await.unwrap();
    let mut c = h.connect().await.unwrap();
    h.handshake(&mut c)
        .await
        .unwrap()
        .result
        .expect("handshake ok");

    let proj = tempdir().unwrap();
    let plans_dir = proj.path().join("docs/superpowers/plans");
    std::fs::create_dir_all(&plans_dir).unwrap();
    std::fs::write(
        plans_dir.join("phase-1.md"),
        "# Phase 1 Plan\n\n## Phase 1: Foundation\n\n- [x] task one\n- [ ] task two\n\n## Phase 2: CLI\n\n- [ ] task three\n",
    )
    .unwrap();

    let add = c
        .call(
            "project.add",
            serde_json::json!({"path": proj.path(), "name": "plans-test"}),
        )
        .await
        .unwrap();
    let project_id = add.result.unwrap()["id"].as_str().unwrap().to_string();

    let resp = c
        .call("plans.get", serde_json::json!({"project_id": project_id}))
        .await
        .unwrap();
    assert!(resp.error.is_none(), "plans.get error: {:?}", resp.error);
    let plans = resp.result.unwrap()["plans"].as_array().cloned().unwrap();
    assert_eq!(plans.len(), 1);
    let plan = &plans[0];
    assert_eq!(plan["title"].as_str().unwrap(), "Phase 1 Plan");
    let phases = plan["phases"].as_array().unwrap();
    assert_eq!(phases.len(), 2);
    assert_eq!(phases[0]["name"].as_str().unwrap(), "Phase 1: Foundation");
    assert_eq!(phases[0]["tasks"].as_array().unwrap().len(), 2);
    assert!(phases[0]["tasks"][0]["done"].as_bool().unwrap());
    assert!(!phases[0]["tasks"][1]["done"].as_bool().unwrap());
}

#[tokio::test]
async fn plans_get_returns_empty_for_project_without_plans() {
    let h = Harness::spawn().await.unwrap();
    let mut c = h.connect().await.unwrap();
    h.handshake(&mut c)
        .await
        .unwrap()
        .result
        .expect("handshake ok");

    let proj = tempdir().unwrap();
    let add = c
        .call(
            "project.add",
            serde_json::json!({"path": proj.path(), "name": "no-plans"}),
        )
        .await
        .unwrap();
    let project_id = add.result.unwrap()["id"].as_str().unwrap().to_string();

    let resp = c
        .call("plans.get", serde_json::json!({"project_id": project_id}))
        .await
        .unwrap();
    assert!(resp.error.is_none());
    let plans = resp.result.unwrap()["plans"].as_array().cloned().unwrap();
    assert!(plans.is_empty());
}
