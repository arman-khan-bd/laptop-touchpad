//! Configuration schema, default bindings, persistence, and filesystem watcher.

use crate::gesture::{Gesture, GestureSensitivity};
use notify::{Event, EventKind, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::channel;
use std::thread;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ActionConfig {
    Simple(String),
    Detailed {
        #[serde(default)]
        hotkey: Option<String>,
        #[serde(default)]
        command: Option<String>,
    },
}

impl ActionConfig {
    pub fn hotkey(&self) -> Option<&str> {
        match self {
            ActionConfig::Simple(s) => {
                let lower = s.to_lowercase();
                if lower.starts_with("sh:") || lower.starts_with("cmd:") || lower.starts_with("exec:") {
                    None
                } else {
                    Some(s.as_str())
                }
            }
            ActionConfig::Detailed { hotkey, .. } => hotkey.as_deref(),
        }
    }

    pub fn command(&self) -> Option<&str> {
        match self {
            ActionConfig::Simple(s) => {
                let trimmed = s.trim();
                if let Some(rest) = trimmed.strip_prefix("sh:") {
                    Some(rest.trim())
                } else if let Some(rest) = trimmed.strip_prefix("cmd:") {
                    Some(rest.trim())
                } else if let Some(rest) = trimmed.strip_prefix("exec:") {
                    Some(rest.trim())
                } else {
                    None
                }
            }
            ActionConfig::Detailed { command, .. } => command.as_deref(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub sensitivity: GestureSensitivity,

    #[serde(default = "default_cooldown_ms")]
    pub gesture_cooldown_ms: u64,

    #[serde(default = "default_gestures")]
    pub gestures: HashMap<String, ActionConfig>,
}

fn default_cooldown_ms() -> u64 {
    300
}

fn default_gestures() -> HashMap<String, ActionConfig> {
    let mut m = HashMap::new();

    // 2-finger gestures
    m.insert("swipe_left_2".into(), ActionConfig::Simple("browser_back".into()));
    m.insert("swipe_right_2".into(), ActionConfig::Simple("browser_forward".into()));
    m.insert("pinch_in".into(), ActionConfig::Simple("zoom_out".into()));
    m.insert("pinch_out".into(), ActionConfig::Simple("zoom_in".into()));
    m.insert("rotate_cw".into(), ActionConfig::Simple("volume_up".into()));
    m.insert("rotate_ccw".into(), ActionConfig::Simple("volume_down".into()));

    // 3-finger navigation and window control
    m.insert("swipe_left_3".into(), ActionConfig::Simple("Alt+Left".into()));
    m.insert("swipe_right_3".into(), ActionConfig::Simple("Alt+Right".into()));
    m.insert("swipe_up_3".into(), ActionConfig::Simple("task_view".into()));
    m.insert("swipe_down_3".into(), ActionConfig::Simple("show_desktop".into()));
    m.insert("pinch_in_3".into(), ActionConfig::Simple("show_desktop".into()));
    m.insert("pinch_out_3".into(), ActionConfig::Simple("task_view".into()));
    m.insert("tap_3".into(), ActionConfig::Simple("browser_reopen_tab".into()));
    m.insert("hold_3".into(), ActionConfig::Simple("volume_mute".into()));

    // 4-finger virtual desktops and global actions
    m.insert("swipe_left_4".into(), ActionConfig::Simple("desktop_left".into()));
    m.insert("swipe_right_4".into(), ActionConfig::Simple("desktop_right".into()));
    m.insert("swipe_up_4".into(), ActionConfig::Simple("app_launcher".into()));
    m.insert("swipe_down_4".into(), ActionConfig::Simple("minimize".into()));
    m.insert("tap_4".into(), ActionConfig::Simple("Super+A".into()));
    m.insert("hold_4".into(), ActionConfig::Simple("media_play_pause".into()));

    m
}

impl Default for Config {
    fn default() -> Self {
        Self {
            sensitivity: GestureSensitivity::default(),
            gesture_cooldown_ms: default_cooldown_ms(),
            gestures: default_gestures(),
        }
    }
}

impl Config {
    pub fn get_action(&self, gesture: &Gesture) -> Option<&ActionConfig> {
        let key = gesture.to_string();
        self.gestures.get(&key)
    }

    pub fn load_or_create(path: &Path) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        if path.exists() {
            let content = fs::read_to_string(path)?;
            let cfg: Config = toml::from_str(&content)?;
            log::info!("Loaded configuration from {}", path.display());
            Ok(cfg)
        } else {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            let default_cfg = Config::default();
            let toml_str = toml::to_string_pretty(&default_cfg)?;
            let header = "# Touchpad Gesture Manager & Remapper Configuration\n\
                          # Supported actions: hotkeys (e.g., 'Super+Tab', 'Alt+Left'),\n\
                          # named actions ('task_view', 'show_desktop', 'volume_up', 'media_play_pause', etc.),\n\
                          # or shell commands ('cmd: <command>' or 'sh: <command>').\n\n";
            fs::write(path, format!("{}{}", header, toml_str))?;
            log::info!("Created default configuration at {}", path.display());
            Ok(default_cfg)
        }
    }

    pub fn default_path() -> PathBuf {
        if let Some(mut base) = dirs::config_dir() {
            base.push("gesture-manager");
            base.push("config.toml");
            base
        } else {
            PathBuf::from("config.toml")
        }
    }

    pub fn spawn_watcher(
        config_path: PathBuf,
        tx: crossbeam_channel::Sender<Config>,
    ) -> Result<thread::JoinHandle<()>, Box<dyn std::error::Error + Send + Sync>> {
        let watch_dir = config_path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));

        let handle = thread::Builder::new()
            .name("config-watcher".into())
            .spawn(move || {
                let (notify_tx, notify_rx) = channel();
                let mut watcher = match notify::recommended_watcher(notify_tx) {
                    Ok(w) => w,
                    Err(e) => {
                        log::error!("Failed to initialize config file watcher: {}", e);
                        return;
                    }
                };

                if let Err(e) = watcher.watch(&watch_dir, RecursiveMode::NonRecursive) {
                    log::warn!("Could not watch directory {}: {}", watch_dir.display(), e);
                    return;
                }

                let canonical_target = config_path.file_name().map(|n| n.to_os_string());

                while let Ok(event_res) = notify_rx.recv() {
                    if let Ok(Event { kind, paths, .. }) = event_res {
                        if matches!(kind, EventKind::Modify(_) | EventKind::Create(_)) {
                            let matches_target = paths.iter().any(|p| {
                                p.file_name().map(|n| n.to_os_string()) == canonical_target
                            });

                            if matches_target {
                                thread::sleep(Duration::from_millis(50));
                                if let Ok(content) = fs::read_to_string(&config_path) {
                                    if let Ok(reloaded) = toml::from_str::<Config>(&content) {
                                        log::info!("Hot-reloaded configuration from {}", config_path.display());
                                        let _ = tx.send(reloaded);
                                    }
                                }
                            }
                        }
                    }
                }
            })?;

        Ok(handle)
    }
}
