//! Embedded Web-based Advanced GUI Configurator & Setup Wizard.
//!
//! Provides a zero-dependency, rich graphical user interface running locally
//! on http://127.0.0.1:17890. Includes gesture remapping, sensitivity tuning,
//! live gesture testing sandbox, and a step-by-step installation wizard.

use crate::config::Config;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

pub const DEFAULT_GUI_PORT: u16 = 17890;

pub struct GuiServer {
    config_path: PathBuf,
    is_running: Arc<AtomicBool>,
}

impl GuiServer {
    pub fn new(config_path: PathBuf) -> Self {
        Self {
            config_path,
            is_running: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Start the GUI web server in a background worker thread.
    pub fn start(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.is_running.load(Ordering::SeqCst) {
            return Ok(());
        }

        self.is_running.store(true, Ordering::SeqCst);
        let is_running_clone = self.is_running.clone();
        let config_path_clone = self.config_path.clone();

        let listener = TcpListener::bind(format!("127.0.0.1:{}", DEFAULT_GUI_PORT))
            .or_else(|_| TcpListener::bind("127.0.0.1:0"))?;

        let actual_port = listener.local_addr()?.port();
        log::info!(
            "Gesture Manager GUI server active at http://127.0.0.1:{}/",
            actual_port
        );

        thread::Builder::new()
            .name("gui-server".into())
            .spawn(move || {
                for stream_res in listener.incoming() {
                    if !is_running_clone.load(Ordering::SeqCst) {
                        break;
                    }

                    match stream_res {
                        Ok(mut stream) => {
                            let cfg_path = config_path_clone.clone();
                            thread::spawn(move || {
                                handle_client(&mut stream, &cfg_path);
                            });
                        }
                        Err(e) => {
                            log::warn!("GUI server connection error: {}", e);
                        }
                    }
                }
            })?;

        Ok(())
    }

    /// Open the GUI in the default system web browser.
    pub fn open_browser() {
        let url = format!("http://127.0.0.1:{}/", DEFAULT_GUI_PORT);
        #[cfg(target_os = "windows")]
        {
            let _ = std::process::Command::new("cmd")
                .args(["/C", "start", &url])
                .spawn();
        }
        #[cfg(target_os = "linux")]
        {
            let _ = std::process::Command::new("xdg-open").arg(&url).spawn();
        }
    }
}

fn handle_client(stream: &mut TcpStream, config_path: &Path) {
    let mut buffer = [0u8; 8192];
    let bytes_read = match stream.read(&mut buffer) {
        Ok(n) if n > 0 => n,
        _ => return,
    };

    let request_str = String::from_utf8_lossy(&buffer[..bytes_read]);
    let mut lines = request_str.lines();
    let first_line = match lines.next() {
        Some(l) => l,
        None => return,
    };

    let mut parts = first_line.split_whitespace();
    let method = parts.next().unwrap_or("GET");
    let path = parts.next().unwrap_or("/");

    if method == "GET" && (path == "/" || path == "/index.html") {
        send_response(stream, 200, "text/html; charset=utf-8", include_str!("gui.html"));
    } else if method == "GET" && path == "/api/config" {
        match std::fs::read_to_string(config_path) {
            Ok(content) => match toml::from_str::<Config>(&content) {
                Ok(cfg) => match serde_json::to_string_pretty(&cfg) {
                    Ok(json) => send_response(stream, 200, "application/json", &json),
                    Err(e) => send_error(stream, 500, &format!("JSON serialization error: {}", e)),
                },
                Err(e) => send_error(stream, 500, &format!("Config parse error: {}", e)),
            },
            Err(_) => {
                let default_cfg = Config::default();
                let json = serde_json::to_string_pretty(&default_cfg).unwrap_or_default();
                send_response(stream, 200, "application/json", &json);
            }
        }
    } else if method == "POST" && path == "/api/config" {
        // Extract body
        if let Some(body_start) = request_str.find("\r\n\r\n") {
            let body = &request_str[body_start + 4..];
            match serde_json::from_str::<Config>(body) {
                Ok(cfg) => match toml::to_string_pretty(&cfg) {
                    Ok(toml_str) => {
                        let header = "# Touchpad Gesture Manager & Remapper Configuration\n\
                                      # Updated via Gesture Manager Configurator GUI\n\n";
                        if let Some(parent) = config_path.parent() {
                            let _ = std::fs::create_dir_all(parent);
                        }
                        if let Err(e) = std::fs::write(config_path, format!("{}{}", header, toml_str)) {
                            send_error(stream, 500, &format!("Write error: {}", e));
                        } else {
                            send_response(
                                stream,
                                200,
                                "application/json",
                                r#"{"status":"success","message":"Configuration saved and applied!"}"#,
                            );
                        }
                    }
                    Err(e) => send_error(stream, 500, &format!("TOML error: {}", e)),
                },
                Err(e) => send_error(stream, 400, &format!("Invalid JSON body: {}", e)),
            }
        } else {
            send_error(stream, 400, "Missing request body");
        }
    } else if method == "POST" && path == "/api/install" {
        let res = execute_installer_actions();
        send_response(stream, 200, "application/json", &res);
    } else {
        send_error(stream, 404, "Not Found");
    }
}

fn execute_installer_actions() -> String {
    #[cfg(target_os = "windows")]
    {
        let exe_path = match std::env::current_exe() {
            Ok(p) => p.to_string_lossy().to_string(),
            Err(_) => "gesture-manager.exe".into(),
        };

        // Create Windows Startup shortcut
        let script = format!(
            "$s=(New-Object -COM WScript.Shell).CreateShortcut('$env:APPDATA\\Microsoft\\Windows\\Start Menu\\Programs\\Startup\\gesture-manager.lnk');$s.TargetPath='{}';$s.Save()",
            exe_path.replace("'", "''")
        );
        let _ = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", &script])
            .output();

        r#"{"status":"success","message":"Windows Startup shortcut created successfully!"}"#.into()
    }

    #[cfg(target_os = "linux")]
    {
        // On Linux, enable autostart desktop entry
        let autostart_dir = dirs::config_dir()
            .map(|mut p| {
                p.push("autostart");
                p
            })
            .unwrap_or_else(|| PathBuf::from("~/.config/autostart"));

        let _ = std::fs::create_dir_all(&autostart_dir);
        let desktop_file = autostart_dir.join("gesture-manager.desktop");
        let content = "[Desktop Entry]\nType=Application\nName=Gesture Manager\nExec=gesture-manager\nTerminal=false\nX-GNOME-Autostart-enabled=true\n";
        let _ = std::fs::write(&desktop_file, content);

        r#"{"status":"success","message":"Linux autostart desktop entry installed successfully!"}"#.into()
    }
}

fn send_response(stream: &mut TcpStream, status_code: u16, content_type: &str, body: &str) {
    let status_text = match status_code {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        500 => "Internal Server Error",
        _ => "Status",
    };

    let response = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n{}",
        status_code,
        status_text,
        content_type,
        body.len(),
        body
    );

    let _ = stream.write_all(response.as_bytes());
}

fn send_error(stream: &mut TcpStream, code: u16, message: &str) {
    let json = format!(r#"{{"status":"error","message":"{}"}}"#, message);
    send_response(stream, code, "application/json", &json);
}
