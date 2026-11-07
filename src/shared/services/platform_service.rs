//! Which OS the app runs on, for the few places that read differently per
//! platform (keyboard-shortcut hints).

use std::cell::OnceCell;

thread_local! {
    static IS_MAC: OnceCell<bool> = const { OnceCell::new() };
}

/// Static platform service: unit struct, static methods.
pub struct PlatformService;

impl PlatformService {
    /// True in the macOS WebView (WKWebView reports "Macintosh").
    pub fn is_mac() -> bool {
        IS_MAC.with(|cell| {
            *cell.get_or_init(|| {
                web_sys::window()
                    .and_then(|w| w.navigator().user_agent().ok())
                    .is_some_and(|ua| ua.contains("Mac"))
            })
        })
    }

    /// A hint written in Mac notation ("Copy (⇧⌘C)"), as this platform reads it.
    pub fn keys(text: &str) -> String {
        shared::keys::shortcut_text(text, Self::is_mac())
    }
}
