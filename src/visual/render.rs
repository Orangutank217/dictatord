use cairo::Context;

/// Draw the complete visualizer: background panel + pulsing orb + text
pub fn draw_visualizer(
    cr: &Context,
    width: f64,
    height: f64,
    preview_text: &str,
    rms: f32,
    color: (f64, f64, f64),
    opacity: f64,
) {
    // Clear to transparent
    cr.set_source_rgba(0.0, 0.0, 0.0, 0.0);
    cr.set_operator(cairo::Operator::Source);
    cr.paint().expect("Failed to clear background");
    cr.set_operator(cairo::Operator::Over);

    // Draw rounded background panel
    let corner_radius = 10.0;
    draw_rounded_rect(cr, 0.0, 0.0, width, height, corner_radius);
    cr.set_source_rgba(0.1, 0.1, 0.12, opacity);
    cr.fill().expect("Failed to fill background");

    // Draw the pulsing orb on the left side
    let orb_cx = 40.0;
    let orb_cy = height / 2.0;

    // Convert RMS (0.0-1.0) to amplitude with some amplification for quieter speech
    let amplitude = (rms * 8.0).clamp(0.05, 1.0) as f64;

    draw_pulse_orb(cr, orb_cx, orb_cy, amplitude, color);

    // Determine status text
    let status_text = if preview_text == "Processing\u{2026}" {
        "Processing\u{2026}"
    } else if preview_text.is_empty() {
        "Listening\u{2026}"
    } else {
        preview_text
    };

    // Draw recording indicator dot (green, pulsing)
    let dot_amplitude = if preview_text == "Processing\u{2026}" {
        0.5
    } else {
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();
        (t * 4.0).sin().abs() * 0.5 + 0.5
    };

    cr.set_source_rgba(0.0, 1.0, 0.3, 0.5 + dot_amplitude * 0.5);
    cr.arc(62.0, orb_cy, 3.0, 0.0, 2.0 * std::f64::consts::PI);
    cr.fill().expect("Failed to fill indicator dot");

    // Draw text next to the orb
    cr.set_source_rgba(1.0, 1.0, 1.0, 0.85);
    cr.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
    cr.set_font_size(13.0);

    let text_x = 72.0;
    let text_y = orb_cy + 4.5;

    // Truncate long text
    let display_text = if status_text.len() > 22 {
        format!("{}...", &status_text[..19])
    } else {
        status_text.to_string()
    };

    cr.move_to(text_x, text_y);
    cr.show_text(&display_text).expect("Failed to show text");
}

/// Draw the pulsing cyan orb with glow effect
fn draw_pulse_orb(cr: &Context, cx: f64, cy: f64, amplitude: f64, color: (f64, f64, f64)) {
    let amp = amplitude.clamp(0.01, 1.0);

    let base_radius = 7.0;
    let max_additional = 10.0;
    let radius = base_radius + amp * max_additional;

    // Outer glow - radial gradient
    let glow_radius = radius * 3.5;
    let gradient = cairo::RadialGradient::new(cx, cy, radius * 0.3, cx, cy, glow_radius);
    gradient.add_color_stop_rgba(0.0, color.0, color.1, color.2, amp * 0.35);
    gradient.add_color_stop_rgba(0.5, color.0, color.1, color.2, amp * 0.12);
    gradient.add_color_stop_rgba(1.0, color.0, color.1, color.2, 0.0);
    let _ = cr.set_source(&gradient);
    cr.arc(cx, cy, glow_radius, 0.0, 2.0 * std::f64::consts::PI);
    cr.fill().expect("Failed to fill glow");

    // Main orb body
    cr.set_source_rgba(color.0, color.1, color.2, 0.9);
    cr.arc(cx, cy, radius, 0.0, 2.0 * std::f64::consts::PI);
    cr.fill().expect("Failed to fill orb");

    // Inner highlight (top-left reflection)
    cr.set_source_rgba(1.0, 1.0, 1.0, 0.25);
    cr.arc(
        cx - radius * 0.2,
        cy - radius * 0.2,
        radius * 0.35,
        0.0,
        2.0 * std::f64::consts::PI,
    );
    cr.fill().expect("Failed to fill highlight");

    // Subtle ring outline
    cr.set_source_rgba(color.0, color.1, color.2, 0.3);
    cr.set_line_width(1.5);
    cr.arc(cx, cy, radius + 1.5, 0.0, 2.0 * std::f64::consts::PI);
    cr.stroke().expect("Failed to stroke ring");
}

/// Draw a rounded rectangle path
fn draw_rounded_rect(cr: &Context, x: f64, y: f64, w: f64, h: f64, r: f64) {
    let r = r.min(w / 2.0).min(h / 2.0);
    const PI: f64 = std::f64::consts::PI;
    const PI_2: f64 = PI / 2.0;

    cr.new_path();
    cr.arc(x + r, y + r, r, PI, 3.0 * PI_2);
    cr.arc(x + w - r, y + r, r, 3.0 * PI_2, 0.0);
    cr.arc(x + w - r, y + h - r, r, 0.0, PI_2);
    cr.arc(x + r, y + h - r, r, PI_2, PI);
    cr.close_path();
}
