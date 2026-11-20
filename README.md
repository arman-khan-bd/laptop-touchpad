# Laptop Touchpad Gesture Manager & Remapper

A lightweight, cross-platform laptop touchpad gesture manager and keystroke remapper written in Rust. It intercepts multi-touch gestures (directional swipes, pinches, two-finger twist rotation, and taps) and maps them to customizable keyboard shortcuts, system media controls, window management actions, or shell commands.

## Architecture

```
                 +--------------------------------------+
                 |      Physical Laptop Touchpad        |
                 +-------------------+------------------+
                                     |
              +----------------------+----------------------+
              |                                             |
   [Linux Backend: libinput/udev]             [Windows Backend: Win32 Raw Input]
   - Reads /dev/input/event*                  - Intercepts HID Page 0x0D / Usage 0x05
   - Independent of X11/Wayland               - Direct Precision Touchpad reports
              |                                             |
              +----------------------+----------------------+
                                     |
                                     v
                       +---------------------------+
                       |   Gesture State Machine   |
                       |  - Centroid displacement  |
                       |  - Velocity thresholds    |
                       |  - Twist rotation angle   |
                       |  - Pairwise pinch spreads |
                       +-------------+-------------+
                                     |
                                     v
                       +---------------------------+
                       |      Event Dispatcher     |
                       |  - Cooldown & Debounce    |
                       |  - Active/Paused state    |
                       +-------------+-------------+
                                     |
              +----------------------+----------------------+
              |                                             |
              v                                             v
     [Action Executor]                             [System Tray & UI]
     - Built-in system actions                     - Tray icon & menu
     - Simulated hotkeys (Enigo)                   - 2-finger swipe toggle
     - Background shell scripts                    - Hot-reload config watcher
```

## Quick Start & Setup Instructions

### Linux Permissions (libinput / udev)

On Linux, `gesture-manager` accesses raw event nodes (`/dev/input/event*`) via `libinput` and `udev`, bypassing compositor lock-in (compatible with X11, GNOME Wayland, KDE Plasma Wayland, Hyprland, Sway, etc.).

1. **Install udev rules:**
   ```bash
   sudo cp assets/99-touchpad-gestures.rules /etc/udev/rules.d/
   sudo udevadm control --reload-rules && sudo udevadm trigger
   ```

2. **Add user to the `input` group:**
   ```bash
   sudo usermod -aG input $USER
   ```
   *Log out and log back in for group membership changes to take effect.*

### Windows Requirements

* Supported on any Windows 10/11 laptop with a **Microsoft Precision Touchpad** (HID Usage Page `0x0D`, Usage `0x05`). Check in `Windows Settings > Bluetooth & devices > Touchpad`.
* No admin rights or custom drivers required.

---

## Supported Touchpad Gestures

| Gesture Category | Gesture Identifiers | Description |
|---|---|---|
| **2-Finger Gestures** | `swipe_left_2`, `swipe_right_2`<br>`swipe_up_2`, `swipe_down_2`<br>`pinch_in`, `pinch_out`<br>`rotate_cw`, `rotate_ccw`<br>`tap_2` | Navigation, horizontal/vertical swipes, pinch zoom, 2-finger twist rotation, and taps. |
| **3-Finger Gestures** | `swipe_left_3`, `swipe_right_3`<br>`swipe_up_3`, `swipe_down_3`<br>`pinch_in_3`, `pinch_out_3`<br>`tap_3`, `hold_3` | Window switching, desktop overview, desktop minimizing, tab restore, and volume mute. |
| **4-Finger Gestures** | `swipe_left_4`, `swipe_right_4`<br>`swipe_up_4`, `swipe_down_4`<br>`pinch_in_4`, `pinch_out_4`<br>`tap_4`, `hold_4` | Virtual workspace navigation, application launcher, media play/pause, and notification center. |
| **Edge Swipes** | `edge_swipe_left`, `edge_swipe_right`<br>`edge_swipe_up`, `edge_swipe_down` | Inward swipes originating from the touchpad borders. |

---

## Built-In Action Mappings

In `config.toml`, you can assign either high-level semantic action names, custom hotkey strings, or shell commands:

### 1. Semantic Named Actions
* **Media & Volume:** `volume_up`, `volume_down`, `volume_mute`, `media_play_pause`, `media_next`, `media_prev`
* **Desktop & Windows:** `task_view`, `show_desktop`, `app_launcher`, `snap_left`, `snap_right`, `maximize`, `minimize`, `close_window`, `desktop_left`, `desktop_right`
* **Browser & Navigation:** `browser_back`, `browser_forward`, `browser_refresh`, `browser_new_tab`, `browser_close_tab`, `browser_reopen_tab`, `zoom_in`, `zoom_out`, `zoom_reset`

### 2. Custom Hotkey Combinations
* Modifiers: `Super` (or `Win`), `Ctrl`, `Alt`, `Shift`
* Keys: `Tab`, `Esc`, `Space`, `Left`, `Right`, `Up`, `Down`, `Return`, `Backspace`, `F1`..`F12`, `Plus`, `Minus`
* Examples: `"Super+Tab"`, `"Alt+Left"`, `"Ctrl+Shift+T"`, `"Ctrl+Plus"`

### 3. Shell Commands
* Prefixed with `cmd:` (Windows) or `sh:` (Linux)
* Examples: `"cmd: notepad.exe"` or `"sh: notify-send 'Gesture' '3-finger swipe'"`

---

## Configuration & Hot-Reloading

Default configuration file paths:
* **Linux:** `~/.config/gesture-manager/config.toml`
* **Windows:** `%APPDATA%\gesture-manager\config.toml`

Any modifications made to `config.toml` are immediately hot-reloaded without needing to restart the daemon.

```toml
[sensitivity]
swipe_threshold = 45.0
min_swipe_velocity = 0.10
pinch_threshold = 0.20
rotation_threshold_deg = 24.0
tap_max_duration_ms = 280
hold_min_duration_ms = 650
tap_max_displacement = 18.0
two_finger_swipes_enabled = true
invert_swipe_x = false
invert_swipe_y = false

gesture_cooldown_ms = 300

[gestures]
# 2-finger gestures
swipe_left_2 = "browser_back"
swipe_right_2 = "browser_forward"
pinch_in = "zoom_out"
pinch_out = "zoom_in"
rotate_cw = "volume_up"
rotate_ccw = "volume_down"

# 3-finger gestures
swipe_left_3 = "Alt+Left"
swipe_right_3 = "Alt+Right"
swipe_up_3 = "task_view"
swipe_down_3 = "show_desktop"
pinch_in_3 = "show_desktop"
pinch_out_3 = "task_view"
tap_3 = "browser_reopen_tab"
hold_3 = "volume_mute"

# 4-finger gestures
swipe_left_4 = "desktop_left"
swipe_right_4 = "desktop_right"
swipe_up_4 = "app_launcher"
swipe_down_4 = "minimize"
tap_4 = "Super+A"
hold_4 = "media_play_pause"
```

---

## Advanced GUI Configurator & Live Test Sandbox

Launch the advanced GUI control panel anytime:
```bash
gesture-manager --gui
```
Or right-click the system tray icon and select **"Open GUI Configurator"**.

### GUI Features:
* **Interactive Trackpad Sandbox:** Visualize multi-finger contacts, pinch deltas, and live swipe vectors directly on screen in real time.
* **Sensitivity Sliders:** Adjust swipe distance, velocity, pinch thresholds, and rotation sensitivity dynamically.
* **Point-and-Click Remapper:** Easily map 2, 3, and 4-finger swipes, pinches, rotation, and taps to any shortcut or system command.
* **Step-by-Step Setup Wizard:** Built-in walkthrough guiding users through OS permissions, touchpad checks, and autostart registration.

---

## Windows Step-by-Step Setup Installer

For Windows users, an automated NSIS installer (`gesture-manager-setup.exe`) is generated in GitHub Actions releases:
* **Welcome Screen** & license overview.
* **Components Selection** (Daemon executable, Desktop shortcuts, Start menu items).
* **Install Directory Selection** (`%LOCALAPPDATA%\GestureManager`).
* **Windows Login Autostart** setup.
* **1-Click Launch Finish Screen** with automatic GUI wizard popup.
* **Clean Uninstaller** registered with Windows *Settings > Apps & Features*.

---

## CLI Commands

```bash
# Launch the interactive GUI configurator & touchpad test sandbox
gesture-manager --gui

# Run step-by-step installation / autostart setup
gesture-manager --install

# Display help and usage options
gesture-manager --help

# List all recognized touchpad gestures
gesture-manager --list-gestures

# List all available built-in actions and key syntax
gesture-manager --list-actions

# Validate configuration file syntax
gesture-manager --check-config

# Run without system tray icon (headless/systemd mode)
gesture-manager --no-tray
```
