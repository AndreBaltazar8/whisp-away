use anyhow::{Context, Result};
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::process::Command;
use crate::typing;

/// Send a transcription request to the daemon via Unix socket
pub fn send_transcription_request(
    socket_path: &str,
    audio_file: &str,
    backend_name: &str,
    use_clipboard: bool,
) -> Result<()> {
    match UnixStream::connect(socket_path) {
        Ok(mut stream) => {
            // Send request
            let request = format!(r#"{{"audio_path": "{}"}}"#, audio_file);
            stream.write_all(request.as_bytes())
                .context("Failed to send request to daemon")?;
            
            // Read response
            let mut response = String::new();
            stream.read_to_string(&mut response)
                .context("Failed to read response from daemon")?;
            
            // Check if transcription was successful
            let success = serde_json::from_str::<serde_json::Value>(&response)
                .ok()
                .and_then(|v| v.get("success")?.as_bool())
                .unwrap_or(false);
            
            if success {
                // Parse the transcribed text from JSON response
                let text = extract_text_from_response(&response);
                
                if let Some(transcribed_text) = text {
                    typing::output_text(transcribed_text.trim(), use_clipboard, &format!("{} daemon", backend_name))?;
                } else {
                    Command::new("notify-send")
                        .args(&[
                            "Voice Input",
                            &format!("⚠️ Could not parse response\nBackend: {}", backend_name),
                            "-t", "2000",
                            "-h", "string:x-canonical-private-synchronous:voice"
                        ])
                        .spawn()?;
                }
            } else {
                Command::new("notify-send")
                    .args(&[
                        "Voice Input",
                        &format!("❌ Transcription failed\nBackend: {}", backend_name),
                        "-t", "2000",
                        "-h", "string:x-canonical-private-synchronous:voice"
                    ])
                    .spawn()?;
            }
            
            Ok(())
        }
        Err(e) => {
            // Return the error so the caller can handle fallback logic
            Err(anyhow::anyhow!("Failed to connect to daemon: {}", e))
        }
    }
}

/// Extract the "text" field value from a JSON response string
fn extract_text_from_response(response: &str) -> Option<String> {
    let parsed: serde_json::Value = serde_json::from_str(response).ok()?;
    parsed.get("text")?.as_str().map(|s| s.to_string())
}