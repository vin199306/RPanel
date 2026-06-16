use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub username: String,
    pub password_hash: String,
    pub jwt_secret: String,
    pub jwt_expiry_hours: i64,
    pub session_timeout_minutes: u64,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            username: "admin".to_string(),
            password_hash: "5e884898da28047151d0e56f8dc6292773603d0d6aabbdd62a11ef721d1542d8"
                .to_string(), // "password"
            jwt_secret: Self::generate_secret(),
            jwt_expiry_hours: 24,
            session_timeout_minutes: 30,
        }
    }
}

impl AppConfig {
    pub async fn load_or_default<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        if path.as_ref().exists() {
            let content = tokio::fs::read_to_string(&path).await?;
            let config: AppConfig = serde_json::from_str(&content)?;
            Ok(config)
        } else {
            let config = AppConfig::default();
            config.save(&path).await?;
            Ok(config)
        }
    }

    pub async fn save<P: AsRef<Path>>(&self, path: P) -> anyhow::Result<()> {
        let content = serde_json::to_string_pretty(self)?;
        tokio::fs::write(&path, content).await?;
        Ok(())
    }

    fn generate_secret() -> String {
        use rand::Rng;
        const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
        let mut rng = rand::thread_rng();
        (0..64)
            .map(|_| {
                let idx = rng.gen_range(0..CHARSET.len());
                CHARSET[idx] as char
            })
            .collect()
    }
}
