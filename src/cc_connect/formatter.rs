use crate::cc_connect::store::TopicSession;

/// Format Claude's response text for Telegram.
/// Truncates at 4000 chars (Telegram limit is 4096).
pub fn format_response(text: &str) -> String {
    let text = text.trim();
    if text.len() > 4000 {
        format!(
            "{}…\n\n_(truncated, {} chars total)_",
            &text[..4000],
            text.len()
        )
    } else {
        text.to_string()
    }
}

pub fn format_error(err: &str) -> String {
    format!("❌ Error: {}", err)
}

pub fn format_status(sessions: &[TopicSession]) -> String {
    if sessions.is_empty() {
        return "No active sessions.".to_string();
    }
    let mut lines = vec!["Active sessions:".to_string()];
    for s in sessions {
        let short_id = if s.session_id.len() >= 8 {
            &s.session_id[..8]
        } else {
            &s.session_id
        };
        lines.push(format!(
            "• topic {} in chat {} → session {}…",
            s.thread_id, s.chat_id, short_id
        ));
    }
    lines.join("\n")
}
