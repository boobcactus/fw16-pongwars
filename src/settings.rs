use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub dual_mode: bool,
    pub balls: u8,
    pub speed: u8,
    pub brightness: u8,
    pub debug: bool,
    pub start_with_windows: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            dual_mode: false,
            balls: 2,
            speed: 32,
            brightness: 40,
            debug: false,
            start_with_windows: true,
        }
    }
}

impl Settings {
    /// Load from path. If file doesn't exist, create it with defaults.
    pub fn load_or_create(path: &Path) -> anyhow::Result<Self> {
        if path.exists() {
            let contents = std::fs::read_to_string(path)?;
            let settings: Settings = toml::from_str(&contents)?;
            Ok(settings)
        } else {
            let settings = Settings::default();
            settings.save(path)?;
            Ok(settings)
        }
    }

    /// Save current settings to the given path.
    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let contents = toml::to_string_pretty(self)?;
        std::fs::write(path, contents)?;
        Ok(())
    }

    /// Apply the start_with_windows setting to the Windows registry.
    /// When enabled, writes the Run key; when disabled, removes it.
    /// `settings_path` is the path to the TOML file (passed via --settings).
    #[cfg(windows)]
    pub fn apply_startup_registry(&self, settings_path: &Path) -> anyhow::Result<()> {
        use winreg::enums::*;
        use winreg::RegKey;

        let hkcu = RegKey::predef(HKEY_CURRENT_USER);

        if self.start_with_windows {
            let exe = std::env::current_exe()?;
            let cmd = format!(
                "\"{}\" --settings=\"{}\"",
                exe.display(),
                settings_path.display()
            );
            let (key, _) =
                hkcu.create_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Run")?;
            key.set_value("FW16PongWars", &cmd)?;
        } else {
            if let Ok(key) = hkcu.open_subkey_with_flags(
                "Software\\Microsoft\\Windows\\CurrentVersion\\Run",
                KEY_WRITE,
            ) {
                let _ = key.delete_value("FW16PongWars");
            }
        }
        Ok(())
    }
}
