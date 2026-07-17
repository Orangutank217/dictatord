use crate::config::AsrConfig;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

/// Speech-to-text engine wrapping whisper.cpp.
///
/// The model is kept in system RAM as a byte buffer.
/// A GPU-accelerated WhisperContext is created once at startup (if GPU is available)
/// and reused for every transcription — avoiding costly GPU re-initialization.
pub struct ASREngine {
    model_buffer: Vec<u8>,
    config: AsrConfig,
    /// Cached GPU context, created eagerly at startup.
    /// If GPU init fails, this is `None` and we fall back to CPU-only per-call.
    gpu_ctx: Option<WhisperContext>,
}

impl ASREngine {
    /// Initialize the ASR engine from a model buffer.
    ///
    /// 1. Validates the model on CPU (quick check).
    /// 2. If GPU offload is enabled, pre-initializes the GPU context so the
    ///    first dictation does not pay the CUDA setup cost.
    pub fn new(model_buffer: Vec<u8>, config: &AsrConfig) -> anyhow::Result<Self> {
        log::info!("Validating model from buffer ({} MB)...", model_buffer.len() / 1_048_576);

        // Quick CPU-side validation: create a context to verify the model is valid
        let validate_params = WhisperContextParameters {
            use_gpu: false,
            ..Default::default()
        };
        let _validate_ctx = WhisperContext::new_from_buffer_with_params(&model_buffer, validate_params)
            .map_err(|e| anyhow::anyhow!("Invalid or corrupted model: {}", e))?;
        log::info!("Model validated successfully");

        // Pre-initialize GPU context at startup (if enabled)
        let gpu_ctx = if config.gpu_offload {
            log::info!(
                "Pre-initializing GPU context (device={}, flash_attn={})...",
                config.gpu_device,
                config.flash_attn,
            );
            let ctx_params = WhisperContextParameters {
                use_gpu: true,
                gpu_device: config.gpu_device,
                flash_attn: config.flash_attn,
                ..Default::default()
            };
            match WhisperContext::new_from_buffer_with_params(&model_buffer, ctx_params) {
                Ok(ctx) => {
                    log::info!("GPU context initialized and cached — subsequent transcriptions will reuse it");
                    Some(ctx)
                }
                Err(e) => {
                    log::warn!(
                        "GPU context creation failed at startup: {}. \
                         Will fall back to CPU for this session.",
                        e
                    );
                    None
                }
            }
        } else {
            log::info!(
                "GPU acceleration disabled, using CPU ({} threads)",
                config.num_threads
            );
            None
        };

        Ok(ASREngine {
            model_buffer,
            config: config.clone(),
            gpu_ctx,
        })
    }

    /// Transcribe audio samples to text.
    ///
    /// Reuses the cached GPU context when available (fast path), or creates a
    /// fresh CPU context each call (fallback path).
    pub fn transcribe(&self, audio: &[i16]) -> anyhow::Result<String> {
        if audio.is_empty() {
            log::warn!("transcribe called with empty audio");
            return Ok(String::new());
        }

        log::info!(
            "Transcribing {} samples ({:.1}s @ 16kHz)",
            audio.len(),
            audio.len() as f64 / 16000.0
        );

        // Compute and log audio level stats for debugging
        let sum_sq: f64 = audio.iter().map(|&s| (s as f64) * (s as f64)).sum();
        let rms = (sum_sq / audio.len() as f64).sqrt();
        let peak = audio.iter().map(|&s| s.abs()).max().unwrap_or(0);
        log::debug!("Audio stats: RMS={:.1}, peak={}, len={}", rms, peak, audio.len());

        // Convert i16 samples directly to f32 [-1.0, 1.0]
        let audio_f32: Vec<f32> = audio
            .iter()
            .map(|&s| (s as f32 / 32768.0).clamp(-1.0, 1.0))
            .collect();

        // --- Create a WhisperState from our cached or fallback context ---
        let mut state = if let Some(ref ctx) = self.gpu_ctx {
            // Fast path: reuse the cached GPU context
            ctx.create_state()
                .map_err(|e| anyhow::anyhow!("Failed to create whisper state from cached GPU context: {}", e))?
        } else {
            // Fallback: create a fresh CPU context each call
            // Don't retry GPU if it failed at startup — use CPU-only context
            let ctx_params = WhisperContextParameters {
                use_gpu: false,
                ..Default::default()
            };
            let ctx = WhisperContext::new_from_buffer_with_params(&self.model_buffer, ctx_params)
                .map_err(|e| anyhow::anyhow!("Failed to create whisper CPU context: {}", e))?;
            ctx.create_state()
                .map_err(|e| anyhow::anyhow!("Failed to create whisper state: {}", e))?
        };

        // --- Transcription params ---
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 5 });
        params.set_n_threads(self.config.num_threads);
        params.set_suppress_blank(true);
        params.set_suppress_nst(true);
        params.set_language(Some("en"));
        params.set_print_progress(false);
        params.set_print_timestamps(false);
        params.set_print_special(false);

        log::debug!("Calling whisper_full...");
        let result = state.full(params, &audio_f32);
        log::debug!("whisper_full returned: {:?}", result);

        if let Err(e) = result {
            anyhow::bail!("Transcription failed: {:?}", e);
        }

        let num_segments = state.full_n_segments();
        log::debug!("whisper produced {} segments", num_segments);

        let mut result = String::new();

        for i in 0..num_segments {
            let text = state
                .get_segment(i)
                .ok_or_else(|| anyhow::anyhow!("Failed to get segment text"))?
                .to_string();
            if !text.is_empty() {
                log::debug!("Segment {}: \"{}\"", i, text.trim());
                result.push_str(&text);
            }
        }

        let result = result.trim().to_string();
        log::info!("Transcribed: \"{}\"", &result);

        // Debug: save raw audio if transcription is empty (helps diagnose issues)
        if result.is_empty() && audio.len() > 1000 {
            let path = format!("/tmp/dictation_fail_{}.raw", std::process::id());
            let bytes: Vec<u8> = audio
                .iter()
                .flat_map(|&s| s.to_le_bytes())
                .collect();
            if let Err(e) = std::fs::write(&path, &bytes) {
                log::warn!("Failed to save debug audio: {}", e);
            } else {
                log::warn!(
                    "Empty transcription! Saved {} bytes of audio to {}",
                    bytes.len(),
                    path
                );
            }
        }

        Ok(result)
    }
}

/// Download the Whisper model if it doesn't exist
pub fn ensure_model_downloaded(config: &AsrConfig) -> anyhow::Result<std::path::PathBuf> {
    let model_path = model_file_path(config);

    if model_path.exists() {
        let size_mb = model_path.metadata()?.len() as f64 / 1_048_576.0;
        log::info!(
            "Model already cached: {} ({:.1} MB)",
            model_path.display(),
            size_mb
        );
        return Ok(model_path);
    }

    // Create parent directory
    if let Some(parent) = model_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let url = model_download_url(config);
    let model_name = &config.model;

    log::info!("Downloading Whisper '{}' model...", model_name);
    log::info!("URL: {}", url);
    eprintln!(
        "Downloading Whisper '{}' model (~{} MB)...",
        model_name,
        model_size_mb(model_name)
    );

    // Download using curl with progress display
    let model_path_str = model_path
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("Invalid model path"))?;

    eprintln!(
        "Downloading Whisper '{}' model (~{} MB)...",
        model_name,
        model_size_mb(model_name)
    );
    log::info!("Downloading model from: {}", url);

    let status = std::process::Command::new("curl")
        .args(["-L", "-o", model_path_str, "--progress-bar", &url])
        .status()
        .map_err(|e| anyhow::anyhow!("Failed to run curl: {}", e))?;

    if !status.success() {
        // Fallback: try wget
        eprintln!("curl failed, trying wget...");
        let status = std::process::Command::new("wget")
            .args(["-O", model_path_str, &url])
            .status()
            .map_err(|e| anyhow::anyhow!("Failed to run wget: {}", e))?;

        if !status.success() {
            anyhow::bail!("Failed to download model using both curl and wget");
        }
    }

    // Verify the download
    if !model_path.exists() || model_path.metadata()?.len() == 0 {
        anyhow::bail!("Downloaded model file is empty or missing");
    }

    let size_mb = model_path.metadata()?.len() as f64 / 1_048_576.0;
    log::info!(
        "Model downloaded to {} ({:.1} MB)",
        model_path.display(),
        size_mb
    );

    Ok(model_path)
}

fn model_file_path(config: &AsrConfig) -> std::path::PathBuf {
    let path = if config.model_path.starts_with("~/") {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        format!("{}/{}", home, &config.model_path[2..])
    } else {
        config.model_path.clone()
    };
    let mut path = std::path::PathBuf::from(path);
    path.push(format!("ggml-{}.bin", config.model));
    path
}

fn model_download_url(config: &AsrConfig) -> String {
    format!(
        "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-{}.bin",
        config.model
    )
}

fn model_size_mb(model: &str) -> &str {
    match model {
        "tiny" => "75",
        "base" => "142",
        "small" => "466",
        "medium" => "769",
        "large" => "1549",
        _ => "?",
    }
}
