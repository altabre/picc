use std::collections::HashSet;
use std::env;

fn dirs_next() -> std::path::PathBuf {
    let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
    std::path::PathBuf::from(home).join(".cc-connect").join("config.toml")
}

fn set_from_toml(val: &toml::Value, key_path: &str, env_key: &str) {
    if env::var(env_key).is_ok() { return; } // already set, don't override
    let mut cur = val;
    for part in key_path.split('.') {
        cur = match cur.get(part) { Some(v) => v, None => return };
    }
    let s = match cur {
        toml::Value::String(s) => s.clone(),
        toml::Value::Array(arr) => arr.iter()
            .filter_map(|v| v.as_str().or_else(|| v.as_integer().map(|_| "")).map(|_| {
                match v {
                    toml::Value::String(s) => s.clone(),
                    toml::Value::Integer(i) => i.to_string(),
                    _ => String::new(),
                }
            }))
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(","),
        toml::Value::Integer(i) => i.to_string(),
        _ => return,
    };
    if !s.is_empty() {
        env::set_var(env_key, s);
    }
}

/// Load order: CLI flag > config.toml > .env > defaults
///
/// Config file locations checked in order:
///   ./config.toml
///   ~/.cc-connect/config.toml
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
        // Load config.toml (./config.toml or ~/.cc-connect/config.toml)
        Self::load_toml_to_env();
        // Then load .env (overrides toml values)
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

    /// Load config.toml values into env vars (only if not already set)
    fn load_toml_to_env() {
        let paths = [
            std::path::PathBuf::from("config.toml"),
            dirs_next(),
        ];
        for path in &paths {
            if path.exists() {
                if let Ok(content) = std::fs::read_to_string(path) {
                    if let Ok(val) = content.parse::<toml::Value>() {
                        set_from_toml(&val, "bot.telegram_bot_token", "TELEGRAM_BOT_TOKEN");
                        set_from_toml(&val, "bot.approved_directory", "APPROVED_DIRECTORY");
                        set_from_toml(&val, "bot.allowed_users", "ALLOWED_USERS");
                        set_from_toml(&val, "bot.allowed_topics", "ALLOWED_TOPICS");
                        set_from_toml(&val, "db.path", "DB_PATH");
                    }
                }
                break;
            }
        }
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
