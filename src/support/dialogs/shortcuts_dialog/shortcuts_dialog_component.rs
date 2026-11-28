//! ShortcutsDialog — every keyboard shortcut, as this platform writes them.
//!
//! Author: Keval Thumar

use leptos::prelude::*;

use super::shortcuts_dialog_view::shortcuts_dialog_view;
use crate::shared::services::platform_service::PlatformService;
use crate::support::support_constant::SHORTCUTS;
use crate::support::support_structure::SupportService;

#[component]
pub fn ShortcutsDialog() -> impl IntoView {
    let support = expect_context::<SupportService>();
    let open = support.shortcuts_open;
    // (section, [(keys, action)]) in the order the list gives them.
    let mut sections: Vec<(&'static str, Vec<(String, &'static str)>)> = Vec::new();
    for (section, keys, action) in SHORTCUTS {
        let row = (PlatformService::keys(keys), *action);
        match sections.last_mut() {
            Some((s, rows)) if s == section => rows.push(row),
            _ => sections.push((section, vec![row])),
        }
    }
    let on_close = Callback::new(move |_: ()| open.set(false));
    shortcuts_dialog_view(open, sections, on_close)
}
