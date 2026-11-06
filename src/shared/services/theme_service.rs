//! ThemeService
//!
//! System / Light / Dark. The choice is saved in localStorage and applied as
//! Bootstrap's `data-bs-theme` on `<html>`; "System" follows the OS setting
//! live. Other Querybook windows pick up a change through the `storage` event.
//!
//! ```
//! let theme = expect_context::<ThemeService>();
//! theme.cycle();
//! ```
//!
//! Author: Keval Thumar

use leptos::ev;
use leptos::prelude::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use crate::shared::common_structure::{ThemeMode, ThemeService};
use crate::shared::services::local_storage_service::LocalStorageService;

pub const STORAGE_THEME: &str = "querybook.theme";
const DARK_QUERY: &str = "(prefers-color-scheme: dark)";

impl Default for ThemeService {
    fn default() -> Self {
        Self::new()
    }
}

impl ThemeService {
    pub fn new() -> Self {
        let mode = RwSignal::new(ThemeMode::parse(LocalStorageService::get(STORAGE_THEME).as_deref()));
        apply(mode.get_untracked());

        // Another window changed the theme.
        let _ = window_event_listener(ev::storage, move |e| {
            if e.key().as_deref() == Some(STORAGE_THEME) {
                let m = ThemeMode::parse(e.new_value().as_deref());
                apply(m);
                mode.set(m);
            }
        });

        // The OS switched light/dark while we follow it.
        if let Ok(Some(query)) = window().match_media(DARK_QUERY) {
            let on_change = Closure::<dyn Fn()>::new(move || {
                if mode.get_untracked() == ThemeMode::System {
                    apply(ThemeMode::System);
                }
            });
            let _ = query.add_event_listener_with_callback("change", on_change.as_ref().unchecked_ref());
            // ponytail: app-lifetime listener, intentionally never removed.
            on_change.forget();
        }

        Self { mode }
    }

    /// System → Light → Dark → System.
    pub fn cycle(&self) {
        let m = self.mode.get_untracked().next();
        apply(m);
        LocalStorageService::set(STORAGE_THEME, m.as_str());
        self.mode.set(m);
    }
}

fn apply(mode: ThemeMode) {
    let dark = match mode {
        ThemeMode::System => window().match_media(DARK_QUERY).ok().flatten().is_some_and(|q| q.matches()),
        ThemeMode::Light => false,
        ThemeMode::Dark => true,
    };
    if let Some(root) = document().document_element() {
        let _ = root.set_attribute("data-bs-theme", if dark { "dark" } else { "light" });
    }
}
