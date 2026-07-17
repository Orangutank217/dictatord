use crate::config::AudioConfig;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use sonora_ns::config::{NsConfig, SuppressionLevel};
use sonora_ns::noise_suppressor::NoiseSuppressor;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

/// Ring buffer that keeps a rolling window of audio samples
pub struct RingBuffer {
    buffer: Vec<i16>,
    capacity: usize,
    write_pos: usize,
    filled: bool,
}

impl RingBuffer {
    pub fn new(capacity_samples: usize) -> Self {
        RingBuffer {
            buffer: vec![0i16; capacity_samples],
            capacity: capacity_samples,
            write_pos: 0,
            filled: false,
        }
    }

    /// Push a chunk of samples into the ring buffer
    pub fn push(&mut self, samples: &[i16]) {
        for &sample in samples {
            self.buffer[self.write_pos] = sample;
            self.write_pos = (self.write_pos + 1) % self.capacity;
            if self.write_pos == 0 {
                self.filled = true;
            }
        }
    }

    /// Get the last `num_samples` from the buffer (most recent tail)
    pub fn snapshot(&self, num_samples: usize) -> Vec<i16> {
        let available = if self.filled {
            self.capacity
        } else {
            self.write_pos
        };
        let take = num_samples.min(available);
        let mut result = Vec::with_capacity(take);
        if take == 0 {
            return result;
        }
        if self.filled {
            // Buffer wrapped around
            let start = if self.write_pos >= take {
                self.write_pos - take
            } else {
                self.capacity - (take - self.write_pos)
            };
            for i in 0..take {
                result.push(self.buffer[(start + i) % self.capacity]);
            }
        } else {
            result.extend_from_slice(&self.buffer[0..take]);
        }
        result
    }
}

/// Compute RMS (Root Mean Square) of audio samples as a normalized value 0.0-1.0
pub fn compute_rms(samples: &[i16]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum_sq: f64 = samples
        .iter()
        .map(|&s| {
            let f = s as f64 / 32768.0;
            f * f
        })
        .sum();
    (sum_sq / samples.len() as f64).sqrt() as f32
}

/// Parse suppression level string to sonora's `SuppressionLevel`.
/// Returns `Moderate` for unknown values as a safe default.
fn parse_suppression_level(level: &str) -> SuppressionLevel {
    match level.to_lowercase().as_str() {
        "low" | "6db" => SuppressionLevel::K6dB,
        "moderate" | "12db" => SuppressionLevel::K12dB,
        "high" | "18db" => SuppressionLevel::K18dB,
        "very_high" | "very high" | "21db" => SuppressionLevel::K21dB,
        _ => {
            log::warn!("Unknown suppression level '{}', using moderate", level);
            SuppressionLevel::K12dB
        }
    }
}

const NS_FRAME_SIZE: usize = 160; // 10ms at 16 kHz

/// Apply WebRTC noise suppression to 16 kHz audio samples.
///
/// Processes audio through a Wiener filter-based noise suppressor
/// to reduce background noise (fans, hum, keyboard rumble, etc.)
/// while preserving speech.
///
/// # Safety
/// - Handles partial final frames by zero-padding
/// - Returns original audio if processing fails (graceful degradation)
/// - Accepts empty input gracefully
pub fn denoise_audio(samples: &[i16], level: &str) -> Vec<i16> {
    if samples.is_empty() {
        return Vec::new();
    }

    let suppression_level = parse_suppression_level(level);
    let mut ns = NoiseSuppressor::new(NsConfig {
        target_level: suppression_level,
    });

    let num_frames = samples.len().div_ceil(NS_FRAME_SIZE);
    let mut output = Vec::with_capacity(num_frames * NS_FRAME_SIZE);

    for chunk in samples.chunks(NS_FRAME_SIZE) {
        // Convert i16 → f32 and pad with zeros if partial frame
        let mut frame = [0.0f32; NS_FRAME_SIZE];
        for (i, &s) in chunk.iter().enumerate() {
            frame[i] = (s as f32) / 32768.0;
        }

        // Analyze + suppress in-place
        ns.analyze(&frame);
        ns.process(&mut frame);

        // Convert back to i16 (only the actual samples for the last frame)
        for &s in frame.iter().take(chunk.len()) {
            output.push((s.clamp(-1.0, 1.0) * 32767.0) as i16);
        }
    }

    output
}

/// Voice Activity Detector with adaptive noise floor
pub struct Vad {
    noise_floor: f32,
    threshold_mult: f32,
    silence_frames: u32,
    speech_frames: u32,
    is_speech: bool,
    silence_timeout_frames: u32,
    min_speech_frames: u32,
}

impl Vad {
    pub fn new(config: &AudioConfig, _sample_rate: u32) -> Self {
        let silence_frames = (config.silence_timeout_ms as f64 / 30.0) as u32;
        Vad {
            noise_floor: 0.01,
            threshold_mult: config.vad_threshold,
            silence_frames: 0,
            speech_frames: 0,
            is_speech: false,
            silence_timeout_frames: silence_frames.max(3),
            min_speech_frames: 2,
        }
    }

    /// Process a buffer of samples. Returns the current VAD state.
    pub fn process(&mut self, samples: &[i16]) -> VADState {
        let rms = compute_rms(samples);

        // Update noise floor (slow adaptation when silent)
        if !self.is_speech {
            self.noise_floor = self.noise_floor * 0.998 + rms * 0.002;
        }

        let threshold = (self.noise_floor * self.threshold_mult).max(0.005);

        if rms > threshold {
            self.speech_frames += 1;
            self.silence_frames = 0;
        } else {
            self.silence_frames += 1;
            self.speech_frames = 0;
        }

        match self.is_speech {
            false => {
                if self.speech_frames >= self.min_speech_frames {
                    self.is_speech = true;
                    self.speech_frames = 0;
                    VADState::SpeechStarted
                } else {
                    VADState::Silence
                }
            }
            true => {
                if self.silence_frames >= self.silence_timeout_frames {
                    self.is_speech = false;
                    self.silence_frames = 0;
                    VADState::SpeechEnded
                } else {
                    VADState::Speech
                }
            }
        }
    }

    pub fn noise_floor(&self) -> f32 {
        self.noise_floor
    }

    pub fn reset(&mut self) {
        self.silence_frames = 0;
        self.speech_frames = 0;
        self.is_speech = false;
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VADState {
    Silence,
    SpeechStarted,
    Speech,
    SpeechEnded,
}

/// Shared state between audio thread and main thread
pub struct AudioShared {
    pub ring_buffer: RingBuffer,
    pub recording_buffer: Vec<i16>,
    pub is_recording: bool,
    pub current_rms: f32,
    pub noise_floor: f32,
}

/// Events from the audio thread to the main thread
#[derive(Debug, Clone)]
pub enum AudioEvent {
    SpeechEnded,
}

/// Start continuous audio capture. Returns the stream handle (must be kept alive).
pub fn start_audio_capture(
    config: &AudioConfig,
    shared: Arc<Mutex<AudioShared>>,
    event_tx: mpsc::Sender<AudioEvent>,
) -> anyhow::Result<cpal::Stream> {
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or_else(|| anyhow::anyhow!("No microphone found"))?;

    log::info!("Using audio device (default input)");

    // Get default input config
    let _supported_config = device
        .default_input_config()
        .map_err(|e| anyhow::anyhow!("Failed to get default input config: {}", e))?;

    // Build a stream config with our desired sample rate
    let stream_config = cpal::StreamConfig {
        channels: 1,
        sample_rate: config.sample_rate,
        buffer_size: cpal::BufferSize::Default,
    };

    let sample_rate = stream_config.sample_rate;
    let num_channels = stream_config.channels as usize;

    log::info!("Audio stream: {} Hz, {} channels", sample_rate, num_channels);

    let shared_clone = shared.clone();
    let event_tx_clone = event_tx;
    let mut vad = Vad::new(config, sample_rate);

    let stream = device.build_input_stream(
        stream_config,
        move |data: &[f32], _: &cpal::InputCallbackInfo| {
            // Convert f32 samples to i16, take first channel if stereo
            let mono: Vec<i16> = data
                .chunks(num_channels)
                .map(|chunk| (chunk[0].clamp(-1.0, 1.0) * 32767.0) as i16)
                .collect();

            if let Ok(mut shared) = shared_clone.lock() {
                // Always push to ring buffer
                shared.ring_buffer.push(&mono);
                shared.current_rms = compute_rms(&mono);
                shared.noise_floor = vad.noise_floor();

                if shared.is_recording {
                    // Append to recording buffer
                    shared.recording_buffer.extend_from_slice(&mono);

                    // Run VAD
                    match vad.process(&mono) {
                        VADState::SpeechEnded => {
                            log::debug!("VAD: speech ended");
                            event_tx_clone.send(AudioEvent::SpeechEnded).ok();
                        }
                        VADState::SpeechStarted => {
                            log::debug!("VAD: speech started");
                        }
                        _ => {}
                    }
                } else {
                    vad.reset();
                }
            }
        },
        move |err| {
            log::error!("Audio stream error: {}", err);
        },
        None, // timeout
    )?;

    stream.play()?;
    log::info!("Audio capture started");

    Ok(stream)
}
