use server::{build_router, config, gateway, presence, storage, telemetry, AppState};
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::info;
use uuid::Uuid;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    telemetry::init();

    let config = config::Config::from_env()?;
    info!(
        "Starting secure WebRTC signaling gateway on port {}",
        config.port
    );

    let db_conn = storage::init_db(&config.turso_url, &config.turso_token).await?;
    let gateway_state = Arc::new(gateway::GatewayState::new(Uuid::new_v4().to_string()));

    let presence_manager = Arc::new(presence::PresenceManager::new());

    let state = Arc::new(AppState {
        config: config.clone(),
        db: Arc::new(db_conn),
        gateway: gateway_state,
        presence: presence_manager,
    });

    let ack_manager_clone = state.gateway.ack_manager.clone();
    tokio::spawn(async move {
        gateway::retry::retry_task(ack_manager_clone).await;
    });

    let app = build_router(state.clone());

    let listener = TcpListener::bind(format!("0.0.0.0:{}", config.port)).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
