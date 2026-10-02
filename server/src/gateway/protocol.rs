use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const PROTOCOL_VERSION: u8 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MessageType {
    Hello = 1,
    Auth = 2,
    Ping = 3,
    Pong = 4,
    RouteCallEvent = 5,
    RouteSignal = 6,
    Ack = 7,
    Nack = 8,
    Close = 9,
    PresenceUpdate = 10,
}

impl TryFrom<u8> for MessageType {
    type Error = &'static str;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(MessageType::Hello),
            2 => Ok(MessageType::Auth),
            3 => Ok(MessageType::Ping),
            4 => Ok(MessageType::Pong),
            5 => Ok(MessageType::RouteCallEvent),
            6 => Ok(MessageType::RouteSignal),
            7 => Ok(MessageType::Ack),
            8 => Ok(MessageType::Nack),
            9 => Ok(MessageType::Close),
            10 => Ok(MessageType::PresenceUpdate),
            _ => Err("Unknown message type"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayFrame {
    pub protocol_version: u8,
    pub msg_type: u8,
    pub request_id: Uuid,
    pub sequence: u64,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteCallEventPayload {
    pub target_user_id: String,
    pub call_id: String,
    pub event: String,
    pub event_payload: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresenceUpdatePayload {
    pub user_id: String,
    pub is_online: bool,
    pub call_state: String,
    pub gateway_id: String,
}
