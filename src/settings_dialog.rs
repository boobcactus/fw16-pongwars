#[cfg(windows)]
mod windows_dialog {
    use crate::settings::Settings;
    use std::path::Path;

    use windows::core::*;
    use windows::Win32::Foundation::*;
    use windows::Win32::Graphics::Gdi::*;
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::WindowsAndMessaging::*;

    // Control IDs
    const ID_DUALMODE_CHECK: i32 = 101;
    const ID_BALLS_EDIT: i32 = 102;
    const ID_BALLS_UPDOWN: i32 = 103;
    const ID_SPEED_EDIT: i32 = 104;
    const ID_SPEED_UPDOWN: i32 = 105;
    const ID_BRIGHTNESS_EDIT: i32 = 106;
    const ID_BRIGHTNESS_UPDOWN: i32 = 107;
    const ID_DEBUG_CHECK: i32 = 108;
    const ID_STARTUP_CHECK: i32 = 109;
    const ID_SAVE_BTN: i32 = 201;
    const ID_CANCEL_BTN: i32 = 202;

    // Win32 style constants not exposed by windows crate
    const BS_AUTOCHECKBOX: u32 = 0x0003;
    const BS_PUSHBUTTON: u32 = 0x0000;
    const ES_NUMBER: u32 = 0x2000;
    const BN_CLICKED: u32 = 0;

    // UpDown control messages
    const UDM_SETRANGE32: u32 = 0x046F;
    const UDM_SETPOS32: u32 = 0x0471;
    const UDM_GETPOS32: u32 = 0x0472;
    const UDM_SETBUDDY: u32 = 0x0469;

    struct DialogState {
        settings: Settings,
        settings_path: std::path::PathBuf,
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn hmenu_from_id(id: i32) -> HMENU {
        HMENU(id as *mut _)
    }

    pub fn show_settings_dialog(current: &Settings, settings_path: &Path) {
        let state = Box::new(DialogState {
            settings: current.clone(),
            settings_path: settings_path.to_path_buf(),
        });
        let state_ptr = Box::into_raw(state);

        unsafe {
            let hinstance = GetModuleHandleW(None).unwrap_or_default();
            let class_name = w!("FW16PongWarsSettings");

            let wc = WNDCLASSW {
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(dialog_proc),
                hInstance: hinstance.into(),
                hCursor: LoadCursorW(HINSTANCE::default(), IDC_ARROW).unwrap_or_default(),
                hbrBackground: HBRUSH((15 + 1) as *mut _), // COLOR_BTNFACE = 15
                lpszClassName: class_name,
                ..Default::default()
            };
            RegisterClassW(&wc);

            let width = 360;
            let height = 360;
            let screen_w = GetSystemMetrics(SM_CXSCREEN);
            let screen_h = GetSystemMetrics(SM_CYSCREEN);
            let x = (screen_w - width) / 2;
            let y = (screen_h - height) / 2;

            let hwnd = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                class_name,
                w!("FW16 Pong Wars \u{2014} Settings"),
                WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU,
                x,
                y,
                width,
                height,
                HWND::default(),
                HMENU::default(),
                hinstance,
                Some(state_ptr as *const _),
            )
            .unwrap_or_default();

            if hwnd == HWND::default() {
                let _ = Box::from_raw(state_ptr);
                return;
            }

            let _ = ShowWindow(hwnd, SW_SHOW);
            let _ = UpdateWindow(hwnd);

            let mut msg = MSG::default();
            while GetMessageW(&mut msg, HWND::default(), 0, 0).as_bool() {
                if !IsDialogMessageW(hwnd, &msg).as_bool() {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }

            let _ = UnregisterClassW(class_name, hinstance);
        }
    }

    unsafe fn create_label(parent: HWND, text: &str, x: i32, y: i32, w: i32, h: i32) {
        unsafe {
            let text_w = wide(text);
            let _ = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("STATIC"),
                PCWSTR(text_w.as_ptr()),
                WS_CHILD | WS_VISIBLE,
                x, y, w, h,
                parent,
                HMENU::default(),
                HINSTANCE::default(),
                None,
            );
        }
    }

    unsafe fn create_checkbox(
        parent: HWND, text: &str,
        x: i32, y: i32, w: i32, h: i32,
        id: i32, checked: bool,
    ) -> HWND {
        unsafe {
            let text_w = wide(text);
            let hwnd = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("BUTTON"),
                PCWSTR(text_w.as_ptr()),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_AUTOCHECKBOX),
                x, y, w, h,
                parent,
                hmenu_from_id(id),
                HINSTANCE::default(),
                None,
            )
            .unwrap_or_default();
            if checked {
                SendMessageW(hwnd, BM_SETCHECK, WPARAM(1), LPARAM(0)); // BST_CHECKED = 1
            }
            hwnd
        }
    }

    unsafe fn create_edit_with_updown(
        parent: HWND,
        x: i32, y: i32, edit_w: i32, h: i32,
        edit_id: i32, updown_id: i32,
        min: i32, max: i32, value: i32,
    ) -> (HWND, HWND) {
        unsafe {
            let edit = CreateWindowExW(
                WS_EX_CLIENTEDGE,
                w!("EDIT"),
                w!(""),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(ES_NUMBER),
                x, y, edit_w, h,
                parent,
                hmenu_from_id(edit_id),
                HINSTANCE::default(),
                None,
            )
            .unwrap_or_default();

            // UDS_SETBUDDYINT=0x20 | UDS_ALIGNRIGHT=0x10 | UDS_ARROWKEYS=0x02
            let updown = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("msctls_updown32"),
                w!(""),
                WS_CHILD | WS_VISIBLE | WINDOW_STYLE(0x0020 | 0x0010 | 0x0002),
                x + edit_w, y, 16, h,
                parent,
                hmenu_from_id(updown_id),
                HINSTANCE::default(),
                None,
            )
            .unwrap_or_default();

            SendMessageW(updown, UDM_SETBUDDY, WPARAM(edit.0 as usize), LPARAM(0));
            SendMessageW(updown, UDM_SETRANGE32, WPARAM(min as usize), LPARAM(max as isize));
            SendMessageW(updown, UDM_SETPOS32, WPARAM(0), LPARAM(value as isize));

            (edit, updown)
        }
    }

    unsafe fn create_button(
        parent: HWND, text: &str,
        x: i32, y: i32, w: i32, h: i32, id: i32,
    ) -> HWND {
        unsafe {
            let text_w = wide(text);
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("BUTTON"),
                PCWSTR(text_w.as_ptr()),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_PUSHBUTTON),
                x, y, w, h,
                parent,
                hmenu_from_id(id),
                HINSTANCE::default(),
                None,
            )
            .unwrap_or_default()
        }
    }

    unsafe extern "system" fn dialog_proc(
        hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM,
    ) -> LRESULT {
        unsafe {
            match msg {
                WM_CREATE => {
                    let cs = &*(lparam.0 as *const CREATESTRUCTW);
                    let state_ptr = cs.lpCreateParams as *mut DialogState;
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, state_ptr as isize);
                    let state = &*state_ptr;

                    let margin = 20i32;
                    let label_w = 120i32;
                    let control_x = margin + label_w + 10;
                    let row_h = 26i32;
                    let mut y = 15i32;

                    create_checkbox(hwnd, "Dual Mode", margin, y, 200, row_h,
                        ID_DUALMODE_CHECK, state.settings.dual_mode);
                    y += row_h + 10;

                    create_label(hwnd, "Balls per team:", margin, y + 3, label_w, row_h);
                    create_edit_with_updown(hwnd, control_x, y, 60, row_h,
                        ID_BALLS_EDIT, ID_BALLS_UPDOWN, 1, 20, state.settings.balls as i32);
                    y += row_h + 10;

                    create_label(hwnd, "Speed (FPS):", margin, y + 3, label_w, row_h);
                    create_edit_with_updown(hwnd, control_x, y, 60, row_h,
                        ID_SPEED_EDIT, ID_SPEED_UPDOWN, 1, 64, state.settings.speed as i32);
                    y += row_h + 10;

                    create_label(hwnd, "Brightness %:", margin, y + 3, label_w, row_h);
                    create_edit_with_updown(hwnd, control_x, y, 60, row_h,
                        ID_BRIGHTNESS_EDIT, ID_BRIGHTNESS_UPDOWN, 0, 100, state.settings.brightness as i32);
                    y += row_h + 10;

                    create_checkbox(hwnd, "Debug mode", margin, y, 200, row_h,
                        ID_DEBUG_CHECK, state.settings.debug);
                    y += row_h + 10;

                    create_checkbox(hwnd, "Start with Windows", margin, y, 200, row_h,
                        ID_STARTUP_CHECK, state.settings.start_with_windows);
                    y += row_h + 20;

                    create_button(hwnd, "Save && Restart", margin, y, 140, 32, ID_SAVE_BTN);
                    create_button(hwnd, "Cancel", margin + 150, y, 100, 32, ID_CANCEL_BTN);

                    LRESULT(0)
                }
                WM_COMMAND => {
                    let id = (wparam.0 & 0xFFFF) as u32;
                    let notification = ((wparam.0 >> 16) & 0xFFFF) as u32;

                    if notification == BN_CLICKED {
                        if id == ID_SAVE_BTN as u32 {
                            let state_ptr =
                                GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut DialogState;
                            if !state_ptr.is_null() {
                                let state = &*state_ptr;

                                let dual_mode_hwnd = GetDlgItem(hwnd, ID_DUALMODE_CHECK).unwrap_or_default();
                                let dual_mode = SendMessageW(dual_mode_hwnd, BM_GETCHECK, WPARAM(0), LPARAM(0)).0 == 1;

                                let balls_updown = GetDlgItem(hwnd, ID_BALLS_UPDOWN).unwrap_or_default();
                                let balls = SendMessageW(balls_updown, UDM_GETPOS32, WPARAM(0), LPARAM(0)).0 as u8;

                                let speed_updown = GetDlgItem(hwnd, ID_SPEED_UPDOWN).unwrap_or_default();
                                let speed = SendMessageW(speed_updown, UDM_GETPOS32, WPARAM(0), LPARAM(0)).0 as u8;

                                let brightness_updown = GetDlgItem(hwnd, ID_BRIGHTNESS_UPDOWN).unwrap_or_default();
                                let brightness = SendMessageW(brightness_updown, UDM_GETPOS32, WPARAM(0), LPARAM(0)).0 as u8;

                                let debug_hwnd = GetDlgItem(hwnd, ID_DEBUG_CHECK).unwrap_or_default();
                                let debug = SendMessageW(debug_hwnd, BM_GETCHECK, WPARAM(0), LPARAM(0)).0 == 1;

                                let startup_hwnd = GetDlgItem(hwnd, ID_STARTUP_CHECK).unwrap_or_default();
                                let start_with_windows = SendMessageW(startup_hwnd, BM_GETCHECK, WPARAM(0), LPARAM(0)).0 == 1;

                                let new_settings = Settings {
                                    dual_mode,
                                    balls: balls.clamp(1, 20),
                                    speed: speed.clamp(1, 64),
                                    brightness: brightness.clamp(0, 100),
                                    debug,
                                    start_with_windows,
                                };

                                if let Err(e) = new_settings.save(&state.settings_path) {
                                    let msg_text = wide(&format!("Failed to save settings: {}", e));
                                    MessageBoxW(hwnd, PCWSTR(msg_text.as_ptr()), w!("Error"),
                                        MB_OK | MB_ICONERROR);
                                } else {
                                    MessageBoxW(hwnd,
                                        w!("Settings saved. Restart the application for changes to take effect."),
                                        w!("Settings Saved"),
                                        MB_OK | MB_ICONINFORMATION);
                                    let _ = DestroyWindow(hwnd);
                                }
                            }
                        } else if id == ID_CANCEL_BTN as u32 {
                            let _ = DestroyWindow(hwnd);
                        }
                    }
                    LRESULT(0)
                }
                WM_DESTROY => {
                    let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut DialogState;
                    if !state_ptr.is_null() {
                        let _ = Box::from_raw(state_ptr);
                        SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                    }
                    PostQuitMessage(0);
                    LRESULT(0)
                }
                _ => DefWindowProcW(hwnd, msg, wparam, lparam),
            }
        }
    }
}

#[cfg(windows)]
pub use windows_dialog::show_settings_dialog;
