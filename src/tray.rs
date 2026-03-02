#[cfg(windows)]
mod windows_tray {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;

    use image::ImageReader;
    use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
    use tray_icon::{Icon, TrayIconBuilder};
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        DispatchMessageW, PeekMessageW, TranslateMessage, MSG, PM_REMOVE,
    };

    const ICON_BYTES: &[u8] = include_bytes!("../assets/icon.ico");

    fn load_icon() -> Icon {
        let reader = ImageReader::new(std::io::Cursor::new(ICON_BYTES))
            .with_guessed_format()
            .expect("Failed to detect icon format");
        let img = reader.decode().expect("Failed to decode icon").to_rgba8();
        Icon::from_rgba(img.to_vec(), img.width(), img.height())
            .expect("Failed to create tray icon from RGBA data")
    }

    pub fn run_tray(
        shutdown: &'static AtomicBool,
        paused: &'static AtomicBool,
        reset_requested: &'static AtomicBool,
    ) {
        let icon = load_icon();

        let pause_item = MenuItem::new("Pause", true, None);
        let reset_item = MenuItem::new("Reset Game", true, None);
        let exit_item = MenuItem::new("Exit", true, None);

        let menu = Menu::new();
        let _ = menu.append(&pause_item);
        let _ = menu.append(&PredefinedMenuItem::separator());
        let _ = menu.append(&reset_item);
        let _ = menu.append(&PredefinedMenuItem::separator());
        let _ = menu.append(&exit_item);

        let _tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("FW16 Pong Wars")
            .with_icon(icon)
            .build()
            .expect("Failed to build tray icon");

        let menu_receiver = MenuEvent::receiver();
        let mut is_paused = false;

        loop {
            // Pump Win32 messages so tray-icon can process events
            unsafe {
                let mut msg = MSG::default();
                while PeekMessageW(&mut msg, HWND::default(), 0, 0, PM_REMOVE).as_bool() {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }

            // Handle menu events
            while let Ok(event) = menu_receiver.try_recv() {
                if event.id() == pause_item.id() {
                    is_paused = !is_paused;
                    paused.store(is_paused, Ordering::Release);
                    pause_item.set_text(if is_paused { "Resume" } else { "Pause" });
                } else if event.id() == reset_item.id() {
                    reset_requested.store(true, Ordering::Release);
                } else if event.id() == exit_item.id() {
                    shutdown.store(true, Ordering::Release);
                }
            }

            if shutdown.load(Ordering::Relaxed) {
                break;
            }

            std::thread::sleep(Duration::from_millis(50));
        }

        // _tray dropped here — icon removed from system tray
    }
}

#[cfg(windows)]
pub use windows_tray::run_tray;
