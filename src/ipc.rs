//! Unix domain socket IPC for live config management.
//!
//! The daemon listens on `~/.cache/dictatord/ctl.sock` for JSON commands:
//! - `{"cmd": "get_config"}` → returns current config as JSON
//! - `{"cmd": "set", "key": "audio.noise_suppression", "value": "false"}` → apply and save

use crate::config::Config;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::Duration;

/// Socket path: ~/.cache/dictatord/ctl.sock
fn socket_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    PathBuf::from(format!("{}/.cache/dictatord/ctl.sock", home))
}

/// Clean up the socket file (called on startup and shutdown)
pub fn cleanup_socket() {
    let path = socket_path();
    if path.exists() {
        let _ = std::fs::remove_file(&path);
    }
}

/// Start the IPC server in a background thread.
/// Returns a handle that will clean up the socket when dropped.
pub fn start_ipc_server(config: Arc<RwLock<Config>>) -> anyhow::Result<IpcHandle> {
    let path = socket_path();

    // Remove stale socket from previous run
    cleanup_socket();

    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let listener = UnixListener::bind(&path)?;
    // Set non-blocking so we can check for shutdown between connections
    listener.set_nonblocking(true)?;

    let socket_path_str = path.to_string_lossy().to_string();
    log::info!("IPC server listening on {}", socket_path_str);

    std::thread::Builder::new()
        .name("ipc-server".into())
        .spawn(move || {
            run_ipc_loop(listener, config);
        })?;

    Ok(IpcHandle { _socket_path: socket_path_str })
}

/// Handle that ensures socket cleanup on drop
pub struct IpcHandle {
    _socket_path: String,
}

impl Drop for IpcHandle {
    fn drop(&mut self) {
        cleanup_socket();
    }
}

fn run_ipc_loop(listener: UnixListener, config: Arc<RwLock<Config>>) {
    // Buffer for reading commands
    let mut buf = Vec::new();

    loop {
        match listener.accept() {
            Ok((stream, _addr)) => {
                buf.clear();
                if let Err(e) = handle_connection(stream, &config, &mut buf) {
                    log::debug!("IPC connection error: {}", e);
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                // No pending connection — sleep briefly, then retry
                std::thread::sleep(Duration::from_millis(200));
            }
            Err(e) => {
                log::error!("IPC accept error: {}", e);
                std::thread::sleep(Duration::from_secs(1));
            }
        }
    }
}

fn handle_connection(
    mut stream: UnixStream,
    config: &Arc<RwLock<Config>>,
    buf: &mut Vec<u8>,
) -> anyhow::Result<()> {
    // Set timeout for read/write so a stuck client doesn't hang the server
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;

    // Read one line (newline-delimited JSON)
    let mut reader = BufReader::new(&stream);
    buf.clear();
    reader.read_until(b'\n', buf)?;

    if buf.is_empty() {
        return Ok(()); // EOF, client disconnected
    }

    // Parse JSON request
    let request: serde_json::Value = serde_json::from_slice(buf)?;
    let cmd = request
        .get("cmd")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let response = match cmd {
        "get_config" => {
            let cfg = config.read().unwrap();
            let data = cfg.to_json_value();
            serde_json::json!({"status": "ok", "data": data})
        }
        "set" => {
            let key = request
                .get("key")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let value = request
                .get("value")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            if key.is_empty() {
                serde_json::json!({"status": "error", "message": "missing 'key' field"})
            } else {
                let mut cfg = config.write().unwrap();
                match cfg.apply(key, value) {
                    Ok(()) => {
                        let needs_restart = Config::requires_restart(key);
                        let mut resp = serde_json::json!({"status": "ok"});
                        if needs_restart {
                            resp["restart_required"] = serde_json::Value::Bool(true);
                            resp["message"] =
                                serde_json::Value::String("restart required to take effect".into());
                        }
                        resp
                    }
                    Err(e) => {
                        serde_json::json!({"status": "error", "message": e.to_string()})
                    }
                }
            }
        }
        "restart" => {
            // Signal restart is needed — handled by the CLI, not daemon
            serde_json::json!({"status": "ok", "message": "restart the service manually: systemctl --user restart dictatord"})
        }
        _ => serde_json::json!({"status": "error", "message": format!("unknown command: {}", cmd)}),
    };

    // Send response as newline-delimited JSON
    let mut response_bytes = serde_json::to_vec(&response)?;
    response_bytes.push(b'\n');
    stream.write_all(&response_bytes)?;

    Ok(())
}

/// Connect to the daemon's IPC socket, send a command, and return the response.
/// Returns None if the socket doesn't exist or the daemon isn't responding.
pub fn send_command(cmd: &serde_json::Value) -> anyhow::Result<serde_json::Value> {
    let path = socket_path();
    if !path.exists() {
        anyhow::bail!("Daemon not running (socket not found at {})", path.display());
    }

    // Connect (with 2-second timeout via socket options)
    let stream = UnixStream::connect(&path)?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;

    // Send request
    let mut request_bytes = serde_json::to_vec(cmd)?;
    request_bytes.push(b'\n');
    let mut writer = std::io::BufWriter::new(&stream);
    writer.write_all(&request_bytes)?;
    writer.flush()?;
    drop(writer);

    // Read response
    let mut reader = BufReader::new(&stream);
    let mut response = String::new();
    reader.read_line(&mut response)?;

    if response.is_empty() {
        anyhow::bail!("Empty response from daemon");
    }

    let value: serde_json::Value = serde_json::from_str(&response)?;
    // Check for error status
    if value.get("status").and_then(|v| v.as_str()) == Some("error") {
        let msg = value
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown error");
        anyhow::bail!("Daemon error: {}", msg);
    }

    Ok(value)
}
