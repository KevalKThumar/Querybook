//! Welcome — shown in a window that has no database yet: Open / New / Recent.
//! Route: `/database/welcome`.
//!
//! Author: Keval Thumar

use leptos::ev;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::hooks::use_navigate;
use leptos_router::NavigateOptions;
use shared::OpenOutcome;

use super::welcome_view::welcome_view;
use crate::database::database_constant::ROUTE_BROWSER;
use crate::database::database_structure::DatabaseService;
use crate::shared::common_structure::ThemeService;
use crate::shared::services::toast_service::ToastService;

// Keval Thumar
/// Empty-window start screen. Opening a file binds it to this window and
/// switches to the browser feature.
#[component]
pub fn Welcome() -> impl IntoView {
    let toast = expect_context::<ToastService>();
    let theme = expect_context::<ThemeService>();
    let navigate = use_navigate();

    let recent = RwSignal::new(DatabaseService::recent_files());
    // (path, file name, folder) for the list.
    let recent_items = Signal::derive(move || {
        recent.get().into_iter().map(|path| {
            let (dir, name) = path.rsplit_once(['/', '\\']).unwrap_or(("", path.as_str()));
            (path.clone(), name.to_string(), dir.to_string())
        }).collect::<Vec<_>>()
    });

    // Navigate from an Effect, never synchronously inside the router's own update.
    let go_browser = RwSignal::new(false);
    Effect::new(move |_| {
        if go_browser.get() {
            navigate(ROUTE_BROWSER, NavigateOptions::default());
        }
    });

    let on_open = Callback::new(move |create: bool| {
        spawn_local(async move {
            if DatabaseService::open_dialog(create, toast).await == OpenOutcome::ThisWindow {
                go_browser.set(true);
            }
        });
    });

    let on_open_recent = Callback::new(move |path: String| {
        spawn_local(async move {
            let outcome = DatabaseService::open_path(&path, toast).await;
            recent.set(DatabaseService::recent_files());
            if outcome == OpenOutcome::ThisWindow {
                go_browser.set(true);
            }
        });
    });

    let on_remove_recent = Callback::new(move |path: String| {
        DatabaseService::remove_recent(&path);
        recent.set(DatabaseService::recent_files());
    });

    let on_theme = Callback::new(move |_: ()| theme.cycle());

    // ⌘O / ⌘N (Ctrl on Windows/Linux)
    let shortcuts = window_event_listener(ev::keydown, move |e| {
        if !(e.meta_key() || e.ctrl_key()) {
            return;
        }
        match e.key().as_str() {
            "o" => {
                e.prevent_default();
                on_open.run(false);
            }
            "n" => {
                e.prevent_default();
                on_open.run(true);
            }
            _ => {}
        }
    });
    on_cleanup(move || shortcuts.remove());

    welcome_view(recent_items, theme.mode.into(), on_open, on_open_recent, on_remove_recent, on_theme)
}
