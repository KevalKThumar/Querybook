//! SettingsDialog — app-wide preferences (⌘,).
//!
//! Author: Keval Thumar

use leptos::prelude::*;

use super::settings_dialog_view::settings_dialog_view;
use crate::shared::common_structure::{ThemeMode, ThemeService};
use crate::shared::services::toast_service::ToastService;
use crate::support::support_structure::SupportService;

#[component]
pub fn SettingsDialog() -> impl IntoView {
    let support = expect_context::<SupportService>();
    let theme = expect_context::<ThemeService>();
    let toast = expect_context::<ToastService>();
    let open = support.settings_open;

    let on_close = Callback::new(move |_: ()| open.set(false));
    let on_theme = Callback::new(move |m: ThemeMode| theme.set(m));
    let on_logs = Callback::new(move |_: ()| SupportService::open_logs_folder(toast));
    let on_reports = Callback::new(move |_: ()| {
        open.set(false);
        support.show_reports(None);
    });

    let on_auto_update = Callback::new(move |on: bool| SupportService::set_auto_update(on, toast));
    let on_check = Callback::new(move |_: ()| support.check_now(toast));

    settings_dialog_view(
        open,
        theme.mode.into(),
        support.update.into(),
        support.update_checking.into(),
        on_close,
        on_theme,
        on_auto_update,
        on_check,
        on_logs,
        on_reports,
    )
}
