# FW16 Pong Wars
Pong Wars written in Rust for Framework Laptop 16 LED Matrix modules.

https://github.com/user-attachments/assets/85d59a7b-30c6-42e0-a397-b23e60094e47

## Requirements
- Windows 10 or later
- One or two [Framework Laptop 16 LED Matrix](https://frame.work/products/16-led-matrix) modules
- (Building locally) Rust stable toolchain. Optionally, [NSIS](https://nsis.sourceforge.io/) and `cargo-packager` for the installer

## Installation
### Portable executable
Download `fw16-pongwars.exe` the from the [latest releases](https://github.com/boobcactus/fw16-pongwars/releases). 
Store and run it from anywhere, using either commandline arguments or the `settings.toml` file (generated on first-run).

### Installer
Download the installer (`fw16-pongwars_x.x.x_x64-setup`) from the [latest releases](https://github.com/boobcactus/fw16-pongwars/releases). 
The portable exe will be saved to `%APPDATA%\FW16PongWars` for you. You can also create a shortcut.

### Build from source
To build the portable .exe: 
```powershell
cargo build --release
```
The executable is written to `target\release\fw16-pongwars.exe`.

To build the installer:
```powershell
cargo install cargo-packager --locked
cargo packager --release --formats nsis
```
This will also build the portable .exe. The installer is just a wrapper for the portable version. 

## Usage
### Command line
```powershell
fw16-pongwars-portable.exe --dualmode --speed 48 --balls 5 --brightness 10
```
Pass game parameters directly on the command line for one-off or scripted runs. No `settings.toml` is created.
| Flag | Description | Default |
|------|-------------|---------|
| `-d`, `--dualmode` | Drive two modules side-by-side (18x34) | off |
| `-b`, `--balls [1-20]` | Balls per team | 2 |
| `-s`, `--speed <1-64>` | Target FPS | 32 |
| `-B`, `--brightness <0-100>` | Brightness percent | 40 |
| `--settings <path>` | Path to persistent TOML settings file | -- |
| `--debug` | Extra timing output and debug console | off |

### Settings file
```powershell
fw16-pongwars-portable.exe --settings path\to\settings.toml
```
Points the app at a specific TOML file instead of the default location. If the file does not exist one is created with defaults. 
The `--settings` flag cannot be combined with any other flags.
Changes made in the Settings dialog only take effect on restart. There's a button for this in the menu.

When running in `--settings` mode, all configuration is read from a TOML file.
| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `dual_mode` | bool | `false` | Use both modules as one 18x34 display |
| `left_serial` | string | `""` | USB serial number of the left module (set by calibration) |
| `right_serial` | string | `""` | USB serial number of the right module (set by calibration) |
| `module` | string | `"right"` | Which module to use in single-module mode (`"left"` or `"right"`) |
| `balls` | integer | `2` | Balls per team (1--20) |
| `speed` | integer | `32` | Target FPS (1--64) |
| `brightness` | integer | `40` | Brightness percent (0--100) |
| `debug` | bool | `false` | Enable debug timing output |
| `start_with_windows` | bool | `true` | Register a Windows startup entry for the app |

## System Tray
The app places an icon in the system tray with the following controls:
- **Pause / Resume** -- toggle game state.
- **Reset Game** -- restart the match from the kickoff.
- **Settings** -- opens a window to configure all game options, choose which module side to use, trigger recalibration, and toggle auto-start. *Only appears when running with a settings file.*
- **Exit** -- exit the program.

## License

This project is licensed under the GNU General Public License v3.0. See [LICENSE](LICENSE) for details.

## Acknowledgments

- Original Pong Wars by Koen van Gilst: https://github.com/vnglst/pong-wars
- Framework Computer for the LED Matrix hardware and open-source firmware
- Windsurf and Claude Opus 4.6 enabling me to bring this idea to life
