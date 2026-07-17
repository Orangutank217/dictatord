use crate::config::{parse_hex_color, VisualConfig};
use crate::visual::render::draw_visualizer;
use gtk::prelude::{GtkWindowExt, MonitorExt, WidgetExt};
use gtk::{Window, WindowType};
use std::cell::RefCell;
use std::rc::Rc;

pub struct VisualizerWindow {
    pub window: Window,
    preview_text: Rc<RefCell<String>>,
    current_rms: Rc<RefCell<f32>>,
}

impl VisualizerWindow {
    pub fn new(config: &VisualConfig) -> Self {
        let window = Window::new(WindowType::Popup);
        window.set_default_size(200, 56);
        window.set_resizable(false);
        window.set_decorated(false);
        window.set_keep_above(true);
        window.set_skip_taskbar_hint(true);
        window.set_skip_pager_hint(true);
        window.set_accept_focus(false);

        // Enable RGBA/transparency
        if let Some(screen) = WidgetExt::screen(&window) {
            if let Some(visual) = screen.rgba_visual() {
                window.set_visual(Some(&visual));
            } else {
                log::warn!("RGBA visual not available, visualizer may have solid background");
            }
        }

        // Set app-paintable so we handle all drawing ourselves
        window.set_app_paintable(true);

        let color = parse_hex_color(&config.color_hex);
        let opacity = config.opacity;

        let preview_text: Rc<RefCell<String>> = Rc::new(RefCell::new(String::new()));
        let current_rms: Rc<RefCell<f32>> = Rc::new(RefCell::new(0.0));

        // Clones for the draw callback
        let preview_clone = preview_text.clone();
        let rms_clone = current_rms.clone();

        // Connect draw signal — captures color and opacity by value (they are Copy)
        window.connect_draw(move |_w, cr| {
            let text = preview_clone.borrow();
            let rms = *rms_clone.borrow();
            draw_visualizer(cr, 200.0, 56.0, &text, rms, color, opacity);
            glib::Propagation::Proceed
        });

        // Position at top-right of primary monitor
        position_top_right(&window);

        let viz_window = VisualizerWindow {
            window,
            preview_text,
            current_rms,
        };

        // Start hidden
        viz_window.window.hide();

        viz_window
    }

    pub fn queue_draw(&self) {
        self.window.queue_draw();
    }

    pub fn set_preview_text(&self, text: &str) {
        *self.preview_text.borrow_mut() = text.to_string();
        self.queue_draw();
    }

    pub fn set_rms(&self, rms: f32) {
        *self.current_rms.borrow_mut() = rms;
    }

    pub fn show_all(&self) {
        self.window.show_all();
    }

    pub fn hide(&self) {
        self.window.hide();
    }
}

fn position_top_right(window: &Window) {
    if let Some(screen) = WidgetExt::screen(window) {
        let display = screen.display();
        let monitor = display.primary_monitor();

        if let Some(monitor) = monitor {
            let geometry = MonitorExt::geometry(&monitor);
            let win_width = 200;
            let _win_height = 56;
            let padding = 16;
            let x = geometry.x() + geometry.width() - win_width - padding;
            let y = geometry.y() + padding;
            window.move_(x, y);
            log::debug!("Visualizer positioned at ({}, {})", x, y);
            return;
        }

        // Fallback: use display width from the first monitor
        let display = screen.display();
        if let Some(monitor) = display.monitor(0) {
            let rect = MonitorExt::geometry(&monitor);
            let x = rect.x() + rect.width() - 216;
            window.move_(x, 16);
        }
    }
}
