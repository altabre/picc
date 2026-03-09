/// JavaScript injection for iframe access via Safari
use std::process::Command;

pub fn inject_js(script: &str) -> Result<String, String> {
    // 通过 Safari 执行 JavaScript 来穿透 iframe
    // 使用 osascript 调用 Safari 的 JavaScript context

    // 正确转义 JavaScript 字符串
    let escaped = script
        .replace("\\", "\\\\")
        .replace("\"", "\\\"")
        .replace("\n", " ");

    let apple_script = format!(
        "tell application \"Safari\"\n    tell current tab of window 1\n        execute javascript \"{}\"\n    end tell\nend tell",
        escaped
    );

    let output = Command::new("osascript")
        .arg("-e")
        .arg(&apple_script)
        .output()
        .map_err(|e| e.to_string())?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}

pub fn fill_email(email: &str) -> Result<(), String> {
    // 填写邮箱字段

    let js = format!(
        "document.querySelector('input[type=\"email\"]').value = '{}'; document.querySelector('input[type=\"email\"]').dispatchEvent(new Event('input', {{ bubbles: true }}))",
        email.replace("'", "\\'")
    );

    inject_js(&js)?;
    Ok(())
}

pub fn fill_password(password: &str) -> Result<(), String> {
    // 填写密码字段

    let js = format!(
        "document.querySelector('input[type=\"password\"]').value = '{}'; document.querySelector('input[type=\"password\"]').dispatchEvent(new Event('input', {{ bubbles: true }}))",
        password.replace("'", "\\'")
    );

    inject_js(&js)?;
    Ok(())
}

pub fn click_button(selector: &str) -> Result<(), String> {
    // 点击按钮

    let js = format!(
        "document.querySelector('{}').click();",
        selector.replace("'", "\\'")
    );

    inject_js(&js)?;
    Ok(())
}
