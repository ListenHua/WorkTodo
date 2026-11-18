use crate::error::{AppError, AppResult};
use block2::{DynBlock, RcBlock};
use objc2::{
    define_class, msg_send,
    rc::Retained,
    runtime::{Bool, ProtocolObject},
    AnyThread, DefinedClass,
};
use objc2_foundation::{
    NSBundle, NSDictionary, NSError, NSObject, NSObjectProtocol, NSProcessInfo, NSString,
};
use objc2_user_notifications::{
    UNAuthorizationOptions, UNAuthorizationStatus, UNMutableNotificationContent, UNNotification,
    UNNotificationPresentationOptions, UNNotificationRequest, UNNotificationResponse,
    UNNotificationSettings, UNNotificationSound, UNUserNotificationCenter,
    UNUserNotificationCenterDelegate,
};
use std::{ptr::NonNull, sync::mpsc, time::Duration};
use tauri::{AppHandle, Emitter, Manager};

define_class!(
    #[unsafe(super(NSObject))]
    #[name = "ProjectManageNotificationDelegate"]
    #[ivars = AppHandle]
    struct NotificationDelegate;

    unsafe impl NSObjectProtocol for NotificationDelegate {}
    unsafe impl UNUserNotificationCenterDelegate for NotificationDelegate {
        #[unsafe(method(userNotificationCenter:willPresentNotification:withCompletionHandler:))]
        fn present(
            &self,
            _center: &UNUserNotificationCenter,
            _notification: &UNNotification,
            completion: &DynBlock<dyn Fn(UNNotificationPresentationOptions)>,
        ) {
            // Foreground reminders should also appear in Notification Center and play a sound.
            completion.call((foreground_options(),));
        }
        #[unsafe(method(userNotificationCenter:didReceiveNotificationResponse:withCompletionHandler:))]
        fn respond(
            &self,
            _center: &UNUserNotificationCenter,
            response: &UNNotificationResponse,
            completion: &DynBlock<dyn Fn()>,
        ) {
            let info = response.notification().request().content().userInfo();
            if let Some(value) = info.objectForKey(&NSString::from_str("taskId")) {
                if let Some(task) = value.downcast_ref::<NSString>() {
                    let app = self.ivars().clone();
                    let task = task.to_string();
                    let _ = self.ivars().run_on_main_thread(move || {
                        crate::desktop::show_main(&app);
                        let _ = app.emit_to("main", "open-task", task);
                    });
                }
            }
            completion.call(());
        }
    }
);
// All ivars are thread-safe AppHandle data; delegate methods may be called by Apple's queue.
unsafe impl Send for NotificationDelegate {}
unsafe impl Sync for NotificationDelegate {}

pub fn initialize(app: &AppHandle) {
    // Apple's notification center raises an Objective-C exception for unbundled binaries.
    // Development/test executables must report this as unavailable instead of crashing.
    if NSBundle::mainBundle().bundleIdentifier().is_none() {
        return;
    }
    let delegate: Retained<NotificationDelegate> = unsafe {
        msg_send![
            super(NotificationDelegate::alloc().set_ivars(app.clone())),
            init
        ]
    };
    UNUserNotificationCenter::currentNotificationCenter()
        .setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
    // The notification center only holds a weak reference to its delegate.
    app.manage(delegate);
}

#[allow(deprecated)] // Alert is required for macOS 10.15, the minimum supported version.
fn foreground_options() -> UNNotificationPresentationOptions {
    let visible = if NSProcessInfo::processInfo()
        .operatingSystemVersion()
        .majorVersion
        >= 11
    {
        UNNotificationPresentationOptions::Banner | UNNotificationPresentationOptions::List
    } else {
        UNNotificationPresentationOptions::Alert
    };
    visible | UNNotificationPresentationOptions::Sound
}

fn error_message(error: *mut NSError) -> String {
    // NSError is only borrowed during Apple's completion callback.
    unsafe { error.as_ref() }
        .map(|error| error.localizedDescription().to_string())
        .unwrap_or_default()
}
fn timeout() -> AppError {
    AppError::new("NOTIFICATION_TIMEOUT", "系统通知服务未响应，请重试")
}

pub fn permission(request: bool) -> AppResult<String> {
    if NSBundle::mainBundle().bundleIdentifier().is_none() {
        return Err(AppError::new(
            "NOTIFICATION_UNAVAILABLE",
            "请安装并打开项目工作台后使用系统通知",
        ));
    }
    let center = UNUserNotificationCenter::currentNotificationCenter();
    let (tx, rx) = mpsc::sync_channel(1);
    let callback = RcBlock::new(move |settings: NonNull<UNNotificationSettings>| {
        let status = unsafe { settings.as_ref() }.authorizationStatus();
        let _ = tx.send(status.0);
    });
    center.getNotificationSettingsWithCompletionHandler(&callback);
    let status = rx
        .recv_timeout(Duration::from_secs(5))
        .map_err(|_| timeout())?;
    if status == UNAuthorizationStatus::NotDetermined.0 && request {
        let (tx, rx) = mpsc::sync_channel(1);
        let callback = RcBlock::new(move |granted: Bool, error: *mut NSError| {
            let _ = tx.send((granted.as_bool(), error_message(error)));
        });
        center.requestAuthorizationWithOptions_completionHandler(
            UNAuthorizationOptions::Alert
                | UNAuthorizationOptions::Sound
                | UNAuthorizationOptions::Badge,
            &callback,
        );
        let (granted, error) = rx
            .recv_timeout(Duration::from_secs(120))
            .map_err(|_| timeout())?;
        if !error.is_empty() {
            return Err(AppError::new("NOTIFICATION_ERROR", &error));
        }
        return Ok(if granted { "granted" } else { "denied" }.into());
    }
    Ok(if status == UNAuthorizationStatus::NotDetermined.0 {
        "notDetermined"
    } else if status == UNAuthorizationStatus::Denied.0 {
        "denied"
    } else {
        "granted"
    }
    .into())
}

pub fn send(id: &str, title: &str, body: &str, task_id: Option<&str>) -> AppResult<()> {
    let content = UNMutableNotificationContent::new();
    content.setTitle(&NSString::from_str(title));
    content.setBody(&NSString::from_str(body));
    content.setSound(Some(&UNNotificationSound::defaultSound()));
    if let Some(task) = task_id {
        let info = NSDictionary::from_slices(
            &[&*NSString::from_str("taskId")],
            &[&*NSString::from_str(task)],
        );
        unsafe {
            // Erase NSDictionary's generic parameters; its NSString keys/values are objects.
            content.setUserInfo(&Retained::cast_unchecked::<NSDictionary>(info));
        }
    }
    let request = UNNotificationRequest::requestWithIdentifier_content_trigger(
        &NSString::from_str(id),
        &content,
        None,
    );
    let (tx, rx) = mpsc::sync_channel(1);
    let callback = RcBlock::new(move |error: *mut NSError| {
        let _ = tx.send(error_message(error));
    });
    UNUserNotificationCenter::currentNotificationCenter()
        .addNotificationRequest_withCompletionHandler(&request, Some(&callback));
    let error = rx
        .recv_timeout(Duration::from_secs(5))
        .map_err(|_| timeout())?;
    if error.is_empty() {
        Ok(())
    } else {
        Err(AppError::new("NOTIFICATION_ERROR", &error))
    }
}
