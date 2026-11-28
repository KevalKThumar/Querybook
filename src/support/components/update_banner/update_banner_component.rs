//! UpdateBanner — a small card in the corner that asks once whether to check for
//! updates automatically, and later says when a new version is available.
//! It never blocks the window.
//!
//! Author: Keval Thumar

use leptos::prelude::*;

use super::update_banner_view::update_banner_view;
use crate::shared::services::toast_service::ToastService;
use crate::support::support_structure::SupportService;

#[component]
pub fn UpdateBanner() -> impl IntoView {
    let support = expect_context::<SupportService>();
    let toast = expect_context::<ToastService>();
    let status = support.update;

    let ask = Signal::derive(move || status.with(|s| s.enabled && s.auto_check.is_none()));
    let available = Signal::derive(move || {
        if support.update_open.get() || support.update_progress.with(Option::is_some) {
            None
        } else {
            status.with(|s| s.available.as_ref().map(|a| a.version.clone()))
        }
    });

    let on_allow = Callback::new(move |_: ()| SupportService::set_auto_update(true, toast));
    let on_deny = Callback::new(move |_: ()| SupportService::set_auto_update(false, toast));
    let on_details = Callback::new(move |_: ()| support.update_open.set(true));
    let on_later = Callback::new(move |_: ()| support.dismiss_update(false, toast));

    update_banner_view(ask, available, on_allow, on_deny, on_details, on_later)
}
