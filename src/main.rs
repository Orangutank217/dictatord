mod asr;
mod audio;
mod config;
mod feedback;
mod hotkey;
mod ipc;
mod output;
mod settings;
mod visual;

use crate::audio::{AudioEvent, AudioShared, RingBuffer};
use crate::config::Config;
use crate::hotkey::HotkeyEvent;
use crate::visual::{VisualCommand, VisualizerState};
use clap::Parser;
use std::sync::mpsc;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

/// Native speech-to-text daemon with waveform visualization
#[derive(Parser)]
#[command(name = "dictatord", version, about)]
struct Cli {
    /// Print debug logs
    #[arg(long)]
    debug: bool,
    /// Open interactive settings menu (instead of running the daemon)
    #[arg(long)]
    settings: bool,
}

/// State machine for the dictation lifecycle
#[derive(Debug, Clone, Copy, PartialEq)]
enum DictationState {
    Idle,
    Listening,
    Processing,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    if cli.debug {
        std::env::set_var("RUST_LOG", "debug");
    } else {
        std::env::set_var("RUST_LOG", "info");
    }
    env_logger::init();

    log::info!("dictatord v{} starting...", env!("CARGO_PKG_VERSION"));

    // -- Settings Mode (interactive CLI, doesn't start the daemon) --

    if cli.settings {
        return settings::run();
    }

    // Load configuration
    let config = Config::load()?;
    log::info!("Hotkey: {}", config.hotkey.key);
    log::info!("Model: {}", config.asr.model);

    // Wrap config in Arc<RwLock> so the IPC server can modify it live.
    // `config_arc` is used by handle_stop_listening for live-read settings.
    // `config` stays as a plain reference for startup/clone-once paths.
    let config_arc = Arc::new(RwLock::new(config.clone()));

    // Start IPC server (background thread) for live settings management
    let _ipc_handle = ipc::start_ipc_server(config_arc.clone())?;

    // --- Shared State Setup ---

    // Visualizer state (shared between audio thread and GTK thread)
    let visual_state = Arc::new(Mutex::new(VisualizerState {
        current_rms: 0.0,
    }));

    // Audio shared state (shared between main thread and audio callback)
    let pre_roll_samples = (config.audio.sample_rate as u64 * config.audio.pre_roll_ms / 1000) as usize;
    let audio_shared = Arc::new(Mutex::new(AudioShared {
        ring_buffer: RingBuffer::new(pre_roll_samples),
        recording_buffer: Vec::with_capacity(config.audio.sample_rate as usize * 10), // 10s max
        is_recording: false,
        current_rms: 0.0,
        noise_floor: 0.01,
    }));

    // Channels
    let (hotkey_tx, cmd_rx) = mpsc::channel::<HotkeyEvent>();
    let (audio_event_tx, audio_event_rx) = mpsc::channel::<AudioEvent>();
    let (visual_tx, visual_rx) = mpsc::channel::<VisualCommand>();

    // Arc for visual_tx so it can be shared
    let visual_tx = Arc::new(visual_tx);

    // -- Start GTK Visualizer Thread --

    if config.visual.enabled {
        let vs = visual_state.clone();
        let vr = visual_rx;
        let vc = config.visual.clone();
        std::thread::Builder::new()
            .name("visualizer".into())
            .spawn(move || {
                visual::run_visualizer(vs, vr, vc);
            })?;
        log::info!("Visualizer thread spawned");
    } else {
        log::info!("Visualizer disabled");
    }

    // -- Start Audio Capture --

    let as_audio = audio_shared.clone();
    let audio_tx = audio_event_tx.clone();
    let _audio_stream = audio::start_audio_capture(&config.audio, as_audio, audio_tx)?;
    log::info!("Audio capture started");

    // -- Download ASR Model (if needed) --

    let model_path = asr::ensure_model_downloaded(&config.asr)?;
    log::info!("Model ready at: {}", model_path.display());

    // -- Load Model Into Memory Buffer (no GPU yet) --

    log::info!("Loading model into system memory buffer...");
    let model_buffer = std::fs::read(&model_path)?;
    log::info!("Model loaded ({} MB)", model_buffer.len() / 1_048_576);

    // -- Initialize ASR Engine (validates model on CPU, GPU will be on-demand) --

    let asr_engine = asr::ASREngine::new(model_buffer, &config.asr)?;
    log::info!("ASR engine initialized");

    // -- Start Hotkey Listener --

    let hk_config = config.hotkey.clone();
    let hk_tx = hotkey_tx.clone();
    std::thread::Builder::new()
        .name("hotkey".into())
        .spawn(move || {
            if let Err(e) = hotkey::run_hotkey_listener(&hk_config, hk_tx) {
                log::error!("Hotkey listener error: {}", e);
            }
        })?;
    log::info!("Hotkey listener started");

    // -- Main Event Loop --

    let mut state = DictationState::Idle;
    let mut key_press_time: Option<Instant> = None;
    let ptt_threshold_ms: u64 = 300;

    log::info!("dictatord ready. Press {} to start dictation.", config.hotkey.key);

    loop {
        // Check for audio events (non-blocking)
        while let Ok(audio_evt) = audio_event_rx.try_recv() {
            match audio_evt {
                AudioEvent::SpeechEnded => {
                    // In PTT mode, only the key release controls stop.
                    // VAD auto-stop would trigger prematurely from ambient
                    // noise before the user even starts speaking.
                    if state == DictationState::Listening && config.hotkey.mode != "ptt" {
                        log::info!("VAD detected end of speech");
                        handle_stop_listening(
                            &config_arc,
                            &audio_shared,
                            &asr_engine,
                            &visual_tx,
                            &mut state,
                        );
                    }
                }
            }
        }

        // Wait for the next command (with timeout for responsiveness)
        let cmd = match cmd_rx.recv_timeout(Duration::from_millis(100)) {
            Ok(cmd) => cmd,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // Timeout - just continue the loop (allows audio events to be processed)
                continue;
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                log::error!("Hotkey channel disconnected");
                break;
            }
        };

        match state {
            DictationState::Idle => {
                match cmd {
                    HotkeyEvent::Pressed => {
                        log::info!("Hotkey pressed - starting dictation");
                        handle_start_listening(&config, &audio_shared, &visual_tx, &mut state);
                        key_press_time = Some(Instant::now());
                    }
                    HotkeyEvent::Released => {
                        // Ignore release in idle state
                    }
                }
            }

            DictationState::Listening => {
                match cmd {
                    HotkeyEvent::Pressed => {
                        if config.hotkey.mode == "toggle" || config.hotkey.mode == "both" {
                            // Second press = stop (toggle behavior)
                            log::info!("Hotkey pressed again - stopping dictation");
                            handle_stop_listening(
                                &config_arc,
                                &audio_shared,
                                &asr_engine,
                                &visual_tx,
                                &mut state,
                            );
                        }
                    }
                    HotkeyEvent::Released => {
                        match config.hotkey.mode.as_str() {
                            "ptt" => {
                                log::info!("Hotkey released - stopping dictation (PTT)");
                                handle_stop_listening(
                                    &config_arc, &audio_shared, &asr_engine, &visual_tx, &mut state,
                                );
                            }
                            "both" => {
                                // Check if it was a hold (>= threshold) → PTT stop
                                // or a tap (< threshold) → stay listening for next press
                                if let Some(pt) = key_press_time {
                                    if pt.elapsed().as_millis() as u64 >= ptt_threshold_ms {
                                        log::info!("Hotkey held - stopping dictation (PTT)");
                                        handle_stop_listening(
                                            &config_arc, &audio_shared, &asr_engine, &visual_tx, &mut state,
                                        );
                                    } else {
                                        log::debug!("Quick tap - staying in listening mode");
                                    }
                                    key_press_time = None;
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }

            DictationState::Processing => {
                match cmd {
                    HotkeyEvent::Pressed | HotkeyEvent::Released => {
                        log::debug!("Ignoring hotkey while processing");
                    }
                }
            }
        }

        // Update visualizer RMS from audio
        if config.visual.enabled {
            if let Ok(audio) = audio_shared.lock() {
                if let Ok(mut vis) = visual_state.lock() {
                    vis.current_rms = audio.current_rms;
                }
            }
        }
    }

    log::info!("dictatord shutting down");
    // Send shutdown to visualizer
    visual_tx.send(VisualCommand::Shutdown).ok();
    // Give threads time to clean up
    std::thread::sleep(Duration::from_millis(200));

    Ok(())
}

/// Handle the start of dictation
fn handle_start_listening(
    config: &Config,
    audio_shared: &Arc<Mutex<AudioShared>>,
    visual_tx: &Arc<mpsc::Sender<VisualCommand>>,
    state: &mut DictationState,
) {
    // Unmute microphone if needed
    unmute_microphone();

    if let Ok(mut shared) = audio_shared.lock() {
        // Get pre-roll buffer (audio from before the button press)
        let pre_roll = shared.ring_buffer.snapshot(
            (config.audio.sample_rate as u64 * config.audio.pre_roll_ms / 1000) as usize,
        );

        // Start recording with pre-roll audio
        shared.recording_buffer = pre_roll;
        shared.is_recording = true;
    }

    *state = DictationState::Listening;

    // Show visualizer
    if config.visual.enabled {
        visual_tx.send(VisualCommand::Show).ok();
    }

    // Play start sound
    feedback::play_start_sound();

    log::info!("Listening...");
}

/// Handle the stop of dictation and start transcription
/// Reads config from the Arc<RwLock> to pick up live changes from IPC.
fn handle_stop_listening(
    config_arc: &Arc<RwLock<Config>>,
    audio_shared: &Arc<Mutex<AudioShared>>,
    asr_engine: &asr::ASREngine,
    visual_tx: &Arc<mpsc::Sender<VisualCommand>>,
    state: &mut DictationState,
) {
    *state = DictationState::Processing;

    // Stop recording
    let audio_data = if let Ok(mut shared) = audio_shared.lock() {
        shared.is_recording = false;
        shared.recording_buffer.clone()
    } else {
        Vec::new()
    };

    // Read latest config (may have been updated via IPC)
    let config = config_arc.read().unwrap();

    // Update visualizer - show processing
    if config.visual.enabled {
        visual_tx.send(VisualCommand::SetPreviewText("Processing\u{2026}".into())).ok();
    }

    // Apply noise suppression (if enabled)
    let audio_data = if config.audio.noise_suppression && !audio_data.is_empty() {
        let level = &config.audio.suppression_level;
        log::info!("Applying noise suppression (level: {})...", level);
        let denoised = audio::denoise_audio(&audio_data, level);
        log::debug!("Denoised: {} -> {} samples", audio_data.len(), denoised.len());
        denoised
    } else {
        audio_data
    };

    log::info!("Transcribing {} samples ({:.1}s@16kHz)...", audio_data.len(), audio_data.len() as f64 / 16000.0);

    // Release the config lock before the blocking transcription
    drop(config);

    // Perform transcription (blocking — GPU context created on-demand)
    let start_time = Instant::now();
    let result = asr_engine.transcribe(&audio_data);
    let elapsed = start_time.elapsed();

    // Re-read config for output settings (may have changed during transcription)
    let config = config_arc.read().unwrap();

    match result {
        Ok(text) => {
            let elapsed_ms = elapsed.as_secs_f64() * 1000.0;
            log::info!("Transcription completed in {:.0}ms", elapsed_ms);

            // Type the text
            if !text.is_empty() {
                if let Err(e) = output::type_text(&text, &config.output) {
                    log::error!("Failed to type text: {}", e);
                    feedback::play_error_sound();
                } else {
                    feedback::play_stop_sound();
                }
            } else {
                log::info!("No speech detected (empty transcription)");
            }
        }
        Err(e) => {
            log::error!("Transcription failed: {}", e);
            feedback::play_error_sound();
        }
    }

    // Re-mute microphone if needed
    mute_microphone();

    // Hide visualizer
    if config.visual.enabled {
        visual_tx.send(VisualCommand::Hide).ok();
    }

    *state = DictationState::Idle;
}

/// Try to unmute the microphone via PulseAudio
fn unmute_microphone() {
    let result = std::process::Command::new("pactl")
        .args(["set-source-mute", "@DEFAULT_SOURCE@", "0"])
        .output();
    if let Err(e) = result {
        log::debug!("Could not unmute mic: {}", e);
    }
}

/// Try to mute the microphone via PulseAudio
fn mute_microphone() {
    let result = std::process::Command::new("pactl")
        .args(["set-source-mute", "@DEFAULT_SOURCE@", "1"])
        .output();
    if let Err(e) = result {
        log::debug!("Could not mute mic: {}", e);
    }
}
