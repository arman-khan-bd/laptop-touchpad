//! Keystroke synthesis, built-in action remapping, and shell command executor.

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

    /// Execute an action mapping (built-in action, hotkey, shell command, or combined).
    pub fn execute(&self, action: &ActionConfig) {
        if let Some(cmd) = action.command() {
            let cmd_str = cmd.to_string();
            thread::Builder::new()
                .name("cmd-executor".into())
                .spawn(move || {
                    #[cfg(target_os = "windows")]
                    let res = std::process::Command::new("cmd").args(["/C", &cmd_str]).output();

                    #[cfg(not(target_os = "windows"))]
                    let res = std::process::Command::new("sh").args(["-c", &cmd_str]).output();

                    if let Err(e) = res {
                        log::error!("Failed to execute command '{}': {}", cmd_str, e);
                    }
                })
                .ok();
        }

        if let Some(hotkey_or_action) = action.hotkey() {
            if self.execute_named_action(hotkey_or_action) {
                return;
            }

            if let Err(e) = self.send_hotkey(hotkey_or_action) {
                log::error!("Failed to simulate hotkey '{}': {}", hotkey_or_action, e);
            }
        }
    }

    /// Check and execute high-level semantic named actions.
    fn execute_named_action(&self, action_name: &str) -> bool {
        let normalized = action_name.trim().to_lowercase();
        match normalized.as_str() {
            // Volume and audio controls
            "volume_up" | "vol_up" => {
                let _ = self.send_single_key(Key::VolumeUp);
                true
            }
            "volume_down" | "vol_down" => {
                let _ = self.send_single_key(Key::VolumeDown);
                true
            }
            "volume_mute" | "mute" => {
                let _ = self.send_single_key(Key::VolumeMute);
                true
            }

            // Media playback controls
            "media_play_pause" | "play_pause" => {
                let _ = self.send_single_key(Key::MediaPlayPause);
                true
            }
            "media_next" | "next_track" => {
                let _ = self.send_single_key(Key::MediaNextTrack);
                true
            }
            "media_prev" | "prev_track" => {
                let _ = self.send_single_key(Key::MediaPrevTrack);
                true
            }

            // Window and desktop management
            "task_view" | "overview" => {
                let _ = self.send_hotkey("Super+Tab");
                true
            }
            "show_desktop" => {
                let _ = self.send_hotkey("Super+D");
                true
            }
            "app_launcher" | "start_menu" => {
                let _ = self.send_hotkey("Super");
                true
            }
            "snap_left" => {
                let _ = self.send_hotkey("Super+Left");
                true
            }
            "snap_right" => {
                let _ = self.send_hotkey("Super+Right");
                true
            }
            "maximize" => {
                let _ = self.send_hotkey("Super+Up");
                true
            }
            "minimize" => {
                let _ = self.send_hotkey("Super+Down");
                true
            }
            "desktop_left" => {
                let _ = self.send_hotkey("Ctrl+Super+Left");
                true
            }
            "desktop_right" => {
                let _ = self.send_hotkey("Ctrl+Super+Right");
                true
            }
            "close_window" => {
                #[cfg(target_os = "windows")]
                let _ = self.send_hotkey("Alt+F4");
                #[cfg(not(target_os = "windows"))]
                let _ = self.send_hotkey("Ctrl+Q");
                true
            }

            // Browser navigation
            "browser_back" => {
                let _ = self.send_hotkey("Alt+Left");
                true
            }
            "browser_forward" => {
                let _ = self.send_hotkey("Alt+Right");
                true
            }
            "browser_refresh" => {
                let _ = self.send_hotkey("Ctrl+R");
                true
            }
            "browser_new_tab" => {
                let _ = self.send_hotkey("Ctrl+T");
                true
            }
            "browser_close_tab" => {
                let _ = self.send_hotkey("Ctrl+W");
                true
            }
            "browser_reopen_tab" => {
                let _ = self.send_hotkey("Ctrl+Shift+T");
                true
            }
            "zoom_in" => {
                let _ = self.send_hotkey("Ctrl+Plus");
                true
            }
            "zoom_out" => {
                let _ = self.send_hotkey("Ctrl+Minus");
                true
            }
            "zoom_reset" => {
                let _ = self.send_hotkey("Ctrl+0");
                true
            }

            _ => false,
        }
    }

    fn send_single_key(&self, key: Key) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut enigo = self.enigo.lock().map_err(|_| "Enigo lock poisoned")?;
        enigo.key(key, Direction::Click).map_err(|e| format!("{:?}", e))?;
        Ok(())
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
                "super" | "win" | "meta" | "windows" | "cmd" => modifiers.push(Key::Meta),
                _ => {
                    let key = parse_single_key(token)?;
                    base_keys.push(key);
                }
            }
        }

        let mut enigo = self.enigo.lock().map_err(|_| "Enigo lock poisoned")?;

        for &m in &modifiers {
            enigo.key(m, Direction::Press).map_err(|e| format!("{:?}", e))?;
        }

        if !modifiers.is_empty() {
            thread::sleep(Duration::from_millis(15));
        }

        for &k in &base_keys {
            enigo.key(k, Direction::Click).map_err(|e| format!("{:?}", e))?;
            thread::sleep(Duration::from_millis(10));
        }

        if !modifiers.is_empty() {
            thread::sleep(Duration::from_millis(15));
        }

        for &m in modifiers.iter().rev() {
            enigo.key(m, Direction::Release).map_err(|e| format!("{:?}", e))?;
        }

        log::info!("Executed hotkey action: {}", hotkey);
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
        "volumeup" | "vol_up" => Key::VolumeUp,
        "volumedown" | "vol_down" => Key::VolumeDown,
        "volumemute" | "mute" => Key::VolumeMute,
        "mediaplaypause" | "playpause" => Key::MediaPlayPause,
        "medianexttrack" | "nexttrack" | "medianext" => Key::MediaNextTrack,
        "mediaprevtrack" | "prevtrack" | "mediaprev" => Key::MediaPrevTrack,
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
                    return Err(format!("Unrecognized key '{}'", token).into());
                }
            } else {
                return Err("Empty key token".into());
            }
        }
    };
    Ok(key)
}
