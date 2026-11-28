//! HelpMenu — the "?" button in the toolbar and on the Welcome screen.
//!
//! Opens a menu of Help & support actions under the button. The dialogs live in
//! `App` (see `SupportService`), so this only flips their signals.
//!
//! Author: Keval Thumar

use leptos::prelude::*;
use shared::support::{SUPPORT_EMAIL, SupportLink};
use wasm_bindgen::JsCast;

use super::help_menu_view::help_menu_view;
use crate::shared::common_structure::MenuItem;
use crate::shared::services::toast_service::ToastService;
use crate::support::support_structure::SupportService;

#[component]
pub fn HelpMenu(
    /// Extra classes for the button (the toolbar and Welcome screen style it differently).
    #[prop(into, optional)]
    class: String,
    /// Show "Help" next to the icon.
    #[prop(optional)]
    labelled: bool,
) -> impl IntoView {
    let support = expect_context::<SupportService>();
    let toast = expect_context::<ToastService>();
    let menu = RwSignal::new(None::<(f64, f64)>);

    let on_toggle = Callback::new(move |ev: leptos::ev::MouseEvent| {
        if menu.get_untracked().is_some() {
            menu.set(None);
            return;
        }
        // Under the button, right-aligned to it when it sits at the window's right edge.
        let rect = ev
            .current_target()
            .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
            .map(|el| el.get_bounding_client_rect());
        let (x, y) = rect.map_or((ev.client_x() as f64, ev.client_y() as f64), |r| (r.right() - 230.0, r.bottom() + 4.0));
        menu.set(Some((x, y)));
    });

    let items = Signal::derive(move || {
        let mut items = vec![
            MenuItem::action("Keyboard Shortcuts", "bi bi-keyboard", Callback::new(move |_| support.shortcuts_open.set(true)))
                .shortcut("⌘/"),
            MenuItem::action("Settings…", "bi bi-gear", Callback::new(move |_| support.settings_open.set(true))).shortcut("⌘,"),
            MenuItem::Divider,
            MenuItem::action(
                "Report a Problem…",
                "bi bi-bug",
                Callback::new(move |_| SupportService::open_link(SupportLink::NewIssue { report_id: None }, toast)),
            ),
            MenuItem::action("Crash Reports…", "bi bi-journal-medical", Callback::new(move |_| support.show_reports(None))),
        ];
        // Only builds made with the updater offer it (not development builds).
        if support.update.with(|s| s.enabled) {
            let checking = support.update_checking.get();
            items.insert(
                2,
                MenuItem::action(
                    if checking { "Checking for Updates…" } else { "Check for Updates…" },
                    "bi bi-arrow-repeat",
                    Callback::new(move |_| support.check_now(toast)),
                )
                .enabled(!checking),
            );
        }
        if SUPPORT_EMAIL.is_some() {
            items.push(MenuItem::action(
                "Contact Support…",
                "bi bi-envelope",
                Callback::new(move |_| SupportService::open_link(SupportLink::Email { report_id: None }, toast)),
            ));
        }
        items.extend([
            MenuItem::action("Open Logs Folder", "bi bi-folder2", Callback::new(move |_| SupportService::open_logs_folder(toast))),
            MenuItem::Divider,
            MenuItem::action(
                "Querybook on GitHub",
                "bi bi-github",
                Callback::new(move |_| SupportService::open_link(SupportLink::Home, toast)),
            ),
            MenuItem::action("About Querybook", "bi bi-info-circle", Callback::new(move |_| support.about_open.set(true))),
        ]);
        items
    });

    help_menu_view(class, labelled, menu, items, on_toggle)
}
