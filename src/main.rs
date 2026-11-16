//! Laptop Touchpad Gesture Manager & Remapper
//! Main entry point, system tray daemon, and event dispatcher.

mod backend;
mod config;
mod executor;
mod gesture;

use backend::create_backend;
use config::Config;
use executor::ActionExecutor;
use gesture::Gesture;
use std::env;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tray_icon::menu::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

fn print_help() {
    println!(
        r#"Laptop Touchpad Gesture Manager & Remapper v{version}

USAGE:
    gesture-manager [OPTIONS]

OPTIONS:
    -c, --config <PATH>    Path to custom configuration file
        --no-tray          Run in headless mode without system tray icon
        --check-config     Validate the configuration file syntax and exit
        --list-gestures    List all recognized gesture identifiers and exit
    -h, --help             Print help information
    -v, --version          Print version information

CONFIGURATION:
    Default location: ~/.config/gesture-manager/config.toml (Linux)
                      %APPDATA%\gesture-manager\config.toml (Windows)
"#,
        version = env!("CARGO_PKG_VERSION")
    );
}

fn print_gestures() {
    println!(
        r#"Available Touchpad Gestures:
  - swipe_left_3     (3-finger swipe left)
  - swipe_right_3    (3-finger swipe right)
  - swipe_up_3       (3-finger swipe up)
  - swipe_down_3     (3-finger swipe down)
  - swipe_left_4     (4-finger swipe left)
  - swipe_right_4    (4-finger swipe right)
  - swipe_up_4       (4-finger swipe up)
  - swipe_down_4     (4-finger swipe down)
  - pinch_in         (2-finger pinch inwards / zoom out)
  - pinch_out        (2-finger pinch outwards / zoom in)
  - tap_2            (2-finger tap)
  - tap_3            (3-finger tap)
  - tap_4            (4-finger tap)

Action Mappings:
  - Hotkeys: e.g. "Super+Tab", "Alt+Left", "Ctrl+Shift+T", "Ctrl+Plus"
  - Commands: e.g. "cmd: notify-send 'Gesture' 'Activated'" or "sh: ./my-script.sh"
"#
    );
}

fn create_default_tray_icon() -> Icon {
    let width = 32u32;
    let height = 32u32;
    let mut rgba = Vec::with_capacity((width * height * 4) as usize);

    for y in 0..height {
        for x in 0..width {
            let is_outer_border = (x == 3 || x == 28) && (y >= 6 && y <= 25)
                || (y == 6 || y == 25) && (x >= 3 && x <= 28);
            let is_button_split = y == 21 && (x >= 4 && x <= 27);
            let is_divider = y >= 21 && y <= 25 && x == 16;

            let dx = x as i32 - 16;
            let dy = y as i32 - 13;
            let is_contact = (dx * dx + dy * dy) <= 8;

            if is_contact {
                rgba.extend_from_slice(&[59, 130, 246, 255]); // Azure Blue
            } else if is_outer_border || is_button_split || is_divider {
                rgba.extend_from_slice(&[203, 213, 225, 255]); // Slate 300
            } else if x > 3 && x < 28 && y > 6 && y < 25 {
                rgba.extend_from_slice(&[30, 41, 59, 240]); // Slate 800
            } else {
                rgba.extend_from_slice(&[0, 0, 0, 0]); // Transparent
            }
        }
    }

    Icon::from_rgba(rgba, width, height).expect("Failed to create procedural tray icon")
}

fn open_in_system_editor(path: &std::path::Path) {
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("explorer")
            .arg(path)
            .spawn();
    }
    #[cfg(target_os = "linux")]
    {
        let _ = std::process::Command::new("xdg-open")
            .arg(path)
            .spawn();
    }
}

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let args: Vec<String> = env::args().collect();
    let mut config_path = Config::default_path();
    let mut headless = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-h" | "--help" => {
                print_help();
                return Ok(());
            }
            "-v" | "--version" => {
                println!("gesture-manager {}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            "--list-gestures" => {
                print_gestures();
                return Ok(());
            }
            "-c" | "--config" => {
                if i + 1 < args.len() {
                    config_path = PathBuf::from(&args[i + 1]);
                    i += 1;
                } else {
                    eprintln!("Error: --config requires a path argument.");
                    std::process::exit(1);
                }
            }
            "--check-config" => {
                let cfg = Config::load_or_create(&config_path)?;
                println!(
                    "Configuration at {} is VALID. Loaded {} gesture bindings.",
                    config_path.display(),
                    cfg.gestures.len()
                );
                return Ok(());
            }
            "--no-tray" => {
                headless = true;
            }
            unknown => {
                eprintln!("Unknown argument '{}'. Use --help for usage.", unknown);
                std::process::exit(1);
            }
        }
        i += 1;
    }

    log::info!(
        "Starting Laptop Touchpad Gesture Manager v{}...",
        env!("CARGO_PKG_VERSION")
    );

    // Load initial configuration
    let mut config = Config::load_or_create(&config_path)?;
    log::info!(
        "Loaded {} gesture mappings from {}",
        config.gestures.len(),
        config_path.display()
    );

    // Initialize Action Executor
    let executor = ActionExecutor::new()?;

    // Set up hot-reload config watcher
    let (config_tx, config_rx) = crossbeam_channel::unbounded::<Config>();
    let _watcher_handle = Config::spawn_watcher(config_path.clone(), config_tx)?;

    // Set up cross-platform gesture backend
    let (gesture_tx, gesture_rx) = crossbeam_channel::unbounded::<Gesture>();
    let mut backend = create_backend();
    backend.start(gesture_tx)?;

    // Set up System Tray (unless in headless mode)
    let is_paused = Arc::new(AtomicBool::new(false));
    let mut _tray_icon: Option<TrayIcon> = None;
    let reload_item_id: MenuId;
    let pause_item_id: MenuId;
    let open_config_id: MenuId;
    let quit_item_id: MenuId;

    if !headless {
        let menu = Menu::new();

        let title_item = MenuItem::new("Touchpad Gestures", false, None);
        let reload_item = MenuItem::new("Reload Configuration", true, None);
        let pause_item = MenuItem::new("Pause Gestures", true, None);
        let open_config_item = MenuItem::new("Open Config File", true, None);
        let quit_item = MenuItem::new("Quit", true, None);

        reload_item_id = reload_item.id().clone();
        pause_item_id = pause_item.id().clone();
        open_config_id = open_config_item.id().clone();
        quit_item_id = quit_item.id().clone();

        let _ = menu.append(&title_item);
        let _ = menu.append(&PredefinedMenuItem::separator());
        let _ = menu.append(&pause_item);
        let _ = menu.append(&reload_item);
        let _ = menu.append(&open_config_item);
        let _ = menu.append(&PredefinedMenuItem::separator());
        let _ = menu.append(&quit_item);

        let icon = create_default_tray_icon();
        match TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("Touchpad Gesture Manager (Active)")
            .with_icon(icon)
            .build()
        {
            Ok(tray) => {
                _tray_icon = Some(tray);
                log::info!("System tray icon initialized.");
            }
            Err(e) => {
                log::warn!("System tray icon could not be created (headless environment?): {}. Continuing in headless mode.", e);
            }
        }
    } else {
        reload_item_id = MenuId::new("reload");
        pause_item_id = MenuId::new("pause");
        open_config_id = MenuId::new("open_config");
        quit_item_id = MenuId::new("quit");
    }

    let menu_channel = MenuEvent::receiver();
    let mut last_trigger = Instant::now() - Duration::from_secs(10);
    let mut running = true;

    log::info!("Gesture Manager daemon is active and listening for multi-touch gestures...");

    while running {
        // 1. Process Windows message loop events (required for tray icon on Windows)
        #[cfg(target_os = "windows")]
        unsafe {
            use windows::Win32::Foundation::HWND;
            use windows::Win32::UI::WindowsAndMessaging::{
                DispatchMessageW, PeekMessageW, TranslateMessage, MSG, PM_REMOVE,
            };
            let mut msg = MSG::default();
            while PeekMessageW(&mut msg, HWND(std::ptr::null_mut()), 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }

        // 2. Check for menu events from system tray
        while let Ok(event) = menu_channel.try_recv() {
            if event.id == quit_item_id {
                log::info!("Quit requested via tray menu. Terminating...");
                running = false;
                break;
            } else if event.id == reload_item_id {
                match Config::load_or_create(&config_path) {
                    Ok(reloaded) => {
                        config = reloaded;
                        log::info!("Configuration manually reloaded ({} bindings).", config.gestures.len());
                    }
                    Err(e) => log::error!("Failed to reload config: {}", e),
                }
            } else if event.id == pause_item_id {
                let currently_paused = is_paused.load(Ordering::SeqCst);
                let new_state = !currently_paused;
                is_paused.store(new_state, Ordering::SeqCst);
                if new_state {
                    log::info!("Gestures paused by user.");
                    if let Some(tray) = &_tray_icon {
                        let _ = tray.set_tooltip(Some("Touchpad Gesture Manager (Paused)"));
                    }
                } else {
                    log::info!("Gestures resumed by user.");
                    if let Some(tray) = &_tray_icon {
                        let _ = tray.set_tooltip(Some("Touchpad Gesture Manager (Active)"));
                    }
                }
            } else if event.id == open_config_id {
                log::info!("Opening config file in default editor: {}", config_path.display());
                open_in_system_editor(&config_path);
            }
        }

        if !running {
            break;
        }

        // 3. Check for hot-reloaded configuration from watcher
        while let Ok(reloaded) = config_rx.try_recv() {
            config = reloaded;
            log::info!("Applied hot-reloaded configuration ({} bindings).", config.gestures.len());
        }

        // 4. Process incoming gestures from input backend
        while let Ok(gesture) = gesture_rx.try_recv() {
            if is_paused.load(Ordering::SeqCst) {
                log::debug!("Gesture '{}' ignored because daemon is paused.", gesture);
                continue;
            }

            let now = Instant::now();
            let elapsed_ms = now.duration_since(last_trigger).as_millis() as u64;
            if elapsed_ms < config.gesture_cooldown_ms {
                log::debug!(
                    "Gesture '{}' throttled by debounce ({}ms < {}ms).",
                    gesture,
                    elapsed_ms,
                    config.gesture_cooldown_ms
                );
                continue;
            }

            if let Some(action) = config.get_action(&gesture) {
                log::info!("Triggered gesture: {} -> executing action", gesture);
                executor.execute(action);
                last_trigger = now;
            } else {
                log::debug!("Gesture '{}' triggered but has no action mapped in config.", gesture);
            }
        }

        // Yield CPU and prevent busy-spinning
        std::thread::sleep(Duration::from_millis(15));
    }

    // Clean shutdown
    log::info!("Shutting down gesture capture backend...");
    backend.stop()?;
    log::info!("Gesture Manager daemon stopped cleanly.");

    Ok(())
}
