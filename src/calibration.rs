#[cfg(windows)]
mod windows_calibration {
    use crate::led_matrix;
    use crate::settings::Settings;
    use std::path::Path;
    use std::thread;
    use std::time::Duration;

    use windows::core::*;
    use windows::Win32::UI::WindowsAndMessaging::*;

    /// Run the interactive calibration flow.
    ///
    /// - If only 1 module is detected, auto-assign it as right_serial (no dialog).
    /// - If 2 modules are detected, flash one at 60% brightness and ask the user
    ///   "Is this your LEFT module?" via a Yes/No MessageBox.
    ///
    /// Updates `settings` in-place and saves to `settings_path`.
    pub fn run_calibration(
        detected: &[(String, String)],
        settings: &mut Settings,
        settings_path: &Path,
    ) -> anyhow::Result<()> {
        match detected.len() {
            0 => {
                println!("Calibration: no modules detected, skipping.");
                return Ok(());
            }
            1 => {
                // Single module: auto-assign as right (the common slot)
                let (_port, serial) = &detected[0];
                println!("Calibration: single module detected ({}), auto-assigning as right.", serial);
                settings.right_serial = serial.clone();
                settings.left_serial.clear();
                settings.save(settings_path)?;
                return Ok(());
            }
            _ => {
                // Two modules: interactive calibration
            }
        }

        // --- Two-module calibration ---
        let (port_a_name, serial_a) = &detected[0];
        let (port_b_name, serial_b) = &detected[1];

        println!(
            "Calibration: two modules detected: {} ({}) and {} ({})",
            port_a_name, serial_a, port_b_name, serial_b
        );

        // Open both ports
        let mut port_a = led_matrix::open_port_by_serial(serial_a)?;
        let mut port_b = led_matrix::open_port_by_serial(serial_b)?;

        // Blank both first
        let _ = led_matrix::blank_module(&mut port_a);
        let _ = led_matrix::blank_module(&mut port_b);
        thread::sleep(Duration::from_millis(200));

        // Flash module A at 60% brightness (0.6 * 255 ≈ 153)
        let _ = led_matrix::flash_module(&mut port_a, 153);

        // Ask the user
        let msg_text: Vec<u16> = "A module is now lit up.\nIs this your LEFT module?"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let title: Vec<u16> = "Module Calibration"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();

        let result = unsafe {
            MessageBoxW(
                None,
                PCWSTR(msg_text.as_ptr()),
                PCWSTR(title.as_ptr()),
                MB_YESNO | MB_ICONQUESTION | MB_TOPMOST,
            )
        };

        let a_is_left = result == MESSAGEBOX_RESULT(6); // IDYES = 6

        // Blank module A
        let _ = led_matrix::blank_module(&mut port_a);

        if a_is_left {
            settings.left_serial = serial_a.clone();
            settings.right_serial = serial_b.clone();
            println!("Calibration result: {} = LEFT, {} = RIGHT", serial_a, serial_b);
        } else {
            settings.left_serial = serial_b.clone();
            settings.right_serial = serial_a.clone();
            println!("Calibration result: {} = LEFT, {} = RIGHT", serial_b, serial_a);
        }

        // Confirmation flash: briefly light up the OTHER module (now identified)
        let _ = led_matrix::flash_module(&mut port_b, 153);
        thread::sleep(Duration::from_millis(500));
        let _ = led_matrix::blank_module(&mut port_b);

        // Close ports before the game opens them
        drop(port_a);
        drop(port_b);
        thread::sleep(Duration::from_millis(200));

        settings.save(settings_path)?;
        println!("Calibration complete. Settings saved.");

        Ok(())
    }
}

#[cfg(windows)]
pub use windows_calibration::run_calibration;
