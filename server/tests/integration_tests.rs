use futures_util::StreamExt;
use jsonwebtoken::{encode, EncodingKey, Header};
use reqwest::Client;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio_tungstenite::connect_async;
use uuid::Uuid;

use server::{auth::Claims, build_router, config, gateway::GatewayState, storage, AppState};

async fn spawn_app() -> (String, u16, Arc<AppState>) {
    let mut conf = config::Config::from_env().unwrap();
    // Override to ensure local DB and Redis are used
    conf.turso_url = "file::memory:".to_string();
    let db = storage::init_db(&conf.turso_url, &conf.turso_token)
        .await
        .unwrap();

    let gateway_state = Arc::new(GatewayState::new(Uuid::new_v4().to_string()));

    let state = Arc::new(AppState {
        config: conf.clone(),
        db: Arc::new(db),
        gateway: gateway_state,
    });

    let app = build_router(state.clone());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let gateway_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let gateway_port = gateway_listener.local_addr().unwrap().port();
    let state_clone = state.clone();
    tokio::spawn(async move {
        while let Ok((stream, _)) = gateway_listener.accept().await {
            let s = state_clone.clone();
            tokio::spawn(async move {
                server::gateway::peer::handle_peer_connection(stream, s, false, None).await;
            });
        }
    });

    (format!("127.0.0.1:{}", port), gateway_port, state)
}

fn create_jwt(sub: &str, secret: &str) -> String {
    let claims = Claims {
        sub: sub.to_string(),
        session_id: Uuid::new_v4().to_string(),
        exp: (chrono::Utc::now() + chrono::Duration::hours(1)).timestamp() as usize,
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .unwrap()
}

#[tokio::test]
async fn test_cross_node_signaling() {
    // We need TURSO_AUTH_TOKEN and REDIS_URL in env for the config to parse correctly.
    // If they aren't there, this will fail. Let's make sure they are.
    std::env::set_var("TURSO_URL", "file::memory:");
    std::env::set_var("TURSO_AUTH_TOKEN", "dummy");
    std::env::set_var("JWT_SECRET", uuid::Uuid::new_v4().to_string());
    std::env::set_var("PORT", "8080");

    let (addr1, _gport1, state1) = spawn_app().await;
    let (addr2, gport2, state2) = spawn_app().await;

    // Connect Gateway 1 to Gateway 2
    let stream = tokio::net::TcpStream::connect(format!("127.0.0.1:{}", gport2))
        .await
        .unwrap();
    let peer_id = state2.gateway.local_gateway_id.clone();
    let s1 = state1.clone();
    tokio::spawn(async move {
        server::gateway::peer::handle_peer_connection(stream, s1, true, Some(peer_id)).await;
    });

    // Wait for handshake
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    let secret = &state1.config.jwt_secret;

    let token_a = create_jwt("user_a", secret);
    let token_b = create_jwt("user_b", secret);

    // Connect User B to Gateway B
    let ws_req_b = http::Request::builder()
        .uri(format!("ws://{}/v1/ws", addr2))
        .header("Authorization", format!("Bearer {}", token_b))
        .header("Sec-WebSocket-Key", "dGhlIHNhbXBsZSBub25jZQ==")
        .header("Sec-WebSocket-Version", "13")
        .header("Connection", "Upgrade")
        .header("Upgrade", "websocket")
        .header("Host", addr2.clone())
        .body(())
        .unwrap();

    let (mut ws_b, _) = connect_async(ws_req_b).await.expect("B failed to connect");

    // Wait a bit to ensure connection is active
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Connect User A to Gateway A
    let ws_req_a = http::Request::builder()
        .uri(format!("ws://{}/v1/ws", addr1))
        .header("Authorization", format!("Bearer {}", token_a))
        .header("Sec-WebSocket-Key", "dGhlIHNhbXBsZSBub25jZQ==")
        .header("Sec-WebSocket-Version", "13")
        .header("Connection", "Upgrade")
        .header("Upgrade", "websocket")
        .header("Host", addr1.clone())
        .body(())
        .unwrap();

    let (mut _ws_a, _) = connect_async(ws_req_a).await.expect("A failed to connect");

    // User A starts call to B
    let client = Client::new();
    let resp = client
        .post(format!("http://{}/v1/calls", addr1))
        .header("Authorization", format!("Bearer {}", token_a))
        .json(&serde_json::json!({
            "target_user_id": "user_b",
            "call_type": "audio"
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 201);

    // Consume initial SDUI schema sent on connect
    let _initial_msg = ws_b.next().await.unwrap().unwrap();

    // Check if User B receives the CallIncoming message
    let msg_b = ws_b.next().await.unwrap().unwrap();
    let text = msg_b.to_text().unwrap();
    assert!(text.contains("incoming_call"));
}
