use std::env;

#[derive(Clone, Debug)]
pub struct Config {
    pub port: u16,
    pub turso_url: String,
    pub turso_token: String,
    pub jwt_secret: String,
    pub turn_secret: String,
    pub turn_url: String,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        let _ = dotenvy::dotenv();

        Ok(Self {
            port: env::var("PORT").unwrap_or_else(|_| "8080".into()).parse()?,
            turso_url: env::var("TURSO_DATABASE_URL").unwrap_or_else(|_| "file::memory:".into()),
            turso_token: env::var("TURSO_AUTH_TOKEN").unwrap_or_else(|_| "".into()),
            jwt_secret: env::var("JWT_SECRET").expect("JWT_SECRET must be set in environment"),
            turn_secret: env::var("TURN_SECRET").unwrap_or_else(|_| "changeme_turn_secret_32chars_xxx".into()),
            turn_url: env::var("TURN_URL").unwrap_or_else(|_| "turn:10.0.2.2:3478".into()),
        })
    }
}
