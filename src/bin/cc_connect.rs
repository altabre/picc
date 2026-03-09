/// cc-connect: Telegram bot bridging Claude Code sessions to forum topics.
///
/// Uses direct HTTP long-polling (no teloxide dispatcher) to avoid
/// teloxide's getWebhookInfo compatibility issues with newer Bot API fields.
use std::sync::Arc;
use serde::{Deserialize, Serialize};

use picc::cc_connect::{
    config::Config,
    formatter,
    session::run_claude,
    store::{SqliteStore, TopicSession},
};

// --- Minimal Telegram API types ---

#[derive(Debug, Deserialize)]
struct TgResponse<T> {
    ok: bool,
    result: Option<T>,
    description: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
struct TgUser {
    id: u64,
}

#[derive(Debug, Deserialize, Clone)]
struct TgChat {
    id: i64,
}

#[derive(Debug, Deserialize, Clone)]
struct TgMessage {
    message_id: i64,
    from: Option<TgUser>,
    chat: TgChat,
    /// Forum topic thread ID (present for supergroup forum messages)
    message_thread_id: Option<i32>,
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TgUpdate {
    update_id: i64,
    message: Option<TgMessage>,
}

#[derive(Debug, Serialize)]
struct SendMessageParams {
    chat_id: i64,
    text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    message_thread_id: Option<i32>,
}

#[derive(Debug, Serialize)]
struct DeleteMessageParams {
    chat_id: i64,
    message_id: i64,
}

// --- Telegram API client ---

struct TgBot {
    token: String,
    client: reqwest::Client,
}

impl TgBot {
    fn new(token: &str) -> Self {
        Self {
            token: token.to_string(),
            client: reqwest::Client::new(),
        }
    }

    fn api_url(&self, method: &str) -> String {
        format!("https://api.telegram.org/bot{}/{}", self.token, method)
    }

    async fn delete_webhook(&self) {
        let _ = self.client
            .post(self.api_url("deleteWebhook"))
            .send()
            .await;
    }

    async fn get_updates(&self, offset: i64, timeout: u64) -> Vec<TgUpdate> {
        let url = format!(
            "{}?offset={}&timeout={}&limit=100&allowed_updates=[\"message\"]",
            self.api_url("getUpdates"), offset, timeout
        );
        let resp = match self.client.get(&url).timeout(
            std::time::Duration::from_secs(timeout + 10)
        ).send().await {
            Ok(r) => r,
            Err(e) => { log::warn!("getUpdates error: {e}"); return vec![]; }
        };
        let result: TgResponse<Vec<TgUpdate>> = match resp.json().await {
            Ok(r) => r,
            Err(e) => { log::warn!("getUpdates parse error: {e}"); return vec![]; }
        };
        if result.ok { result.result.unwrap_or_default() } else {
            log::warn!("getUpdates failed: {:?}", result.description);
            vec![]
        }
    }

    async fn send_message(&self, chat_id: i64, text: &str, thread_id: Option<i32>) -> Option<TgMessage> {
        let params = SendMessageParams {
            chat_id,
            text: text.to_string(),
            message_thread_id: thread_id,
        };
        let resp = match self.client
            .post(self.api_url("sendMessage"))
            .json(&params)
            .send().await
        {
            Ok(r) => r,
            Err(e) => { log::warn!("sendMessage error: {e}"); return None; }
        };
        let result: TgResponse<TgMessage> = match resp.json().await {
            Ok(r) => r,
            Err(_) => return None,
        };
        result.result
    }

    async fn delete_message(&self, chat_id: i64, message_id: i64) {
        let params = DeleteMessageParams { chat_id, message_id };
        let _ = self.client
            .post(self.api_url("deleteMessage"))
            .json(&params)
            .send().await;
    }
}

// --- Main ---

#[tokio::main]
async fn main() {
    pretty_env_logger::init();

    let config = Arc::new(Config::from_env().expect("Failed to load config"));
    let store = Arc::new(
        SqliteStore::open(&config.db_path).expect("Failed to open database")
    );
    let bot = Arc::new(TgBot::new(&config.telegram_bot_token));

    // Clear any existing webhook so long-polling works cleanly
    bot.delete_webhook().await;
    if config.allowed_topics.is_empty() {
        log::info!("cc-connect started — listening to ALL topics");
    } else {
        let topics: Vec<_> = config.allowed_topics.iter().collect();
        log::info!("cc-connect started — allowed topics: {:?}", topics);
    }

    let mut offset: i64 = 0;

    loop {
        let updates = bot.get_updates(offset, 30).await;

        for update in updates {
            offset = update.update_id + 1;

            if let Some(msg) = update.message {
                let bot = Arc::clone(&bot);
                let config = Arc::clone(&config);
                let store = Arc::clone(&store);
                tokio::spawn(async move {
                    handle_message(bot, msg, config, store).await;
                });
            }
        }
    }
}

async fn handle_message(
    bot: Arc<TgBot>,
    msg: TgMessage,
    config: Arc<Config>,
    store: Arc<SqliteStore>,
) {
    // 1. Only respond to allowed users
    let user_id = msg.from.as_ref().map(|u| u.id).unwrap_or(0);
    if !config.is_allowed_user(user_id) {
        return;
    }

    let text = match &msg.text {
        Some(t) => t.clone(),
        None => return,
    };

    let chat_id = msg.chat.id;
    let thread_id = msg.message_thread_id.unwrap_or(0);

    // 2. Only respond to allowed topics (if configured)
    if !config.is_allowed_topic(thread_id) {
        log::debug!("ignoring thread={thread_id} (not in ALLOWED_TOPICS)");
        return;
    }

    log::info!("msg chat={chat_id} thread={thread_id} user={user_id}: {text:?}");

    // 2. /status command
    if text == "/status" {
        let sessions = store.list_all();
        let reply = formatter::format_status(&sessions);
        bot.send_message(chat_id, &reply, msg.message_thread_id).await;
        return;
    }

    // 3. /new command — clear session for this topic
    if text == "/new" {
        let _ = store.delete(chat_id, thread_id);
        bot.send_message(
            chat_id,
            "🆕 Session cleared. Next message starts a fresh Claude session.",
            msg.message_thread_id,
        ).await;
        return;
    }

    // 4. Look up existing session for this topic
    let existing = store.get(chat_id, thread_id);
    let session_id_opt = existing.as_ref().map(|s| s.session_id.clone());
    let cwd = existing
        .as_ref()
        .map(|s| s.cwd.clone())
        .unwrap_or_else(|| config.approved_directory.clone());

    // 5. Send thinking indicator
    let thinking = bot.send_message(chat_id, "⏳", msg.message_thread_id).await;

    // 6. Run claude subprocess in blocking thread
    let cwd_owned = cwd.clone();
    let result = tokio::task::spawn_blocking(move || {
        run_claude(&text, &cwd_owned, session_id_opt.as_deref())
    })
    .await
    .unwrap();

    // 7. Delete thinking indicator
    if let Some(t) = thinking {
        bot.delete_message(chat_id, t.message_id).await;
    }

    // 8. Send response
    match result {
        Ok((response, new_session_id)) => {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64;
            let _ = store.save(&TopicSession {
                chat_id,
                thread_id,
                session_id: new_session_id,
                cwd,
                created_at: existing.map(|s| s.created_at).unwrap_or(now),
            });
            bot.send_message(
                chat_id,
                &formatter::format_response(&response),
                msg.message_thread_id,
            ).await;
        }
        Err(e) => {
            bot.send_message(
                chat_id,
                &formatter::format_error(&e),
                msg.message_thread_id,
            ).await;
        }
    }
}
