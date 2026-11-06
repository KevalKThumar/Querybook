//! LocalStorageService
//!
//! Thin wrapper over `window.localStorage`. Every Querybook window shares the
//! same origin, so values written here are seen by all windows. Failures are
//! ignored: storage is a convenience, never required.
//!
//! Author: Keval Thumar

use leptos::prelude::window;

pub struct LocalStorageService;

impl LocalStorageService {
    fn storage() -> Option<web_sys::Storage> {
        window().local_storage().ok().flatten()
    }

    pub fn get(key: &str) -> Option<String> {
        Self::storage()?.get_item(key).ok().flatten()
    }

    pub fn set(key: &str, value: &str) {
        if let Some(storage) = Self::storage() {
            let _ = storage.set_item(key, value);
        }
    }
}
