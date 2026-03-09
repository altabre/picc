use std::sync::Arc;
use teloxide::prelude::*;

use picc::cc_connect::{
    config::Config,
    formatter,
    session::run_claude,
    store::{SqliteStore, TopicSession},
};

#[tokio::main]
async fn main() {
    pretty_env_logger::init();

    let config = Arc::new(Config::from_env().expect("Failed to load config"));
    let store = Arc::new(SqliteStore::open(&config.db_path).expect("Failed to open database"));

    let bot = Bot::new(&config.telegram_bot_token);

    log::info!("cc-connect bot started (@lmozi_bot)");

    teloxide::repl(bot, move |bot: Bot, msg: Message| {
        let config = Arc::clone(&config);
        let store = Arc::clone(&store);
        async move { handle_message(bot, msg, config, store).await }
    })
    .await;
}

async fn handle_message(
    bot: Bot,
    msg: Message,
    config: Arc<Config>,
    store: Arc<SqliteStore>,
) -> ResponseResult<()> {
    // 1. Only respond to allowed users
    let user_id = msg.from.as_ref().map(|u| u.id.0).unwrap_or(0);
    if !config.is_allowed(user_id) {
        return Ok(());
    }

    // 2. Only handle text messages
    let text = match msg.text() {
        Some(t) => t.to_string(),
        None => return Ok(()),
    };

    let chat_id = msg.chat.id;
    let thread_id = msg.thread_id.map(|t| t.0 .0).unwrap_or(0);

    // 3. Handle /status command
    if text == "/status" {
        let sessions = store.list_all();
        let reply = formatter::format_status(&sessions);
        let mut req = bot.send_message(chat_id, reply);
        if let Some(tid) = msg.thread_id {
            req = req.message_thread_id(tid);
        }
        req.await?;
        return Ok(());
    }

    // 4. Handle /new command — clears session for this topic (not persisted yet, just skip resume)
    let force_new = text == "/new";
    if force_new {
        let mut req = bot.send_message(chat_id, "Starting a new session for this topic.");
        if let Some(tid) = msg.thread_id {
            req = req.message_thread_id(tid);
        }
        req.await?;
        return Ok(());
    }

    // 5. Look up existing session for this topic
    let existing = store.get(chat_id.0, thread_id);
    let session_id_opt = existing.as_ref().map(|s| s.session_id.clone());
    let cwd = existing
        .as_ref()
        .map(|s| s.cwd.clone())
        .unwrap_or_else(|| config.approved_directory.clone());

    // 6. Send "thinking" indicator
    let mut thinking_req = bot.send_message(chat_id, "⏳");
    if let Some(tid) = msg.thread_id {
        thinking_req = thinking_req.message_thread_id(tid);
    }
    let thinking = thinking_req.await?;

    // 7. Run claude in a blocking thread (subprocess)
    let cwd_owned = cwd.clone();
    let result = tokio::task::spawn_blocking(move || {
        run_claude(&text, &cwd_owned, session_id_opt.as_deref())
    })
    .await
    .unwrap();

    // 8. Delete "thinking" indicator
    bot.delete_message(chat_id, thinking.id).await.ok();

    // 9. Send response
    match result {
        Ok((response, new_session_id)) => {
            // Persist session
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64;
            let _ = store.save(&TopicSession {
                chat_id: chat_id.0,
                thread_id,
                session_id: new_session_id,
                cwd,
                created_at: existing.map(|s| s.created_at).unwrap_or(now),
            });

            let reply_text = formatter::format_response(&response);
            let mut req = bot.send_message(chat_id, reply_text);
            if let Some(tid) = msg.thread_id {
                req = req.message_thread_id(tid);
            }
            req.await?;
        }
        Err(e) => {
            let mut req = bot.send_message(chat_id, formatter::format_error(&e));
            if let Some(tid) = msg.thread_id {
                req = req.message_thread_id(tid);
            }
            req.await?;
        }
    }

    Ok(())
}
