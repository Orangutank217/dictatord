//! Interactive settings menu for dictatord.
//!
//! Connects to the daemon's IPC socket, reads current config,
//! and lets the user modify settings via `inquire` prompts.

use crate::ipc;

/// Run the interactive settings menu.
pub fn run() -> anyhow::Result<()> {
    // Verify daemon is running
    let mut config = match fetch_config() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error: {}", e);
            eprintln!();
            eprintln!("Make sure the dictatord daemon is running:");
            eprintln!("  systemctl --user start dictatord");
            std::process::exit(1);
        }
    };

    println!();
    println!("  ⚙  dictatord Settings");
    println!("  {}", "-".repeat(36));
    println!();

    loop {
        let category = inquire::Select::new(
            "Select a category:",
            vec![
                "Audio",
                "Speech Recognition (ASR)",
                "Output",
                "Visualizer",
                "Hotkey",
                "⚠  Save & Exit",
                "✖  Exit without saving",
            ],
        )
        .with_vim_mode(true)
        .prompt()?;

        match category {
            "Audio" => edit_audio(&mut config)?,
            "Speech Recognition (ASR)" => edit_asr(&mut config)?,
            "Output" => edit_output(&mut config)?,
            "Visualizer" => edit_visual(&mut config)?,
            "Hotkey" => edit_hotkey(&mut config)?,
            "⚠  Save & Exit" => {
                println!("  ✓ All changes saved to disk.");
                break;
            }
            "✖  Exit without saving" => {
                // All changes are applied live and saved immediately,
                // so this just exits without further action.
                break;
            }
            _ => unreachable!(),
        }
    }

    Ok(())
}

/// Fetch current config from the daemon.
fn fetch_config() -> anyhow::Result<serde_json::Value> {
    let resp = ipc::send_command(&serde_json::json!({"cmd": "get_config"}))?;
    Ok(resp["data"].clone())
}

/// Send a set command and display feedback.
fn set_setting(key: &str, value: &str) -> bool {
    match ipc::send_command(&serde_json::json!({"cmd": "set", "key": key, "value": value})) {
        Ok(resp) => {
            if resp.get("restart_required").and_then(|v| v.as_bool()) == Some(true) {
                println!(
                    "  ⚠  Setting '{}' requires restart: systemctl --user restart dictatord",
                    key
                );
            } else {
                println!("  ✓ {} = {}", key, value);
            }
            true
        }
        Err(e) => {
            println!("  ✗ Failed: {}", e);
            false
        }
    }
}

/// Toggle a boolean setting via confirm prompt.
fn toggle_bool(config: &serde_json::Value, key: &str, label: &str) {
    let current = config.pointer(&format!("/{}", key.replace('.', "/")))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let msg = format!("{} (currently {})", label, if current { "ON" } else { "OFF" });
    match inquire::Confirm::new(&msg).with_default(current).prompt() {
        Ok(new_val) if new_val != current => {
            set_setting(key, if new_val { "true" } else { "false" });
        }
        Ok(_) => println!("  – No change"),
        Err(inquire::InquireError::OperationCanceled) => {}
        Err(e) => println!("  ✗ Error: {}", e),
    }
}

/// Edit audio settings.
fn edit_audio(config: &mut serde_json::Value) -> anyhow::Result<()> {
    loop {
        let levels = ["low", "moderate", "high", "very_high"];
        let current_level = config["audio"]["suppression_level"]
            .as_str()
            .unwrap_or("moderate");

        let choices = vec![
            format!(
                "Noise Suppression:  {}",
                if config["audio"]["noise_suppression"].as_bool().unwrap_or(true) {
                    "ON "
                } else {
                    "OFF"
                }
            ),
            format!("Suppression Level: {}", current_level),
            "← Back to main menu".to_string(),
        ];

        let sel = inquire::Select::new("Audio settings:", choices)
            .with_vim_mode(true)
            .prompt()?;

        match sel.as_str() {
            s if s.starts_with("Noise Suppression") => {
                toggle_bool(config, "audio.noise_suppression", "Noise Suppression");
                // Refresh local config
                if let Ok(new_config) = fetch_config() {
                    *config = new_config;
                }
            }
            s if s.starts_with("Suppression Level") => {
                let sel = inquire::Select::new("Suppression Level:", levels.to_vec())
                    .with_starting_cursor(
                        levels.iter().position(|&l| l == current_level).unwrap_or(1),
                    )
                    .prompt()?;
                if sel != current_level {
                    set_setting("audio.suppression_level", sel);
                    if let Ok(new_config) = fetch_config() {
                        *config = new_config;
                    }
                } else {
                    println!("  – No change");
                }
            }
            _ => break,
        }
    }
    Ok(())
}

/// Edit ASR settings.
fn edit_asr(config: &mut serde_json::Value) -> anyhow::Result<()> {
    loop {
        let models = ["tiny", "base", "small", "medium"];
        let current_model = config["asr"]["model"].as_str().unwrap_or("small");

        let choices = vec![
            format!(
                "GPU Acceleration: {}",
                if config["asr"]["gpu_offload"].as_bool().unwrap_or(true) {
                    "ON "
                } else {
                    "OFF"
                }
            ),
            format!("Model: {}", current_model),
            format!("CPU Threads: {}", config["asr"]["num_threads"].as_i64().unwrap_or(4)),
            "← Back to main menu".to_string(),
        ];

        let sel = inquire::Select::new("Speech Recognition settings:", choices)
            .with_vim_mode(true)
            .prompt()?;

        match sel.as_str() {
            s if s.starts_with("GPU Acceleration") => {
                toggle_bool(config, "asr.gpu_offload", "GPU Acceleration");
                if let Ok(new_config) = fetch_config() {
                    *config = new_config;
                }
            }
            s if s.starts_with("Model") => {
                let sel = inquire::Select::new(
                    "Model (requires restart):",
                    models.to_vec(),
                )
                .with_starting_cursor(
                    models.iter().position(|&m| m == current_model).unwrap_or(2),
                )
                .prompt()?;
                if sel != current_model {
                    set_setting("asr.model", sel);
                    println!("  ⚠  Restart required: systemctl --user restart dictatord");
                    if let Ok(new_config) = fetch_config() {
                        *config = new_config;
                    }
                } else {
                    println!("  – No change");
                }
            }
            s if s.starts_with("CPU Threads") => {
                let current = config["asr"]["num_threads"].as_i64().unwrap_or(4);
                let msg = format!("CPU Threads (current: {}):", current);
                match inquire::CustomType::<i32>::new(&msg)
                    .with_default(current as i32)
                    .with_validator(|v: &i32| {
                        if *v >= 1 && *v <= 32 {
                            Ok(inquire::validator::Validation::Valid)
                        } else {
                            Ok(inquire::validator::Validation::Invalid("Must be between 1 and 32".into()))
                        }
                    })
                    .prompt()
                {
                    Ok(v) if v != current as i32 => {
                        set_setting("asr.num_threads", &v.to_string());
                        if let Ok(new_config) = fetch_config() {
                            *config = new_config;
                        }
                    }
                    Ok(_) => println!("  – No change"),
                    Err(inquire::InquireError::OperationCanceled) => {}
                    Err(e) => println!("  ✗ Error: {}", e),
                }
            }
            _ => break,
        }
    }
    Ok(())
}

/// Edit output settings.
fn edit_output(config: &mut serde_json::Value) -> anyhow::Result<()> {
    loop {
        let choices = vec![
            format!(
                "Capitalize: {}",
                if config["output"]["capitalize"].as_bool().unwrap_or(true) {
                    "ON "
                } else {
                    "OFF"
                }
            ),
            format!(
                "Trailing Space: {}",
                if config["output"]["add_trailing_space"].as_bool().unwrap_or(true) {
                    "ON "
                } else {
                    "OFF"
                }
            ),
            format!(
                "Typing Delay: {} ms",
                config["output"]["typing_delay_ms"].as_i64().unwrap_or(5)
            ),
            "← Back to main menu".to_string(),
        ];

        let sel = inquire::Select::new("Output settings:", choices)
            .with_vim_mode(true)
            .prompt()?;

        match sel.as_str() {
            s if s.starts_with("Capitalize") => {
                toggle_bool(config, "output.capitalize", "Auto-capitalize");
                if let Ok(new_config) = fetch_config() {
                    *config = new_config;
                }
            }
            s if s.starts_with("Trailing Space") => {
                toggle_bool(config, "output.add_trailing_space", "Trailing Space");
                if let Ok(new_config) = fetch_config() {
                    *config = new_config;
                }
            }
            s if s.starts_with("Typing Delay") => {
                let current = config["output"]["typing_delay_ms"].as_i64().unwrap_or(5);
                let msg = format!("Typing delay in ms (current: {}):", current);
                match inquire::CustomType::<u64>::new(&msg)
                    .with_default(current as u64)
                    .with_validator(|v: &u64| {
                        if *v <= 1000 {
                            Ok(inquire::validator::Validation::Valid)
                        } else {
                            Ok(inquire::validator::Validation::Invalid("Max 1000ms".into()))
                        }
                    })
                    .prompt()
                {
                    Ok(v) if v != current as u64 => {
                        set_setting("output.typing_delay_ms", &v.to_string());
                        if let Ok(new_config) = fetch_config() {
                            *config = new_config;
                        }
                    }
                    Ok(_) => println!("  – No change"),
                    Err(inquire::InquireError::OperationCanceled) => {}
                    Err(e) => println!("  ✗ Error: {}", e),
                }
            }
            _ => break,
        }
    }
    Ok(())
}

/// Edit visualizer settings.
fn edit_visual(config: &mut serde_json::Value) -> anyhow::Result<()> {
    loop {
        let choices = vec![
            format!(
                "Visualizer: {}",
                if config["visual"]["enabled"].as_bool().unwrap_or(true) {
                    "ON "
                } else {
                    "OFF"
                }
            ),
            format!("Color: {}", config["visual"]["color_hex"].as_str().unwrap_or("#00CCFF")),
            format!("Opacity: {:.2}", config["visual"]["opacity"].as_f64().unwrap_or(0.85)),
            "← Back to main menu".to_string(),
        ];

        let sel = inquire::Select::new("Visualizer settings:", choices)
            .with_vim_mode(true)
            .prompt()?;

        match sel.as_str() {
            s if s.starts_with("Visualizer:") => {
                toggle_bool(config, "visual.enabled", "Visualizer");
                if let Ok(new_config) = fetch_config() {
                    *config = new_config;
                }
            }
            s if s.starts_with("Color") => {
                let current = config["visual"]["color_hex"].as_str().unwrap_or("#00CCFF");
                let msg = format!("Color hex (current: {}):", current);
                match inquire::Text::new(&msg).with_default(current).prompt() {
                    Ok(v) if v != current => {
                        set_setting("visual.color_hex", &v);
                        if let Ok(new_config) = fetch_config() {
                            *config = new_config;
                        }
                    }
                    Ok(_) => println!("  – No change"),
                    Err(inquire::InquireError::OperationCanceled) => {}
                    Err(e) => println!("  ✗ Error: {}", e),
                }
            }
            s if s.starts_with("Opacity") => {
                let current = config["visual"]["opacity"].as_f64().unwrap_or(0.85);
                let msg = format!("Opacity 0.0-1.0 (current: {:.2}):", current);
                match inquire::CustomType::<f64>::new(&msg)
                    .with_default(current)
                    .with_validator(|v: &f64| {
                        if *v >= 0.0 && *v <= 1.0 {
                            Ok(inquire::validator::Validation::Valid)
                        } else {
                            Ok(inquire::validator::Validation::Invalid("Must be between 0.0 and 1.0".into()))
                        }
                    })
                    .prompt()
                {
                    Ok(v) if (v - current).abs() > 0.01 => {
                        set_setting("visual.opacity", &format!("{:.2}", v));
                        if let Ok(new_config) = fetch_config() {
                            *config = new_config;
                        }
                    }
                    Ok(_) => println!("  – No change"),
                    Err(inquire::InquireError::OperationCanceled) => {}
                    Err(e) => println!("  ✗ Error: {}", e),
                }
            }
            _ => break,
        }
    }
    Ok(())
}

/// Edit hotkey settings.
fn edit_hotkey(config: &mut serde_json::Value) -> anyhow::Result<()> {
    loop {
        let modes = ["ptt", "toggle", "both"];
        let current_mode = config["hotkey"]["mode"].as_str().unwrap_or("ptt");

        let choices = vec![
            format!("Key: {}", config["hotkey"]["key"].as_str().unwrap_or("§")),
            format!("Mode: {}", current_mode),
            "← Back to main menu".to_string(),
        ];

        let sel = inquire::Select::new("Hotkey settings (restart required):", choices)
            .with_vim_mode(true)
            .prompt()?;

        match sel.as_str() {
            s if s.starts_with("Key") => {
                let current = config["hotkey"]["key"].as_str().unwrap_or("§");
                println!("  ℹ  Enter the X11 keysym name (e.g. 'Super+D', 'F2', '§')");
                let msg = format!("Hotkey key (current: {}):", current);
                match inquire::Text::new(&msg).with_default(current).prompt() {
                    Ok(v) if v != current => {
                        set_setting("hotkey.key", &v);
                        println!("  ⚠  Restart required: systemctl --user restart dictatord");
                        if let Ok(new_config) = fetch_config() {
                            *config = new_config;
                        }
                    }
                    Ok(_) => println!("  – No change"),
                    Err(inquire::InquireError::OperationCanceled) => {}
                    Err(e) => println!("  ✗ Error: {}", e),
                }
            }
            s if s.starts_with("Mode") => {
                let sel = inquire::Select::new("Hotkey mode:", modes.to_vec())
                    .with_starting_cursor(
                        modes.iter().position(|&m| m == current_mode).unwrap_or(0),
                    )
                    .prompt()?;
                if sel != current_mode {
                    set_setting("hotkey.mode", sel);
                    println!("  ⚠  Restart required: systemctl --user restart dictatord");
                    if let Ok(new_config) = fetch_config() {
                        *config = new_config;
                    }
                } else {
                    println!("  – No change");
                }
            }
            _ => break,
        }
    }
    Ok(())
}
