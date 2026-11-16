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

/// An action mapped to a touchpad gesture.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ActionConfig {
    /// Concise shortcut or shell command string.
    /// Prefixes like "sh:", "cmd:", "exec:" designate shell commands; otherwise treated as hotkeys.
    Simple(String),
    /// Explicit mapping with dedicated hotkey and/or shell command execution.
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
                if lower.starts_with("sh:")
                    || lower.starts_with("cmd:")
                    || lower.starts_with("exec:")
                {
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

/// Global daemon configuration settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Gesture sensitivity thresholds and tolerances.
    #[serde(default)]
    pub sensitivity: GestureSensitivity,

    /// Minimum cooldown in milliseconds between distinct gesture triggers.
    #[serde(default = "default_cooldown_ms")]
    pub gesture_cooldown_ms: u64,

    /// Touchpad gesture to action mappings.
    #[serde(default = "default_gestures")]
    pub gestures: HashMap<String, ActionConfig>,
}

fn default_cooldown_ms() -> u64 {
    350
}

fn default_gestures() -> HashMap<String, ActionConfig> {
    let mut m = HashMap::new();

    // 3-finger navigation
    m.insert("swipe_left_3".into(), ActionConfig::Simple("Alt+Left".into()));
    m.insert("swipe_right_3".into(), ActionConfig::Simple("Alt+Right".into()));
    m.insert("swipe_up_3".into(), ActionConfig::Simple("Super+Tab".into()));
    m.insert("swipe_down_3".into(), ActionConfig::Simple("Super+D".into()));

    // 4-finger virtual desktop and window control
    m.insert(
        "swipe_left_4".into(),
        ActionConfig::Simple("Ctrl+Super+Left".into()),
    );
    m.insert(
        "swipe_right_4".into(),
        ActionConfig::Simple("Ctrl+Super+Right".into()),
    );
    m.insert("swipe_up_4".into(), ActionConfig::Simple("Super".into()));
    m.insert("swipe_down_4".into(), ActionConfig::Simple("Super+M".into()));

    // Pinch zoom
    m.insert("pinch_in".into(), ActionConfig::Simple("Ctrl+Minus".into()));
    m.insert("pinch_out".into(), ActionConfig::Simple("Ctrl+Plus".into()));

    // Multi-finger taps
    m.insert("tap_3".into(), ActionConfig::Simple("Ctrl+Shift+T".into()));
    m.insert("tap_4".into(), ActionConfig::Simple("Super+A".into()));

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
    /// Resolve an action for a given triggered gesture.
    pub fn get_action(&self, gesture: &Gesture) -> Option<&ActionConfig> {
        let key = gesture.to_string();
        self.gestures.get(&key)
    }

    /// Load configuration from file, or generate default config if not found.
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
                          # Hotkeys support: Super, Ctrl, Alt, Shift, Tab, Left, Right, Up, Down, Return, Esc, etc.\n\
                          # Shell scripts can be specified as: cmd: <command> or sh: <command>\n\n";
            fs::write(path, format!("{}{}", header, toml_str))?;
            log::info!("Created default configuration at {}", path.display());
            Ok(default_cfg)
        }
    }

    /// Determine default platform configuration file path.
    pub fn default_path() -> PathBuf {
        if let Some(mut base) = dirs::config_dir() {
            base.push("gesture-manager");
            base.push("config.toml");
            base
        } else {
            PathBuf::from("config.toml")
        }
    }

    /// Spawn a filesystem watcher on `config_path` that emits reloaded configurations over a channel.
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

                log::info!("Watching configuration file changes in {}", watch_dir.display());

                let canonical_target = config_path.file_name().map(|n| n.to_os_string());

                while let Ok(event_res) = notify_rx.recv() {
                    match event_res {
                        Ok(Event { kind, paths, .. }) => {
                            let is_relevant = match kind {
                                EventKind::Modify(_) | EventKind::Create(_) => true,
                                _ => false,
                            };

                            if is_relevant {
                                let matches_target = paths.iter().any(|p| {
                                    p.file_name().map(|n| n.to_os_string()) == canonical_target
                                });

                                if matches_target {
                                    // Debounce momentary file locks during text editor writes
                                    thread::sleep(Duration::from_millis(50));
                                    match fs::read_to_string(&config_path) {
                                        Ok(content) => match toml::from_str::<Config>(&content) {
                                            Ok(reloaded) => {
                                                log::info!("Hot-reloaded configuration from {}", config_path.display());
                                                let _ = tx.send(reloaded);
                                            }
                                            Err(e) => {
                                                log::error!("Syntax error in {}: {}", config_path.display(), e);
                                            }
                                        },
                                        Err(e) => {
                                            log::warn!("Failed to read {}: {}", config_path.display(), e);
                                        }
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            log::warn!("Config watcher event error: {}", e);
                        }
                    }
                }
            })?;

        Ok(handle)
    }
}
