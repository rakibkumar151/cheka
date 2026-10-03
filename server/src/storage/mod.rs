use libsql::{Builder, Database};
use tracing::info;

pub async fn init_db(url: &str, token: &str) -> anyhow::Result<Database> {
    info!("Initializing libSQL/Turso connection");

    // Check if local or remote
    let db = if url.starts_with("libsql://") || url.starts_with("https://") {
        Builder::new_remote(url.to_string(), token.to_string())
            .build()
            .await?
    } else {
        Builder::new_local(url).build().await?
    };

    // Run migrations on a fresh connection, then release it
    let conn = db.connect()?;
    run_migrations(&conn).await?;

    Ok(db)  // Return the Database handle, not the Connection
}

async fn run_migrations(conn: &Connection) -> anyhow::Result<()> {
    info!("Running database migrations...");
    let schema = vec![
        "CREATE TABLE IF NOT EXISTS users (
            id TEXT PRIMARY KEY,
            display_name TEXT NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP
        )",
        "CREATE TABLE IF NOT EXISTS sessions (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            device_id TEXT NOT NULL,
            expires_at DATETIME NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY(user_id) REFERENCES users(id)
        )",
        "CREATE TABLE IF NOT EXISTS devices (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            push_token TEXT,
            FOREIGN KEY(user_id) REFERENCES users(id)
        )",
        "CREATE TABLE IF NOT EXISTS call_sessions (
            id TEXT PRIMARY KEY,
            caller_id TEXT NOT NULL,
            status TEXT NOT NULL,
            kind TEXT NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
        )",
        "CREATE TABLE IF NOT EXISTS call_participants (
            call_id TEXT NOT NULL,
            user_id TEXT NOT NULL,
            role TEXT NOT NULL,
            status TEXT NOT NULL,
            PRIMARY KEY (call_id, user_id),
            FOREIGN KEY(call_id) REFERENCES call_sessions(id)
        )",
        "CREATE TABLE IF NOT EXISTS call_events (
            id TEXT PRIMARY KEY,
            call_id TEXT NOT NULL,
            event_type TEXT NOT NULL,
            payload TEXT,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY(call_id) REFERENCES call_sessions(id)
        )",
        "CREATE TABLE IF NOT EXISTS sdui_documents (
            id TEXT PRIMARY KEY,
            screen TEXT NOT NULL,
            current_revision INTEGER NOT NULL
        )",
        "CREATE TABLE IF NOT EXISTS sdui_revisions (
            id TEXT PRIMARY KEY,
            document_id TEXT NOT NULL,
            revision INTEGER NOT NULL,
            schema JSON NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY(document_id) REFERENCES sdui_documents(id)
        )",
        "CREATE TABLE IF NOT EXISTS feature_flags (
            key TEXT PRIMARY KEY,
            enabled BOOLEAN NOT NULL DEFAULT 0
        )",
        "CREATE TABLE IF NOT EXISTS audit_events (
            id TEXT PRIMARY KEY,
            user_id TEXT,
            action TEXT NOT NULL,
            details TEXT,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP
        )",
    ];

    for stmt in schema {
        conn.execute(stmt, ()).await?;
    }

    info!("Migrations complete.");
    Ok(())
}
