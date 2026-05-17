//! Method routing: parse `params` for the named method, call the handler,
//! produce a `Response`.

use crate::rpc::handlers::{self, err, E_AUTH, E_INVALID_PARAMS};
use crate::state::AppState;
use ccdash_core::protocol::{
    HandshakeParams, ProjectAddParams, ProjectRemoveParams, Request, Response, SessionKillParams,
    SessionLaunchParams, SubscribeParams,
};
use serde_json::json;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Per-connection state held by the dispatch loop.
pub struct ConnContext {
    pub authed: bool,
    pub subscriptions: HashSet<ccdash_core::protocol::Topic>,
}

impl ConnContext {
    pub fn new() -> Self {
        Self {
            authed: false,
            subscriptions: HashSet::new(),
        }
    }
}

impl Default for ConnContext {
    fn default() -> Self {
        Self::new()
    }
}

pub async fn dispatch(req: Request, state: &AppState, ctx: &Arc<RwLock<ConnContext>>) -> Response {
    let id = req.id.clone();

    // handshake is the only method allowed pre-auth.
    if req.method != "handshake" && !ctx.read().await.authed {
        return Response::err(id, err(E_AUTH, "must call handshake first"));
    }

    match req.method.as_str() {
        "handshake" => {
            let params: HandshakeParams = match serde_json::from_value(req.params) {
                Ok(p) => p,
                Err(e) => return Response::err(id, err(E_INVALID_PARAMS, e.to_string())),
            };
            match handlers::handle_handshake(params, state) {
                Ok(r) => {
                    ctx.write().await.authed = true;
                    Response::ok(id, serde_json::to_value(r).unwrap())
                }
                Err(e) => Response::err(id, e),
            }
        }
        "subscribe" => {
            let params: SubscribeParams = match serde_json::from_value(req.params) {
                Ok(p) => p,
                Err(e) => return Response::err(id, err(E_INVALID_PARAMS, e.to_string())),
            };
            ctx.write().await.subscriptions.extend(params.topics);
            Response::ok(id, json!({"subscribed": true}))
        }
        "project.list" => {
            let r = handlers::handle_project_list(state).await;
            Response::ok(id, serde_json::to_value(r).unwrap())
        }
        "project.add" => {
            let params: ProjectAddParams = match serde_json::from_value(req.params) {
                Ok(p) => p,
                Err(e) => return Response::err(id, err(E_INVALID_PARAMS, e.to_string())),
            };
            match handlers::handle_project_add(params, state).await {
                Ok(p) => Response::ok(id, serde_json::to_value(p).unwrap()),
                Err(e) => Response::err(id, e),
            }
        }
        "project.reorder" => {
            let params: ccdash_core::protocol::ProjectReorderParams =
                match serde_json::from_value(req.params) {
                    Ok(p) => p,
                    Err(e) => return Response::err(id, err(E_INVALID_PARAMS, e.to_string())),
                };
            match handlers::handle_project_reorder(params, state).await {
                Ok(()) => Response::ok(id, json!({"ok": true})),
                Err(e) => Response::err(id, e),
            }
        }
        "project.remove" => {
            let params: ProjectRemoveParams = match serde_json::from_value(req.params) {
                Ok(p) => p,
                Err(e) => return Response::err(id, err(E_INVALID_PARAMS, e.to_string())),
            };
            match handlers::handle_project_remove(params, state).await {
                Ok(()) => Response::ok(id, json!({"ok": true})),
                Err(e) => Response::err(id, e),
            }
        }
        "session.list" => match handlers::handle_session_list(state).await {
            Ok(r) => Response::ok(id, serde_json::to_value(r).unwrap()),
            Err(e) => Response::err(id, e),
        },
        "session.launch" => {
            let params: SessionLaunchParams = match serde_json::from_value(req.params) {
                Ok(p) => p,
                Err(e) => return Response::err(id, err(E_INVALID_PARAMS, e.to_string())),
            };
            match handlers::handle_session_launch(params, state).await {
                Ok(r) => Response::ok(id, serde_json::to_value(r).unwrap()),
                Err(e) => Response::err(id, e),
            }
        }
        "session.kill" => {
            let params: SessionKillParams = match serde_json::from_value(req.params) {
                Ok(p) => p,
                Err(e) => return Response::err(id, err(E_INVALID_PARAMS, e.to_string())),
            };
            match handlers::handle_session_kill(params, state).await {
                Ok(()) => Response::ok(id, json!({"ok": true})),
                Err(e) => Response::err(id, e),
            }
        }
        "ports.list" => match handlers::handle_ports_list(state).await {
            Ok(r) => Response::ok(id, serde_json::to_value(r).unwrap()),
            Err(e) => Response::err(id, e),
        },
        "plans.get" => {
            let params: ccdash_core::protocol::PlanGetParams =
                match serde_json::from_value(req.params) {
                    Ok(p) => p,
                    Err(e) => return Response::err(id, err(E_INVALID_PARAMS, e.to_string())),
                };
            match handlers::handle_plans_get(params, state).await {
                Ok(r) => Response::ok(id, serde_json::to_value(r).unwrap()),
                Err(e) => Response::err(id, e),
            }
        }
        "daemon.first_run_status" => {
            let r = handlers::handle_first_run_status(state).await;
            Response::ok(id, serde_json::to_value(r).unwrap())
        }
        "daemon.first_run_complete" => {
            handlers::handle_first_run_complete(state).await;
            Response::ok(id, json!({"ok": true}))
        }
        "daemon.scan_paths" => {
            let params: ccdash_core::protocol::ScanPathsParams =
                match serde_json::from_value(req.params) {
                    Ok(p) => p,
                    Err(e) => return Response::err(id, err(E_INVALID_PARAMS, e.to_string())),
                };
            let r = handlers::handle_scan_paths(params, state).await;
            Response::ok(id, serde_json::to_value(r).unwrap())
        }
        other => Response::err(id, err(-32601, format!("method not found: {}", other))),
    }
}
