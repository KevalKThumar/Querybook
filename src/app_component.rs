use leptos::ev;
use leptos::prelude::*;

use crate::{
    app_route::AppRoutes,
    shared::common_structure::ThemeService,
    shared::components::toast::toast_component::ToastContainer,
    shared::services::toast_service::ToastService,
    support::dialogs::{
        about_dialog::about_dialog_component::AboutDialog, crash_prompt_dialog::crash_prompt_dialog_component::CrashPromptDialog,
        crash_reports_dialog::crash_reports_dialog_component::CrashReportsDialog,
        settings_dialog::settings_dialog_component::SettingsDialog, shortcuts_dialog::shortcuts_dialog_component::ShortcutsDialog,
        update_dialog::update_dialog_component::UpdateDialog,
    },
    support::components::update_banner::update_banner_component::UpdateBanner,
    support::support_structure::SupportService,
};

#[component]
pub fn App() -> impl IntoView {
    // Root service for toast notifications.
    let toast = ToastService::new();
    provide_context(toast);
    // Root service for the System / Light / Dark theme (shared by all windows).
    provide_context(ThemeService::new());
    // Root service for Help & support: its dialogs are mounted once, here.
    let support = SupportService::new();
    provide_context(support);
    // The updater's status, and the backend telling every window when it changes.
    support.watch_updates();

    // ⌘/ keyboard shortcuts · ⌘, settings — in every window, with or without a database.
    let shortcuts = window_event_listener(ev::keydown, move |e| {
        if !(e.meta_key() || e.ctrl_key()) {
            return;
        }
        match e.key().as_str() {
            "/" => support.shortcuts_open.set(true),
            "," => support.settings_open.set(true),
            _ => return,
        }
        e.prevent_default();
    });
    on_cleanup(move || shortcuts.remove());

    // Drawn: tell the blank-window watchdog, and offer last run's crash report.
    Effect::new(move || support.on_mounted(toast));

    view! {
        <ToastContainer />
        <AppRoutes />
        <CrashPromptDialog />
        <CrashReportsDialog />
        <AboutDialog />
        <ShortcutsDialog />
        <SettingsDialog />
        <UpdateDialog />
        <UpdateBanner />
    }
}
