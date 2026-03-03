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
| `-s`, `--speed <1-64>` | Target FPS (default 64) |
| `-B`, `--brightness <0-100>` | Brightness percent (default 50) |
| `--install` | Register as a Windows startup application (with current flags) |
| `--uninstall` | Remove the Windows startup entry |
| `--hide-console` | Detach and hide the console window |
| `--debug` | Extra timing and log output |

### System Tray
On Windows the app places an icon in the system tray with **Pause**, **Reset Game**, and **Exit** controls.

### Power Management
The app automatically blanks the display on suspend and reconnects the LED Matrix on resume — no user intervention needed. If a module is disconnected while running, it will periodically attempt to reconnect.

## Acknowledgments
- Original Pong Wars by Koen van Gilst: https://github.com/vnglst/pong-wars
- Framework Computer for the LED Matrix hardware and open-source firmware
- Windsurf and OpenAI's GPT-5 enabling me to bring this idea to life
