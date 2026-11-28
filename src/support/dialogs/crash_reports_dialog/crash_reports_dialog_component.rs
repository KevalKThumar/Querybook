//! CrashReportsDialog — the reports kept on this computer, newest first, and
//! everything in the selected one. Shows exactly what would be shared; nothing
//! leaves the computer unless the user copies, saves or reports it.
//!
//! Author: Keval Thumar

use leptos::prelude::*;
use leptos::task::spawn_local;
use shared::support::{CrashReport, CrashSummary, SupportLink, report_text};

use super::crash_reports_dialog_view::crash_reports_dialog_view;
use crate::shared::services::clipboard_service::ClipboardService;
use crate::shared::services::toast_service::ToastService;
use crate::support::support_structure::SupportService;

#[component]
pub fn CrashReportsDialog() -> impl IntoView {
    let support = expect_context::<SupportService>();
    let toast = expect_context::<ToastService>();
    let open = support.reports_open;
    let list = RwSignal::new(Vec::<CrashSummary>::new());
    let selected = RwSignal::new(None::<String>);
    let detail = RwSignal::new(None::<CrashReport>);
    let loading = RwSignal::new(false);

    let reload = move |prefer: Option<String>| {
        loading.set(true);
        spawn_local(async move {
            let items = SupportService::list_reports(toast).await;
            let pick = prefer.filter(|id| items.iter().any(|s| &s.id == id)).or_else(|| items.first().map(|s| s.id.clone()));
            list.set(items);
            selected.set(pick);
            loading.set(false);
        });
    };

    // Opening the dialog (re)reads the folder.
    Effect::new(move || {
        if open.get() {
            reload(support.reports_select.get_untracked());
        } else {
            detail.set(None);
        }
    });

    // The selected report's contents.
    Effect::new(move || match selected.get() {
        Some(id) => spawn_local(async move {
            let r = SupportService::report(&id, toast).await;
            if selected.get_untracked().as_deref() == Some(id.as_str()) {
                detail.set(r);
            }
        }),
        None => detail.set(None),
    });

    let on_select = Callback::new(move |id: String| selected.set(Some(id)));
    let on_close = Callback::new(move |_: ()| open.set(false));
    let on_copy = Callback::new(move |_: ()| {
        if let Some(r) = detail.get_untracked() {
            let text = report_text(&r);
            spawn_local(async move {
                if ClipboardService::write_text(&text).await {
                    toast.success("Report copied.");
                } else {
                    toast.error("Couldn't copy the report.");
                }
            });
        }
    });
    let on_save = Callback::new(move |_: ()| {
        if let Some(id) = selected.get_untracked() {
            spawn_local(async move {
                if SupportService::save_report(&id, toast).await.is_some() {
                    toast.success("Report saved. Attach it to your issue or email.");
                }
            });
        }
    });
    let on_issue = Callback::new(move |_: ()| {
        SupportService::open_link(SupportLink::NewIssue { report_id: selected.get_untracked() }, toast);
    });
    let on_email = Callback::new(move |_: ()| {
        SupportService::open_link(SupportLink::Email { report_id: selected.get_untracked() }, toast);
    });
    let on_reveal = Callback::new(move |_: ()| {
        if let Some(id) = selected.get_untracked() {
            spawn_local(async move { SupportService::reveal_report(&id, toast).await });
        }
    });
    let on_delete = Callback::new(move |_: ()| {
        if let Some(id) = selected.get_untracked() {
            spawn_local(async move {
                SupportService::delete_reports(Some(vec![id]), toast).await;
                reload(None);
            });
        }
    });
    let on_delete_all = Callback::new(move |_: ()| {
        spawn_local(async move {
            SupportService::delete_reports(None, toast).await;
            reload(None);
        });
    });
    let on_logs = Callback::new(move |_: ()| SupportService::open_logs_folder(toast));

    crash_reports_dialog_view(
        open,
        list.into(),
        selected.into(),
        detail.into(),
        loading.into(),
        on_select,
        on_close,
        on_copy,
        on_save,
        on_issue,
        on_email,
        on_reveal,
        on_delete,
        on_delete_all,
        on_logs,
    )
}
