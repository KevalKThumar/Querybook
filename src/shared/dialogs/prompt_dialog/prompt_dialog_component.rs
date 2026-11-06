//! Generic single-text-field dialog (rename table, rename column, …).
//!
//! The caller owns `open` and `value` (pre-fill it before opening) and gets the
//! trimmed, non-empty text back through `on_submit`.
//!
//! Author: Keval Thumar

use leptos::prelude::*;

use super::prompt_dialog_view::prompt_dialog_view;

#[component]
pub fn PromptDialog(
    open: RwSignal<bool>,
    #[prop(into)]
    title: Signal<String>,
    #[prop(into)]
    label: Signal<String>,
    /// The text field; pre-fill before opening.
    value: RwSignal<String>,
    #[prop(into, default = "OK".to_string())]
    pos_label: String,
    on_submit: Callback<String>,
    on_cancel: Callback<()>,
) -> impl IntoView {
    let on_ok = Callback::new(move |_: ()| {
        let text = value.get_untracked().trim().to_string();
        if !text.is_empty() {
            on_submit.run(text);
        }
    });
    prompt_dialog_view(open, title, label, value, pos_label, on_ok, on_cancel)
}
