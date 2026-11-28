//! AboutDialog — version, system, licence and privacy, with links.
//!
//! Author: Keval Thumar

use leptos::prelude::*;
use leptos::task::spawn_local;
use shared::support::{AppInfo, SupportLink};

use super::about_dialog_view::about_dialog_view;
use crate::shared::services::clipboard_service::ClipboardService;
use crate::shared::services::toast_service::ToastService;
use crate::support::support_structure::SupportService;

#[component]
pub fn AboutDialog() -> impl IntoView {
    let support = expect_context::<SupportService>();
    let toast = expect_context::<ToastService>();
    let open = support.about_open;
    let info = RwSignal::new(None::<AppInfo>);

    Effect::new(move || {
        if open.get() && info.with_untracked(Option::is_none) {
            spawn_local(async move { info.set(SupportService::app_info(toast).await) });
        }
    });

    let on_close = Callback::new(move |_: ()| open.set(false));
    let on_link = Callback::new(move |link: SupportLink| SupportService::open_link(link, toast));
    // For pasting into an issue or email.
    let on_copy = Callback::new(move |_: ()| {
        if let Some(i) = info.get_untracked() {
            let text = format!("Querybook {}\n{} ({})\nWebView {}", i.version, i.os, i.arch, i.webview);
            spawn_local(async move {
                if ClipboardService::write_text(&text).await {
                    toast.success("Version details copied.");
                }
            });
        }
    });

    about_dialog_view(open, info.into(), on_close, on_link, on_copy)
}
