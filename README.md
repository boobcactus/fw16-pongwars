# FW16 Pong Wars
A Rust program that plays Pong Wars on Framework Laptop 16 LED Matrix modules.

https://github.com/user-attachments/assets/2d7a4b85-f580-4dbc-9378-3473213b643f

## Requirements
- Windows 10 or later
- One or two [Framework Laptop 16 LED Matrix](https://frame.work/products/16-led-matrix) modules
- (Local builds) Rust stable toolchain. [NSIS](https://nsis.sourceforge.io/) and `cargo-packager` for the installer

## Installation
### Portable executable
Download `fw16-pongwars.exe` from the latest release. The executable is fully portable and has no runtime dependencies.

### Installer
Download the NSIS installer (`fw16-pongwars_x.x.x_x64-setup`) from the latest release. It installs the app to `%LOCALAPPDATA%\FW16 Pong Wars`, creates a desktop shortcut, and stores its settings file at `%APPDATA%\FW16PongWars\settings.toml`.

### Build from source
To build the portable .exe: 

```powershell
cargo build --release
```

The portable executable is written to `target\release\fw16-pongwars.exe`.

To build the NSIS installer:

```powershell
cargo install cargo-packager --locked
cargo packager --release --formats nsis
```

This will also build the portable .exe. The installer is basically a wrapper of the portable version. 

## Usage
The app runs in one of three modes depending on how it is launched.

### Bare run (default)
Double-click the executable or run it without any flags. It loads (or creates) a `settings.toml` file next to the executable. This is the simplest way to use the portable build.

### Settings file
```powershell
fw16-pongwars-portable.exe --settings path\to\settings.toml
```

Points the app at a specific TOML file instead of the default location. If the file does not exist it is created with defaults. This is the mode the installer uses. The `--settings` flag cannot be combined with any CLI-based game flags.

When running in bare-run or `--settings` mode, all configuration is read from a TOML file. See `settings.example.toml` in the repository for a reference copy.

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

### CLI flags
```powershell
fw16-pongwars-portable.exe --dualmode --speed 48 --balls 5 --brightness 10
```

Pass game parameters directly on the command line for one-off runs. Nothing is persisted to disk.

| Flag | Description | Default |
|------|-------------|---------|
| `-d`, `--dualmode` | Drive two modules side-by-side (18x34) | off |
| `-b`, `--balls [1-20]` | Balls per team | 2 |
| `-s`, `--speed <1-64>` | Target FPS | 32 |
| `-B`, `--brightness <0-100>` | Brightness percent | 40 |
| `--settings <path>` | Path to persistent TOML settings file | -- |
| `--debug` | Extra timing output and debug console | off |

## System Tray
The app places an icon in the system tray with the following controls:
- **Pause / Resume** -- toggle game updates.
- **Reset Game** -- restart the match from the initial state.
- **Settings** -- opens a native dialog to configure all game options, choose which module side to use, trigger recalibration, and toggle auto-start. Only appears when running with a settings file.
- **Exit** -- shut down and blank the display.

Changes made in the Settings dialog take effect after an automatic restart.

## Module Calibration
When two LED Matrix modules are detected for the first time, the app runs an interactive calibration step: it lights up one module and asks whether it is the LEFT module that's lit up. The result is saved to the settings file as `left_serial` and `right_serial` so the app can address each module by position on future launches.

- Single-module setups are auto-assigned with no dialog.
- If a stored serial number no longer matches any connected hardware, calibration runs again automatically.
- The Settings menu includes a Recalibrate button that clears the stored serials and restarts.

## License

This project is licensed under the GNU General Public License v3.0. See [LICENSE](LICENSE) for details.

## Acknowledgments

- Original Pong Wars by Koen van Gilst: https://github.com/vnglst/pong-wars
- Framework Computer for the LED Matrix hardware and open-source firmware
- Windsurf and Claude Opus 4.6 enabling me to bring this idea to life
