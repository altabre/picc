use crate::cc_connect::store::TopicSession;

/// Format Claude's final response with optional tool activity summary.
///
/// Layout (if tools were used):
/// ```
/// 📖 Read  `file.rs`
/// 💻 Bash  `cargo build`
/// ───────────────
/// <response text>
/// ```
pub fn format_response(text: &str, tool_lines: &[String]) -> String {
    let text = text.trim();

    let header = if tool_lines.is_empty() {
        String::new()
    } else {
        let summary = tool_lines.iter()
            .rev().take(8).rev()
            .cloned().collect::<Vec<_>>().join("\n");
        format!("{}\n─────────────\n", summary)
    };

    let full = format!("{}{}", header, text);

    if full.len() > 4000 {
        let limit = 4000usize.saturating_sub(header.len());
        format!(
            "{}{}…\n_(truncated, {} chars total)_",
            header,
            &text[..limit],
            text.len()
        )
    } else {
        full
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
