//! Template for the database info tab.
//!
//! Author: Keval Thumar

use leptos::prelude::*;

use crate::database::database_constant::MAINTENANCE_ACTIONS;

#[allow(clippy::too_many_arguments)]
pub fn database_info_view(
    facts: Signal<Vec<(&'static str, String)>>,
    fk_on: Signal<Option<bool>>,
    in_txn: Signal<bool>,
    running: Signal<Option<&'static str>>,
    report: Signal<Option<(bool, String, Vec<String>)>>,
    on_action: Callback<(&'static str, &'static str, bool)>,
    on_stop: Callback<()>,
    on_toggle_fk: Callback<()>,
) -> impl IntoView {
    view! {
        <div class="dbinfo h-100 overflow-auto">
            <div class="dbinfo-content">
                <h6 class="dbinfo-heading">"Database"</h6>
                <dl class="dbinfo-facts">
                    <For
                        each=move || facts.get()
                        key=|(label, value)| (*label, value.clone())
                        children=|(label, value)| view! {
                            <dt>{label}</dt>
                            <dd title=value.clone()>{value.clone()}</dd>
                        }
                    />
                </dl>
                <label class="form-check form-switch dbinfo-switch">
                    <input
                        class="form-check-input"
                        type="checkbox"
                        role="switch"
                        prop:checked=move || fk_on.get().unwrap_or(false)
                        disabled=move || in_txn.get() || fk_on.get().is_none()
                        on:change=move |_| on_toggle_fk.run(())
                    />
                    <span class="form-check-label">"Enforce foreign keys"</span>
                    <div class="dbinfo-note">"For this window only; back on whenever the database is reopened."</div>
                </label>

                <h6 class="dbinfo-heading">"Maintenance"</h6>
                <Show when=move || in_txn.get() fallback=|| ()>
                    <div class="dbinfo-warning">
                        <i class="bi bi-exclamation-circle"></i>" Write or revert your changes to optimize, vacuum or toggle foreign keys."
                    </div>
                </Show>
                <div class="dbinfo-actions">
                    {MAINTENANCE_ACTIONS
                        .into_iter()
                        .map(|(label, icon, sql, needs_clean, about)| view! {
                            <button
                                class="dbinfo-action"
                                disabled=move || running.get().is_some() || (needs_clean && in_txn.get())
                                on:click=move |_| on_action.run((label, sql, needs_clean))
                            >
                                <i class=icon></i>
                                <span class="dbinfo-action-label">{label}</span>
                                <span class="dbinfo-action-about">{about}</span>
                            </button>
                        })
                        .collect_view()}
                </div>

                <Show when=move || running.get().is_some() fallback=|| ()>
                    <div class="dbinfo-running d-flex align-items-center gap-2">
                        <span class="spinner-border spinner-border-sm"></span>
                        <span>{move || format!("{}…", running.get().unwrap_or_default())}</span>
                        <button class="btn btn-sm btn-outline-danger ms-2" on:click=move |_| on_stop.run(())>"Stop"</button>
                    </div>
                </Show>
                {move || report.get().map(|(ok, heading, lines)| view! {
                    <div class="dbinfo-report" class:failed=!ok>
                        <div class="dbinfo-report-heading">
                            <i class=if ok { "bi bi-check-circle-fill" } else { "bi bi-exclamation-triangle-fill" }></i>
                            " "{heading}
                        </div>
                        <Show when={let empty = lines.is_empty(); move || !empty} fallback=|| ()>
                            <pre class="dbinfo-lines">{lines.join("\n")}</pre>
                        </Show>
                    </div>
                })}
            </div>
        </div>
    }
}
