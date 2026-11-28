//! UpdateDialog — "Querybook X is available": the release notes, then Update Now
//! (download, install, restart), Later, or Skip This Version. Installs that their
//! package manager updates (deb, rpm) get a Download button instead.
//!
//! Author: Keval Thumar

use leptos::prelude::*;

use super::update_dialog_view::update_dialog_view;
use crate::shared::services::toast_service::ToastService;
use crate::support::support_structure::SupportService;

#[component]
pub fn UpdateDialog() -> impl IntoView {
    let support = expect_context::<SupportService>();
    let toast = expect_context::<ToastService>();
    let open = support.update_open;
    let status = support.update;
    let progress = support.update_progress;

    // While it installs, the dialog can't be closed (the app restarts at the end).
    let busy = Signal::derive(move || progress.with(Option::is_some));
    let on_close = Callback::new(move |_: ()| {
        if !busy.get_untracked() {
            open.set(false);
        }
    });
    let on_install = Callback::new(move |_: ()| support.install_update(toast));
    let on_later = Callback::new(move |_: ()| support.dismiss_update(false, toast));
    let on_skip = Callback::new(move |_: ()| support.dismiss_update(true, toast));
    let on_download = Callback::new(move |_: ()| {
        let version = status.with_untracked(|s| s.available.as_ref().map(|a| a.version.clone())).unwrap_or_default();
        SupportService::open_release_page(version, toast);
    });

    update_dialog_view(open, status.into(), progress.into(), busy, on_close, on_install, on_later, on_skip, on_download)
}
