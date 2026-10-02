use axum::{routing::get, Router};
use std::sync::Arc;

pub mod auth;
pub mod calls;
pub mod config;
pub mod error;
pub mod gateway;
pub mod http;
pub mod presence;
pub mod rate_limit;
pub mod sdui;
pub mod signaling;
pub mod storage;
pub mod telemetry;
pub mod ws;

#[derive(Clone)]
pub struct AppState {
    pub config: config::Config,
    pub db: Arc<libsql::Connection>,
    pub gateway: Arc<crate::gateway::GatewayState>,
    pub presence: Arc<crate::presence::PresenceManager>,
}

pub fn build_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/v1/health", get(health_check))
        .route("/v1/ready", get(ready_check))
        .route("/v1/auth/anonymous", axum::routing::post(anonymous_login))
        .route("/v1/debug/push", axum::routing::post(push_sdui_debug))
        .route("/v1/ws", get(ws::ws_handler))
        .route("/v1/calls", axum::routing::post(http::calls::create_call))
        .route("/v1/calls/:call_id/accept", axum::routing::post(http::calls::accept_call))
        .route("/v1/calls/:call_id/reject", axum::routing::post(http::calls::reject_call))
        .route("/v1/calls/:call_id/action", axum::routing::post(http::calls::handle_action))
        .route("/v1/calls/:call_id/turn-credentials", axum::routing::get(http::calls::get_turn_credentials))
        .with_state(state)
}

async fn health_check() -> &'static str {
    "OK"
}

async fn ready_check() -> &'static str {
    "READY"
}

#[derive(serde::Deserialize)]
pub struct AuthReq {
    pub user_id: String,
}

#[derive(serde::Serialize)]
pub struct AuthRes {
    pub token: String,
}

async fn anonymous_login(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
    axum::Json(req): axum::extract::Json<AuthReq>,
) -> axum::response::Json<AuthRes> {
    let exp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as usize
        + 3600;

    let claims = crate::auth::Claims {
        sub: req.user_id,
        session_id: uuid::Uuid::new_v4().to_string(),
        exp,
    };

    let token = jsonwebtoken::encode(
        &jsonwebtoken::Header::default(),
        &claims,
        &jsonwebtoken::EncodingKey::from_secret(state.config.jwt_secret.as_bytes()),
    )
    .unwrap();

    axum::response::Json(AuthRes { token })
}

#[derive(serde::Deserialize)]
pub struct PushSduiReq {
    pub target_user_id: String,
    pub revision: i32,
    pub title: String,
}

async fn push_sdui_debug(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
    axum::Json(req): axum::extract::Json<PushSduiReq>,
) -> &'static str {
    if let Some(tx) = state.gateway.routing.get_local_user(&req.target_user_id).await {
        let schema = serde_json::json!({
            "schema_version": 1,
            "screen": "home",
            "revision": req.revision,
            "title": req.title,
            "components": [
                {
                    "id": "btn_start",
                    "type": "button",
                    "text": "Start Call",
                    "action": "call.start_audio"
                }
            ]
        });
        
        let msg = crate::ws::WsMessage {
            msg_type: crate::ws::WsMessageType::SduiUpdate,
            request_id: uuid::Uuid::new_v4().to_string(),
            session_id: "debug".to_string(),
            call_id: None,
            seq: 1,
            payload: schema,
        };
        let _ = tx.send(msg).await;
        "PUSHED"
    } else {
        "NOT FOUND"
    }
}
