// Set the app's native UI language before Tauri initializes AppKit/WebView.
// The frontend locale alone cannot localize operating-system file panels.
pub fn initialize() {
    #[cfg(target_os = "macos")]
    macos::initialize();
    #[cfg(target_os = "windows")]
    windows::initialize();
}

#[cfg(target_os = "macos")]
mod macos {
    use objc2_foundation::{
        ns_string, NSArgumentDomain, NSArray, NSMutableDictionary, NSUserDefaults,
    };

    pub(super) fn initialize() {
        objc2::rc::autoreleasepool(|_| {
            let defaults = NSUserDefaults::standardUserDefaults();
            // NSArgumentDomain has priority over the user's global AppleLanguages.
            // Keep existing launch arguments and override only this process's language.
            // No setObject/synchronize: nothing is written to user or system preferences.
            let domain = unsafe { NSArgumentDomain };
            let arguments = defaults.volatileDomainForName(domain);
            let arguments = NSMutableDictionary::dictionaryWithDictionary(&arguments);
            let languages = NSArray::from_slice(&[ns_string!("zh-Hans")]);
            arguments.insert(ns_string!("AppleLanguages"), &languages);
            // SAFETY: NSString keys and an NSArray<NSString> are valid preference values.
            unsafe { defaults.setVolatileDomain_forName(&arguments, domain) };
        });
    }
}

#[cfg(target_os = "windows")]
mod windows {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn SetProcessPreferredUILanguages(
            flags: u32,
            languages: *const u16,
            count: *mut u32,
        ) -> i32;
    }

    pub(super) fn initialize() {
        const MUI_LANGUAGE_NAME: u32 = 0x8;
        // A double-null-terminated language list. System-owned controls still
        // require the corresponding Windows language resources to be installed.
        let languages: Vec<u16> = "zh-CN\0\0".encode_utf16().collect();
        let mut count = 0;
        // SAFETY: valid UTF-16 MULTI_SZ and writable count, used only for this process.
        let result = unsafe {
            SetProcessPreferredUILanguages(MUI_LANGUAGE_NAME, languages.as_ptr(), &mut count)
        };
        if result == 0 {
            eprintln!("无法设置原生界面中文偏好，将使用系统可用语言");
        }
    }
}
