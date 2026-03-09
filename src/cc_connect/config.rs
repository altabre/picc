use std::collections::HashSet;

pub struct Config {
    pub telegram_bot_token: String,
    pub approved_directory: String,
    pub allowed_users: HashSet<u64>,
    /// Allowed Telegram forum topic thread IDs.
    /// Empty = respond to all topics.
    /// Set via ALLOWED_TOPICS=2,84 (comma-separated thread_ids from topic URLs)
    /// e.g. https://t.me/3816634435/84 → thread_id = 84
    pub allowed_topics: HashSet<i32>,
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

        // Parse ALLOWED_TOPICS from URLs or raw IDs:
        //   ALLOWED_TOPICS=2,84
        //   ALLOWED_TOPICS=https://t.me/3816634435/84,https://t.me/3816634435/2
        let allowed_topics: HashSet<i32> = std::env::var("ALLOWED_TOPICS")
            .unwrap_or_default()
            .split(',')
            .filter_map(|s| {
                let s = s.trim();
                if s.is_empty() { return None; }
                // Extract last path segment from URL: https://t.me/xxx/84 → 84
                if s.contains("t.me/") {
                    s.rsplit('/').next()?.parse().ok()
                } else {
                    s.parse().ok()
                }
            })
            .collect();

        let db_path = std::env::var("DB_PATH")
            .unwrap_or_else(|_| "cc_connect.db".to_string());

        Ok(Config {
            telegram_bot_token: token,
            approved_directory: approved_dir,
            allowed_users,
            allowed_topics,
            db_path,
            tmux_session: "cc-connect".to_string(),
        })
    }

    pub fn is_allowed_user(&self, user_id: u64) -> bool {
        self.allowed_users.is_empty() || self.allowed_users.contains(&user_id)
    }

    /// Check if a thread_id is in the allowed topics list.
    /// thread_id=0 means the message is in the general (non-topic) area.
    pub fn is_allowed_topic(&self, thread_id: i32) -> bool {
        self.allowed_topics.is_empty() || self.allowed_topics.contains(&thread_id)
    }
}
