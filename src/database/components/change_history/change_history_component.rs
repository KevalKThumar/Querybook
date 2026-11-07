//! ChangeHistory — the "History" tab: the last saves made to this database on
//! this device, each with its schema changes and the rows it inserted, updated
//! and deleted (old → new values). The backend records and keeps them.
//!
//! Author: Keval Thumar

use std::collections::HashSet;

use leptos::prelude::*;
use leptos::task::spawn_local;
use shared::history::HistoryPage;
use wasm_bindgen::JsValue;

use super::change_history_view::change_history_view;
use crate::database::database_constant::HISTORY_SHOWN;
use crate::database::database_structure::DatabaseService;
use crate::shared::services::toast_service::ToastService;

/// "today at 16:40", "yesterday at 09:05" or "7 Nov 2026 at 16:40", in local time.
pub fn saved_when(ms: i64) -> String {
    let at = js_sys::Date::new(&JsValue::from_f64(ms as f64));
    let now = js_sys::Date::new_0();
    let day = |d: &js_sys::Date| (d.get_full_year(), d.get_month(), d.get_date());
    let yesterday = js_sys::Date::new(&JsValue::from_f64(now.get_time() - 86_400_000.0));
    let time = format!("{:02}:{:02}", at.get_hours(), at.get_minutes());
    if day(&at) == day(&now) {
        format!("today at {time}")
    } else if day(&at) == day(&yesterday) {
        format!("yesterday at {time}")
    } else {
        let date: String = at.to_locale_date_string("default", &JsValue::UNDEFINED).into();
        format!("{date} at {time}")
    }
}

// Keval Thumar
#[component]
pub fn ChangeHistory(
    #[prop(into)] in_txn: Signal<bool>,
    #[prop(into)] data_version: Signal<u32>,
    #[prop(into)] encrypted: Signal<bool>,
    #[prop(into)] file_name: Signal<String>,
) -> impl IntoView {
    let toast = expect_context::<ToastService>();
    // None until the first load.
    let page = RwSignal::new(None::<HistoryPage>);
    // (save id, table) whose rows are all shown.
    let expanded = RwSignal::new(HashSet::<(i64, String)>::new());
    let confirm_open = RwSignal::new(false);

    let load = move || {
        spawn_local(async move {
            if let Some(p) = DatabaseService::change_history(HISTORY_SHOWN, toast).await {
                page.set(Some(p));
            }
        });
    };
    // On open, and whenever pending changes end: written (a new save) or reverted.
    Effect::new(move |_| {
        data_version.track();
        if !in_txn.get() {
            load();
        }
    });

    let on_expand = Callback::new(move |key: (i64, String)| expanded.update(|e| {
        e.insert(key);
    }));
    let on_clear = Callback::new(move |_: ()| confirm_open.set(true));
    let on_confirm_clear = Callback::new(move |_: ()| {
        confirm_open.set(false);
        spawn_local(async move {
            if DatabaseService::clear_change_history(toast).await {
                toast.success("Change history cleared.");
                load();
            }
        });
    });
    let on_cancel_clear = Callback::new(move |_: ()| confirm_open.set(false));
    let confirm_message = Signal::derive(move || {
        format!("Delete the saved change history of {}? This can't be undone. The database itself isn't changed.", file_name.get())
    });

    change_history_view(
        page.into(),
        expanded.into(),
        encrypted,
        confirm_open,
        confirm_message,
        on_expand,
        on_clear,
        on_confirm_clear,
        on_cancel_clear,
    )
}
