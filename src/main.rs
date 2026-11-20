//! Laptop Touchpad Gesture Manager & Remapper
//! Main entry point, system tray daemon, and event dispatcher.

mod backend;
mod config;
mod executor;
mod gesture;
mod gui;

use backend::create_backend;
use config::Config;
use executor::ActionExecutor;
use gesture::Gesture;
use gui::GuiServer;
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
        --gui              Launch the Advanced Graphical User Interface in browser
        --install          Run the Step-by-Step Installation Wizard
    -c, --config <PATH>    Path to custom configuration file
        --no-tray          Run in headless mode without system tray icon
        --check-config     Validate the configuration file syntax and exit
        --list-gestures    List all recognized gesture identifiers and exit
        --list-actions     List all built-in named actions and exit
    -h, --help             Print help information
    -v, --version          Print version information

CONFIGURATION:
    Default location: ~/.config/gesture-manager/config.toml (Linux)
                      %APPDATA%\gesture-manager\config.toml (Windows)
    GUI Web Interface: http://127.0.0.1:17890/
"#,
        version = env!("CARGO_PKG_VERSION")
    );
}

fn print_gestures() {
    println!(
        r#"Available Touchpad Gestures:
  2-Finger Gestures:
    - swipe_left_2          (2-finger swipe left)
    - swipe_right_2         (2-finger swipe right)
    - swipe_up_2            (2-finger swipe up)
    - swipe_down_2          (2-finger swipe down)
    - pinch_in / pinch_in_2 (2-finger pinch inwards / zoom out)
    - pinch_out / pinch_out_2 (2-finger pinch outwards / zoom in)
    - rotate_cw             (2-finger twist clockwise)
    - rotate_ccw            (2-finger twist counter-clockwise)
    - tap_2                 (2-finger tap)

  3-Finger Gestures:
    - swipe_left_3          (3-finger swipe left)
    - swipe_right_3         (3-finger swipe right)
    - swipe_up_3            (3-finger swipe up)
    - swipe_down_3          (3-finger swipe down)
    - pinch_in_3            (3-finger pinch in)
    - pinch_out_3           (3-finger pinch out)
    - tap_3                 (3-finger tap)
    - hold_3                (3-finger tap and hold)

  4-Finger Gestures:
    - swipe_left_4          (4-finger swipe left)
    - swipe_right_4         (4-finger swipe right)
    - swipe_up_4            (4-finger swipe up)
    - swipe_down_4          (4-finger swipe down)
    - pinch_in_4            (4-finger pinch in)
    - pinch_out_4           (4-finger pinch out)
    - tap_4                 (4-finger tap)
    - hold_4                (4-finger tap and hold)

  Edge Swipes:
    - edge_swipe_left       (Swipe from right edge inwards)
    - edge_swipe_right      (Swipe from left edge inwards)
    - edge_swipe_up         (Swipe from bottom edge upwards)
    - edge_swipe_down       (Swipe from top edge downwards)
"#
    );
}

fn print_actions() {
    println!(
        r#"Available Action Mappings:
  High-Level Semantic Named Actions:
    - volume_up, volume_down, volume_mute
    - media_play_pause, media_next, media_prev
    - task_view, show_desktop, app_launcher
    - snap_left, snap_right, maximize, minimize, close_window
    - desktop_left, desktop_right
    - browser_back, browser_forward, browser_refresh, browser_new_tab, browser_close_tab, browser_reopen_tab
    - zoom_in, zoom_out, zoom_reset

  Custom Hotkey Combinations:
    - Modifier keys: Super (or Win), Ctrl, Alt, Shift
    - Key tokens: Tab, Esc, Space, Left, Right, Up, Down, Return, Backspace, Plus, Minus, F1..F12
    - Examples: "Super+Tab", "Alt+Left", "Ctrl+Shift+T", "Ctrl+Plus"

  Shell Commands:
    - Prefixed with cmd: or sh:
    - Examples: "cmd: notepad.exe" or "sh: notify-send 'Gesture' 'Triggered'"
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
                rgba.extend_from_slice(&[59, 130, 246, 255]);
            } else if is_outer_border || is_button_split || is_divider {
                rgba.extend_from_slice(&[203, 213, 225, 255]);
            } else if x > 3 && x < 28 && y > 6 && y < 25 {
                rgba.extend_from_slice(&[30, 41, 59, 240]);
            } else {
                rgba.extend_from_slice(&[0, 0, 0, 0]);
            }
        }
    }

    Icon::from_rgba(rgba, width, height).expect("Failed to create procedural tray icon")
}

fn open_in_system_editor(path: &std::path::Path) {
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("explorer").arg(path).spawn();
    }
    #[cfg(target_os = "linux")]
    {
        let _ = std::process::Command::new("xdg-open").arg(path).spawn();
    }
}

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let args: Vec<String> = env::args().collect();
    let mut config_path = Config::default_path();
    let mut headless = false;
    let mut open_gui_on_launch = false;

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
            "--list-actions" => {
                print_actions();
                return Ok(());
            }
            "--gui" => {
                open_gui_on_launch = true;
            }
            "--install" => {
                open_gui_on_launch = true;
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

    log::info!("Starting Laptop Touchpad Gesture Manager v{}...", env!("CARGO_PKG_VERSION"));

    let mut config = Config::load_or_create(&config_path)?;
    let executor = ActionExecutor::new()?;

    // Start background GUI configurator web server
    let gui_server = GuiServer::new(config_path.clone());
    if let Err(e) = gui_server.start() {
        log::warn!("Could not bind GUI server: {}. Continuing without local GUI server.", e);
    } else if open_gui_on_launch {
        GuiServer::open_browser();
    }

    let (config_tx, config_rx) = crossbeam_channel::unbounded::<Config>();
    let _watcher_handle = Config::spawn_watcher(config_path.clone(), config_tx)?;

    let (gesture_tx, gesture_rx) = crossbeam_channel::unbounded::<Gesture>();
    let mut backend = create_backend();
    backend.start(gesture_tx)?;

    let is_paused = Arc::new(AtomicBool::new(false));
    let mut _tray_icon: Option<TrayIcon> = None;
    let open_gui_id: MenuId;
    let reload_item_id: MenuId;
    let pause_item_id: MenuId;
    let open_config_id: MenuId;
    let toggle_two_finger_id: MenuId;
    let quit_item_id: MenuId;

    if !headless {
        let menu = Menu::new();

        let title_item = MenuItem::new("Touchpad Gestures", false, None);
        let open_gui_item = MenuItem::new("⚙ Open Gesture Configurator GUI", true, None);
        let reload_item = MenuItem::new("Reload Configuration", true, None);
        let pause_item = MenuItem::new("Pause Gestures", true, None);
        let toggle_two_finger_item = MenuItem::new(
            if config.sensitivity.two_finger_swipes_enabled {
                "2-Finger Swipes: Enabled"
            } else {
                "2-Finger Swipes: Disabled"
            },
            true,
            None,
        );
        let open_config_item = MenuItem::new("Open Config File (TOML)", true, None);
        let quit_item = MenuItem::new("Quit", true, None);

        open_gui_id = open_gui_item.id().clone();
        reload_item_id = reload_item.id().clone();
        pause_item_id = pause_item.id().clone();
        toggle_two_finger_id = toggle_two_finger_item.id().clone();
        open_config_id = open_config_item.id().clone();
        quit_item_id = quit_item.id().clone();

        let _ = menu.append(&title_item);
        let _ = menu.append(&PredefinedMenuItem::separator());
        let _ = menu.append(&open_gui_item);
        let _ = menu.append(&PredefinedMenuItem::separator());
        let _ = menu.append(&pause_item);
        let _ = menu.append(&toggle_two_finger_item);
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
                log::warn!("System tray icon could not be created: {}. Continuing in headless mode.", e);
            }
        }
    } else {
        open_gui_id = MenuId::new("open_gui");
        reload_item_id = MenuId::new("reload");
        pause_item_id = MenuId::new("pause");
        toggle_two_finger_id = MenuId::new("two_finger");
        open_config_id = MenuId::new("open_config");
        quit_item_id = MenuId::new("quit");
    }

    let menu_channel = MenuEvent::receiver();
    let mut last_trigger = Instant::now() - Duration::from_secs(10);
    let mut running = true;

    while running {
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

        while let Ok(event) = menu_channel.try_recv() {
            if event.id == quit_item_id {
                log::info!("Quit requested via tray menu. Terminating...");
                running = false;
                break;
            } else if event.id == open_gui_id {
                log::info!("Opening Gesture Configurator GUI in web browser...");
                GuiServer::open_browser();
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
                    if let Some(tray) = &_tray_icon {
                        let _ = tray.set_tooltip(Some("Touchpad Gesture Manager (Paused)"));
                    }
                    log::info!("Gestures paused.");
                } else {
                    if let Some(tray) = &_tray_icon {
                        let _ = tray.set_tooltip(Some("Touchpad Gesture Manager (Active)"));
                    }
                    log::info!("Gestures resumed.");
                }
            } else if event.id == toggle_two_finger_id {
                config.sensitivity.two_finger_swipes_enabled = !config.sensitivity.two_finger_swipes_enabled;
                log::info!(
                    "Two-finger swipes enabled: {}",
                    config.sensitivity.two_finger_swipes_enabled
                );
            } else if event.id == open_config_id {
                open_in_system_editor(&config_path);
            }
        }

        if !running {
            break;
        }

        while let Ok(reloaded) = config_rx.try_recv() {
            config = reloaded;
            log::info!("Applied hot-reloaded configuration ({} bindings).", config.gestures.len());
        }

        while let Ok(gesture) = gesture_rx.try_recv() {
            if is_paused.load(Ordering::SeqCst) {
                continue;
            }

            let now = Instant::now();
            let elapsed_ms = now.duration_since(last_trigger).as_millis() as u64;
            if elapsed_ms < config.gesture_cooldown_ms {
                continue;
            }

            if let Some(action) = config.get_action(&gesture) {
                log::info!("Triggered gesture: {} -> executing action", gesture);
                executor.execute(action);
                last_trigger = now;
            }
        }

        std::thread::sleep(Duration::from_millis(15));
    }

    backend.stop()?;
    log::info!("Gesture Manager daemon stopped cleanly.");
    Ok(())
}
