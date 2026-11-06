//! Pure view template for the Dialog Shell — the `*_view.rs` half.
//!
//! Author: Keval Thumar

use leptos::prelude::*;

pub fn dialog_shell_view(
    open: RwSignal<bool>,
    on_close: Callback<()>,
    class: String,
    children: ChildrenFn,
) -> impl IntoView {
    let card_class = format!("shell-dialog {class}");
    view! {
        <Show when=move || open.get() fallback=|| ()>
            <div
                class="shell-overlay"
                on:mousedown=move |_| on_close.run(())
                on:keydown=move |ev| {
                    if ev.key() == "Escape" {
                        on_close.run(());
                    }
                }
            >
                // Card — stop propagation so a click inside never closes the dialog
                <div class=card_class.clone() on:mousedown=move |ev| ev.stop_propagation()>
                    {children()}
                </div>
            </div>
        </Show>
    }
}
