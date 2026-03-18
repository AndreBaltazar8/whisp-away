use anyhow::{Context, Result};
use std::process::{Command, Stdio};
use std::io::Write;

/// Typing method for text output
#[derive(Clone, Debug, PartialEq)]
pub enum TypingMethod {
    /// Clipboard paste via ydotool (Ctrl+V) — instant, works in Chrome
    Paste,
    /// Virtual keystrokes via ydotool type (Wayland/X11)
    Ydotool,
    /// Virtual keystrokes via wtype (Wayland)
    Wtype,
    /// Virtual keystrokes via xdotool (X11)
    Xdotool,
    /// Try paste first, fall back to ydotool, wtype, then xdotool
    Auto,
}

impl TypingMethod {
    pub fn from_str(s: &str) -> Self {
        match s {
            "paste" => Self::Paste,
            "ydotool" => Self::Ydotool,
            "wtype" => Self::Wtype,
            "xdotool" => Self::Xdotool,
            _ => Self::Auto,
        }
    }
}

/// Resolve the typing method from CLI env override, then config
pub fn resolve_typing_method() -> TypingMethod {
    // CLI override via env var
    if let Ok(method) = std::env::var("WHISP_AWAY_TYPING_METHOD") {
        return TypingMethod::from_str(&method);
    }
    // Config file
    crate::config::read_config()
        .and_then(|c| c.typing_method)
        .map(|s| TypingMethod::from_str(&s))
        .unwrap_or(TypingMethod::Auto)
}

/// Output transcribed text to clipboard or type at cursor
pub fn output_text(text: &str, use_clipboard: bool, backend_name: &str) -> Result<()> {
    if text.trim().is_empty() {
        Command::new("notify-send")
            .args(&[
                "Voice Input",
                &format!("⚠️ No speech detected\nBackend: {}", backend_name),
                "-t", "2000",
                "-h", "string:x-canonical-private-synchronous:voice"
            ])
            .spawn()?;
        return Ok(());
    }

    if use_clipboard {
        copy_to_clipboard(text.trim())?;

        Command::new("notify-send")
            .args(&[
                "Voice Input",
                &format!("✅ Copied to clipboard\nBackend: {}", backend_name),
                "-t", "1000",
                "-h", "string:x-canonical-private-synchronous:voice"
            ])
            .spawn()?;
    } else {
        // Small delay before typing
        std::thread::sleep(std::time::Duration::from_millis(30));

        type_at_cursor(text.trim(), backend_name)?;
    }

    Ok(())
}

/// Type text at cursor using the configured typing method
fn type_at_cursor(text: &str, backend_name: &str) -> Result<()> {
    let method = resolve_typing_method();

    let success = match method {
        TypingMethod::Paste => type_via_paste(text),
        TypingMethod::Ydotool => type_via_ydotool(text),
        TypingMethod::Wtype => type_via_wtype(text),
        TypingMethod::Xdotool => type_via_xdotool(text),
        TypingMethod::Auto => {
            type_via_paste(text)
                .or_else(|_| type_via_ydotool(text))
                .or_else(|_| type_via_wtype(text))
                .or_else(|_| type_via_xdotool(text))
        }
    };

    success.context("Failed to type text (check typing_method in config)")?;

    Command::new("notify-send")
        .args(&[
            "Voice Input",
            &format!("✅ Transcribed\nBackend: {}", backend_name),
            "-t", "1000",
            "-h", "string:x-canonical-private-synchronous:voice"
        ])
        .spawn()?;

    Ok(())
}

/// Copy to clipboard, paste with ydotool (Ctrl+V), then restore previous clipboard
fn type_via_paste(text: &str) -> Result<()> {
    // Save current clipboard content
    let prev_clipboard = Command::new("wl-paste")
        .arg("--no-newline")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| o.stdout);

    copy_to_clipboard(text)?;

    // ydotool key codes: 29=Ctrl, 47=V
    let status = Command::new("ydotool")
        .args(&["key", "29:1", "47:1", "47:0", "29:0"])
        .status()
        .context("Failed to run ydotool")?;

    if !status.success() {
        return Err(anyhow::anyhow!("ydotool failed"));
    }

    // Small delay to ensure paste completes before we change clipboard
    std::thread::sleep(std::time::Duration::from_millis(50));

    // Remove the transcribed text entry from cliphist
    let _ = Command::new("cliphist")
        .arg("delete")
        .stdin(Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            if let Some(mut stdin) = child.stdin.take() {
                stdin.write_all(text.as_bytes())?;
                drop(stdin);
            }
            child.wait()
        });

    // Restore previous clipboard content
    if let Some(prev) = prev_clipboard {
        let _ = Command::new("wl-copy")
            .stdin(Stdio::piped())
            .spawn()
            .and_then(|mut child| {
                if let Some(mut stdin) = child.stdin.take() {
                    stdin.write_all(&prev)?;
                    drop(stdin);
                }
                child.wait()
            });
    }

    Ok(())
}

/// Type via ydotool type (Wayland/X11) with no key delay
fn type_via_ydotool(text: &str) -> Result<()> {
    let status = Command::new("ydotool")
        .args(&["type", "--key-delay", "0", "--", text])
        .status()
        .context("Failed to run ydotool")?;
    if status.success() {
        Ok(())
    } else {
        Err(anyhow::anyhow!("ydotool type failed"))
    }
}

/// Type via wtype (Wayland) using stdin
fn type_via_wtype(text: &str) -> Result<()> {
    let mut child = Command::new("wtype")
        .arg("-")
        .stdin(Stdio::piped())
        .spawn()
        .context("Failed to run wtype")?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(text.as_bytes())?;
        drop(stdin);
    }
    let status = child.wait()?;
    if status.success() {
        Ok(())
    } else {
        Err(anyhow::anyhow!("wtype failed"))
    }
}

/// Type via xdotool (X11)
fn type_via_xdotool(text: &str) -> Result<()> {
    let status = Command::new("xdotool")
        .args(&["type", "--clearmodifiers", "--", text])
        .status()
        .context("Failed to run xdotool")?;
    if status.success() {
        Ok(())
    } else {
        Err(anyhow::anyhow!("xdotool failed"))
    }
}

/// Copy text to clipboard using wl-copy (Wayland) or xclip (X11)
fn copy_to_clipboard(text: &str) -> Result<()> {
    // Try wl-copy first (Wayland)
    let wl_copy_result = Command::new("wl-copy")
        .stdin(Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            if let Some(mut stdin) = child.stdin.take() {
                stdin.write_all(text.as_bytes())?;
                drop(stdin);
            }
            child.wait()
        });

    if let Ok(status) = wl_copy_result {
        if status.success() {
            return Ok(());
        }
    }

    // Fallback to xclip (X11)
    let mut child = Command::new("xclip")
        .args(&["-selection", "clipboard"])
        .stdin(Stdio::piped())
        .spawn()
        .context("Failed to run clipboard command (tried wl-copy and xclip)")?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(text.as_bytes())?;
        drop(stdin);
    }

    child.wait()
        .context("Clipboard command failed")?;

    Ok(())
}

/// Legacy function for backwards compatibility - uses typing mode
pub fn type_text(text: &str, backend_name: &str) -> Result<()> {
    output_text(text, false, backend_name)
}
