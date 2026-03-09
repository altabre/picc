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
        if result.ok {
            result.result.unwrap_or_default()
        } else {
            let desc = result.description.as_deref().unwrap_or("");
            if desc.contains("Conflict") {
                // Another instance has an active long-poll; wait for it to expire
                log::warn!("getUpdates conflict — waiting 35s for previous request to expire...");
                tokio::time::sleep(std::time::Duration::from_secs(35)).await;
            } else {
                log::warn!("getUpdates failed: {:?}", result.description);
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;
            }
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

    async fn edit_message(&self, chat_id: i64, message_id: i64, text: &str, thread_id: Option<i32>) {
        #[derive(serde::Serialize)]
        struct EditParams {
            chat_id: i64,
            message_id: i64,
            text: String,
            #[serde(skip_serializing_if = "Option::is_none")]
            message_thread_id: Option<i32>,
        }
        let params = EditParams { chat_id, message_id, text: text.to_string(), message_thread_id: thread_id };
        let _ = self.client
            .post(self.api_url("editMessageText"))
            .json(&params)
            .send().await;
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
        let updates = bot.get_updates(offset, 5).await;

        for update in updates {
            offset = update.update_id + 1;

            if let Some(msg) = update.message {
                // Debug: log every incoming message before any filtering
                log::debug!(
                    "raw msg: chat={} thread={:?} user={:?} text={:?}",
                    msg.chat.id,
                    msg.message_thread_id,
                    msg.from.as_ref().map(|u| u.id),
                    msg.text.as_deref().map(|t| &t[..t.len().min(40)])
                );
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

    // 5. Send initial progress message
    let progress = bot.send_message(chat_id, "⏳ Thinking...", msg.message_thread_id).await;
    let progress_id = progress.as_ref().map(|m| m.message_id);

    // 6. Create event channel for streaming progress
    let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel::<picc::cc_connect::session::ClaudeEvent>();

    // 7. Spawn progress updater: receives tool events, edits the progress message
    let bot_progress = Arc::clone(&bot);
    let thread_id_opt = msg.message_thread_id;
    let progress_task = tokio::spawn(async move {
        use picc::cc_connect::session::{ClaudeEvent, tool_icon};

        // Lines shown in the progress message (tool calls, results, pre-text)
        let mut progress_lines: Vec<String> = Vec::new();
        // Tool lines only for final message header
        let mut tool_lines: Vec<String> = Vec::new();
        let mut last_edit = std::time::Instant::now();

        let do_edit = |lines: &Vec<String>, mid: i64, bot: &Arc<TgBot>| {
            let _ = (lines, mid, bot); // capture hint
        };
        let _ = do_edit; // suppress unused warning

        while let Some(event) = event_rx.recv().await {
            let line = match event {
                ClaudeEvent::ToolUse { ref name, ref summary } => {
                    let icon = tool_icon(name);
                    let l = if summary.is_empty() {
                        format!("{icon} **{name}**")
                    } else {
                        format!("{icon} **{name}**  `{summary}`")
                    };
                    tool_lines.push(l.clone());
                    Some(l)
                }
                ClaudeEvent::ToolResult(ref output) => {
                    // Show result indented under the tool
                    Some(format!("```\n{output}\n```"))
                }
                ClaudeEvent::PreText(ref text) => {
                    // Claude's reasoning text before calling tools — show immediately
                    let trimmed = text.trim();
                    if trimmed.is_empty() { None }
                    else {
                        // Show up to 200 chars of reasoning
                        let display = if trimmed.len() > 200 {
                            format!("{}…", &trimmed[..200])
                        } else {
                            trimmed.to_string()
                        };
                        Some(format!("💭 {display}"))
                    }
                }
                ClaudeEvent::TextDelta(_) => None,
            };

            if let Some(l) = line {
                progress_lines.push(l);

                // PreText (Claude's reasoning): update immediately, no throttle
                // Other events: throttle to 500ms
                let force_update = matches!(event, ClaudeEvent::PreText(_));
                if force_update || last_edit.elapsed().as_millis() > 500 {
                    if let Some(mid) = progress_id {
                        // Show last 10 lines in progress message
                        let body = progress_lines.iter()
                            .rev().take(10).rev()
                            .cloned().collect::<Vec<_>>().join("\n");
                        let text = format!("⏳ Working...\n\n{body}");
                        // Truncate to Telegram limit
                        let text = if text.len() > 3800 { format!("{}…", &text[..3800]) } else { text };
                        bot_progress.edit_message(chat_id, mid, &text, thread_id_opt).await;
                        last_edit = std::time::Instant::now();
                    }
                }
            }
        }
        (tool_lines, progress_lines)
    });

    // 8. Run claude subprocess in blocking thread
    let cwd_owned = cwd.clone();
    let result = tokio::task::spawn_blocking(move || {
        run_claude(&text, &cwd_owned, session_id_opt.as_deref(), Some(event_tx))
    })
    .await
    .unwrap();

    // Wait for progress task to drain remaining events
    let (tool_lines, _progress_lines) = progress_task.await.unwrap_or_default();

    // 9. Send response
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

            // Build final message: tool summary + response
            let reply_text = formatter::format_response(&response, &tool_lines);

            // Edit progress message into final response (no flash/delete)
            if let Some(mid) = progress_id {
                bot.edit_message(chat_id, mid, &reply_text, msg.message_thread_id).await;
            } else {
                bot.send_message(chat_id, &reply_text, msg.message_thread_id).await;
            }
        }
        Err(e) => {
            let err_text = formatter::format_error(&e);
            if let Some(mid) = progress_id {
                bot.edit_message(chat_id, mid, &err_text, msg.message_thread_id).await;
            } else {
                bot.send_message(chat_id, &err_text, msg.message_thread_id).await;
            }
        }
    }
}
