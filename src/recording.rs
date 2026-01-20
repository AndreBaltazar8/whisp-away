use anyhow::{Context, Result};
use std::fs;
use std::process::Command;
use crate::helpers::is_process_running;

/// Stop the recording process and return the audio file path
pub fn stop_recording(audio_file_override: Option<&str>) -> Result<Option<String>> {
    let pidfile = "/tmp/whisp-away-recording.pid";
    let uid = unsafe { libc::getuid() };
    
    // Wait a bit for the pidfile to appear if it doesn't exist yet
    let mut attempts = 0;
    while !std::path::Path::new(&pidfile).exists() && attempts < 10 {
        std::thread::sleep(std::time::Duration::from_millis(20));
        attempts += 1;
    }
    
    // Stop the recording process if it's running
    if let Ok(pid_str) = fs::read_to_string(&pidfile) {
        let pid_str = pid_str.trim();
        if pid_str.is_empty() {
            let _ = fs::remove_file(&pidfile);
            return Ok(None);
        }
        
        if let Ok(pid) = pid_str.parse::<u32>() {
            if !is_process_running(pid) {
                // Process already stopped
                let _ = fs::remove_file(&pidfile);
                let _ = fs::remove_file(format!("/run/user/{}/voice-audio-file.tmp", uid));
                return Ok(None);
            }
            
            // Try graceful shutdown first (use libc::kill directly for NixOS compatibility)
            std::thread::sleep(std::time::Duration::from_millis(100));

            unsafe { libc::kill(pid as i32, libc::SIGINT) };

            std::thread::sleep(std::time::Duration::from_millis(50));

            // Force kill if still running
            if is_process_running(pid) {
                unsafe { libc::kill(pid as i32, libc::SIGTERM) };
            }
            
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }
    
    let _ = fs::remove_file(&pidfile);

    // Get the audio file path
    let audio_file = if let Some(override_path) = audio_file_override {
        // Copy the override file to a temporary location so it can be cleaned up
        let runtime_dir = crate::helpers::get_runtime_dir();
        let temp_audio = format!("{}/voice-recording-override-{}.wav", runtime_dir, 
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis());
        fs::copy(override_path, &temp_audio)
            .context("Failed to copy audio file to temporary location")?;
        temp_audio
    } else {
        match fs::read_to_string(format!("/run/user/{}/voice-audio-file.tmp", uid)) {
            Ok(path) => {
                let path = path.trim().to_string();
                let _ = fs::remove_file(format!("/run/user/{}/voice-audio-file.tmp", uid));
                path
            },
            Err(_) => {
                return Ok(None);
            }
        }
    };
    
    Ok(Some(audio_file))
}

/// Common function to start recording audio
pub fn start_recording(backend_name: &str) -> Result<()> {
    let pidfile = "/tmp/whisp-away-recording.pid";
    let uid = unsafe { libc::getuid() };
    
    // Kill any existing recording process (use libc::kill directly for NixOS compatibility)
    if let Ok(pid_str) = fs::read_to_string(&pidfile) {
        if let Ok(pid) = pid_str.trim().parse::<u32>() {
            if is_process_running(pid) {
                unsafe { libc::kill(pid as i32, libc::SIGTERM) };
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }
        let _ = fs::remove_file(&pidfile);
    }
    
    let runtime_dir = crate::helpers::get_runtime_dir();
    let audio_file = format!("{}/voice-recording-{}.wav", runtime_dir, 
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis());

    // Clean up old recording files
    if let Ok(entries) = fs::read_dir(&runtime_dir) {
        for entry in entries.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                if name.starts_with("voice-recording-") && name.ends_with(".wav") {
                    if entry.path().to_str() != Some(&audio_file) {
                        let _ = fs::remove_file(entry.path());
                    }
                }
            }
        }
    }
    
    // Store the audio file path for later retrieval
    fs::write(format!("/run/user/{}/voice-audio-file.tmp", uid), &audio_file)
        .context("Failed to write audio file path")?;

    // Start recording
    let child = Command::new("pw-record")
        .args(&[
            "--channels", "1",
            "--rate", "16000",
            "--format", "s16",
            "--volume", "1.5",
            &audio_file,
        ])
        .spawn()
        .context("Failed to start pw-record")?;

    fs::write(&pidfile, child.id().to_string())
        .context("Failed to write PID file")?;

    // Get model from environment/state for notification
    let model = crate::helpers::resolve_model(None);
    let acceleration = crate::helpers::get_acceleration_type();
    let recording_msg = format!("🎤 Recording... (release to stop)\nBackend: {} ({}) | Model: {}", backend_name, acceleration, model);
    
    Command::new("notify-send")
        .args(&[
            "Voice Input",
            &recording_msg,
            "-t", "30000",
            "-h", "string:x-canonical-private-synchronous:voice"
        ])
        .spawn()?;

    Ok(())
}