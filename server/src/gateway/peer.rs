use futures::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio_util::codec::Framed;
use tracing::{error, info, warn};

use super::framing::GatewayCodec;
use super::protocol::{GatewayFrame, MessageType, PROTOCOL_VERSION};
use crate::AppState;

pub async fn handle_peer_connection(
    stream: TcpStream,
    state: Arc<AppState>,
    is_outbound: bool,
    peer_gateway_id: Option<String>, // Known if outbound
) {
    let framed = Framed::new(stream, GatewayCodec);
    let (mut writer, mut reader) = framed.split();

    // Handshake
    if is_outbound {
        let auth_frame =
            super::identity::create_auth_frame(&state.gateway.local_gateway_id, "shared_secret", 0);
        if let Err(e) = writer.send(auth_frame).await {
            error!("Failed to send auth frame: {}", e);
            return;
        }
    }

    let (tx, mut rx) = mpsc::channel::<GatewayFrame>(100);

    if is_outbound {
        if let Some(ref gid) = peer_gateway_id {
            state
                .gateway
                .routing
                .add_peer(gid.clone(), tx.clone())
                .await;
            info!("Registered outbound peer: {}", gid);
        }
    }

    // Sender task
    let mut sender_task = tokio::spawn(async move {
        while let Some(frame) = rx.recv().await {
            if writer.send(frame).await.is_err() {
                break;
            }
        }
    });

    let mut authenticated_peer_id = peer_gateway_id.clone();
    let tx_clone = tx.clone();

    // Receiver task
    let state_clone = state.clone();
    let mut receiver_task = tokio::spawn(async move {
        while let Some(Ok(frame)) = reader.next().await {
            match MessageType::try_from(frame.msg_type) {
                Ok(MessageType::Auth) => {
                    // In a real system, parse payload and verify secret.
                    if !is_outbound {
                        // Very naive parsing just for integration tests
                        if let Ok(payload) = String::from_utf8(frame.payload.clone()) {
                            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&payload) {
                                if let Some(gid) = json.get("gateway_id").and_then(|v| v.as_str()) {
                                    authenticated_peer_id = Some(gid.to_string());
                                    state_clone
                                        .gateway
                                        .routing
                                        .add_peer(gid.to_string(), tx_clone.clone())
                                        .await;
                                    info!("Received Auth from peer: {}", gid);
                                }
                            }
                        }
                    }
                }
                Ok(MessageType::Ack) => {
                    state_clone
                        .gateway
                        .ack_manager
                        .remove_pending(&frame.request_id)
                        .await;
                }
                Ok(MessageType::RouteCallEvent) | Ok(MessageType::RouteSignal) => {
                    if state_clone
                        .gateway
                        .ack_manager
                        .is_duplicate(&frame.request_id)
                        .await
                    {
                        warn!("Duplicate frame received: {}", frame.request_id);
                    } else {
                        // Deserialize and process
                        if let Ok(payload) = String::from_utf8(frame.payload.clone()) {
                            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&payload) {
                                if let Some(target_user_id) =
                                    json.get("target_user_id").and_then(|v| v.as_str())
                                {
                                    // Let signaling handle it
                                    info!(
                                        "Received routed message for user {}: {}",
                                        target_user_id, frame.request_id
                                    );
                                    if let Some(msg_val) = json.get("msg") {
                                        if let Ok(ws_msg) = serde_json::from_value(msg_val.clone())
                                        {
                                            crate::signaling::route_to_user(
                                                target_user_id,
                                                &ws_msg,
                                                &state_clone,
                                            )
                                            .await;
                                        } else {
                                            warn!("Failed to decode ws_msg from msg_val");
                                        }
                                    } else {
                                        warn!("Missing msg field in route payload");
                                    }
                                }
                            }
                        }
                    }

                    // Send ACK
                    let ack_frame = GatewayFrame {
                        protocol_version: PROTOCOL_VERSION,
                        msg_type: MessageType::Ack as u8,
                        request_id: frame.request_id,
                        sequence: frame.sequence,
                        payload: vec![],
                    };
                    let _ = tx_clone.send(ack_frame).await;
                }
                Ok(MessageType::PresenceUpdate) => {
                    if state_clone.gateway.ack_manager.is_duplicate(&frame.request_id).await {
                        warn!("Duplicate PresenceUpdate frame: {}", frame.request_id);
                    } else {
                        if let Ok(presence_payload) = serde_json::from_slice::<crate::gateway::protocol::PresenceUpdatePayload>(&frame.payload) {
                            let user_presence = crate::presence::UserPresence {
                                is_online: presence_payload.is_online,
                                call_state: presence_payload.call_state,
                                gateway_id: presence_payload.gateway_id,
                                last_seen: tokio::time::Instant::now(),
                            };
                            state_clone.presence.update_presence(&presence_payload.user_id, user_presence).await;

                            // Push SDUI update to all local users
                            let online_users = state_clone.presence.get_online_users().await;
                            let local_users = state_clone.gateway.routing.local_users.read().await;
                            for (local_uid, (_, tx)) in local_users.iter() {
                                let other_users: Vec<_> = online_users.iter().filter(|(uid, _)| uid != local_uid).cloned().collect();
                                let schema = serde_json::to_value(crate::sdui::build_home_screen(
                                    local_uid,
                                    &other_users,
                                    1, // could increment revision
                                )).unwrap();

                                let msg = crate::ws::WsMessage {
                                    msg_type: crate::ws::WsMessageType::SduiUpdate,
                                    request_id: uuid::Uuid::new_v4().to_string(),
                                    session_id: "presence_update".to_string(),
                                    call_id: None,
                                    seq: 1,
                                    payload: schema,
                                };
                                let _ = tx.send(msg).await;
                            }
                        }
                    }
                    
                    let ack_frame = GatewayFrame {
                        protocol_version: PROTOCOL_VERSION,
                        msg_type: MessageType::Ack as u8,
                        request_id: frame.request_id,
                        sequence: frame.sequence,
                        payload: vec![],
                    };
                    let _ = tx_clone.send(ack_frame).await;
                }
                _ => {}
            }
        }

        if let Some(id) = authenticated_peer_id {
            state_clone.gateway.routing.remove_peer(&id).await;
        }
    });

    tokio::select! {
        _ = (&mut sender_task) => receiver_task.abort(),
        _ = (&mut receiver_task) => sender_task.abort(),
    };
}
