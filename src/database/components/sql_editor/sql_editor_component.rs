//! SqlEditor — a SQL tab: highlighted editor, Run (all or selection), Stop,
//! status line and the result grid.
//!
//! The editor is a transparent `<textarea>` over a highlighted `<pre>` (same
//! font metrics, scroll kept in sync), so typing stays native.
//!
//! Author: Keval Thumar

use leptos::html::{Pre, Textarea};
use leptos::prelude::*;
use leptos::task::spawn_local;
use shared::lexer::has_keyword;
use shared::ScriptOutcome;

use super::sql_editor_view::sql_editor_view;
use crate::database::database_constant::{EDITOR_INDENT, MODIFYING_KEYWORDS};
use crate::database::database_structure::DatabaseService;
use crate::shared::services::toast_service::ToastService;

/// The selected text, if any. Textarea offsets are UTF-16 code units.
fn selected_sql(textarea: NodeRef<Textarea>) -> Option<String> {
    let ta = textarea.get_untracked()?;
    let (start, end) = (ta.selection_start().ok()??, ta.selection_end().ok()??);
    if start == end {
        return None;
    }
    let units: Vec<u16> = ta.value().encode_utf16().collect();
    String::from_utf16(units.get(start as usize..end as usize)?).ok()
}

// Keval Thumar
/// One SQL tab. Statements join the pending transaction if one is open,
/// otherwise they apply immediately (like the sqlite3 shell).
#[component]
pub fn SqlEditor(in_txn: RwSignal<bool>, on_changed: Callback<bool>) -> impl IntoView {
    let toast = expect_context::<ToastService>();

    let text = RwSignal::new(String::new());
    let running = RwSignal::new(false);
    let outcome = RwSignal::new(None::<ScriptOutcome>);
    let textarea = NodeRef::<Textarea>::new();
    let overlay = NodeRef::<Pre>::new();

    // Trailing newline keeps the highlight layer as tall as the textarea.
    let highlighted = Signal::derive(move || format!("{}\n", text.get()));
    let result = Signal::derive(move || outcome.with(|o| o.as_ref().and_then(|o| o.result.clone())));
    // (is_error, message)
    let status = Signal::derive(move || {
        outcome.with(|o| {
            o.as_ref().map(|o| match &o.error {
                Some(e) => (true, e.clone()),
                None => {
                    let rows = o.result.as_ref().map(|r| {
                        let more = if r.truncated { " (first rows only)" } else { "" };
                        format!(" · {} rows returned{more}", r.rows.len())
                    });
                    (
                        false,
                        format!(
                            "{} statement(s) · {} row(s) changed{} · {} ms",
                            o.statements,
                            o.affected,
                            rows.unwrap_or_default(),
                            o.millis
                        ),
                    )
                }
            })
        })
    });

    let on_run = Callback::new(move |_: ()| {
        if running.get_untracked() {
            return;
        }
        let sql = selected_sql(textarea).unwrap_or_else(|| text.get_untracked());
        if sql.trim().is_empty() {
            return;
        }
        running.set(true);
        spawn_local(async move {
            let out = DatabaseService::exec_script(&sql, toast).await;
            running.set(false);
            if let Some(out) = out {
                if has_keyword(&sql, &MODIFYING_KEYWORDS) {
                    on_changed.run(out.in_txn);
                } else {
                    in_txn.set(out.in_txn);
                }
                outcome.set(Some(out));
            }
        });
    });
    let on_stop = Callback::new(move |_: ()| spawn_local(DatabaseService::cancel()));
    let on_input = Callback::new(move |value: String| text.set(value));
    let on_keydown = Callback::new(move |ev: web_sys::KeyboardEvent| {
        let key = ev.key();
        if key == "Enter" && (ev.meta_key() || ev.ctrl_key()) {
            ev.prevent_default();
            on_run.run(());
        } else if key == "Tab" {
            ev.prevent_default();
            let Some(ta) = textarea.get_untracked() else { return };
            let start = ta.selection_start().ok().flatten().unwrap_or(0);
            let end = ta.selection_end().ok().flatten().unwrap_or(start);
            let _ = ta.set_range_text_with_start_and_end(EDITOR_INDENT, start, end);
            let caret = start + EDITOR_INDENT.len() as u32;
            let _ = ta.set_selection_range(caret, caret);
            text.set(ta.value());
        }
    });
    let on_scroll = Callback::new(move |_: ()| {
        if let (Some(ta), Some(pre)) = (textarea.get_untracked(), overlay.get_untracked()) {
            pre.set_scroll_top(ta.scroll_top());
            pre.set_scroll_left(ta.scroll_left());
        }
    });

    sql_editor_view(
        highlighted,
        running.into(),
        status,
        result,
        textarea,
        overlay,
        on_run,
        on_stop,
        on_input,
        on_keydown,
        on_scroll,
    )
}
