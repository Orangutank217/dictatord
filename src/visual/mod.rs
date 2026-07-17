pub mod render;
pub mod window;

use crate::config::VisualConfig;
use crate::visual::window::VisualizerWindow;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

/// Commands from the main thread to the visualizer thread
#[derive(Debug, Clone)]
pub enum VisualCommand {
    /// Show the visualizer window (listening started)
    Show,
    /// Hide the visualizer window (done)
    Hide,
    /// Update the preview text shown under the orb
    SetPreviewText(String),
    /// Shut down the visualizer thread
    Shutdown,
}

/// Audio levels shared between audio thread and visualizer thread
pub struct VisualizerState {
    pub current_rms: f32,
}

/// Run the GTK visualizer in a separate thread.
/// This function blocks until Shutdown is received.
pub fn run_visualizer(
    shared_state: Arc<Mutex<VisualizerState>>,
    rx: mpsc::Receiver<VisualCommand>,
    config: VisualConfig,
) {
    // Initialize GTK
    if gtk::init().is_err() {
        log::error!("Failed to initialize GTK. Visualizer disabled.");
        return;
    }

    let viz = std::rc::Rc::new(VisualizerWindow::new(&config));

    // Set up a 60fps timer to update the visualizer
    let shared = shared_state.clone();
    glib::timeout_add_local(std::time::Duration::from_millis(16), move || {
        // Process incoming commands
        while let Ok(cmd) = rx.try_recv() {
            match cmd {
                VisualCommand::Show => {
                    log::debug!("Visualizer: show");
                    viz.show_all();
                }
                VisualCommand::Hide => {
                    log::debug!("Visualizer: hide");
                    if let Ok(mut state) = shared.lock() {
                        state.current_rms = 0.0;
                    }
                    viz.set_rms(0.0);
                    viz.set_preview_text("");
                    viz.hide();
                }
                VisualCommand::SetPreviewText(text) => {
                    viz.set_preview_text(&text);
                }
                VisualCommand::Shutdown => {
                    viz.hide();
                    gtk::main_quit();
                    return glib::ControlFlow::Break;
                }
            }
        }

        // Read RMS from shared state and update the window for animation
        if let Ok(state) = shared.lock() {
            viz.set_rms(state.current_rms);
        }

        // Queue a redraw for the animation
        viz.queue_draw();
        glib::ControlFlow::Continue
    });

    log::info!("GTK visualizer started");
    gtk::main();
    log::info!("GTK visualizer stopped");
}
