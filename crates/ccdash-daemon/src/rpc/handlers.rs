//! Method-handler functions. Each takes parsed params + AppState, returns a
//! serializable result or RpcError.

use crate::broadcast::Event;
use crate::state::AppState;
use crate::{tmux, worktrees};
use ccdash_core::domain::{ProjectId, Session, SessionState};
use ccdash_core::protocol::{
    HandshakeParams, HandshakeResult, ProjectAddParams, ProjectListResult, ProjectRemoveParams,
    RpcError, SessionKillParams, SessionLaunchParams, SessionLaunchResult, SessionListResult,
    PROTOCOL_VERSION,
};

pub const E_AUTH: i32 = -32001;
pub const E_INVALID_PARAMS: i32 = -32602;
pub const E_INTERNAL: i32 = -32000;
pub const E_NOT_FOUND: i32 = -32004;

pub fn err(code: i32, msg: impl Into<String>) -> RpcError {
    RpcError {
        code,
        message: msg.into(),
        data: None,
    }
}

pub fn handle_handshake(
    params: HandshakeParams,
    state: &AppState,
) -> Result<HandshakeResult, RpcError> {
    if params.token != *state.auth_token {
        return Err(err(E_AUTH, "invalid auth token"));
    }
    Ok(HandshakeResult {
        daemon_version: env!("CARGO_PKG_VERSION").to_string(),
        protocol_version: PROTOCOL_VERSION,
    })
}

pub async fn handle_project_list(state: &AppState) -> ProjectListResult {
    ProjectListResult {
        projects: state.projects.list().await,
    }
}

pub async fn handle_project_add(
    params: ProjectAddParams,
    state: &AppState,
) -> Result<ccdash_core::domain::Project, RpcError> {
    let project = state
        .projects
        .add(params.path, params.name)
        .await
        .map_err(|e| err(E_INTERNAL, e.to_string()))?;
    // Best-effort worktree discovery; failures don't block project add.
    if let Ok(wts) = worktrees::list(&project.path).await {
        state.projects.set_worktrees(&project.id, wts).await;
    }
    let updated = state
        .projects
        .list()
        .await
        .into_iter()
        .find(|p| p.id == project.id)
        .unwrap_or(project.clone());
    state.bus.publish(Event::ProjectUpdated {
        project: updated.clone(),
    });
    Ok(updated)
}

pub async fn handle_project_remove(
    params: ProjectRemoveParams,
    state: &AppState,
) -> Result<(), RpcError> {
    let removed = state
        .projects
        .remove(&params.id)
        .await
        .map_err(|e| err(E_INTERNAL, e.to_string()))?;
    if !removed {
        return Err(err(E_NOT_FOUND, "no such project"));
    }
    state.bus.publish(Event::ProjectRemoved { id: params.id });
    Ok(())
}

pub async fn handle_session_list(state: &AppState) -> Result<SessionListResult, RpcError> {
    let (current, _) = state
        .sessions
        .refresh()
        .await
        .map_err(|e| err(E_INTERNAL, e.to_string()))?;
    Ok(SessionListResult { sessions: current })
}

pub async fn handle_ports_list(
    state: &AppState,
) -> Result<ccdash_core::protocol::PortListResult, RpcError> {
    state
        .ports
        .refresh()
        .await
        .map_err(|e| err(E_INTERNAL, e.to_string()))?;
    Ok(ccdash_core::protocol::PortListResult {
        running: state.ports.running().await,
        declared: state.ports.declared().await,
    })
}

pub async fn handle_session_launch(
    params: SessionLaunchParams,
    state: &AppState,
) -> Result<SessionLaunchResult, RpcError> {
    let projects = state.projects.list().await;
    let project = projects
        .iter()
        .find(|p| p.id == params.project_id)
        .ok_or_else(|| err(E_NOT_FOUND, "no such project"))?
        .clone();

    // Conflict gating: refresh ports, look for conflicts, return PortConflictData
    // in error.data unless caller supplied a valid force_token.
    match &params.force_token {
        Some(supplied) => {
            let mut tokens = state.conflict_tokens.lock().await;
            if !tokens.remove(supplied) {
                return Err(err(E_AUTH, "invalid or expired force_token"));
            }
        }
        None => {
            state
                .ports
                .refresh()
                .await
                .map_err(|e| err(E_INTERNAL, e.to_string()))?;
            let conflicts = state.ports.conflicts_for(&project.id).await;
            if !conflicts.is_empty() {
                let token: String = ccdash_core::auth::generate_token();
                state.conflict_tokens.lock().await.insert(token.clone());
                let data = ccdash_core::protocol::PortConflictData {
                    conflicts: conflicts
                        .into_iter()
                        .map(|(port, binding)| ccdash_core::protocol::PortConflict {
                            port,
                            holder: format!(
                                "{} (pid {})",
                                binding.command.unwrap_or_else(|| "?".into()),
                                binding
                                    .pid
                                    .map(|p| p.to_string())
                                    .unwrap_or_else(|| "?".into())
                            ),
                        })
                        .collect(),
                    force_token: token,
                };
                return Err(RpcError {
                    code: -32002,
                    message: "port conflict; pass force_token to bypass".into(),
                    data: Some(serde_json::to_value(data).unwrap()),
                });
            }
        }
    }

    let worktree_name = params
        .worktree
        .clone()
        .unwrap_or_else(|| "main".to_string());
    let cwd = project
        .worktrees
        .iter()
        .find(|w| w.branch == worktree_name || (worktree_name == "main" && w.is_primary))
        .map(|w| w.path.clone())
        .unwrap_or_else(|| project.path.clone());
    let cmd = params.command.unwrap_or_else(|| "claude".to_string());

    let safe_wt = sanitize(&worktree_name);
    let safe_proj = sanitize(&project.name);
    let name = format!("ccdash_{}_{}", safe_proj, safe_wt);

    let session_id = tmux::new_session(&name, &cwd, &cmd)
        .await
        .map_err(|e| err(E_INTERNAL, e.to_string()))?;
    state
        .sessions
        .record_launch(session_id.clone(), project.id.clone(), Some(worktree_name))
        .await
        .map_err(|e| err(E_INTERNAL, e.to_string()))?;

    let (current, _) = state
        .sessions
        .refresh()
        .await
        .map_err(|e| err(E_INTERNAL, e.to_string()))?;
    let session = current
        .into_iter()
        .find(|s| s.tmux_session_id == session_id)
        .unwrap_or_else(|| Session {
            tmux_session_id: session_id,
            name,
            project_id: Some(project.id.clone()),
            worktree: None,
            cwd,
            pid: 0,
            state: SessionState::Running,
            first_seen: 0,
        });
    state.bus.publish(Event::SessionLaunched {
        session: session.clone(),
    });
    Ok(SessionLaunchResult { session })
}

pub async fn handle_session_kill(
    params: SessionKillParams,
    state: &AppState,
) -> Result<(), RpcError> {
    tmux::kill_session(&params.tmux_session_id)
        .await
        .map_err(|e| err(E_INTERNAL, e.to_string()))?;
    state
        .sessions
        .forget(&params.tmux_session_id)
        .await
        .map_err(|e| err(E_INTERNAL, e.to_string()))?;
    state.bus.publish(Event::SessionRemoved {
        tmux_session_id: params.tmux_session_id,
    });
    Ok(())
}

pub async fn handle_plans_get(
    params: ccdash_core::protocol::PlanGetParams,
    state: &AppState,
) -> Result<ccdash_core::protocol::PlanGetResult, RpcError> {
    let projects = state.projects.list().await;
    let project = projects
        .iter()
        .find(|p| p.id == params.project_id)
        .ok_or_else(|| err(E_NOT_FOUND, "no such project"))?
        .clone();
    let plans = state
        .plans
        .refresh(&project.id, &project.path)
        .await
        .map_err(|e| err(E_INTERNAL, e.to_string()))?;
    Ok(ccdash_core::protocol::PlanGetResult { plans })
}

/// Sanitize a string for use in a tmux session name: replace ':' and whitespace with '_'.
fn sanitize(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c == ':' || c.is_whitespace() {
                '_'
            } else {
                c
            }
        })
        .collect()
}

#[allow(dead_code)] // used by handle_session_launch via ProjectId path
fn _ensure_used(_: ProjectId) {}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn handshake_rejects_bad_token() {
        let dir = tempdir().unwrap();
        let state = AppState::for_test(dir.path().to_path_buf()).await.unwrap();
        let result = handle_handshake(
            HandshakeParams {
                token: "wrong".into(),
                client: ccdash_core::protocol::ClientKind::Cli,
            },
            &state,
        );
        assert_eq!(result.unwrap_err().code, E_AUTH);
    }

    #[tokio::test]
    async fn handshake_accepts_correct_token() {
        let dir = tempdir().unwrap();
        let state = AppState::for_test(dir.path().to_path_buf()).await.unwrap();
        let token = (*state.auth_token).clone();
        let result = handle_handshake(
            HandshakeParams {
                token,
                client: ccdash_core::protocol::ClientKind::Cli,
            },
            &state,
        );
        assert_eq!(result.unwrap().protocol_version, PROTOCOL_VERSION);
    }

    #[test]
    fn sanitize_replaces_colon() {
        assert_eq!(sanitize("foo:bar baz"), "foo_bar_baz");
    }

    #[tokio::test]
    async fn project_add_publishes_event() {
        let dir = tempdir().unwrap();
        let state = AppState::for_test(dir.path().to_path_buf()).await.unwrap();
        let mut rx = state.bus.subscribe();
        let proj_dir = dir.path().join("p1");
        std::fs::create_dir(&proj_dir).unwrap();
        let _ = handle_project_add(
            ProjectAddParams {
                path: proj_dir,
                name: None,
            },
            &state,
        )
        .await
        .unwrap();
        let evt = rx.recv().await.unwrap();
        assert!(matches!(evt, Event::ProjectUpdated { .. }));
    }
}
