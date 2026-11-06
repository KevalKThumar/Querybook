//! SnippetsDialog — saved SQL snippets, shared by every database window.
//!
//! Save the editor's SQL (or its selection) under a name, insert a snippet at
//! the caret, or delete one. Saving an existing name replaces it.
//!
//! Author: Keval Thumar

use leptos::prelude::*;

use super::snippets_dialog_view::snippets_dialog_view;
use crate::database::database_structure::DatabaseService;
use crate::shared::services::toast_service::ToastService;

// Keval Thumar
#[component]
pub fn SnippetsDialog(
    open: RwSignal<bool>,
    /// What "Save" stores: the editor's selection, else all of its SQL.
    #[prop(into)]
    current_sql: Signal<String>,
    on_insert: Callback<String>,
) -> impl IntoView {
    let toast = expect_context::<ToastService>();
    let snippets = RwSignal::new(Vec::<(String, String)>::new());
    let name = RwSignal::new(String::new());

    // Re-read on open: another window may have saved one.
    Effect::new(move |_| {
        if open.get() {
            snippets.set(DatabaseService::snippets());
            name.set(String::new());
        }
    });

    let on_save = Callback::new(move |_: ()| {
        let (label, sql) = (name.get_untracked().trim().to_string(), current_sql.get_untracked().trim().to_string());
        if label.is_empty() || sql.is_empty() {
            return toast.error(if sql.is_empty() { "The editor is empty." } else { "Give the snippet a name." });
        }
        snippets.update(|list| match list.iter_mut().find(|(n, _)| *n == label) {
            Some(existing) => existing.1 = sql,
            None => list.push((label.clone(), sql)),
        });
        DatabaseService::set_snippets(&snippets.get_untracked());
        name.set(String::new());
        toast.success(&format!("Saved “{label}”."));
    });
    let on_pick = Callback::new(move |index: usize| {
        if let Some((_, sql)) = snippets.with_untracked(|s| s.get(index).cloned()) {
            open.set(false);
            on_insert.run(sql);
        }
    });
    let on_delete = Callback::new(move |index: usize| {
        snippets.update(|list| {
            if index < list.len() {
                list.remove(index);
            }
        });
        DatabaseService::set_snippets(&snippets.get_untracked());
    });
    let on_close = Callback::new(move |_: ()| open.set(false));

    snippets_dialog_view(open, snippets.into(), name, current_sql, on_save, on_pick, on_delete, on_close)
}
