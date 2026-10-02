use super::protocol::{GatewayFrame, MessageType, PROTOCOL_VERSION};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct AuthPayload {
    pub gateway_id: String,
    pub secret: String,
}

pub fn create_auth_frame(gateway_id: &str, secret: &str, seq: u64) -> GatewayFrame {
    let payload = serde_json::to_vec(&AuthPayload {
        gateway_id: gateway_id.to_string(),
        secret: secret.to_string(),
    })
    .unwrap();

    GatewayFrame {
        protocol_version: PROTOCOL_VERSION,
        msg_type: MessageType::Auth as u8,
        request_id: Uuid::new_v4(),
        sequence: seq,
        payload,
    }
}
