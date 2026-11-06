//! Pure view template for the ConfirmationDialog — the `*_view.rs` half.
//!
//! Author: Keval Thumar

use leptos::prelude::*;

use crate::shared::dialogs::dialog_shell::dialog_shell_component::DialogShell;

#[allow(clippy::too_many_arguments)]
pub fn confirmation_dialog_view(
    open: RwSignal<bool>,
    title: Signal<String>,
    message: Signal<String>,
    pos_label: String,
    neg_label: String,
    danger: bool,
    on_confirm: Callback<()>,
    on_cancel: Callback<()>,
) -> impl IntoView {
    let pos_class = if danger { "btn btn-sm btn-danger" } else { "btn btn-sm btn-primary" };
    view! {
        <DialogShell open on_close=on_cancel>
            <div class="shell-header">{move || title.get()}</div>
            <div class="shell-body">
                <p class="confirm-message mb-0">{move || message.get()}</p>
            </div>
            <div class="shell-footer">
                <button class="btn btn-sm btn-outline-secondary" on:click=move |_| on_cancel.run(())>
                    {neg_label.clone()}
                </button>
                <button class=pos_class on:click=move |_| on_confirm.run(())>
                    {pos_label.clone()}
                </button>
            </div>
        </DialogShell>
    }
}
