//! Platform input handling. macOS rdev 0.5 does not translate OtherMouseDown/Up.
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

#[derive(Default)]
pub struct HoldControl {
    enabled: AtomicBool,
    hold_ms: AtomicU64,
    generation: AtomicU64,
}
impl HoldControl {
    pub fn configure(&self, enabled: bool, hold_ms: u64) {
        let was_enabled = self.enabled.swap(enabled, Ordering::SeqCst);
        let previous_ms = self.hold_ms.swap(hold_ms, Ordering::SeqCst);
        if was_enabled != enabled || previous_ms != hold_ms {
            self.release();
        }
    }
    pub fn enabled(&self) -> bool {
        self.enabled.load(Ordering::SeqCst)
    }
    pub fn press(&self) -> Option<(u64, u64)> {
        let ticket = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        self.enabled
            .load(Ordering::SeqCst)
            .then(|| (ticket, self.hold_ms.load(Ordering::SeqCst)))
    }
    pub fn release(&self) {
        self.generation.fetch_add(1, Ordering::SeqCst);
    }
    pub fn is_current(&self, ticket: u64) -> bool {
        self.enabled.load(Ordering::SeqCst) && self.generation.load(Ordering::SeqCst) == ticket
    }
}

// Keyboard auto-repeat must not reset the start time or produce duplicate captures.
#[derive(Default)]
pub struct KeyboardHold {
    started: Option<(std::time::Instant, std::time::Duration)>,
}
impl KeyboardHold {
    pub fn press(&mut self, now: std::time::Instant, hold_ms: u64) {
        if self.started.is_none() {
            self.started = Some((now, std::time::Duration::from_millis(hold_ms)));
        }
    }
    pub fn release(&mut self, now: std::time::Instant) -> bool {
        self.started
            .take()
            .is_some_and(|(start, threshold)| now.saturating_duration_since(start) >= threshold)
    }
    pub fn reset(&mut self) {
        self.started = None;
    }
}

pub fn modifiers_down() -> bool {
    #[cfg(target_os = "macos")]
    {
        use core_graphics::event::CGEventFlags;
        #[link(name = "ApplicationServices", kind = "framework")]
        unsafe extern "C" {
            fn CGEventSourceFlagsState(state_id: i32) -> u64;
        }
        let mask = CGEventFlags::CGEventFlagShift
            | CGEventFlags::CGEventFlagControl
            | CGEventFlags::CGEventFlagAlternate
            | CGEventFlags::CGEventFlagCommand;
        unsafe { CGEventSourceFlagsState(1) & mask.bits() != 0 }
    }
    #[cfg(target_os = "windows")]
    {
        #[link(name = "user32")]
        unsafe extern "system" {
            fn GetAsyncKeyState(key: i32) -> i16;
        }
        [0x10, 0x11, 0x12, 0x5B, 0x5C]
            .into_iter()
            .any(|key| unsafe { GetAsyncKeyState(key) } < 0)
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        false
    }
}

#[derive(Clone, Copy)]
pub struct Permissions {
    pub accessibility: bool,
    pub input_monitoring: bool,
}
impl Permissions {
    pub fn ready(self) -> bool {
        // Accessibility permits listen-only event taps as well as posting Cmd+C.
        // Do not require a separate Input Monitoring grant for mouse capture.
        self.accessibility
    }
}

#[cfg(target_os = "macos")]
#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXIsProcessTrusted() -> bool;
    fn AXIsProcessTrustedWithOptions(options: core_foundation::dictionary::CFDictionaryRef)
        -> bool;
    static kAXTrustedCheckOptionPrompt: core_foundation::string::CFStringRef;
    fn CGPreflightListenEventAccess() -> bool;
    fn CGRequestListenEventAccess() -> bool;
}

pub fn permissions() -> Permissions {
    #[cfg(target_os = "macos")]
    // These preflight APIs do not display a prompt or modify system permissions.
    unsafe {
        Permissions {
            accessibility: AXIsProcessTrusted(),
            input_monitoring: CGPreflightListenEventAccess(),
        }
    }
    #[cfg(not(target_os = "macos"))]
    Permissions {
        accessibility: true,
        input_monitoring: true,
    }
}

pub fn open_permission(app: &tauri::AppHandle, kind: &str) -> crate::error::AppResult<()> {
    #[cfg(target_os = "macos")]
    {
        use tauri_plugin_opener::OpenerExt;
        let pane = match kind {
            "accessibility" => {
                use core_foundation::{
                    base::TCFType, boolean::CFBoolean, dictionary::CFDictionary, string::CFString,
                };
                unsafe {
                    let key = CFString::wrap_under_get_rule(kAXTrustedCheckOptionPrompt);
                    let options = CFDictionary::from_CFType_pairs(&[(
                        key.as_CFType(),
                        CFBoolean::true_value().as_CFType(),
                    )]);
                    AXIsProcessTrustedWithOptions(options.as_concrete_TypeRef());
                }
                "Privacy_Accessibility"
            }
            "input-monitoring" => {
                // Only request access after the user clicks the permission button.
                unsafe { CGRequestListenEventAccess() };
                "Privacy_ListenEvent"
            }
            _ => {
                return Err(crate::error::AppError::new(
                    "INVALID_ACTION",
                    "无效的权限类型",
                ))
            }
        };
        app.opener()
            .open_url(
                format!("x-apple.systempreferences:com.apple.preference.security?{pane}"),
                None::<&str>,
            )
            .map_err(|_| crate::error::AppError::new("PERMISSION_ERROR", "无法打开系统隐私设置"))
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (app, kind);
        Ok(())
    }
}

// Canonical mouse numbering follows macOS: left=0, right=1, middle=2, back=3, forward=4.
pub fn mouse_button(trigger: &str) -> Option<u8> {
    match trigger {
        "MouseLeft" => Some(0),
        "MouseRight" => Some(1),
        "MouseMiddle" => Some(2),
        "MouseBack" => Some(3),
        "MouseForward" => Some(4),
        _ => trigger.strip_prefix("MouseButton").and_then(|s| {
            let number = s.parse::<u8>().ok()?;
            (s == number.to_string() && (6..=32).contains(&number)).then(|| number - 1)
        }),
    }
}

#[cfg(target_os = "macos")]
fn mouse_transition(event_type: u32, button: i64) -> Option<(u8, bool)> {
    let down = match event_type {
        1 | 3 | 25 => true,
        2 | 4 | 26 => false,
        _ => return None,
    };
    let valid = match event_type {
        1 | 2 => button == 0,
        3 | 4 => button == 1,
        _ => (2..32).contains(&button),
    };
    valid.then_some((button as u8, down))
}

#[cfg(target_os = "macos")]
pub fn listen(
    transition: impl Fn(Option<u8>, bool) + Send + 'static,
    ready: impl Fn(),
) -> Result<(), String> {
    use core_foundation::runloop::{kCFRunLoopDefaultMode, CFRunLoop};
    use core_graphics::event::{
        CGEventTap, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement, CGEventType,
        CallbackResult, EventField,
    };
    use std::sync::Arc;
    let interrupted = Arc::new(AtomicBool::new(false));
    let callback_interrupted = interrupted.clone();
    let tap = CGEventTap::new(
        CGEventTapLocation::Session,
        CGEventTapPlacement::HeadInsertEventTap,
        CGEventTapOptions::ListenOnly,
        vec![
            CGEventType::LeftMouseDown,
            CGEventType::LeftMouseUp,
            CGEventType::RightMouseDown,
            CGEventType::RightMouseUp,
            CGEventType::OtherMouseDown,
            CGEventType::OtherMouseUp,
        ],
        move |_, event_type, event| {
            if matches!(
                event_type,
                CGEventType::TapDisabledByTimeout | CGEventType::TapDisabledByUserInput
            ) {
                transition(None, false);
                callback_interrupted.store(true, Ordering::SeqCst);
            } else if let Some((button, down)) = mouse_transition(
                event_type as u32,
                event.get_integer_value_field(EventField::MOUSE_EVENT_BUTTON_NUMBER),
            ) {
                transition(Some(button), down);
            }
            CallbackResult::Keep
        },
    )
    .map_err(|_| "无法启动鼠标监听，请检查 WorkTodo 的辅助功能授权后重新检测".to_string())?;
    let source = tap
        .mach_port()
        .create_runloop_source(0)
        .map_err(|_| "无法启动鼠标监听线程".to_string())?;
    let run_loop = CFRunLoop::get_current();
    unsafe { run_loop.add_source(&source, kCFRunLoopDefaultMode) };
    tap.enable();
    ready();
    loop {
        unsafe {
            CFRunLoop::run_in_mode(
                kCFRunLoopDefaultMode,
                std::time::Duration::from_millis(500),
                false,
            )
        };
        if !permissions().ready() {
            return Err("鼠标采集权限已变更，请允许 WorkTodo 的辅助功能权限后重新检测".into());
        }
        if interrupted.swap(false, Ordering::SeqCst) {
            tap.enable();
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub fn listen(
    transition: impl Fn(Option<u8>, bool) + Send + 'static,
    ready: impl Fn(),
) -> Result<(), String> {
    fn canonical(button: rdev::Button) -> Option<u8> {
        match button {
            rdev::Button::Left => Some(0),
            rdev::Button::Right => Some(1),
            rdev::Button::Middle => Some(2),
            #[cfg(target_os = "windows")]
            rdev::Button::Unknown(1) => Some(3),
            #[cfg(target_os = "windows")]
            rdev::Button::Unknown(2) => Some(4),
            _ => None,
        }
    }
    ready();
    rdev::listen(move |event| match event.event_type {
        rdev::EventType::ButtonPress(button) => {
            if let Some(button) = canonical(button) {
                transition(Some(button), true);
            }
        }
        rdev::EventType::ButtonRelease(button) => {
            if let Some(button) = canonical(button) {
                transition(Some(button), false);
            }
        }
        _ => {}
    })
    .map_err(|_| "无法启动鼠标监听，请重启应用后重试".into())
}

pub fn clipboard_revision() -> Option<i64> {
    #[cfg(target_os = "macos")]
    {
        Some(objc2_app_kit::NSPasteboard::generalPasteboard().changeCount() as i64)
    }
    #[cfg(target_os = "windows")]
    {
        #[link(name = "user32")]
        unsafe extern "system" {
            fn GetClipboardSequenceNumber() -> u32;
        }
        Some(unsafe { GetClipboardSequenceNumber() } as i64)
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        None
    }
}

#[cfg(target_os = "macos")]
fn copy_events() -> Result<(core_graphics::event::CGEvent, core_graphics::event::CGEvent), String> {
    use core_graphics::{
        event::{CGEvent, CGEventFlags},
        event_source::{CGEventSource, CGEventSourceStateID},
    };
    let create = |down| {
        let source =
            CGEventSource::new(CGEventSourceStateID::Private).map_err(|_| "无法创建复制操作")?;
        let event =
            CGEvent::new_keyboard_event(source, 0x08, down).map_err(|_| "无法创建复制操作")?;
        // The C event must carry Command explicitly; posting a separate modifier event is insufficient.
        event.set_flags(CGEventFlags::CGEventFlagCommand);
        Ok::<_, String>(event)
    };
    Ok((create(true)?, create(false)?))
}

#[cfg(target_os = "macos")]
pub fn copy_selection() -> Result<(), String> {
    use core_graphics::event::CGEventTapLocation;
    if !permissions().accessibility {
        return Err("无法复制所选内容，请先允许辅助功能权限".into());
    }
    let (down, up) = copy_events()?;
    down.post(CGEventTapLocation::Session);
    std::thread::sleep(std::time::Duration::from_millis(10));
    up.post(CGEventTapLocation::Session);
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub fn copy_selection() -> Result<(), String> {
    use rdev::{simulate, EventType, Key};
    simulate(&EventType::KeyPress(Key::ControlLeft)).map_err(|_| "无法发送复制操作")?;
    std::thread::sleep(std::time::Duration::from_millis(10));
    let result = simulate(&EventType::KeyPress(Key::KeyC));
    let _ = simulate(&EventType::KeyRelease(Key::KeyC));
    let _ = simulate(&EventType::KeyRelease(Key::ControlLeft));
    result.map_err(|_| "无法复制所选内容，请检查当前应用是否允许复制".into())
}
