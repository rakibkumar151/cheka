use std::collections::HashMap;
use tokio::sync::RwLock;
use tokio::time::Instant;

#[derive(Debug, Clone)]
pub struct UserPresence {
    pub is_online: bool,
    pub call_state: String,
    pub gateway_id: String,
    pub last_seen: Instant,
}

pub struct PresenceManager {
    users: RwLock<HashMap<String, UserPresence>>,
}

impl PresenceManager {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Default for PresenceManager {
    fn default() -> Self {
        Self {
            users: RwLock::new(HashMap::new()),
        }
    }
}

impl PresenceManager {
    pub async fn update_presence(&self, user_id: &str, presence: UserPresence) {
        self.users.write().await.insert(user_id.to_string(), presence);
    }

    pub async fn get_online_users(&self) -> Vec<(String, UserPresence)> {
        let users = self.users.read().await;
        // Also cleanup stale entries here or in a background task
        let now = Instant::now();
        users
            .iter()
            .filter(|(_, p)| p.is_online && now.duration_since(p.last_seen).as_secs() < 300)
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }
}
