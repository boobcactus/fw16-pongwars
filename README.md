# FW16 Pong Wars
A Rust app that plays Pong Wars on the Framework Laptop 16 LED Matrix.

https://github.com/user-attachments/assets/2d7a4b85-f580-4dbc-9378-3473213b643f

## Requirements
- Windows 10 or later
- At least one [Framework Laptop 16 LED Matrix](https://frame.work/products/16-led-matrix)
- Optional second LED Matrix for dual-mode (`-d`, `--dualmode`)

## Installation

### Pre-built binary
Download `fw16-pongwars-portable.exe` from the latest release. The executable is fully portable — no Visual C++ Redistributable or other runtime dependencies required.

### Installer
Download the NSIS installer (`FW16_Pong_Wars_x64-setup.exe`) from the latest release. It installs the app to `%LOCALAPPDATA%\FW16 Pong Wars`, creates a desktop shortcut, and uses a settings file at `%APPDATA%\FW16PongWars\settings.toml`.

### Build from source
Requires the Rust toolchain (stable).

```powershell
cargo build --release
```

The portable executable is written to `target\release\fw16-pongwars-portable.exe`.

## Usage

```powershell
fw16-pongwars-portable.exe --dualmode --speed 48 --balls 5 --brightness 10
```

### Flags

| Flag | Description |
|------|-------------|
| `-d`, `--dualmode` | Drive two modules side-by-side (18×34) |
| `-b`, `--balls [1-20]` | Balls per team (defaults to 2 if flag is passed without a value) |
| `-s`, `--speed <1-64>` | Target FPS (default 32) |
| `-B`, `--brightness <0-100>` | Brightness percent (default 40) |
| `--settings <path>` | Path to persistent settings TOML file (cannot be combined with game flags) |
| `--debug` | Extra timing and log output |

### System Tray
On Windows the app places an icon in the system tray with **Pause**, **Reset Game**, and **Exit** controls. When running with `--settings`, a **Settings** menu item also appears, opening a dialog to configure all game options and auto-start behavior.

### Persistent Settings
Use `--settings=path/to/settings.toml` to enable persistent configuration. If the file doesn't exist it will be created with defaults (balls=2, speed=32, brightness=40, start_with_windows=true). The `--settings` flag is mutually exclusive with `-b`, `-s`, `-B`, and `-d`.

### Building the Installer
Requires `cargo-packager`:
```powershell
cargo install cargo-packager --locked
cargo packager --release --formats nsis
```
The installer `.exe` will be placed in the output directory.

### Power Management
The app automatically blanks the display on suspend and reconnects the LED Matrix on resume — no user intervention needed. If a module is disconnected while running, it will periodically attempt to reconnect.

## Acknowledgments
- Original Pong Wars by Koen van Gilst: https://github.com/vnglst/pong-wars
- Framework Computer for the LED Matrix hardware and open-source firmware
- Windsurf and OpenAI's GPT-5 enabling me to bring this idea to life
