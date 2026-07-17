use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Main configuration struct for dictatord
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub hotkey: HotkeyConfig,
    pub audio: AudioConfig,
    pub asr: AsrConfig,
    pub output: OutputConfig,
    pub visual: VisualConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotkeyConfig {
    /// Key combination, e.g. "Super+D"
    pub key: String,
    /// "both" | "toggle" | "ptt"
    pub mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioConfig {
    /// PulseAudio source name or "default"
    pub device: String,
    /// Sample rate for capture (Hz)
    pub sample_rate: u32,
    /// Pre-roll buffer size in milliseconds
    pub pre_roll_ms: u64,
    /// Silence duration before auto-stop (ms)
    pub silence_timeout_ms: u64,
    /// VAD threshold multiplier (speech RMS / noise RMS)
    pub vad_threshold: f32,
    /// Enable WebRTC noise suppression before transcription
    #[serde(default = "default_noise_suppression")]
    pub noise_suppression: bool,
    /// Suppression level: "low", "moderate", "high", "very_high"
    #[serde(default = "default_suppression_level")]
    pub suppression_level: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsrConfig {
    /// Model size: "tiny" | "base" | "small" | "medium"
    pub model: String,
    /// Path to store model files
    pub model_path: String,
    /// Language code (e.g. "en", "fr")
    pub language: String,
    /// Use CUDA GPU offload if available
    pub gpu_offload: bool,
    /// GPU device ID to use (0 = first GPU, -1 = auto)
    pub gpu_device: i32,
    /// Use flash attention (faster, less accurate)
    pub flash_attn: bool,
    /// Number of CPU threads for inference
    pub num_threads: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputConfig {
    /// Output method: "xdotool"
    pub method: String,
    /// Auto-capitalize first word
    pub capitalize: bool,
    /// Add trailing space after dictation
    pub add_trailing_space: bool,
    /// Delay between keystrokes (ms)
    pub typing_delay_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualConfig {
    /// Enable the waveform visualizer overlay
    pub enabled: bool,
    /// Position on screen: "top-right"
    pub position: String,
    /// Hex color for the orb, e.g. "#00CCFF"
    pub color_hex: String,
    /// Window background opacity (0.0 - 1.0)
    pub opacity: f64,
}

fn default_noise_suppression() -> bool { true }
fn default_suppression_level() -> String { "moderate".to_string() }

impl Default for Config {
    fn default() -> Self {
        Config {
            hotkey: HotkeyConfig {
                key: "Super+D".to_string(),
                mode: "both".to_string(),
            },
            audio: AudioConfig {
                device: "default".to_string(),
                sample_rate: 16000,
                pre_roll_ms: 2000,
                silence_timeout_ms: 600,
                vad_threshold: 2.5,
                noise_suppression: true,
                suppression_level: "moderate".to_string(),
            },
            asr: AsrConfig {
                model: "tiny".to_string(),
                model_path: "~/.cache/dictatord/models/".to_string(),
                language: "en".to_string(),
                gpu_offload: false,
                gpu_device: 0,
                flash_attn: false,
                num_threads: 4,
            },
            output: OutputConfig {
                method: "xdotool".to_string(),
                capitalize: true,
                add_trailing_space: true,
                typing_delay_ms: 5,
            },
            visual: VisualConfig {
                enabled: true,
                position: "top-right".to_string(),
                color_hex: "#00CCFF".to_string(),
                opacity: 0.85,
            },
        }
    }
}

impl Config {
    /// Load config from ~/.config/dictatord/config.toml
    /// Creates default if not found
    pub fn load() -> anyhow::Result<Self> {
        let config_path = get_config_path();
        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)?;
            log::info!("Loaded config from {}", config_path.display());
            Ok(toml::from_str(&content)?)
        } else {
            let config = Config::default();
            config.save_to_disk()?;
            log::info!("Created default config at {}", config_path.display());
            Ok(config)
        }
    }

    /// Atomically save config to disk (write to tmp, then rename)
    pub fn save_to_disk(&self) -> anyhow::Result<()> {
        let config_path = get_config_path();
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        // Write to a temporary file first, then atomically rename
        let tmp_path = config_path.with_extension("toml.tmp");
        let content = toml::to_string_pretty(self)?;
        std::fs::write(&tmp_path, &content)?;
        std::fs::rename(&tmp_path, &config_path)?;
        log::info!("Saved config to {}", config_path.display());
        Ok(())
    }

    /// Apply a single setting by dotted key path (e.g. "audio.noise_suppression").
    /// Saves to disk on success. Returns an error if the key is unknown or value is invalid.
    pub fn apply(&mut self, key: &str, value: &str) -> anyhow::Result<()> {
        match key {
            // Hotkey
            "hotkey.key" => self.hotkey.key = value.to_string(),
            "hotkey.mode" => {
                let valid = ["ptt", "toggle", "both"];
                if !valid.contains(&value) {
                    anyhow::bail!("hotkey.mode must be one of: {}", valid.join(", "));
                }
                self.hotkey.mode = value.to_string();
            }
            // Audio
            "audio.noise_suppression" => {
                self.audio.noise_suppression = parse_bool(value)?;
            }
            "audio.suppression_level" => {
                let valid = ["low", "moderate", "high", "very_high"];
                if !valid.contains(&value) {
                    anyhow::bail!("suppression_level must be one of: {}", valid.join(", "));
                }
                self.audio.suppression_level = value.to_string();
            }
            "audio.sample_rate" => {
                self.audio.sample_rate = value.parse()?;
            }
            "audio.pre_roll_ms" => {
                self.audio.pre_roll_ms = value.parse()?;
            }
            "audio.silence_timeout_ms" => {
                self.audio.silence_timeout_ms = value.parse()?;
            }
            "audio.vad_threshold" => {
                self.audio.vad_threshold = value.parse()?;
            }
            // ASR
            "asr.model" => {
                let valid = ["tiny", "base", "small", "medium"];
                if !valid.contains(&value) {
                    anyhow::bail!("model must be one of: {}", valid.join(", "));
                }
                self.asr.model = value.to_string();
            }
            "asr.gpu_offload" => {
                self.asr.gpu_offload = parse_bool(value)?;
            }
            "asr.gpu_device" => {
                self.asr.gpu_device = value.parse()?;
            }
            "asr.flash_attn" => {
                self.asr.flash_attn = parse_bool(value)?;
            }
            "asr.num_threads" => {
                self.asr.num_threads = value.parse()?;
            }
            "asr.language" => {
                self.asr.language = value.to_string();
            }
            // Output
            "output.capitalize" => {
                self.output.capitalize = parse_bool(value)?;
            }
            "output.add_trailing_space" => {
                self.output.add_trailing_space = parse_bool(value)?;
            }
            "output.typing_delay_ms" => {
                self.output.typing_delay_ms = value.parse()?;
            }
            // Visual
            "visual.enabled" => {
                self.visual.enabled = parse_bool(value)?;
            }
            "visual.color_hex" => {
                self.visual.color_hex = value.to_string();
            }
            "visual.opacity" => {
                let val: f64 = value.parse()?;
                if !(0.0..=1.0).contains(&val) {
                    anyhow::bail!("opacity must be between 0.0 and 1.0");
                }
                self.visual.opacity = val;
            }
            _ => anyhow::bail!("unknown config key: {}", key),
        }
        self.save_to_disk()?;
        log::info!("Applied setting: {} = {}", key, value);
        Ok(())
    }

    /// Serialize config to a JSON value (for IPC).
    pub fn to_json_value(&self) -> serde_json::Value {
        serde_json::json!({
            "hotkey": {
                "key": self.hotkey.key,
                "mode": self.hotkey.mode,
            },
            "audio": {
                "device": self.audio.device,
                "sample_rate": self.audio.sample_rate,
                "pre_roll_ms": self.audio.pre_roll_ms,
                "silence_timeout_ms": self.audio.silence_timeout_ms,
                "vad_threshold": self.audio.vad_threshold,
                "noise_suppression": self.audio.noise_suppression,
                "suppression_level": self.audio.suppression_level,
            },
            "asr": {
                "model": self.asr.model,
                "gpu_offload": self.asr.gpu_offload,
                "gpu_device": self.asr.gpu_device,
                "flash_attn": self.asr.flash_attn,
                "num_threads": self.asr.num_threads,
                "language": self.asr.language,
            },
            "output": {
                "capitalize": self.output.capitalize,
                "add_trailing_space": self.output.add_trailing_space,
                "typing_delay_ms": self.output.typing_delay_ms,
            },
            "visual": {
                "enabled": self.visual.enabled,
                "color_hex": self.visual.color_hex,
                "opacity": self.visual.opacity,
            },
        })
    }

    /// Check which settings require a daemon restart to take effect.
    pub fn requires_restart(key: &str) -> bool {
        matches!(key,
            "hotkey.key" | "hotkey.mode" |
            "asr.model" |
            "audio.device" | "audio.sample_rate" |
            "visual.enabled" | "visual.position"
        )
    }
}

/// Parse "true"/"false"/"1"/"0"/"yes"/"no" string to bool.
fn parse_bool(value: &str) -> anyhow::Result<bool> {
    match value.to_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Ok(true),
        "false" | "0" | "no" | "off" => Ok(false),
        _ => anyhow::bail!("invalid boolean value: {}", value),
    }
}

fn get_config_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    PathBuf::from(format!("{}/.config/dictatord/config.toml", home))
}

/// Parse hex color string like "#00CCFF" to (r, g, b) floats 0.0-1.0
pub fn parse_hex_color(hex: &str) -> (f64, f64, f64) {
    let hex = hex.trim_start_matches('#');
    if hex.len() == 6 {
        if let (Ok(r), Ok(g), Ok(b)) = (
            u8::from_str_radix(&hex[0..2], 16),
            u8::from_str_radix(&hex[2..4], 16),
            u8::from_str_radix(&hex[4..6], 16),
        ) {
            return (r as f64 / 255.0, g as f64 / 255.0, b as f64 / 255.0);
        }
    }
    (0.0, 0.8, 1.0) // Default cyan
}
