use std::process::Command;

use std::path::PathBuf;

fn sounds_dir() -> PathBuf {
    let mut dir = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
    dir.push("dictatord");
    dir.push("sounds");
    dir
}

/// Play a short sound to indicate listening has started
pub fn play_start_sound() {
    let path = sounds_dir().join("start.ogg");
    let result = Command::new("paplay").arg(&path).output();

    if let Err(e) = result {
        log::debug!("Could not play start sound ({}): {}", path.display(), e);
    }
}

/// Play a short sound to indicate transcription is complete
pub fn play_stop_sound() {
    let path = sounds_dir().join("stop.ogg");
    let result = Command::new("paplay").arg(&path).output();

    if let Err(e) = result {
        log::debug!("Could not play stop sound ({}): {}", path.display(), e);
    }
}

/// Play a short sound to indicate an error
pub fn play_error_sound() {
    let result = Command::new("paplay")
        .arg("/usr/share/sounds/freedesktop/stereo/dialog-error.oga")
        .output();

    if let Err(e) = result {
        log::debug!("Could not play error sound: {}", e);
    }
}
