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

    let call_id = Uuid::new_v4().to_string();

    // 1. Store call session in DB (Turso)
    let db = &state.db;
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

    // 2. Publish call.incoming to target_user_id via internal TCP gateway
    let sdui_payload = crate::sdui::build_incoming_call_screen(&caller_id, "avatar_url", &call_id, 1);

    let msg = WsMessage {
        msg_type: WsMessageType::CallIncoming,
        request_id: Uuid::new_v4().to_string(),
        session_id: claims.session_id.clone(), // Assuming claims has session_id, wait, it has sub
        call_id: Some(call_id.clone()),
        seq: 1,
        payload: serde_json::to_value(sdui_payload).unwrap(),
    };

    route_to_user(&payload.target_user_id, &msg, &state).await;

    // 3. Push SDUI to caller (INITIATING -> RINGING)
    // The client will update its own UI, but we can also push a schema back if needed.
    // For now, the client updates its UI based on the HTTP response.

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
    let _ = state.db.execute("UPDATE call_sessions SET status = 'active' WHERE id = ?1", [call_id.clone()]).await;

    let mut kind_rows = state.db.query("SELECT kind FROM call_sessions WHERE id = ?1", [call_id.clone()]).await.unwrap();
    let mut is_video = false;
    if let Ok(Some(row)) = kind_rows.next().await {
        let kind_str: String = row.get(0).unwrap();
        is_video = kind_str == "video";
    }

    // Get the caller so we can notify them
    let mut rows = state.db.query("SELECT user_id FROM call_participants WHERE call_id = ?1 AND role = 'caller'", [call_id.clone()]).await.unwrap();
    if let Ok(Some(row)) = rows.next().await {
        let caller_id: String = row.get(0).unwrap();
        tracing::info!("Found caller_id for call {}: {}", call_id, caller_id);
        
        let schema = if is_video {
            crate::sdui::build_active_video_call_screen(&claims.sub, false, true, &call_id, 1)
        } else {
            crate::sdui::build_active_audio_call_screen(&claims.sub, false, &call_id, 1)
        };

        let msg = WsMessage {
            msg_type: WsMessageType::CallAccepted,
            request_id: Uuid::new_v4().to_string(),
            session_id: claims.session_id.clone(),
            call_id: Some(call_id.clone()),
            seq: 2,
            payload: serde_json::to_value(schema).unwrap(),
        };
        route_to_user(&caller_id, &msg, &state).await;
    } else {
        tracing::error!("Could not find caller_id for call {}", call_id);
    }
    
    // Also push active call screen to callee
    let callee_schema = if is_video {
        crate::sdui::build_active_video_call_screen("Caller", false, true, &call_id, 1)
    } else {
        crate::sdui::build_active_audio_call_screen("Caller", false, &call_id, 1)
    };

    let callee_msg = WsMessage {
        msg_type: WsMessageType::SduiUpdate, // Use SduiUpdate so callee doesn't create Offer!
        request_id: Uuid::new_v4().to_string(),
        session_id: claims.session_id.clone(),
        call_id: Some(call_id.clone()),
        seq: 2,
        payload: serde_json::to_value(callee_schema).unwrap(),
    };
    route_to_user(&claims.sub, &callee_msg, &state).await;

    StatusCode::OK
}

pub async fn reject_call(
    claims: Claims,
    axum::extract::Path(call_id): axum::extract::Path<String>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let _ = state.db.execute("UPDATE call_sessions SET status = 'ended' WHERE id = ?1", [call_id.clone()]).await;

    // Get the caller so we can notify them
    let mut rows = state.db.query("SELECT user_id FROM call_participants WHERE call_id = ?1 AND role = 'caller'", [call_id.clone()]).await.unwrap();
    if let Ok(Some(row)) = rows.next().await {
        let caller_id: String = row.get(0).unwrap();
        
        let msg = WsMessage {
            msg_type: WsMessageType::CallRejected,
            request_id: Uuid::new_v4().to_string(),
            session_id: claims.session_id.clone(),
            call_id: Some(call_id.clone()),
            seq: 2,
            payload: serde_json::json!({}), // or push home screen
        };
        route_to_user(&caller_id, &msg, &state).await;
    }

    // Push home screen to callee
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
    let mut rows = state.db.query("SELECT role FROM call_participants WHERE call_id = ?1 AND user_id = ?2", [call_id.clone(), claims.sub.clone()]).await.unwrap();
    if let Ok(Some(_)) = rows.next().await {
        // Just mock the state mutation for now based on action_id
        let is_video = req.action_id.contains("camera") || req.action_id == "call.start_video";
        let schema = if is_video {
            crate::sdui::build_active_video_call_screen("Peer", req.action_id == "call.mute", req.action_id != "call.camera_off", &call_id, 2)
        } else {
            crate::sdui::build_active_audio_call_screen("Peer", req.action_id == "call.mute", &call_id, 2)
        };
        
        let msg = WsMessage {
            msg_type: WsMessageType::CallAccepted, // Reuse CallAccepted to replace screen
            request_id: Uuid::new_v4().to_string(),
            session_id: claims.session_id.clone(),
            call_id: Some(call_id.clone()),
            seq: 3,
            payload: serde_json::to_value(schema).unwrap(),
        };
        
        // Only send the updated mute/camera toggle to the user who requested it!
        route_to_user(&claims.sub, &msg, &state).await;
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
    // Verify caller is participant
    let rows = state.db.query(
        "SELECT user_id FROM call_participants WHERE call_id = ?1 AND user_id = ?2",
        (call_id.clone(), claims.sub.clone())
    ).await;

    match rows {
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, axum::Json(serde_json::json!({"error": "db error"}))).into_response(),
        Ok(mut r) => {
            if let Ok(None) = r.next().await {
                return (StatusCode::FORBIDDEN, axum::Json(serde_json::json!({"error": "not a participant"}))).into_response();
            }
        }
    }

    // Generate temporary TURN credentials (RFC 8489 style HMAC-SHA1)
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
