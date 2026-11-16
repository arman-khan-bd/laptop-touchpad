//! Keystroke synthesis and shell command executor.

use crate::config::ActionConfig;
use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

pub struct ActionExecutor {
    enigo: Mutex<Enigo>,
}

impl ActionExecutor {
    pub fn new() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let enigo = Enigo::new(&Settings::default())
            .map_err(|e| format!("Failed to initialize Enigo keyboard engine: {:?}", e))?;
        Ok(Self {
            enigo: Mutex::new(enigo),
        })
    }

    /// Execute an action mapping (hotkey, command, or both).
    pub fn execute(&self, action: &ActionConfig) {
        if let Some(cmd) = action.command() {
            let cmd_str = cmd.to_string();
            thread::Builder::new()
                .name("cmd-executor".into())
                .spawn(move || {
                    log::info!("Executing shell command: {}", cmd_str);
                    #[cfg(target_os = "windows")]
                    let res = std::process::Command::new("cmd")
                        .args(["/C", &cmd_str])
                        .output();

                    #[cfg(not(target_os = "windows"))]
                    let res = std::process::Command::new("sh")
                        .args(["-c", &cmd_str])
                        .output();

                    match res {
                        Ok(output) => {
                            if !output.status.success() {
                                log::warn!(
                                    "Command '{}' exited with status {}: {}",
                                    cmd_str,
                                    output.status,
                                    String::from_utf8_lossy(&output.stderr)
                                );
                            }
                        }
                        Err(e) => {
                            log::error!("Failed to execute command '{}': {}", cmd_str, e);
                        }
                    }
                })
                .ok();
        }

        if let Some(hotkey_str) = action.hotkey() {
            if let Err(e) = self.send_hotkey(hotkey_str) {
                log::error!("Failed to simulate hotkey '{}': {}", hotkey_str, e);
            }
        }
    }

    /// Simulate a multi-key shortcut (e.g., "Ctrl+Alt+Delete", "Super+Tab", "Ctrl+Plus").
    pub fn send_hotkey(&self, hotkey: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let tokens: Vec<&str> = hotkey.split('+').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
        if tokens.is_empty() {
            return Ok(());
        }

        let mut modifiers = Vec::new();
        let mut base_keys = Vec::new();

        for token in tokens {
            let lower = token.to_lowercase();
            match lower.as_str() {
                "ctrl" | "control" => modifiers.push(Key::Control),
                "alt" | "option" => modifiers.push(Key::Alt),
                "shift" => modifiers.push(Key::Shift),
                "super" | "win" | "meta" | "windows" | "cmd" | "command" => modifiers.push(Key::Meta),
                _ => {
                    let key = parse_single_key(token)?;
                    base_keys.push(key);
                }
            }
        }

        let mut enigo = self.enigo.lock().map_err(|_| "Enigo lock poisoned")?;

        // 1. Press modifier keys in order
        for &m in &modifiers {
            enigo.key(m, Direction::Press)
                .map_err(|e| format!("Enigo modifier press error: {:?}", e))?;
        }

        if !modifiers.is_empty() {
            thread::sleep(Duration::from_millis(15));
        }

        // 2. Click base keys (or press and release if no base key was present, e.g. solo "Super")
        if base_keys.is_empty() {
            // Hotkey consisted solely of modifiers (e.g., solo "Super" to open App Launcher)
            // Just releasing will register the key release
        } else {
            for &k in &base_keys {
                enigo.key(k, Direction::Click)
                    .map_err(|e| format!("Enigo key click error: {:?}", e))?;
                thread::sleep(Duration::from_millis(10));
            }
        }

        if !modifiers.is_empty() {
            thread::sleep(Duration::from_millis(15));
        }

        // 3. Release modifier keys in reverse order
        for &m in modifiers.iter().rev() {
            enigo.key(m, Direction::Release)
                .map_err(|e| format!("Enigo modifier release error: {:?}", e))?;
        }

        log::info!("Simulated hotkey: {}", hotkey);
        Ok(())
    }
}

fn parse_single_key(token: &str) -> Result<Key, Box<dyn std::error::Error + Send + Sync>> {
    let lower = token.to_lowercase();
    let key = match lower.as_str() {
        "tab" => Key::Tab,
        "left" | "arrowleft" => Key::LeftArrow,
        "right" | "arrowright" => Key::RightArrow,
        "up" | "arrowup" => Key::UpArrow,
        "down" | "arrowdown" => Key::DownArrow,
        "enter" | "return" => Key::Return,
        "esc" | "escape" => Key::Escape,
        "space" => Key::Space,
        "backspace" => Key::Backspace,
        "delete" | "del" => Key::Delete,
        "pageup" | "pgup" => Key::PageUp,
        "pagedown" | "pgdn" => Key::PageDown,
        "home" => Key::Home,
        "end" => Key::End,
        "f1" => Key::F1,
        "f2" => Key::F2,
        "f3" => Key::F3,
        "f4" => Key::F4,
        "f5" => Key::F5,
        "f6" => Key::F6,
        "f7" => Key::F7,
        "f8" => Key::F8,
        "f9" => Key::F9,
        "f10" => Key::F10,
        "f11" => Key::F11,
        "f12" => Key::F12,
        "plus" => Key::Unicode('+'),
        "minus" => Key::Unicode('-'),
        _ => {
            let mut chars = token.chars();
            if let Some(c) = chars.next() {
                if chars.next().is_none() {
                    Key::Unicode(c)
                } else {
                    return Err(format!("Unrecognized key token '{}'", token).into());
                }
            } else {
                return Err("Empty key token".into());
            }
        }
    };
    Ok(key)
}
