use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Shared handshake for suspend: the power callback sets `requested`, then
/// spins until the render loop sets `acked` (or a timeout expires).  This
/// keeps the OS from actually suspending before we blank the display.
pub struct SuspendSync {
    pub requested: AtomicBool,
    pub acked: AtomicBool,
}

impl SuspendSync {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            requested: AtomicBool::new(false),
            acked: AtomicBool::new(false),
        })
    }
}

#[cfg(windows)]
mod windows_power {
    use super::*;
    use std::time::Instant;

    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::Power::{
        PowerRegisterSuspendResumeNotification, PowerUnregisterSuspendResumeNotification,
        DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS, HPOWERNOTIFY,
    };
    use windows::Win32::UI::WindowsAndMessaging::REGISTER_NOTIFICATION_FLAGS;

    const PBT_APMSUSPEND: u32 = 0x0004;
    const PBT_APMRESUMEAUTOMATIC: u32 = 0x0012;
    const DEVICE_NOTIFY_CALLBACK: REGISTER_NOTIFICATION_FLAGS =
        REGISTER_NOTIFICATION_FLAGS(2u32);
    const SUSPEND_ACK_TIMEOUT_MS: u64 = 1000;

    struct CallbackContext {
        resume_flag: Arc<AtomicBool>,
        suspend_sync: Arc<SuspendSync>,
    }

    unsafe extern "system" fn power_callback(
        context: *const core::ffi::c_void,
        event_type: u32,
        _setting: *const core::ffi::c_void,
    ) -> u32 {
        if context.is_null() {
            return 0;
        }
        let ctx = unsafe { &*(context as *const CallbackContext) };

        match event_type {
            PBT_APMSUSPEND => {
                // Ask the render loop to blank the display, then block until
                // it acknowledges (or we time out).
                ctx.suspend_sync.acked.store(false, Ordering::Release);
                ctx.suspend_sync.requested.store(true, Ordering::Release);

                let deadline = Instant::now()
                    + std::time::Duration::from_millis(SUSPEND_ACK_TIMEOUT_MS);
                while !ctx.suspend_sync.acked.load(Ordering::Acquire) {
                    if Instant::now() >= deadline {
                        break;
                    }
                    std::hint::spin_loop();
                }

                // Reset for next cycle
                ctx.suspend_sync.requested.store(false, Ordering::Release);
                ctx.suspend_sync.acked.store(false, Ordering::Release);
            }
            PBT_APMRESUMEAUTOMATIC => {
                ctx.resume_flag.store(true, Ordering::Release);
            }
            _ => {}
        }
        0
    }

    pub struct PowerNotificationGuard {
        handle: HPOWERNOTIFY,
        // prevent the Arc pointers from being freed while the callback is live
        _ctx: Box<CallbackContext>,
    }

    unsafe impl Send for PowerNotificationGuard {}

    impl Drop for PowerNotificationGuard {
        fn drop(&mut self) {
            unsafe {
                let _ = PowerUnregisterSuspendResumeNotification(self.handle);
            }
        }
    }

    pub fn register_power_notification(
        resume_flag: Arc<AtomicBool>,
        suspend_sync: Arc<SuspendSync>,
    ) -> Result<PowerNotificationGuard, String> {
        let ctx = Box::new(CallbackContext {
            resume_flag,
            suspend_sync,
        });
        let raw_ptr: *const CallbackContext = &*ctx;

        let params = DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS {
            Callback: Some(power_callback),
            Context: raw_ptr as *mut core::ffi::c_void,
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
                _ctx: ctx,
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
pub use windows_power::register_power_notification;
