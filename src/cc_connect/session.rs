use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};

/// A progress event emitted while Claude is running.
#[derive(Debug, Clone)]
pub enum ClaudeEvent {
    /// Claude used a tool (e.g. Read, Bash, Write)
    ToolUse { name: String, summary: String },
    /// Streaming text from Claude's response
    TextDelta(String),
}

/// Map tool name to an emoji icon.
pub fn tool_icon(name: &str) -> &'static str {
    match name {
        "Read"        => "📖",
        "Write"       => "✏️",
        "Edit"        => "✏️",
        "MultiEdit"   => "✏️",
        "Bash"        => "💻",
        "Grep"        => "🔍",
        "Glob"        => "🗂️",
        "WebFetch"    => "🌐",
        "WebSearch"   => "🔎",
        "TodoWrite"   => "📝",
        "Task"        => "🤖",
        _             => "⚙️",
    }
}

/// Summarize tool input to a short string for display.
fn summarize_input(name: &str, input: &serde_json::Value) -> String {
    match name {
        "Read" | "Write" | "Edit" | "MultiEdit" => {
            input["file_path"].as_str()
                .or_else(|| input["path"].as_str())
                .map(|p| {
                    // Show only last 2 path components
                    let parts: Vec<&str> = p.split('/').filter(|s| !s.is_empty()).collect();
                    if parts.len() > 2 {
                        format!("…/{}/{}", parts[parts.len()-2], parts[parts.len()-1])
                    } else {
                        p.to_string()
                    }
                })
                .unwrap_or_default()
        }
        "Bash" => {
            input["command"].as_str()
                .map(|c| {
                    let c = c.trim();
                    if c.len() > 40 { format!("{}…", &c[..40]) } else { c.to_string() }
                })
                .unwrap_or_default()
        }
        "Grep" => {
            let pattern = input["pattern"].as_str().unwrap_or("");
            let path = input["path"].as_str().unwrap_or("");
            if path.is_empty() { pattern.to_string() }
            else { format!("{pattern} in …{}", path.split('/').last().unwrap_or(path)) }
        }
        "WebFetch" | "WebSearch" => {
            input["url"].as_str()
                .or_else(|| input["query"].as_str())
                .map(|s| if s.len() > 40 { format!("{}…", &s[..40]) } else { s.to_string() })
                .unwrap_or_default()
        }
        _ => String::new(),
    }
}

/// Run claude CLI, streaming progress events via `event_tx`.
/// Returns (final_text, session_id).
pub fn run_claude(
    message: &str,
    cwd: &str,
    session_id: Option<&str>,
    event_tx: Option<tokio::sync::mpsc::UnboundedSender<ClaudeEvent>>,
) -> Result<(String, String), String> {
    let mut cmd = Command::new("claude");
    cmd.arg("-p")
        .arg(message)
        .arg("--output-format")
        .arg("stream-json")
        .arg("--verbose")
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env_remove("CLAUDECODE")
        .env_remove("CLAUDE_CODE_ENTRYPOINT");

    if let Some(sid) = session_id {
        cmd.arg("--resume").arg(sid);
    }

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Failed to spawn claude: {}", e))?;
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let reader = BufReader::new(stdout);

    std::thread::spawn(move || {
        for line in BufReader::new(stderr).lines().flatten() {
            eprintln!("[claude stderr] {line}");
        }
    });

    let mut full_text = String::new();
    let mut last_session_id = String::new();

    for line in reader.lines() {
        let line = match line {
            Ok(l) if l.is_empty() => continue,
            Ok(l) => l,
            Err(_) => break,
        };

        let v: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        if let Some(sid) = v["session_id"].as_str() {
            last_session_id = sid.to_string();
        }

        match v["type"].as_str().unwrap_or("") {
            "assistant" => {
                if let Some(content) = v["message"]["content"].as_array() {
                    for block in content {
                        match block["type"].as_str().unwrap_or("") {
                            "text" => {
                                if let Some(text) = block["text"].as_str() {
                                    full_text.push_str(text);
                                    if let Some(tx) = &event_tx {
                                        let _ = tx.send(ClaudeEvent::TextDelta(text.to_string()));
                                    }
                                }
                            }
                            "tool_use" => {
                                if let Some(tx) = &event_tx {
                                    let name = block["name"].as_str().unwrap_or("tool").to_string();
                                    let summary = summarize_input(&name, &block["input"]);
                                    let _ = tx.send(ClaudeEvent::ToolUse { name, summary });
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            "result" => {
                if let Some(result) = v["result"].as_str() {
                    if full_text.is_empty() {
                        full_text = result.to_string();
                    }
                }
                if let Some(sid) = v["session_id"].as_str() {
                    last_session_id = sid.to_string();
                }
            }
            _ => {}
        }
    }

    child.wait().ok();

    if last_session_id.is_empty() {
        return Err("No session_id returned from claude — is claude CLI installed and authenticated?".to_string());
    }
    if full_text.is_empty() {
        full_text = "(no response)".to_string();
    }

    Ok((full_text, last_session_id))
}
