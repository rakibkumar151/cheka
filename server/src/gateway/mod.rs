pub mod ack;
pub mod framing;
pub mod identity;
pub mod peer;
pub mod protocol;
pub mod retry;
pub mod routing;

use std::sync::Arc;

// The central state for the TCP gateway
pub struct GatewayState {
    pub local_gateway_id: String,
    pub routing: Arc<routing::GatewayRoutingTable>,
    pub ack_manager: Arc<ack::AckManager>,
}

impl GatewayState {
    pub fn new(gateway_id: String) -> Self {
        Self {
            local_gateway_id: gateway_id,
            routing: Arc::new(routing::GatewayRoutingTable::new()),
            ack_manager: Arc::new(ack::AckManager::new()),
        }
    }
}

use tokio::net::TcpListener;
use tracing::info;

pub async fn start_server(addr: &str, state: Arc<crate::AppState>) -> std::io::Result<()> {
    let listener = TcpListener::bind(addr).await?;
    info!("Gateway TCP server listening on {}", listener.local_addr()?);

    let state_clone = state.clone();
    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            let s = state_clone.clone();
            tokio::spawn(async move {
                peer::handle_peer_connection(stream, s, false, None).await;
            });
        }
    });

    Ok(())
}
