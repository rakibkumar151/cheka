use std::sync::Arc;
use tracing::info;

use crate::{ws::WsMessage, AppState};

use crate::ws::WsMessageType;

pub async fn process_message(msg: WsMessage, user_id: &str, state: &Arc<AppState>) {
    info!(
        "Processing WS msg from user {}: {:?}",
        user_id, msg.msg_type
    );

    match msg.msg_type {
        WsMessageType::CallOffer | WsMessageType::CallAnswer | WsMessageType::CallIce => {
            if let Some(call_id) = &msg.call_id {
                // Fresh connection per message — never panics on Turso stream expiry
                let db = match state.db.connect() {
                    Ok(c) => c,
                    Err(e) => {
                        tracing::error!("DB connect failed in signaling for call {}: {}", call_id, e);
                        return;
                    }
                };
                match db.query("SELECT user_id FROM call_participants WHERE call_id = ?1 AND user_id != ?2", [call_id.clone(), user_id.to_string()]).await {
                    Ok(mut rows) => {
                        if let Ok(Some(row)) = rows.next().await {
                            let target_user_id: String = row.get(0).unwrap_or_default();
                            tracing::info!("Routing {:?} from {} to {}", msg.msg_type, user_id, target_user_id);
                            route_to_user(&target_user_id, &msg, state).await;
                        } else {
                            tracing::info!("No local connection or peer found for user {}", user_id);
                        }
                    }
                    Err(e) => {
                        tracing::error!("DB query failed in signaling for call {}: {}", call_id, e);
                    }
                }
            }
        }
        _ => {}
    }
}

use crate::gateway::protocol::{GatewayFrame, MessageType, PROTOCOL_VERSION};
use uuid::Uuid;

pub async fn route_to_user(target_user_id: &str, msg: &WsMessage, state: &Arc<AppState>) {
    // 1. Try local route first
    if let Some(tx) = state.gateway.routing.get_local_user(target_user_id).await {
        let _ = tx.send(msg.clone()).await;
        return;
    }

    // 2. Otherwise route over TCP gateway
    if let Some(tx) = state
        .gateway
        .routing
        .get_peer_for_user(target_user_id)
        .await
    {
        let route_payload = serde_json::to_vec(&serde_json::json!({
            "target_user_id": target_user_id,
            "msg": msg,
        }))
        .unwrap();

        let frame = GatewayFrame {
            protocol_version: PROTOCOL_VERSION,
            msg_type: MessageType::RouteCallEvent as u8,
            request_id: Uuid::new_v4(),
            sequence: 0,
            payload: route_payload,
        };

        state.gateway.ack_manager.add_pending(frame.clone(), tx.clone()).await;
        let _ = tx.send(frame).await;
    } else {
        info!(
            "No local connection or peer found for user {}",
            target_user_id
        );
    }
}
