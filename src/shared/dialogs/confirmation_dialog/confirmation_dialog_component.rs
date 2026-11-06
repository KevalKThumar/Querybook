//! Generic ConfirmationDialog component.
//!
//! No service, no context — just props. The caller owns the `open` signal and
//! the confirm / cancel callbacks.
//!
//! # Example
//! ```rust
//! let open = RwSignal::new(false);
//! view! {
//!     <ConfirmationDialog open title="Drop table" message=msg pos_label="Drop" danger=true
//!         on_confirm=on_drop on_cancel=Callback::new(move |_| open.set(false)) />
//! }
//! ```
//!
//! Author: Keval Thumar

use leptos::prelude::*;

use super::confirmation_dialog_view::confirmation_dialog_view;

#[component]
pub fn ConfirmationDialog(
    /// Controls dialog visibility.
    open: RwSignal<bool>,
    #[prop(into)]
    title: Signal<String>,
    #[prop(into)]
    message: Signal<String>,
    /// Positive / confirm button label.
    #[prop(into, default = "Confirm".to_string())]
    pos_label: String,
    /// Negative / cancel button label.
    #[prop(into, default = "Cancel".to_string())]
    neg_label: String,
    /// Red confirm button for destructive actions.
    #[prop(optional)]
    danger: bool,
    on_confirm: Callback<()>,
    /// Called on the cancel button, Escape, or the backdrop.
    on_cancel: Callback<()>,
) -> impl IntoView {
    confirmation_dialog_view(open, title, message, pos_label, neg_label, danger, on_confirm, on_cancel)
}
