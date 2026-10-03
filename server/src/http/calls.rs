use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::{
    auth::Claims,
    signaling::route_to_user,
    ws::{WsMessage, WsMessageType},
    AppState,
};

#[derive(Deserialize)]
pub struct CreateCallRequest {
    pub target_user_id: String,
    pub call_type: String, // "audio" or "video"
}

#[derive(Serialize)]
pub struct CreateCallResponse {
    pub call_id: String,
    pub state: String,
    pub kind: String,
}

pub async fn create_call(
    claims: Claims,
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateCallRequest>,
) -> impl IntoResponse {
    let caller_id = claims.sub.clone();
    let target_id = payload.target_user_id.trim();

    // TARGET UID VALIDATION
    if target_id.is_empty() {
        return (StatusCode::BAD_REQUEST, axum::Json(CreateCallResponse { call_id: "".into(), state: "FAILED".into(), kind: payload.call_type })).into_response();
    }
    if target_id.len() > 64 {
        return (StatusCode::BAD_REQUEST, axum::Json(CreateCallResponse { call_id: "".into(), state: "FAILED".into(), kind: payload.call_type })).into_response();
    }
    if caller_id == target_id {
        return (StatusCode::BAD_REQUEST, axum::Json(CreateCallResponse { call_id: "".into(), state: "FAILED".into(), kind: payload.call_type })).into_response();
    }
    if !target_id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
        return (StatusCode::BAD_REQUEST, axum::Json(CreateCallResponse { call_id: "".into(), state: "FAILED".into(), kind: payload.call_type })).into_response();
    }

    // Fresh connection per request — never panics on stream expiry
    let db = match state.db.connect() {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("DB connect failed in create_call: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, axum::Json(CreateCallResponse { call_id: "".into(), state: "FAILED".into(), kind: payload.call_type })).into_response();
        }
    };

    let call_id = Uuid::new_v4().to_string();

    let _ = db
        .execute(
            "INSERT INTO call_sessions (id, caller_id, status, kind) VALUES (?1, ?2, 'initiating', ?3)",
            (call_id.clone(), caller_id.clone(), payload.call_type.clone()),
        )
        .await;

    let _ = db.execute(
        "INSERT INTO call_participants (call_id, user_id, role, status) VALUES (?1, ?2, 'caller', 'joined'), (?1, ?3, 'callee', 'ringing')",
        (call_id.clone(), caller_id.clone(), payload.target_user_id.clone())
    ).await;

    let sdui_payload = crate::sdui::build_incoming_call_screen(&caller_id, "avatar_url", &call_id, 1);

    let msg = WsMessage {
        msg_type: WsMessageType::CallIncoming,
        request_id: Uuid::new_v4().to_string(),
        session_id: claims.session_id.clone(),
        call_id: Some(call_id.clone()),
        seq: 1,
        payload: serde_json::to_value(sdui_payload).unwrap_or_default(),
    };

    route_to_user(&payload.target_user_id, &msg, &state).await;

    (StatusCode::CREATED, Json(CreateCallResponse {
        call_id,
        state: "RINGING".to_string(),
        kind: payload.call_type
    })).into_response()
}

pub async fn accept_call(
    claims: Claims,
    axum::extract::Path(call_id): axum::extract::Path<String>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    // Fresh connection — safe from stream-not-found panics
    let db = match state.db.connect() {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("DB connect failed in accept_call: {}", e);
            return StatusCode::INTERNAL_SERVER_ERROR;
        }
    };

    let _ = db.execute("UPDATE call_sessions SET status = 'active' WHERE id = ?1", [call_id.clone()]).await;

    // Get the caller so we can notify them
    match db.query("SELECT user_id FROM call_participants WHERE call_id = ?1 AND role = 'caller'", [call_id.clone()]).await {
        Ok(mut rows) => {
            if let Ok(Some(row)) = rows.next().await {
                let caller_id: String = row.get(0).unwrap_or_default();
                tracing::info!("Found caller_id for call {}: {}", call_id, caller_id);

                let schema = crate::sdui::build_active_audio_call_screen(&claims.sub, false, &call_id, 1);

                let msg = WsMessage {
                    msg_type: WsMessageType::CallAccepted,
                    request_id: Uuid::new_v4().to_string(),
                    session_id: claims.session_id.clone(),
                    call_id: Some(call_id.clone()),
                    seq: 2,
                    payload: serde_json::to_value(schema).unwrap_or_default(),
                };
                route_to_user(&caller_id, &msg, &state).await;
            } else {
                tracing::error!("Could not find caller_id for call {}", call_id);
            }
        }
        Err(e) => {
            tracing::error!("DB query failed in accept_call (caller lookup) for {}: {}", call_id, e);
        }
    }

    // Also push active call screen to callee
    let callee_schema = crate::sdui::build_active_audio_call_screen("Caller", false, &call_id, 1);

    let callee_msg = WsMessage {
        msg_type: WsMessageType::SduiUpdate,
        request_id: Uuid::new_v4().to_string(),
        session_id: claims.session_id.clone(),
        call_id: Some(call_id.clone()),
        seq: 2,
        payload: serde_json::to_value(callee_schema).unwrap_or_default(),
    };
    route_to_user(&claims.sub, &callee_msg, &state).await;

    StatusCode::OK
}

pub async fn reject_call(
    claims: Claims,
    axum::extract::Path(call_id): axum::extract::Path<String>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let db = match state.db.connect() {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("DB connect failed in reject_call: {}", e);
            return StatusCode::INTERNAL_SERVER_ERROR;
        }
    };

    let _ = db.execute("UPDATE call_sessions SET status = 'ended' WHERE id = ?1", [call_id.clone()]).await;

    match db.query("SELECT user_id FROM call_participants WHERE call_id = ?1 AND role = 'caller'", [call_id.clone()]).await {
        Ok(mut rows) => {
            if let Ok(Some(row)) = rows.next().await {
                let caller_id: String = row.get(0).unwrap_or_default();

                let msg = WsMessage {
                    msg_type: WsMessageType::CallRejected,
                    request_id: Uuid::new_v4().to_string(),
                    session_id: claims.session_id.clone(),
                    call_id: Some(call_id.clone()),
                    seq: 2,
                    payload: serde_json::json!({}),
                };
                route_to_user(&caller_id, &msg, &state).await;
            }
        }
        Err(e) => tracing::error!("DB query failed in reject_call: {}", e),
    }

    crate::sdui::push_sdui_to_local_users(&state).await;

    StatusCode::OK
}

#[derive(serde::Deserialize)]
pub struct CallActionReq {
    pub action_id: String,
}

pub async fn handle_action(
    claims: Claims,
    axum::extract::Path(call_id): axum::extract::Path<String>,
    State(state): State<Arc<AppState>>,
    axum::Json(req): axum::Json<CallActionReq>,
) -> impl IntoResponse {
    let db = match state.db.connect() {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("DB connect failed in handle_action: {}", e);
            return StatusCode::INTERNAL_SERVER_ERROR;
        }
    };

    match db.query("SELECT role FROM call_participants WHERE call_id = ?1 AND user_id = ?2", [call_id.clone(), claims.sub.clone()]).await {
        Ok(mut rows) => {
            if let Ok(Some(_)) = rows.next().await {
                let mut kind = "audio".to_string();
                if let Ok(mut kind_rows) = db.query("SELECT kind FROM call_sessions WHERE id = ?1", [call_id.clone()]).await {
                    if let Ok(Some(row)) = kind_rows.next().await {
                        kind = row.get(0).unwrap_or_else(|_| "audio".to_string());
                    }
                }

                let schema = crate::sdui::build_active_audio_call_screen(
                    "Peer", 
                    false,
                    &call_id, 
                    2
                );
                let msg = WsMessage {
                    msg_type: WsMessageType::CallAccepted, // Re-use CallAccepted as UI update for calls
                    request_id: Uuid::new_v4().to_string(),
                    session_id: claims.session_id.clone(),
                    call_id: Some(call_id.clone()),
                    seq: 3,
                    payload: serde_json::to_value(schema).unwrap_or_default(),
                };

                // Broadcast to all participants
                if let Ok(mut p_rows) = db.query("SELECT user_id FROM call_participants WHERE call_id = ?1", [call_id.clone()]).await {
                    while let Ok(Some(p_row)) = p_rows.next().await {
                        if let Ok(uid) = p_row.get::<String>(0) {
                            route_to_user(&uid, &msg, &state).await;
                        }
                    }
                }
            }
        }
        Err(e) => tracing::error!("DB query failed in handle_action: {}", e),
    }

    StatusCode::OK
}

#[derive(Serialize)]
pub struct TurnCredentials {
    pub url: String,
    pub username: String,
    pub credential: String,
    pub ttl: u64,
}

pub async fn get_turn_credentials(
    claims: Claims,
    axum::extract::Path(call_id): axum::extract::Path<String>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let db = match state.db.connect() {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("DB connect failed in get_turn_credentials: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, axum::Json(serde_json::json!({"error": "db error"}))).into_response();
        }
    };

    match db.query(
        "SELECT user_id FROM call_participants WHERE call_id = ?1 AND user_id = ?2",
        (call_id.clone(), claims.sub.clone())
    ).await {
        Err(e) => {
            tracing::error!("DB query failed in get_turn_credentials: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, axum::Json(serde_json::json!({"error": "db error"}))).into_response();
        }
        Ok(mut r) => {
            if let Ok(None) = r.next().await {
                return (StatusCode::FORBIDDEN, axum::Json(serde_json::json!({"error": "not a participant"}))).into_response();
            }
        }
    }

    let ttl_secs: u64 = 3600;
    let expiry = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        + ttl_secs;

    let username = format!("{}:{}", expiry, claims.sub);

    use hmac::{Hmac, Mac};
    use sha1::Sha1;
    use base64::Engine;

    type HmacSha1 = Hmac<Sha1>;
    let mut mac = HmacSha1::new_from_slice(state.config.turn_secret.as_bytes())
        .expect("HMAC accepts any key size");
    mac.update(username.as_bytes());
    let credential = base64::engine::general_purpose::STANDARD.encode(mac.finalize().into_bytes());

    tracing::info!(user = %claims.sub, call = %call_id, "Issued TURN credentials (expiry={})", expiry);

    (StatusCode::OK, axum::Json(TurnCredentials {
        url: state.config.turn_url.clone(),
        username,
        credential,
        ttl: ttl_secs,
    })).into_response()
}
