use std::collections::HashSet;

pub struct Config {
    pub telegram_bot_token: String,
    pub approved_directory: String,
    pub allowed_users: HashSet<u64>,
    pub db_path: String,
    pub tmux_session: String,
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        // Load .env file if present
        let _ = dotenvy::dotenv();

        let token = std::env::var("TELEGRAM_BOT_TOKEN")
            .map_err(|_| "TELEGRAM_BOT_TOKEN not set".to_string())?;

        let approved_dir = std::env::var("APPROVED_DIRECTORY")
            .unwrap_or_else(|_| "/home/coder".to_string());

        let allowed_users: HashSet<u64> = std::env::var("ALLOWED_USERS")
            .unwrap_or_default()
            .split(',')
            .filter_map(|s| s.trim().parse().ok())
            .collect();

        let db_path = std::env::var("DB_PATH")
            .unwrap_or_else(|_| "cc_connect.db".to_string());

        Ok(Config {
            telegram_bot_token: token,
            approved_directory: approved_dir,
            allowed_users,
            db_path,
            tmux_session: "cc-connect".to_string(),
        })
    }

    pub fn is_allowed(&self, user_id: u64) -> bool {
        self.allowed_users.is_empty() || self.allowed_users.contains(&user_id)
    }
}
