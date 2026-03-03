#[cfg(windows)]
mod windows_dialog {
    use crate::settings::Settings;
    use std::path::Path;
    use std::sync::atomic::{AtomicBool, Ordering};

    use windows::core::*;
    use windows::Win32::Foundation::*;
    use windows::Win32::Graphics::Gdi::*;
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::Controls::{InitCommonControlsEx, INITCOMMONCONTROLSEX, ICC_STANDARD_CLASSES};
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
    const ID_MODULE_LEFT_RADIO: i32 = 111;
    const ID_MODULE_RIGHT_RADIO: i32 = 112;
    const ID_RECALIBRATE_BTN: i32 = 113;
    const ID_MODULE_SIDE_LABEL: i32 = 114;
    const ID_SAVE_BTN: i32 = 201;
    const ID_CANCEL_BTN: i32 = 202;

    // Win32 style constants not exposed by windows crate
    const BS_AUTOCHECKBOX: u32 = 0x0003;
    const BS_PUSHBUTTON: u32 = 0x0000;
    const BS_AUTORADIOBUTTON: u32 = 0x0009;
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
        shutdown: &'static AtomicBool,
        restart_pending: &'static AtomicBool,
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn hmenu_from_id(id: i32) -> HMENU {
        HMENU(id as *mut _)
    }

    pub fn show_settings_dialog(current: &Settings, settings_path: &Path, shutdown: &'static AtomicBool, restart_pending: &'static AtomicBool) {
        let state = Box::new(DialogState {
            settings: current.clone(),
            settings_path: settings_path.to_path_buf(),
            shutdown,
            restart_pending,
        });
        let state_ptr = Box::into_raw(state);

        unsafe {
            let icc = INITCOMMONCONTROLSEX {
                dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
                dwICC: ICC_STANDARD_CLASSES,
            };
            let _ = InitCommonControlsEx(&icc);

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
            let height = 440;
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

            // Set window icon from embedded resource (resource ID 1)
            if let Ok(h) = LoadImageW(hinstance, PCWSTR(1 as *const u16), IMAGE_ICON, 16, 16, LR_SHARED) {
                SendMessageW(hwnd, WM_SETICON, WPARAM(0), LPARAM(h.0 as isize)); // ICON_SMALL
            }
            if let Ok(h) = LoadImageW(hinstance, PCWSTR(1 as *const u16), IMAGE_ICON, 32, 32, LR_SHARED) {
                SendMessageW(hwnd, WM_SETICON, WPARAM(1), LPARAM(h.0 as isize)); // ICON_BIG
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

                    // Detect installed modules
                    let module_count = crate::led_matrix::detect_modules().len();
                    let multi_module = module_count >= 2;

                    if multi_module {
                        create_checkbox(hwnd, "Dual Mode", margin, y, 200, row_h,
                            ID_DUALMODE_CHECK, state.settings.dual_mode);
                        y += row_h + 10;
                    }

                    if module_count >= 1 {
                        // --- Installed Modules section ---
                        create_label(hwnd, "Installed Modules:", margin, y + 3, 200, row_h);
                        if multi_module {
                            create_button(hwnd, "Recalibrate", margin + 210, y, 100, row_h, ID_RECALIBRATE_BTN);
                        }
                        y += row_h + 2;

                        if multi_module {
                            // Two modules: show L/R prefixed serial numbers
                            let calibrated = !state.settings.left_serial.is_empty() && !state.settings.right_serial.is_empty();
                            if calibrated {
                                let prefix_w = 18;
                                let serial_x = margin + 10 + prefix_w;
                                create_label(hwnd, "L:", margin + 10, y + 3, prefix_w, row_h);
                                create_label(hwnd, &state.settings.left_serial, serial_x, y + 3, 300 - prefix_w, row_h);
                                y += row_h;
                                create_label(hwnd, "R:", margin + 10, y + 3, prefix_w, row_h);
                                create_label(hwnd, &state.settings.right_serial, serial_x, y + 3, 300 - prefix_w, row_h);
                                y += row_h + 4;
                            } else {
                                y += 6;
                            }
                        } else {
                            // Single module: show serial without L/R prefix
                            let serial = if !state.settings.right_serial.is_empty() {
                                &state.settings.right_serial
                            } else if !state.settings.left_serial.is_empty() {
                                &state.settings.left_serial
                            } else {
                                ""
                            };
                            if !serial.is_empty() {
                                create_label(hwnd, serial, margin + 10, y + 3, 300, row_h);
                                y += row_h + 4;
                            } else {
                                y += 6;
                            }
                        }
                    }

                    if multi_module {
                        // --- Module side picker (visible when dual mode is off) ---
                        let side_style = if state.settings.dual_mode {
                            WS_CHILD
                        } else {
                            WS_CHILD | WS_VISIBLE
                        };
                        let side_tab_style = if state.settings.dual_mode {
                            WS_CHILD | WS_TABSTOP
                        } else {
                            WS_CHILD | WS_VISIBLE | WS_TABSTOP
                        };

                        {
                            let label_text = wide("Use side:");
                            let _ = CreateWindowExW(
                                WINDOW_EX_STYLE::default(),
                                w!("STATIC"),
                                PCWSTR(label_text.as_ptr()),
                                side_style,
                                margin, y + 3, 70, row_h,
                                hwnd,
                                hmenu_from_id(ID_MODULE_SIDE_LABEL),
                                HINSTANCE::default(),
                                None,
                            );
                        }

                        let left_text = wide("Left");
                        let left_radio = CreateWindowExW(
                            WINDOW_EX_STYLE::default(),
                            w!("BUTTON"),
                            PCWSTR(left_text.as_ptr()),
                            side_tab_style | WINDOW_STYLE(BS_AUTORADIOBUTTON),
                            margin + 75, y, 70, row_h,
                            hwnd,
                            hmenu_from_id(ID_MODULE_LEFT_RADIO),
                            HINSTANCE::default(),
                            None,
                        ).unwrap_or_default();

                        let right_text = wide("Right");
                        let right_radio = CreateWindowExW(
                            WINDOW_EX_STYLE::default(),
                            w!("BUTTON"),
                            PCWSTR(right_text.as_ptr()),
                            side_tab_style | WINDOW_STYLE(BS_AUTORADIOBUTTON),
                            margin + 150, y, 70, row_h,
                            hwnd,
                            hmenu_from_id(ID_MODULE_RIGHT_RADIO),
                            HINSTANCE::default(),
                            None,
                        ).unwrap_or_default();

                        if state.settings.module == "left" {
                            SendMessageW(left_radio, BM_SETCHECK, WPARAM(1), LPARAM(0));
                        } else {
                            SendMessageW(right_radio, BM_SETCHECK, WPARAM(1), LPARAM(0));
                        }

                        y += row_h + 10;
                    }

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

                    // Set modern font (Segoe UI) on all child controls
                    let font = GetStockObject(DEFAULT_GUI_FONT);
                    unsafe extern "system" fn set_font_callback(
                        child: HWND, lparam: LPARAM,
                    ) -> BOOL {
                        unsafe {
                            SendMessageW(child, WM_SETFONT, WPARAM(lparam.0 as usize), LPARAM(1));
                        }
                        TRUE
                    }
                    let _ = EnumChildWindows(hwnd, Some(set_font_callback), LPARAM(font.0 as isize));

                    // Poll the shutdown flag so tray "Exit" can close this window
                    let _ = SetTimer(hwnd, 1, 200, None);

                    LRESULT(0)
                }
                WM_TIMER => {
                    let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut DialogState;
                    if !state_ptr.is_null() {
                        let state = &*state_ptr;
                        if state.shutdown.load(Ordering::Relaxed) {
                            let _ = DestroyWindow(hwnd);
                        }
                    }
                    LRESULT(0)
                }
                WM_COMMAND => {
                    let id = (wparam.0 & 0xFFFF) as u32;
                    let notification = ((wparam.0 >> 16) & 0xFFFF) as u32;

                    if notification == BN_CLICKED {
                        if id == ID_DUALMODE_CHECK as u32 {
                            let dual_hwnd = GetDlgItem(hwnd, ID_DUALMODE_CHECK).unwrap_or_default();
                            let is_dual = SendMessageW(dual_hwnd, BM_GETCHECK, WPARAM(0), LPARAM(0)).0 == 1;
                            let show_cmd = if is_dual { SW_HIDE } else { SW_SHOW };
                            if let Ok(h) = GetDlgItem(hwnd, ID_MODULE_SIDE_LABEL) { let _ = ShowWindow(h, show_cmd); }
                            if let Ok(h) = GetDlgItem(hwnd, ID_MODULE_LEFT_RADIO) { let _ = ShowWindow(h, show_cmd); }
                            if let Ok(h) = GetDlgItem(hwnd, ID_MODULE_RIGHT_RADIO) { let _ = ShowWindow(h, show_cmd); }
                        } else if id == ID_RECALIBRATE_BTN as u32 {
                            let state_ptr =
                                GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut DialogState;
                            if !state_ptr.is_null() {
                                let state = &*state_ptr;
                                // Clear stored serials so calibration re-triggers on restart
                                let mut recal_settings = state.settings.clone();
                                recal_settings.left_serial.clear();
                                recal_settings.right_serial.clear();
                                let _ = recal_settings.save(&state.settings_path);
                                state.restart_pending.store(true, Ordering::Release);
                                state.shutdown.store(true, Ordering::Release);
                                let _ = DestroyWindow(hwnd);
                            }
                        } else if id == ID_SAVE_BTN as u32 {
                            let state_ptr =
                                GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut DialogState;
                            if !state_ptr.is_null() {
                                let state = &*state_ptr;

                                // Dual mode checkbox only exists with 2+ modules
                                let dual_mode = if let Ok(h) = GetDlgItem(hwnd, ID_DUALMODE_CHECK) {
                                    SendMessageW(h, BM_GETCHECK, WPARAM(0), LPARAM(0)).0 == 1
                                } else {
                                    state.settings.dual_mode
                                };

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

                                // Module radio buttons only exist with 2+ modules
                                let module = if let Ok(h) = GetDlgItem(hwnd, ID_MODULE_LEFT_RADIO) {
                                    if SendMessageW(h, BM_GETCHECK, WPARAM(0), LPARAM(0)).0 == 1 {
                                        "left".to_string()
                                    } else {
                                        "right".to_string()
                                    }
                                } else {
                                    state.settings.module.clone()
                                };

                                let new_settings = Settings {
                                    dual_mode,
                                    left_serial: state.settings.left_serial.clone(),
                                    right_serial: state.settings.right_serial.clone(),
                                    module,
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
                                    // Signal main() to restart after cleanup
                                    state.restart_pending.store(true, Ordering::Release);
                                    state.shutdown.store(true, Ordering::Release);
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
                    let _ = KillTimer(hwnd, 1);
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
