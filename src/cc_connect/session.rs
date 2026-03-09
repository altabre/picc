use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};

/// Run claude CLI and collect the response text + session_id.
///
/// Uses `claude -p "message" --output-format stream-json` for new sessions,
/// and `--resume <session_id>` for continuations.
pub fn run_claude(
    message: &str,
    cwd: &str,
    session_id: Option<&str>,
) -> Result<(String, String), String> {
    let mut cmd = Command::new("claude");
    cmd.arg("-p")
        .arg(message)
        .arg("--output-format")
        .arg("stream-json")
        .arg("--no-color")
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::null());

    if let Some(sid) = session_id {
        cmd.arg("--resume").arg(sid);
    }

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Failed to spawn claude: {}", e))?;
    let stdout = child.stdout.take().unwrap();
    let reader = BufReader::new(stdout);

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

        // Extract session_id from any event that carries it
        if let Some(sid) = v["session_id"].as_str() {
            last_session_id = sid.to_string();
        }

        // Extract text content from assistant messages
        match v["type"].as_str().unwrap_or("") {
            "assistant" => {
                if let Some(content) = v["message"]["content"].as_array() {
                    for block in content {
                        if block["type"] == "text" {
                            if let Some(text) = block["text"].as_str() {
                                full_text.push_str(text);
                            }
                        }
                    }
                }
            }
            "result" => {
                // Final result event — use as fallback if no assistant text collected
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
