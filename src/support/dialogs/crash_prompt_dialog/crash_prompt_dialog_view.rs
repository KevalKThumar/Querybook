//! Template for the launch crash prompt.
//!
//! Author: Keval Thumar

use leptos::prelude::*;
use shared::support::CrashSummary;

use crate::shared::dialogs::dialog_shell::dialog_shell_component::DialogShell;

pub fn crash_prompt_dialog_view(
    open: RwSignal<bool>,
    summary: Signal<Option<CrashSummary>>,
    on_view: Callback<()>,
    on_report: Callback<()>,
    on_close: Callback<()>,
) -> impl IntoView {
    let label = move || summary.get().map(|s| s.kind.label().to_string()).unwrap_or_default();
    let when = move || summary.get().map(|s| s.time.replace('T', " ").replace('Z', " UTC")).unwrap_or_default();
    let title = move || summary.get().map(|s| s.title).unwrap_or_default();
    view! {
        <DialogShell open on_close>
            <div class="shell-header d-flex align-items-center gap-2">
                <i class="bi bi-exclamation-octagon crash-prompt-icon"></i>
                {label}
            </div>
            <div class="shell-body crash-prompt-body">
                <p>
                    "The last time Querybook ran, something went wrong. A report was saved on this computer; "
                    "nothing has been sent anywhere."
                </p>
                <p class="crash-prompt-safe">
                    <i class="bi bi-shield-check"></i>
                    " Changes you hadn't written were not saved to your databases, so your files are as you last wrote them."
                </p>
                <dl class="crash-prompt-facts">
                    <dt>"When"</dt>
                    <dd>{when}</dd>
                    <dt>"What"</dt>
                    <dd class="text-break">{title}</dd>
                </dl>
                <p class="crash-prompt-hint mb-0">
                    "Sending the report helps fix the problem. You'll see exactly what it contains first."
                </p>
            </div>
            <div class="shell-footer">
                <button class="btn btn-sm btn-outline-secondary me-auto" on:click=move |_| on_close.run(())>"Dismiss"</button>
                <button class="btn btn-sm btn-outline-secondary" on:click=move |_| on_view.run(())>"View Report"</button>
                <button class="btn btn-sm btn-primary" on:click=move |_| on_report.run(())>
                    <i class="bi bi-github"></i>" Report a Problem…"
                </button>
            </div>
        </DialogShell>
    }
}
