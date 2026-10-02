use super::protocol::GatewayFrame;
use std::collections::HashMap;
use tokio::sync::Mutex;
use tokio::time::{Duration, Instant};
use uuid::Uuid;

pub struct PendingMessage {
    pub frame: GatewayFrame,
    pub tx: tokio::sync::mpsc::Sender<GatewayFrame>,
    pub sent_at: Instant,
    pub retries: u32,
}

pub struct AckManager {
    pending: Mutex<HashMap<Uuid, PendingMessage>>,
    seen: Mutex<HashMap<Uuid, Instant>>,
}

impl AckManager {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Default for AckManager {
    fn default() -> Self {
        Self {
            pending: Mutex::new(HashMap::new()),
            seen: Mutex::new(HashMap::new()),
        }
    }
}
impl AckManager {
    pub async fn add_pending(&self, frame: GatewayFrame, tx: tokio::sync::mpsc::Sender<GatewayFrame>) {
        let mut pending = self.pending.lock().await;
        pending.insert(
            frame.request_id,
            PendingMessage {
                frame,
                tx,
                sent_at: Instant::now(),
                retries: 0,
            },
        );
    }

    pub async fn remove_pending(&self, id: &Uuid) -> Option<PendingMessage> {
        let mut pending = self.pending.lock().await;
        pending.remove(id)
    }

    pub async fn is_duplicate(&self, id: &Uuid) -> bool {
        let mut seen = self.seen.lock().await;

        // simple cleanup of old seen UUIDs to prevent memory leak
        let now = Instant::now();
        seen.retain(|_, time| now.duration_since(*time) < Duration::from_secs(300));

        if seen.contains_key(id) {
            true
        } else {
            seen.insert(*id, now);
            false
        }
    }

    pub async fn get_unacked_messages(&self, timeout: Duration) -> Vec<(GatewayFrame, tokio::sync::mpsc::Sender<GatewayFrame>)> {
        let mut pending = self.pending.lock().await;
        let now = Instant::now();
        let mut retries = Vec::new();

        for msg in pending.values_mut() {
            if now.duration_since(msg.sent_at) > timeout {
                msg.retries += 1;
                msg.sent_at = now;
                retries.push((msg.frame.clone(), msg.tx.clone()));
            }
        }
        retries
    }
}
