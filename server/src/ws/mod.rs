use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
};
use futures::{sink::SinkExt, stream::StreamExt};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{info, warn};

use crate::{auth::Claims, AppState};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WsMessageType {
    SessionReady,
    SessionRefresh,
    CallIncoming,
    CallAccepted,
    CallRejected,
    CallOffer,
    CallAnswer,
    CallIce,
    CallState,
    CallEnd,
    CallError,
    SduiUpdate,
    PresenceUpdate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsMessage {
    #[serde(rename = "type")]
    pub msg_type: WsMessageType,
    pub request_id: String,
    pub session_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub call_id: Option<String>,
    pub seq: u64,
    pub payload: serde_json::Value,
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    claims: Claims,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    info!("WebSocket upgrade request for user {}", claims.sub);
    ws.on_upgrade(move |socket| handle_socket(socket, claims, state))
}

async fn handle_socket(socket: WebSocket, claims: Claims, state: Arc<AppState>) {
    let (mut sender, mut receiver) = socket.split();

    let user_id = claims.sub.clone();
    let (tx, mut rx) = tokio::sync::mpsc::channel::<WsMessage>(100); // Bounded queue!

    state
        .gateway
        .routing
        .add_local_user(user_id.clone(), claims.session_id.clone(), tx.clone())
        .await;

    let presence = crate::presence::UserPresence {
        is_online: true,
        call_state: "idle".to_string(),
        gateway_id: state.gateway.local_gateway_id.clone(),
        last_seen: tokio::time::Instant::now(),
    };
    state.presence.update_presence(&user_id, presence).await;

    let presence_payload = serde_json::to_vec(&crate::gateway::protocol::PresenceUpdatePayload {
        user_id: user_id.clone(),
        is_online: true,
        call_state: "idle".to_string(),
        gateway_id: state.gateway.local_gateway_id.clone(),
    }).unwrap();

    let presence_frame = crate::gateway::protocol::GatewayFrame {
        protocol_version: crate::gateway::protocol::PROTOCOL_VERSION,
        msg_type: crate::gateway::protocol::MessageType::PresenceUpdate as u8,
        request_id: uuid::Uuid::new_v4(), // Placeholder, updated in broadcast
        sequence: 0,
        payload: presence_payload,
    };
    state.gateway.routing.broadcast_to_peers(presence_frame, &state.gateway.ack_manager).await;

    // Push updated SDUI to all local users (including this new one)
    crate::sdui::push_sdui_to_local_users(&state).await;

    // Sender task
    let mut sender_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if let Ok(text) = serde_json::to_string(&msg) {
                if sender.send(Message::Text(text)).await.is_err() {
                    break;
                }
            }
        }
    });

    // Receiver task
    let state_clone = state.clone();
    let mut receiver_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            match msg {
                Message::Text(text) => {
                    // Check payload size
                    if text.len() > 64 * 1024 {
                        warn!("Oversized WebSocket frame from user {}", user_id);
                        continue;
                    }

                    match serde_json::from_str::<WsMessage>(&text) {
                        Ok(ws_msg) => {
                            // Validate message fields
                            if ws_msg.session_id.is_empty() {
                                warn!("Invalid session_id from user {}", user_id);
                                continue;
                            }

                            // Process message (delegated to signaling logic)
                            crate::signaling::process_message(ws_msg, &user_id, &state_clone).await;
                        }
                        Err(e) => {
                            warn!("Malformed WS payload from user {}: {}", user_id, e);
                        }
                    }
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
    });

    tokio::select! {
        _ = (&mut sender_task) => receiver_task.abort(),
        _ = (&mut receiver_task) => sender_task.abort(),
    };

    state.gateway.routing.remove_local_user(&claims.sub, &claims.session_id).await;

    let presence = crate::presence::UserPresence {
        is_online: false,
        call_state: "idle".to_string(),
        gateway_id: state.gateway.local_gateway_id.clone(),
        last_seen: tokio::time::Instant::now(),
    };
    state.presence.update_presence(&claims.sub, presence).await;

    let presence_payload = serde_json::to_vec(&crate::gateway::protocol::PresenceUpdatePayload {
        user_id: claims.sub.clone(),
        is_online: false,
        call_state: "idle".to_string(),
        gateway_id: state.gateway.local_gateway_id.clone(),
    }).unwrap();

    let presence_frame = crate::gateway::protocol::GatewayFrame {
        protocol_version: crate::gateway::protocol::PROTOCOL_VERSION,
        msg_type: crate::gateway::protocol::MessageType::PresenceUpdate as u8,
        request_id: uuid::Uuid::new_v4(), // Placeholder
        sequence: 0,
        payload: presence_payload,
    };
    state.gateway.routing.broadcast_to_peers(presence_frame, &state.gateway.ack_manager).await;

    // Push updated SDUI to remaining local users
    crate::sdui::push_sdui_to_local_users(&state).await;

    info!("WebSocket closed for user {}", claims.sub);
}
