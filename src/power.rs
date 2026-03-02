#[cfg(windows)]
mod windows_power {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::Power::{
        PowerRegisterSuspendResumeNotification, PowerUnregisterSuspendResumeNotification,
        DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS, HPOWERNOTIFY,
    };
    use windows::Win32::UI::WindowsAndMessaging::REGISTER_NOTIFICATION_FLAGS;

    const PBT_APMRESUMEAUTOMATIC: u32 = 0x0012;
    const DEVICE_NOTIFY_CALLBACK: REGISTER_NOTIFICATION_FLAGS =
        REGISTER_NOTIFICATION_FLAGS(2u32);

    unsafe extern "system" fn power_callback(
        context: *const core::ffi::c_void,
        event_type: u32,
        _setting: *const core::ffi::c_void,
    ) -> u32 {
        if event_type == PBT_APMRESUMEAUTOMATIC {
            if !context.is_null() {
                let flag = unsafe { &*(context as *const AtomicBool) };
                flag.store(true, Ordering::Release);
            }
        }
        0
    }

    pub struct PowerNotificationGuard {
        handle: HPOWERNOTIFY,
        _flag: Arc<AtomicBool>,
    }

    unsafe impl Send for PowerNotificationGuard {}

    impl Drop for PowerNotificationGuard {
        fn drop(&mut self) {
            unsafe {
                let _ = PowerUnregisterSuspendResumeNotification(self.handle);
            }
        }
    }

    pub fn register_resume_notification(
        flag: Arc<AtomicBool>,
    ) -> Result<PowerNotificationGuard, String> {
        let raw_ptr = Arc::as_ptr(&flag) as *mut core::ffi::c_void;
        let params = DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS {
            Callback: Some(power_callback),
            Context: raw_ptr,
        };

        let recipient = HANDLE(&params as *const _ as *mut core::ffi::c_void);
        let mut out_handle: *mut core::ffi::c_void = std::ptr::null_mut();
        let result = unsafe {
            PowerRegisterSuspendResumeNotification(
                DEVICE_NOTIFY_CALLBACK,
                recipient,
                &mut out_handle,
            )
        };

        if result.is_ok() {
            Ok(PowerNotificationGuard {
                handle: HPOWERNOTIFY(out_handle as isize),
                _flag: flag,
            })
        } else {
            Err(format!(
                "PowerRegisterSuspendResumeNotification failed: {:?}",
                result
            ))
        }
    }
}

#[cfg(windows)]
pub use windows_power::{register_resume_notification, PowerNotificationGuard};
