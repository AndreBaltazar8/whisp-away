use anyhow::{Context, Result};
use std::process::{Command, Stdio};
use std::io::Write;

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

/// Type text at cursor using wtype (Wayland) or xdotool (X11)
fn type_at_cursor(text: &str, backend_name: &str) -> Result<()> {
    // Try wtype first (Wayland) - use stdin mode to avoid issues with
    // special characters (quotes, dashes, etc.) in transcribed text
    let wtype_result = Command::new("wtype")
        .arg("-")
        .stdin(Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            if let Some(mut stdin) = child.stdin.take() {
                stdin.write_all(text.as_bytes())?;
                drop(stdin);
            }
            child.wait()
        });
    
    if let Ok(status) = wtype_result {
        if status.success() {
            // Show success notification
            Command::new("notify-send")
                .args(&[
                    "Voice Input",
                    &format!("✅ Transcribed\nBackend: {}", backend_name),
                    "-t", "1000",
                    "-h", "string:x-canonical-private-synchronous:voice"
                ])
                .spawn()?;
            return Ok(());
        }
    }
    
    // Fallback to xdotool (X11)
    Command::new("xdotool")
        .args(&["type", "--clearmodifiers", "--", text])
        .spawn()
        .context("Failed to run typing command (tried wtype and xdotool)")?
        .wait()?;
    
    // Show success notification
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