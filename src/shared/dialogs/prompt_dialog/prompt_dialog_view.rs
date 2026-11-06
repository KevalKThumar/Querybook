//! Pure view template for the PromptDialog.
//!
//! Author: Keval Thumar

use leptos::html::Input;
use leptos::prelude::*;

use crate::shared::dialogs::dialog_shell::dialog_shell_component::DialogShell;

pub fn prompt_dialog_view(
    open: RwSignal<bool>,
    title: Signal<String>,
    label: Signal<String>,
    value: RwSignal<String>,
    pos_label: String,
    on_ok: Callback<()>,
    on_cancel: Callback<()>,
) -> impl IntoView {
    view! {
        <DialogShell open on_close=on_cancel>
            {
                let input = NodeRef::<Input>::new();
                input.on_load(|el| {
                    let _ = el.focus();
                    el.select();
                });
                view! {
                    <div class="shell-header">{move || title.get()}</div>
                    <div class="shell-body">
                        <label class="form-label prompt-label">{move || label.get()}</label>
                        <input
                            node_ref=input
                            class="form-control"
                            prop:value=move || value.get()
                            on:input=move |ev| value.set(event_target_value(&ev))
                            on:keydown=move |ev| {
                                if ev.key() == "Enter" {
                                    on_ok.run(());
                                }
                            }
                        />
                    </div>
                    <div class="shell-footer">
                        <button class="btn btn-sm btn-outline-secondary" on:click=move |_| on_cancel.run(())>
                            "Cancel"
                        </button>
                        <button class="btn btn-sm btn-primary" on:click=move |_| on_ok.run(())>
                            {pos_label.clone()}
                        </button>
                    </div>
                }
            }
        </DialogShell>
    }
}
