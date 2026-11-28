//! Template for the Settings dialog.
//!
//! Author: Keval Thumar

use leptos::prelude::*;

use shared::support::UpdateStatus;

use crate::shared::common_structure::ThemeMode;
use crate::shared::dialogs::dialog_shell::dialog_shell_component::DialogShell;

#[allow(clippy::too_many_arguments)]
pub fn settings_dialog_view(
    open: RwSignal<bool>,
    theme: Signal<ThemeMode>,
    update: Signal<UpdateStatus>,
    checking: Signal<bool>,
    on_close: Callback<()>,
    on_theme: Callback<ThemeMode>,
    on_auto_update: Callback<bool>,
    on_check: Callback<()>,
    on_logs: Callback<()>,
    on_reports: Callback<()>,
) -> impl IntoView {
    view! {
        <DialogShell open on_close class="settings-dialog">
            <div class="shell-header">"Settings"</div>
            <div class="shell-body">
                <section class="settings-section">
                    <div class="settings-title">"Appearance"</div>
                    <div class="btn-group settings-theme" role="radiogroup" aria-label="Theme">
                        {[ThemeMode::System, ThemeMode::Light, ThemeMode::Dark]
                            .into_iter()
                            .map(|m| view! {
                                <button
                                    class="btn btn-sm btn-outline-secondary"
                                    class:active=move || theme.get() == m
                                    role="radio"
                                    aria-checked=move || (theme.get() == m).to_string()
                                    on:click=move |_| on_theme.run(m)
                                >
                                    <i class=m.icon()></i>" "{m.label()}
                                </button>
                            })
                            .collect_view()}
                    </div>
                </section>

                <section class="settings-section">
                    <div class="settings-title">"Updates"</div>
                    <Show
                        when=move || update.with(|s| s.enabled)
                        fallback=move || view! {
                            <p class="settings-hint mb-0">
                                {move || format!(
                                    "Querybook {}: this build doesn't update itself. New versions are on the download page.",
                                    update.with(|s| s.current_version.clone())
                                )}
                            </p>
                        }
                    >
                        <div class="form-check form-switch">
                            <input
                                id="settings-auto-update"
                                class="form-check-input"
                                type="checkbox"
                                role="switch"
                                prop:checked=move || update.with(|s| s.auto_check == Some(true))
                                on:change=move |ev| on_auto_update.run(event_target_checked(&ev))
                            />
                            <label class="form-check-label" for="settings-auto-update">"Check for updates automatically"</label>
                        </div>
                        <p class="settings-hint">
                            "Once a day Querybook asks GitHub whether a newer version exists. Nothing about you or your databases is sent."
                        </p>
                        <div class="d-flex align-items-center gap-2">
                            <button class="btn btn-sm btn-outline-secondary" disabled=move || checking.get() on:click=move |_| on_check.run(())>
                                <i class="bi bi-arrow-repeat"></i>
                                {move || if checking.get() { " Checking…" } else { " Check Now" }}
                            </button>
                            <span class="settings-hint mb-0">
                                {move || update.with(|s| format!("You have Querybook {}.", s.current_version))}
                            </span>
                        </div>
                    </Show>
                </section>

                <section class="settings-section">
                    <div class="settings-title">"Problems & privacy"</div>
                    <p class="settings-hint">
                        "When something goes wrong, Querybook keeps a crash report on this computer. "
                        "Nothing is sent unless you choose to share it."
                    </p>
                    <div class="d-flex gap-2">
                        <button class="btn btn-sm btn-outline-secondary" on:click=move |_| on_reports.run(())>
                            <i class="bi bi-journal-medical"></i>" Crash Reports…"
                        </button>
                        <button class="btn btn-sm btn-outline-secondary" on:click=move |_| on_logs.run(())>
                            <i class="bi bi-folder2"></i>" Open Logs Folder"
                        </button>
                    </div>
                </section>
            </div>
            <div class="shell-footer">
                <button class="btn btn-sm btn-primary" on:click=move |_| on_close.run(())>"Done"</button>
            </div>
        </DialogShell>
    }
}
