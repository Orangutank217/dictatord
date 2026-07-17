use crate::config::OutputConfig;
use libxdo::XDo;

/// Type text at the current cursor position
pub fn type_text(text: &str, config: &OutputConfig) -> anyhow::Result<()> {
    if text.is_empty() {
        log::debug!("Empty text, nothing to type");
        return Ok(());
    }

    let mut processed = text.to_string();

    // Auto-capitalize first letter
    if config.capitalize {
        if let Some(c) = processed.chars().next() {
            if c.is_ascii_lowercase() {
                processed = c.to_ascii_uppercase().to_string() + &processed[1..];
            }
        }
    }

    // Add trailing space
    if config.add_trailing_space && !processed.ends_with(' ') {
        processed.push(' ');
    }

    log::info!("Typing: \"{}\"", &processed);

    let xdo = XDo::new(None).map_err(|e| anyhow::anyhow!("Failed to init XDo: {}", e))?;

    let delay_microsecs = (config.typing_delay_ms * 1000) as u32;

    // Type the entire text (libxdo handles character-by-character internally with delay)
    xdo.enter_text(&processed, delay_microsecs)
        .map_err(|e| anyhow::anyhow!("Failed to type text: {}", e))?;

    log::debug!("Text typed successfully");
    Ok(())
}
