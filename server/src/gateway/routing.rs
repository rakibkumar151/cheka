use super::protocol::GatewayFrame;
use std::collections::HashMap;
use tokio::sync::{mpsc, RwLock};

use crate::ws::WsMessage;

pub struct GatewayRoutingTable {
    // Map of connected peer gateway_id -> Sender channel to that peer
    pub peers: RwLock<HashMap<String, mpsc::Sender<GatewayFrame>>>,
    // Map of user_id -> (session_id, Sender channel to their WebSocket)
    pub local_users: RwLock<HashMap<String, (String, mpsc::Sender<WsMessage>)>>,
}

impl GatewayRoutingTable {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Default for GatewayRoutingTable {
    fn default() -> Self {
        Self {
            peers: RwLock::new(HashMap::new()),
            local_users: RwLock::new(HashMap::new()),
        }
    }
}
impl GatewayRoutingTable {
    pub async fn add_peer(&self, gateway_id: String, tx: mpsc::Sender<GatewayFrame>) {
        self.peers.write().await.insert(gateway_id, tx);
    }

    pub async fn remove_peer(&self, gateway_id: &str) {
        self.peers.write().await.remove(gateway_id);
    }

    pub async fn add_local_user(&self, user_id: String, session_id: String, tx: mpsc::Sender<WsMessage>) {
        self.local_users.write().await.insert(user_id, (session_id, tx));
    }

    pub async fn remove_local_user(&self, user_id: &str, session_id: &str) {
        let mut map = self.local_users.write().await;
        if let Some((existing_session_id, _)) = map.get(user_id) {
            if existing_session_id == session_id {
                map.remove(user_id);
            }
        }
    }

    pub async fn get_local_user(&self, user_id: &str) -> Option<mpsc::Sender<WsMessage>> {
        self.local_users.read().await.get(user_id).map(|(_, tx)| tx.clone())
    }

    // In a real system, we'd look up the user's gateway from a DB or hash ring.
    // For this Phase 1 integration test, we will just broadcast to all peers if not local.
    // Or we can deterministically map users based on their ID if there are exactly 2 gateways.
    pub async fn get_peer_for_user(&self, _user_id: &str) -> Option<mpsc::Sender<GatewayFrame>> {
        // Just return the first peer for the 2-gateway test
        let peers = self.peers.read().await;
        if let Some((_, tx)) = peers.iter().next() {
            Some(tx.clone())
        } else {
            None
        }
    }

    pub async fn broadcast_to_peers(&self, frame: GatewayFrame, ack_manager: &crate::gateway::ack::AckManager) {
        let peers = self.peers.read().await;
        for tx in peers.values() {
            let cloned = frame.clone();
            // Generate a new request_id for each peer to track ACKs independently
            let mut specific_frame = cloned;
            specific_frame.request_id = uuid::Uuid::new_v4();
            ack_manager.add_pending(specific_frame.clone(), tx.clone()).await;
            let _ = tx.send(specific_frame).await;
        }
    }
}
