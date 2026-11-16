# Laptop Touchpad Gesture Manager & Remapper

A lightweight, cross-platform laptop touchpad gesture manager and keystroke remapper written in Rust. It captures multi-touch gestures (3-finger and 4-finger directional swipes, pinches, and taps) and maps them to customizable keyboard shortcuts and shell commands.

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
     - Simulated hotkeys (Enigo)                   - Tray icon & menu
     - Background shell scripts                    - Hot-reload config watcher
```

## Quick Start & Setup Instructions

### Linux Permissions (libinput / udev)

On Linux, `gesture-manager` interacts directly with kernel input event nodes (`/dev/input/event*`) via `libinput` and `udev`, bypassing desktop compositor lock-in.

1. **Install udev rules:**
   Copy `assets/99-touchpad-gestures.rules` into `/etc/udev/rules.d/`:
   ```bash
   sudo cp assets/99-touchpad-gestures.rules /etc/udev/rules.d/
   sudo udevadm control --reload-rules && sudo udevadm trigger
   ```

2. **Add your user to the `input` group:**
   ```bash
   sudo usermod -aG input $USER
   ```
   *Log out and log back in for group membership changes to take effect.*

### Windows Requirements

On Windows, the daemon uses the Win32 Raw Input API targeting **Precision Touchpads (PTP)**:
* Compatible with Windows 10/11 laptops with Microsoft Precision Touchpad drivers (HID Usage Page `0x0D`, Usage `0x05`).
* No administrative elevation is required.

## Default Gesture Mappings

| Gesture | Default Action | Description |
|---|---|---|
| `swipe_left_3` | `Alt+Left` | Browser Back / Navigate Left |
| `swipe_right_3` | `Alt+Right` | Browser Forward / Navigate Right |
| `swipe_up_3` | `Super+Tab` | Task View / Workspace Overview |
| `swipe_down_3` | `Super+D` | Show Desktop / Minimize Windows |
| `swipe_left_4` | `Ctrl+Super+Left` | Switch Virtual Desktop Left |
| `swipe_right_4` | `Ctrl+Super+Right` | Switch Virtual Desktop Right |
| `swipe_up_4` | `Super` | App Launcher / Start Menu |
| `swipe_down_4` | `Super+M` | Minimize All Windows |
| `pinch_in` | `Ctrl+Minus` | Zoom Out |
| `pinch_out` | `Ctrl+Plus` | Zoom In |
| `tap_3` | `Ctrl+Shift+T` | Reopen Closed Browser Tab |
| `tap_4` | `Super+A` | Action Center / Quick Settings |

## Configuration & Hot-Reloading

The configuration file is automatically generated on first launch:
* **Linux:** `~/.config/gesture-manager/config.toml`
* **Windows:** `%APPDATA%\gesture-manager\config.toml`

Any edits saved to `config.toml` are dynamically hot-reloaded by the background daemon via the `notify` filesystem watcher.

```toml
[sensitivity]
swipe_threshold = 45.0
min_swipe_velocity = 0.12
pinch_threshold = 0.22
tap_max_duration_ms = 280
tap_max_displacement = 18.0

gesture_cooldown_ms = 350

[gestures]
swipe_left_3 = "Alt+Left"
swipe_right_3 = "Alt+Right"
swipe_up_3 = "Super+Tab"
swipe_down_3 = "Super+D"
pinch_in = "Ctrl+Minus"
pinch_out = "Ctrl+Plus"
tap_3 = { command = "sh: notify-send 'Gesture' '3-finger tap'" }
```

## CLI Usage

```bash
# Display help and CLI options
gesture-manager --help

# List all supported gesture names
gesture-manager --list-gestures

# Validate config syntax
gesture-manager --check-config

# Run as headless background daemon (e.g., systemd service)
gesture-manager --no-tray
```
