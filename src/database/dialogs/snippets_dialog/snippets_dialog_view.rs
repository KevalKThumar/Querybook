//! Template for the snippets dialog.
//!
//! Author: Keval Thumar

use leptos::prelude::*;

use crate::shared::dialogs::dialog_shell::dialog_shell_component::DialogShell;

/// A snippet on one line, for the list.
fn preview(sql: &str) -> String {
    sql.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[allow(clippy::too_many_arguments)]
pub fn snippets_dialog_view(
    open: RwSignal<bool>,
    snippets: Signal<Vec<(String, String)>>,
    name: RwSignal<String>,
    current_sql: Signal<String>,
    on_save: Callback<()>,
    on_pick: Callback<usize>,
    on_delete: Callback<usize>,
    on_close: Callback<()>,
) -> impl IntoView {
    view! {
        <DialogShell open on_close=on_close class="shell-dialog--wide">
            <div class="shell-header">"Snippets"</div>
            <div class="shell-body snippets-body">
                <div class="d-flex gap-2 mb-3">
                    <input
                        class="form-control form-control-sm"
                        placeholder="Name for the current SQL (or selection)"
                        prop:value=move || name.get()
                        on:input=move |ev| name.set(event_target_value(&ev))
                        on:keydown=move |ev| {
                            if ev.key() == "Enter" {
                                on_save.run(());
                            }
                        }
                    />
                    <button
                        class="btn btn-sm btn-primary text-nowrap"
                        disabled=move || current_sql.with(|s| s.trim().is_empty())
                        on:click=move |_| on_save.run(())
                    >
                        <i class="bi bi-bookmark-plus"></i>" Save"
                    </button>
                </div>
                <Show
                    when=move || snippets.with(|s| !s.is_empty())
                    fallback=|| view! { <div class="snippets-empty">"No snippets yet. Name the SQL in the editor and save it."</div> }
                >
                    <div class="snippets-list">
                        <For
                            each={move || snippets.get().into_iter().enumerate().collect::<Vec<_>>()}
                            key=|(i, (n, sql))| (*i, n.clone(), sql.clone())
                            children=move |(i, (label, sql))| view! {
                                <div class="snippets-item d-flex align-items-center gap-2" title="Insert at the cursor" on:click=move |_| on_pick.run(i)>
                                    <i class="bi bi-code-square"></i>
                                    <span class="snippets-name text-truncate">{label}</span>
                                    <span class="snippets-preview text-truncate flex-grow-1">{preview(&sql)}</span>
                                    <button
                                        class="icon-btn snippets-delete"
                                        title="Delete snippet"
                                        on:click=move |ev| {
                                            ev.stop_propagation();
                                            on_delete.run(i);
                                        }
                                    >
                                        <i class="bi bi-trash"></i>
                                    </button>
                                </div>
                            }
                        />
                    </div>
                </Show>
            </div>
            <div class="shell-footer">
                <button class="btn btn-sm btn-outline-secondary" on:click=move |_| on_close.run(())>"Close"</button>
            </div>
        </DialogShell>
    }
}
