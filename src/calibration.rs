#[cfg(windows)]
mod windows_calibration {
    use crate::led_matrix;
    use crate::settings::Settings;
    use std::path::Path;
    use std::thread;
    use std::time::Duration;

    use windows::core::*;
    use windows::Win32::Foundation::*;
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
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
        led_matrix::blank_module(&mut port_a)?;
        led_matrix::blank_module(&mut port_b)?;
        thread::sleep(Duration::from_millis(200));

        // Flash module A at 60% brightness (0.6 * 255 ≈ 153)
        led_matrix::flash_module(&mut port_a, 153)?;

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
            // Create a hidden owner window so the MessageBox inherits our app icon
            let hinstance = GetModuleHandleW(None).unwrap_or_default();
            let owner_class = w!("FW16CalibOwner");
            unsafe extern "system" fn wnd_proc_stub(
                hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM,
            ) -> LRESULT {
                unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
            }
            let wc = WNDCLASSW {
                lpfnWndProc: Some(wnd_proc_stub),
                hInstance: hinstance.into(),
                lpszClassName: owner_class,
                ..Default::default()
            };
            RegisterClassW(&wc);
            let owner = CreateWindowExW(
                WS_EX_TOOLWINDOW,
                owner_class, w!(""),
                WINDOW_STYLE::default(),
                0, 0, 0, 0,
                HWND::default(), HMENU::default(),
                hinstance, None,
            ).unwrap_or_default();
            if let Ok(h) = LoadImageW(hinstance, PCWSTR(1 as *const u16), IMAGE_ICON, 16, 16, LR_SHARED) {
                SendMessageW(owner, WM_SETICON, WPARAM(0), LPARAM(h.0 as isize));
            }
            if let Ok(h) = LoadImageW(hinstance, PCWSTR(1 as *const u16), IMAGE_ICON, 32, 32, LR_SHARED) {
                SendMessageW(owner, WM_SETICON, WPARAM(1), LPARAM(h.0 as isize));
            }

            let r = MessageBoxW(
                owner,
                PCWSTR(msg_text.as_ptr()),
                PCWSTR(title.as_ptr()),
                MB_YESNO | MB_ICONQUESTION | MB_TOPMOST,
            );

            let _ = DestroyWindow(owner);
            let _ = UnregisterClassW(owner_class, hinstance);
            r
        };

        let a_is_left = result == MESSAGEBOX_RESULT(6); // IDYES = 6

        // Blank module A
        led_matrix::blank_module(&mut port_a)?;

        // Default to dual mode when two modules are installed
        settings.dual_mode = true;

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
        led_matrix::flash_module(&mut port_b, 153)?;
        thread::sleep(Duration::from_millis(500));
        led_matrix::blank_module(&mut port_b)?;

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
