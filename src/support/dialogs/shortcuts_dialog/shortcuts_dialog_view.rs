//! Template for the Keyboard Shortcuts sheet.
//!
//! Author: Keval Thumar

use leptos::prelude::*;

use crate::shared::dialogs::dialog_shell::dialog_shell_component::DialogShell;

pub fn shortcuts_dialog_view(
    open: RwSignal<bool>,
    sections: Vec<(&'static str, Vec<(String, &'static str)>)>,
    on_close: Callback<()>,
) -> impl IntoView {
    view! {
        <DialogShell open on_close class="shell-dialog--wide">
            <div class="shell-header">"Keyboard Shortcuts"</div>
            <div class="shell-body shortcuts-body">
                {sections
                    .clone()
                    .into_iter()
                    .map(|(section, rows)| view! {
                        <div class="shortcuts-section">
                            <div class="shortcuts-title">{section}</div>
                            {rows
                                .into_iter()
                                .map(|(keys, action)| view! {
                                    <div class="shortcuts-row">
                                        <kbd>{keys}</kbd>
                                        <span>{action}</span>
                                    </div>
                                })
                                .collect_view()}
                        </div>
                    })
                    .collect_view()}
            </div>
            <div class="shell-footer">
                <button class="btn btn-sm btn-primary" on:click=move |_| on_close.run(())>"Done"</button>
            </div>
        </DialogShell>
    }
}
