use crate::config::HotkeyConfig;
use std::sync::mpsc;
use x11::keysym::*;
use x11::xlib::*;

/// Try to enable Xkb detectable auto-repeat.
/// This suppresses the auto-repeat KeyRelease events that X11 normally generates,
/// so holding a key only produces: KeyPress (once) → KeyRelease (when released).
/// Without this, auto-repeat generates alternating KeyRelease+KeyPress pairs
/// which our event loop would misinterpret as rapid toggle on/off.
fn enable_detectable_auto_repeat(display: *mut Display) -> bool {
    unsafe {
        let mut supported: i32 = 0;
        let result = XkbSetDetectableAutoRepeat(display, 1, &mut supported);
        if result != 0 && supported != 0 {
            log::info!("Enabled Xkb detectable auto-repeat");
            true
        } else {
            log::warn!(
                "XkbSetDetectableAutoRepeat not supported (result={}, supported={}). \
                 Holding the hotkey may cause rapid toggle.",
                result,
                supported
            );
            false
        }
    }
}

/// Events from the hotkey listener to the main thread
#[derive(Debug, Clone, Copy)]
pub enum HotkeyEvent {
    /// Key was pressed (or held long enough for PTT)
    Pressed,
    /// Key was released
    Released,
}

/// Parse a key config string like "Super+D" into (modifiers, keysym)
fn parse_key_combo(key_str: &str) -> anyhow::Result<(u32, KeySym)> {
    let parts: Vec<&str> = key_str.split('+').collect();
    if parts.is_empty() {
        anyhow::bail!("Empty key combination");
    }

    let mut modifiers: u32 = 0;
    let key_name = parts[parts.len() - 1];

    for &mod_part in &parts[..parts.len() - 1] {
        match mod_part.to_lowercase().as_str() {
            "super" | "mod4" | "win" => modifiers |= Mod4Mask,
            "ctrl" | "control" => modifiers |= ControlMask,
            "alt" | "mod1" => modifiers |= Mod1Mask,
            "shift" => modifiers |= ShiftMask,
            "mod2" | "num" => modifiers |= Mod2Mask,
            "mod3" => modifiers |= Mod3Mask,
            "mod5" => modifiers |= Mod5Mask,
            _ => log::warn!("Unknown modifier: {}", mod_part),
        }
    }

    // Convert key name to keysym
    let keysym = name_to_keysym(key_name)?;

    Ok((modifiers, keysym))
}

fn name_to_keysym(name: &str) -> anyhow::Result<KeySym> {
    // Common keys
    match name.to_lowercase().as_str() {
        "a" => return Ok(XK_a as KeySym),
        "b" => return Ok(XK_b as KeySym),
        "c" => return Ok(XK_c as KeySym),
        "d" => return Ok(XK_d as KeySym),
        "e" => return Ok(XK_e as KeySym),
        "f" => return Ok(XK_f as KeySym),
        "g" => return Ok(XK_g as KeySym),
        "h" => return Ok(XK_h as KeySym),
        "i" => return Ok(XK_i as KeySym),
        "j" => return Ok(XK_j as KeySym),
        "k" => return Ok(XK_k as KeySym),
        "l" => return Ok(XK_l as KeySym),
        "m" => return Ok(XK_m as KeySym),
        "n" => return Ok(XK_n as KeySym),
        "o" => return Ok(XK_o as KeySym),
        "p" => return Ok(XK_p as KeySym),
        "q" => return Ok(XK_q as KeySym),
        "r" => return Ok(XK_r as KeySym),
        "s" => return Ok(XK_s as KeySym),
        "t" => return Ok(XK_t as KeySym),
        "u" => return Ok(XK_u as KeySym),
        "v" => return Ok(XK_v as KeySym),
        "w" => return Ok(XK_w as KeySym),
        "x" => return Ok(XK_x as KeySym),
        "y" => return Ok(XK_y as KeySym),
        "z" => return Ok(XK_z as KeySym),
        "space" => return Ok(XK_space as KeySym),
        "return" | "enter" => return Ok(XK_Return as KeySym),
        "escape" | "esc" => return Ok(XK_Escape as KeySym),
        "tab" => return Ok(XK_Tab as KeySym),
        "backspace" => return Ok(XK_BackSpace as KeySym),
        "delete" => return Ok(XK_Delete as KeySym),
        "home" => return Ok(XK_Home as KeySym),
        "end" => return Ok(XK_End as KeySym),
        "page_up" => return Ok(XK_Page_Up as KeySym),
        "page_down" => return Ok(XK_Page_Down as KeySym),
        "up" => return Ok(XK_Up as KeySym),
        "down" => return Ok(XK_Down as KeySym),
        "left" => return Ok(XK_Left as KeySym),
        "right" => return Ok(XK_Right as KeySym),
        "f1" => return Ok(XK_F1 as KeySym),
        "f2" => return Ok(XK_F2 as KeySym),
        "f3" => return Ok(XK_F3 as KeySym),
        "f4" => return Ok(XK_F4 as KeySym),
        "f5" => return Ok(XK_F5 as KeySym),
        "f6" => return Ok(XK_F6 as KeySym),
        "f7" => return Ok(XK_F7 as KeySym),
        "f8" => return Ok(XK_F8 as KeySym),
        "f9" => return Ok(XK_F9 as KeySym),
        "f10" => return Ok(XK_F10 as KeySym),
        "f11" => return Ok(XK_F11 as KeySym),
        "f12" => return Ok(XK_F12 as KeySym),
        "§" | "section" | "sect" => return Ok(XK_section as KeySym),
        "0" => return Ok(XK_0 as KeySym),
        "1" => return Ok(XK_1 as KeySym),
        "2" => return Ok(XK_2 as KeySym),
        "3" => return Ok(XK_3 as KeySym),
        "4" => return Ok(XK_4 as KeySym),
        "5" => return Ok(XK_5 as KeySym),
        "6" => return Ok(XK_6 as KeySym),
        "7" => return Ok(XK_7 as KeySym),
        "8" => return Ok(XK_8 as KeySym),
        "9" => return Ok(XK_9 as KeySym),
        _ => {}
    }

    anyhow::bail!("Unknown key name: {}", name)
}

/// Run the X11 hotkey listener loop.
/// This function blocks until the daemon should quit (which doesn't happen currently).
pub fn run_hotkey_listener(
    config: &HotkeyConfig,
    event_tx: mpsc::Sender<HotkeyEvent>,
) -> anyhow::Result<()> {
    let (modifiers, keysym) = parse_key_combo(&config.key)?;

    let display_name = std::ptr::null();
    let display = unsafe { XOpenDisplay(display_name) };
    if display.is_null() {
        anyhow::bail!("Cannot open X display. Is X11 running?");
    }

    let root = unsafe { XDefaultRootWindow(display) };
    let keycode = unsafe { XKeysymToKeycode(display, keysym) };

    if keycode == 0 {
        unsafe { XCloseDisplay(display) };
        anyhow::bail!("Invalid keycode for keysym");
    }

    log::info!(
        "Registering hotkey: {} (modifiers=0x{:x}, keycode={})",
        config.key,
        modifiers,
        keycode
    );

    // Enable detectable auto-repeat to prevent auto-repeat KeyRelease events
    // from being misinterpreted as real releases → rapid open/close flicker.
    enable_detectable_auto_repeat(display);

    // Grab the key combination globally
    unsafe {
        XGrabKey(
            display,
            keycode as i32,
            modifiers,
            root,
            1, // owner_events = True
            GrabModeAsync,
            GrabModeAsync,
        );
    }

    // Also grab with NumLock and CapsLock variants
    let numlock_mask = Mod2Mask;
    let caps_lock_mask = LockMask;

    for &extra in &[0, numlock_mask, caps_lock_mask, numlock_mask | caps_lock_mask] {
        unsafe {
            XGrabKey(
                display,
                keycode as i32,
                modifiers | extra,
                root,
                1,
                GrabModeAsync,
                GrabModeAsync,
            );
        }
    }

    unsafe { XSync(display, 0) };
    log::info!("Hotkey listener started. Waiting for key events...");

    // Event loop
    let mut event: XEvent = unsafe { std::mem::zeroed() };
    let mut key_is_down = false;

    #[allow(non_upper_case_globals)]
    unsafe {
        while XNextEvent(display, &mut event) == 0 {
            match event.get_type() {
                KeyPress => {
                    let xkey: &XKeyEvent = std::mem::transmute(&event);
                    if xkey.keycode == keycode as u32 {
                        // Ignore auto-repeat: if key is already down,
                        // this is a synthetic repeat event
                        if key_is_down {
                            continue;
                        }
                        key_is_down = true;
                        event_tx.send(HotkeyEvent::Pressed)?;
                    }
                }
                KeyRelease => {
                    let xkey: &XKeyEvent = std::mem::transmute(&event);
                    if xkey.keycode == keycode as u32 {
                        if !key_is_down {
                            continue; // Ignore orphan releases
                        }
                        key_is_down = false;
                        event_tx.send(HotkeyEvent::Released)?;
                    }
                }
                _ => {}
            }
        }
    }

    unsafe { XCloseDisplay(display) };
    log::info!("Hotkey listener stopped");
    Ok(())
}
