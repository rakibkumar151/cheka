use super::ack::AckManager;
use super::protocol::GatewayFrame;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::time::{interval, Duration};

pub async fn retry_task(ack_manager: Arc<AckManager>) {
    let mut ticker = interval(Duration::from_millis(500));

    loop {
        ticker.tick().await;
        let retries = ack_manager
            .get_unacked_messages(Duration::from_millis(1000))
            .await;
        for (frame, tx) in retries {
            let _ = tx.send(frame).await;
        }
    }
}
