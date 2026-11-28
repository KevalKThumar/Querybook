//! CrashPromptDialog — "Querybook quit unexpectedly", shown once at launch when
//! an earlier run left a report.
//!
//! Author: Keval Thumar

use leptos::prelude::*;
use shared::support::SupportLink;

use super::crash_prompt_dialog_view::crash_prompt_dialog_view;
use crate::shared::services::toast_service::ToastService;
use crate::support::support_structure::SupportService;

#[component]
pub fn CrashPromptDialog() -> impl IntoView {
    let support = expect_context::<SupportService>();
    let toast = expect_context::<ToastService>();
    let summary = support.crash_prompt;
    // DialogShell takes a bool signal; it follows `summary`.
    let open = RwSignal::new(false);
    Effect::new(move || open.set(summary.with(Option::is_some)));

    let on_close = Callback::new(move |_: ()| summary.set(None));
    let on_view = Callback::new(move |_: ()| {
        let id = summary.get_untracked().map(|s| s.id);
        summary.set(None);
        support.show_reports(id);
    });
    let on_report = Callback::new(move |_: ()| {
        let id = summary.get_untracked().map(|s| s.id);
        summary.set(None);
        SupportService::open_link(SupportLink::NewIssue { report_id: id }, toast);
    });

    crash_prompt_dialog_view(open, summary.into(), on_view, on_report, on_close)
}
