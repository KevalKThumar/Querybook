//! Template for the SQLCipher dialog.
//!
//! Author: Keval Thumar

use leptos::html::Input;
use leptos::prelude::*;

use crate::database::database_constant::CIPHER_DEFAULTS;
use crate::database::database_structure::CipherMode;
use crate::shared::dialogs::dialog_shell::dialog_shell_component::DialogShell;

#[allow(clippy::too_many_arguments)]
pub fn cipher_dialog_view(
    open: RwSignal<bool>,
    mode: Signal<CipherMode>,
    title: Signal<String>,
    intro: Signal<&'static str>,
    password: RwSignal<String>,
    confirm: RwSignal<String>,
    raw_key: RwSignal<bool>,
    needs_confirm: Signal<bool>,
    error: Signal<Option<String>>,
    busy: Signal<bool>,
    on_confirm: Callback<()>,
    on_without_key: Callback<()>,
    on_cancel: Callback<()>,
) -> impl IntoView {
    let on_enter = move |ev: web_sys::KeyboardEvent| {
        if ev.key() == "Enter" {
            on_confirm.run(());
        }
    };
    view! {
        <DialogShell open on_close=on_cancel class="cipher-dialog">
            {
                let first = NodeRef::<Input>::new();
                first.on_load(|el| {
                    let _ = el.focus();
                });
                view! {
                    <div class="shell-header d-flex align-items-center gap-2">
                        <i class="bi bi-shield-lock"></i>
                        <span class="text-truncate">{move || title.get()}</span>
                    </div>
                    <div class="shell-body">
                        <p class="cipher-intro">{move || intro.get()}</p>

                        <label class="form-label cipher-label">"Password"</label>
                        <div class="input-group input-group-sm mb-2">
                            <input
                                node_ref=first
                                class="form-control"
                                type="password"
                                autocomplete="off"
                                placeholder=move || if raw_key.get() { "64 hex characters" } else { "" }
                                prop:value=move || password.get()
                                on:input=move |ev| password.set(event_target_value(&ev))
                                on:keydown=on_enter
                            />
                            <select
                                class="form-select cipher-key-type"
                                on:change=move |ev| raw_key.set(event_target_value(&ev) == "raw")
                            >
                                <option value="passphrase" selected=move || !raw_key.get()>"Passphrase"</option>
                                <option value="raw" selected=move || raw_key.get()>"Raw key"</option>
                            </select>
                        </div>

                        <Show when=move || needs_confirm.get() fallback=|| ()>
                            <label class="form-label cipher-label">"Confirm password"</label>
                            <input
                                class="form-control form-control-sm mb-2"
                                type="password"
                                autocomplete="off"
                                prop:value=move || confirm.get()
                                on:input=move |ev| confirm.set(event_target_value(&ev))
                                on:keydown=on_enter
                            />
                        </Show>

                        <div class="cipher-settings">
                            <div class="cipher-settings-title">
                                <i class="bi bi-check-circle-fill"></i>" Encryption settings: SQLCipher 4 defaults"
                            </div>
                            <dl class="cipher-defaults mb-0">
                                {CIPHER_DEFAULTS
                                    .iter()
                                    .map(|(label, value)| view! { <dt>{*label}</dt><dd>{*value}</dd> })
                                    .collect_view()}
                            </dl>
                        </div>

                        <Show when=move || error.get().is_some() fallback=|| ()>
                            <div class="alert alert-danger py-2 px-3 mt-3 mb-0 small">
                                {move || error.get().unwrap_or_default()}
                            </div>
                        </Show>
                    </div>
                    <div class="shell-footer">
                        {move || match mode.get() {
                            CipherMode::Change { encrypted: true } => view! {
                                <button class="btn btn-sm btn-outline-danger me-auto" disabled=move || busy.get() on:click=move |_| on_without_key.run(())>
                                    <i class="bi bi-unlock"></i>" Remove encryption"
                                </button>
                            }.into_any(),
                            _ => ().into_any(),
                        }}
                        <Show when=move || busy.get() fallback=|| ()>
                            <span class="spinner-border spinner-border-sm text-secondary"></span>
                        </Show>
                        {move || match mode.get() {
                            CipherMode::Create => view! {
                                <button class="btn btn-sm btn-outline-secondary" disabled=move || busy.get() on:click=move |_| on_without_key.run(())>
                                    "Don't encrypt"
                                </button>
                            }.into_any(),
                            _ => view! {
                                <button class="btn btn-sm btn-outline-secondary" on:click=move |_| on_cancel.run(())>"Cancel"</button>
                            }.into_any(),
                        }}
                        <button class="btn btn-sm btn-primary" disabled=move || busy.get() on:click=move |_| on_confirm.run(())>
                            {move || match mode.get() {
                                CipherMode::Unlock => "Open",
                                CipherMode::Create => "Encrypt",
                                CipherMode::Change { encrypted: true } => "Change key",
                                CipherMode::Change { encrypted: false } => "Encrypt",
                            }}
                        </button>
                    </div>
                }
            }
        </DialogShell>
    }
}
