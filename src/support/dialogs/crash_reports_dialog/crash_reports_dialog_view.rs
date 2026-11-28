//! Template for the Crash Reports viewer: list on the left, report on the right.
//!
//! Author: Keval Thumar

use leptos::prelude::*;
use shared::support::{CrashReport, CrashSummary, ReportKind, SUPPORT_EMAIL};

use crate::shared::dialogs::dialog_shell::dialog_shell_component::DialogShell;

fn kind_icon(kind: ReportKind) -> &'static str {
    match kind {
        ReportKind::BackendPanic => "bi bi-x-octagon",
        ReportKind::FrontendPanic | ReportKind::FrontendError => "bi bi-window-x",
        ReportKind::BlankWindow => "bi bi-window",
        ReportKind::UncleanExit => "bi bi-power",
    }
}

fn short_time(t: &str) -> String {
    t.get(..16).unwrap_or(t).replace('T', " ")
}

#[allow(clippy::too_many_arguments)]
pub fn crash_reports_dialog_view(
    open: RwSignal<bool>,
    list: Signal<Vec<CrashSummary>>,
    selected: Signal<Option<String>>,
    detail: Signal<Option<CrashReport>>,
    loading: Signal<bool>,
    on_select: Callback<String>,
    on_close: Callback<()>,
    on_copy: Callback<()>,
    on_save: Callback<()>,
    on_issue: Callback<()>,
    on_email: Callback<()>,
    on_reveal: Callback<()>,
    on_delete: Callback<()>,
    on_delete_all: Callback<()>,
    on_logs: Callback<()>,
) -> impl IntoView {
    let empty = move || list.with(Vec::is_empty);
    let none_selected = move || detail.with(Option::is_none);
    view! {
        <DialogShell open on_close class="shell-dialog--wide crash-reports-dialog">
            <div class="shell-header d-flex align-items-center">
                <span class="me-auto">"Crash Reports"</span>
                <span class="crash-reports-local"><i class="bi bi-pc-display"></i>" Kept on this computer"</span>
            </div>
            <div class="crash-reports-body">
                <Show
                    when=move || !empty()
                    fallback=move || view! {
                        <div class="crash-reports-empty">
                            <i class="bi bi-emoji-smile"></i>
                            <div>{move || if loading.get() { "Reading reports…" } else { "No crash reports." }}</div>
                            <div class="crash-reports-empty-hint">
                                "When something goes wrong, Querybook saves a report here. It stays on this computer until you choose to share it."
                            </div>
                        </div>
                    }
                >
                    <div class="crash-reports-list" role="listbox" aria-label="Crash reports">
                        <For
                            each=move || list.get()
                            key=|s| s.id.clone()
                            children=move |s| {
                                let id = s.id.clone();
                                let is_selected = {
                                    let id = id.clone();
                                    move || selected.get().as_deref() == Some(id.as_str())
                                };
                                view! {
                                    <button
                                        class="crash-reports-item"
                                        class:active=is_selected.clone()
                                        role="option"
                                        aria-selected=move || is_selected().to_string()
                                        on:click=move |_| on_select.run(id.clone())
                                    >
                                        <i class=kind_icon(s.kind)></i>
                                        <span class="crash-reports-item-text">
                                            <span class="crash-reports-item-title">{s.kind.label()}</span>
                                            <span class="crash-reports-item-sub">{format!("{} · {}", short_time(&s.time), s.version)}</span>
                                        </span>
                                    </button>
                                }
                            }
                        />
                    </div>
                    <div class="crash-reports-detail">
                        {move || detail.get().map(|r| view! {
                            <div class="crash-reports-head">
                                <div class="crash-reports-title">{r.kind.label()}</div>
                                <div class="crash-reports-message">{r.message.clone()}</div>
                            </div>
                            <dl class="crash-reports-facts">
                                <dt>"When"</dt><dd>{r.time.replace('T', " ").replace('Z', " UTC")}</dd>
                                <dt>"Querybook"</dt><dd>{r.version.clone()}</dd>
                                <dt>"System"</dt><dd>{format!("{} ({})", r.os, r.arch)}</dd>
                                {r.webview.clone().map(|w| view! { <dt>"WebView"</dt><dd>{w}</dd> })}
                                {r.location.clone().map(|l| view! { <dt>"Where"</dt><dd>{l}</dd> })}
                            </dl>
                            {(!r.backtrace.is_empty()).then(|| view! {
                                <div class="crash-reports-section">"Backtrace"</div>
                                <pre class="crash-reports-pre">{r.backtrace.join("\n")}</pre>
                            })}
                            {(!r.log_tail.is_empty()).then(|| view! {
                                <div class="crash-reports-section">"Recent log"</div>
                                <pre class="crash-reports-pre">{r.log_tail.join("\n")}</pre>
                            })}
                            <p class="crash-reports-privacy">
                                <i class="bi bi-shield-lock"></i>
                                " Reports never contain your SQL, data, passwords or file names. This is everything a report holds."
                            </p>
                        })}
                    </div>
                </Show>
            </div>
            <div class="shell-footer flex-wrap">
                <button class="btn btn-sm btn-outline-danger" disabled=empty on:click=move |_| on_delete_all.run(())>"Delete All"</button>
                <button class="btn btn-sm btn-outline-secondary" on:click=move |_| on_logs.run(())>
                    <i class="bi bi-folder2"></i>" Logs Folder"
                </button>
                <div class="me-auto"></div>
                <button class="btn btn-sm btn-outline-secondary" title="Delete this report" disabled=none_selected on:click=move |_| on_delete.run(())>
                    <i class="bi bi-trash"></i>
                </button>
                <button class="btn btn-sm btn-outline-secondary" title="Show the file" disabled=none_selected on:click=move |_| on_reveal.run(())>
                    <i class="bi bi-folder2-open"></i>
                </button>
                <button class="btn btn-sm btn-outline-secondary" disabled=none_selected on:click=move |_| on_copy.run(())>
                    <i class="bi bi-clipboard"></i>" Copy"
                </button>
                <button class="btn btn-sm btn-outline-secondary" disabled=none_selected on:click=move |_| on_save.run(())>
                    <i class="bi bi-download"></i>" Save…"
                </button>
                {SUPPORT_EMAIL.map(|_| view! {
                    <button class="btn btn-sm btn-outline-secondary" disabled=none_selected on:click=move |_| on_email.run(())>
                        <i class="bi bi-envelope"></i>" Email…"
                    </button>
                })}
                <button class="btn btn-sm btn-primary" disabled=none_selected on:click=move |_| on_issue.run(())>
                    <i class="bi bi-github"></i>" Report on GitHub…"
                </button>
                <button class="btn btn-sm btn-outline-secondary" on:click=move |_| on_close.run(())>"Close"</button>
            </div>
        </DialogShell>
    }
}
