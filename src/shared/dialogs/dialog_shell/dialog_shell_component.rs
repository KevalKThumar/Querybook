//! Generic modal shell — the `*_component.rs` half.
//!
//! Dimmed backdrop (mouse-down outside or Escape closes) and a centered card
//! with a body slot. Owns no state: `open` and `on_close` come from the caller.
//! Dialogs render this inside their own view, so their `expect_context` calls
//! stay outside the `ChildrenFn` boundary.
//!
//! Author: Keval Thumar

use leptos::prelude::*;

use super::dialog_shell_view::dialog_shell_view;

#[component]
pub fn DialogShell(
    /// Controls whether the dialog is visible.
    open: RwSignal<bool>,
    /// Called on backdrop mouse-down or Escape.
    on_close: Callback<()>,
    /// Extra class for the card (e.g. a width modifier).
    #[prop(into, optional)]
    class: String,
    /// Content rendered inside the card.
    children: ChildrenFn,
) -> impl IntoView {
    dialog_shell_view(open, on_close, class, children)
}
