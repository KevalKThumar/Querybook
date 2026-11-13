//! SqlEditor — a SQL tab: highlighted editor with autocomplete, run all /
//! run current statement / EXPLAIN, Format (⇧⌥F), snippets, query history,
//! and results in the shared `<Grid/>` with a read-only cell inspector.
//!
//! The editor is a transparent `<textarea>` over a highlighted `<pre>` (same
//! font metrics, scroll kept in sync), so typing stays native.
//!
//! Author: Keval Thumar

use std::collections::{HashMap, HashSet};

use leptos::html::{Pre, Textarea};
use leptos::prelude::*;
use leptos::task::spawn_local;
use shared::complete::{self, CompletionContext, Suggestion};
use shared::format;
use shared::lexer::{has_keyword, statement_at};
use shared::{Cell, ScriptOutcome};

use super::sql_editor_view::sql_editor_view;
use crate::database::database_constant::{EDITOR_INDENT, EDITOR_PADDING, MODIFYING_KEYWORDS, SUGGEST_LIMIT};
use crate::database::database_structure::{DatabaseService, GridSelection, SchemaObject};
use crate::shared::services::toast_service::ToastService;

/// Textarea offsets are UTF-16 code units; Rust strings are bytes.
fn utf16_to_byte(s: &str, units: u32) -> usize {
    let mut seen = 0usize;
    for (i, ch) in s.char_indices() {
        if seen >= units as usize {
            return i;
        }
        seen += ch.len_utf16();
    }
    s.len()
}

fn byte_to_utf16(s: &str, byte: usize) -> u32 {
    s.get(..byte).map_or(0, |p| p.encode_utf16().count() as u32)
}

/// The selected text, if any.
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
/// One SQL tab. Statements that change data or schema run inside the pending
/// savepoint (the backend opens it), so nothing reaches the file until Write
/// Changes (⌘S). VACUUM and PRAGMA run as they are.
#[component]
pub fn SqlEditor(
    in_txn: RwSignal<bool>,
    on_changed: Callback<bool>,
    #[prop(into)] schema: Signal<Vec<SchemaObject>>,
    #[prop(into)] data_version: Signal<u32>,
    /// History is kept per database file.
    #[prop(into)]
    db_path: Signal<String>,
) -> impl IntoView {
    let toast = expect_context::<ToastService>();

    let text = RwSignal::new(String::new());
    let caret = RwSignal::new(0usize);
    let running = RwSignal::new(false);
    let outcome = RwSignal::new(None::<ScriptOutcome>);
    let last_sql = RwSignal::new(String::new());
    let selection = RwSignal::new(GridSelection::None);
    let history = RwSignal::new(DatabaseService::history(&db_path.get_untracked()));
    let textarea = NodeRef::<Textarea>::new();
    let overlay = NodeRef::<Pre>::new();
    let measure = NodeRef::<Pre>::new();
    let inspector_open = RwSignal::new(false);
    let snippets_open = RwSignal::new(false);
    let snippet_sql = RwSignal::new(String::new());

    // ── Autocomplete ──
    let suggest_open = RwSignal::new(false);
    let suggest_index = RwSignal::new(0usize);
    let column_cache = RwSignal::new(HashMap::<String, Vec<String>>::new());
    let loading_columns = StoredValue::new(HashSet::<String>::new());
    let metrics = StoredValue::new(None::<(f64, f64)>);

    // Schema changed (DDL, revert, import…): forget cached columns.
    Effect::new(move |_| {
        data_version.track();
        column_cache.set(HashMap::new());
    });
    let table_names = Memo::new(move |_| {
        schema.with(|s| s.iter().filter(|o| o.is_browsable()).map(|o| o.name.clone()).collect::<Vec<_>>())
    });
    let completion = Memo::new(move |_| -> Option<CompletionContext> {
        if !suggest_open.get() {
            return None;
        }
        text.with(|t| complete::context(t, caret.get()))
    });
    // Load columns for the tables the statement mentions, once each.
    Effect::new(move |_| {
        let Some(ctx) = completion.get() else { return };
        let names = table_names.get_untracked();
        let mentioned = ctx.tables.iter().map(|(t, _)| t.clone()).chain(ctx.qualifier.clone());
        for wanted in mentioned {
            let Some(table) = names.iter().find(|n| n.eq_ignore_ascii_case(&wanted)).cloned() else { continue };
            let cache_key = table.to_lowercase();
            if column_cache.with_untracked(|c| c.contains_key(&cache_key)) || loading_columns.with_value(|l| l.contains(&cache_key)) {
                continue;
            }
            loading_columns.update_value(|l| {
                l.insert(cache_key.clone());
            });
            spawn_local(async move {
                let cols = DatabaseService::load_columns(&table, toast).await.into_iter().map(|c| c.name).collect();
                column_cache.update(|c| {
                    c.insert(cache_key.clone(), cols);
                });
                loading_columns.update_value(|l| {
                    l.remove(&cache_key);
                });
            });
        }
    });
    let suggestions = Memo::new(move |_| -> Vec<Suggestion> {
        let Some(ctx) = completion.get() else { return vec![] };
        let names = table_names.get();
        column_cache.with(|cache| complete::suggest(&ctx, &names, &|t| cache.get(&t.to_lowercase()).cloned(), SUGGEST_LIMIT))
    });
    Effect::new(move |_| {
        suggestions.track();
        suggest_index.set(0);
    });

    // Popup position under the caret, from monospace metrics measured once.
    let popup_at = Signal::derive(move || -> Option<(f64, f64)> {
        if suggestions.with(|s| s.is_empty()) {
            return None;
        }
        let (char_w, line_h) = metrics.get_value().unwrap_or_else(|| {
            let measured = measure
                .get_untracked()
                .map(|pre| (pre.offset_width() as f64 / 100.0, pre.offset_height() as f64 / 10.0))
                .filter(|(w, _)| *w > 0.0)
                .unwrap_or((7.2, 18.6));
            metrics.set_value(Some(measured));
            measured
        });
        let (line, col) = text.with(|t| {
            let before = t.get(..caret.get()).unwrap_or("");
            let last_line = before.rsplit('\n').next().unwrap_or("");
            let col = last_line.chars().fold(0usize, |c, ch| if ch == '\t' { c + 4 - c % 4 } else { c + 1 });
            (before.matches('\n').count(), col)
        });
        let (left, top) = textarea.get_untracked().map_or((0.0, 0.0), |ta| (ta.scroll_left() as f64, ta.scroll_top() as f64));
        Some((EDITOR_PADDING + col as f64 * char_w - left, EDITOR_PADDING + (line + 1) as f64 * line_h - top + 2.0))
    });

    let sync_caret = move || {
        if let Some(ta) = textarea.get_untracked() {
            let units = ta.selection_start().ok().flatten().unwrap_or(0);
            caret.set(text.with_untracked(|t| utf16_to_byte(t, units)));
        }
    };
    let accept = move |index: usize| {
        let (Some(ctx), Some(choice), Some(ta)) = (
            completion.get_untracked(),
            suggestions.with_untracked(|s| s.get(index).cloned()),
            textarea.get_untracked(),
        ) else {
            return;
        };
        let value = ta.value();
        let start = byte_to_utf16(&value, ctx.start);
        let end = ta.selection_start().ok().flatten().unwrap_or(start);
        let _ = ta.set_range_text_with_start_and_end(&choice.insert, start, end);
        let at = start + choice.insert.encode_utf16().count() as u32;
        let _ = ta.set_selection_range(at, at);
        text.set(ta.value());
        sync_caret();
        suggest_open.set(false);
    };

    // ── Run ──
    // Results' "Copy as SQL" uses the first table the query mentions.
    let result_table = Signal::derive(move || {
        last_sql.with(|sql| complete::context(sql, sql.len()).and_then(|c| c.tables.first().map(|t| t.0.clone())))
            .unwrap_or_else(|| "result".to_string())
    });
    let run_sql = move |sql: String| {
        if running.get_untracked() || sql.trim().is_empty() {
            return;
        }
        running.set(true);
        suggest_open.set(false);
        DatabaseService::push_history(&db_path.get_untracked(), &sql);
        history.set(DatabaseService::history(&db_path.get_untracked()));
        spawn_local(async move {
            let out = DatabaseService::exec_script(&sql, toast).await;
            running.set(false);
            if let Some(out) = out {
                if has_keyword(&sql, &MODIFYING_KEYWORDS) {
                    on_changed.run(out.in_txn);
                } else {
                    in_txn.set(out.in_txn);
                }
                selection.set(GridSelection::None);
                last_sql.set(sql);
                outcome.set(Some(out));
            }
        });
    };
    // The selection, else the statement under the caret.
    let current_statement = move || {
        selected_sql(textarea).unwrap_or_else(|| {
            text.with_untracked(|t| {
                let units = textarea.get_untracked().and_then(|ta| ta.selection_start().ok().flatten()).unwrap_or(0);
                t[statement_at(t, utf16_to_byte(t, units))].trim().to_string()
            })
        })
    };
    let on_run_all = Callback::new(move |_: ()| run_sql(selected_sql(textarea).unwrap_or_else(|| text.get_untracked())));
    let on_run_current = Callback::new(move |_: ()| run_sql(current_statement()));
    let on_explain = Callback::new(move |_: ()| {
        let statement = current_statement();
        if !statement.is_empty() {
            run_sql(format!("EXPLAIN QUERY PLAN {statement}"));
        }
    });
    let on_stop = Callback::new(move |_: ()| spawn_local(DatabaseService::cancel()));
    // Format the selection, else everything.
    // ponytail: replacing the textarea's text resets its undo stack.
    let on_format = Callback::new(move |_: ()| {
        let Some(ta) = textarea.get_untracked() else { return };
        match selected_sql(textarea) {
            Some(part) => {
                let start = ta.selection_start().ok().flatten().unwrap_or(0);
                let end = ta.selection_end().ok().flatten().unwrap_or(start);
                let pretty = format::sql(&part);
                let _ = ta.set_range_text_with_start_and_end(&pretty, start, end);
                let _ = ta.set_selection_range(start, start + pretty.encode_utf16().count() as u32);
            }
            None => ta.set_value(&format::sql(&ta.value())),
        }
        text.set(ta.value());
        suggest_open.set(false);
        let _ = ta.focus();
    });
    let on_open_snippets = Callback::new(move |_: ()| {
        snippet_sql.set(selected_sql(textarea).unwrap_or_else(|| text.get_untracked()));
        snippets_open.set(true);
    });
    // A snippet replaces the selection / goes in at the caret.
    let on_insert_snippet = Callback::new(move |snippet: String| {
        let Some(ta) = textarea.get_untracked() else { return };
        let start = ta.selection_start().ok().flatten().unwrap_or(0);
        let end = ta.selection_end().ok().flatten().unwrap_or(start);
        let _ = ta.set_range_text_with_start_and_end(&snippet, start, end);
        let at = start + snippet.encode_utf16().count() as u32;
        let _ = ta.set_selection_range(at, at);
        text.set(ta.value());
        let _ = ta.focus();
    });
    let on_history = Callback::new(move |index: usize| {
        let (Some(sql), Some(ta)) = (history.with_untracked(|h| h.get(index).cloned()), textarea.get_untracked()) else { return };
        ta.set_value(&sql);
        text.set(sql);
        let _ = ta.focus();
    });

    // ── Editor events ──
    let on_input = Callback::new(move |value: String| {
        text.set(value);
        sync_caret();
        let typing_name = text.with_untracked(|t| t.get(..caret.get_untracked()).and_then(|b| b.chars().next_back()))
            .is_some_and(|ch| ch.is_alphanumeric() || ch == '_' || ch == '.');
        suggest_open.set(typing_name);
    });
    let on_keydown = Callback::new(move |ev: web_sys::KeyboardEvent| {
        // Enter that confirms an IME composition is not ours.
        if ev.is_composing() || ev.key_code() == 229 {
            return;
        }
        // ⇧⌥F (the key itself reads "Ï" with ⌥ on a Mac, so match the physical key).
        if ev.code() == "KeyF" && ev.shift_key() && ev.alt_key() {
            ev.prevent_default();
            on_format.run(());
            return;
        }
        let key = ev.key();
        let modified = ev.meta_key() || ev.ctrl_key() || ev.shift_key();
        let open = !suggestions.with_untracked(|s| s.is_empty());
        let count = suggestions.with_untracked(|s| s.len()).max(1);
        let handled = match key.as_str() {
            "ArrowDown" if open => {
                suggest_index.update(|i| *i = (*i + 1) % count);
                true
            }
            "ArrowUp" if open => {
                suggest_index.update(|i| *i = (*i + count - 1) % count);
                true
            }
            "Enter" | "Tab" if open && !modified => {
                accept(suggest_index.get_untracked());
                true
            }
            "Escape" if open => {
                suggest_open.set(false);
                true
            }
            // ⇧↵ / ⌘↵ / Ctrl+↵: run the statement under the caret (or the selection).
            "Enter" if modified => {
                on_run_current.run(());
                true
            }
            " " if ev.ctrl_key() => {
                sync_caret();
                suggest_open.set(true);
                true
            }
            "Tab" => {
                if let Some(ta) = textarea.get_untracked() {
                    let start = ta.selection_start().ok().flatten().unwrap_or(0);
                    let end = ta.selection_end().ok().flatten().unwrap_or(start);
                    let _ = ta.set_range_text_with_start_and_end(EDITOR_INDENT, start, end);
                    let at = start + EDITOR_INDENT.len() as u32;
                    let _ = ta.set_selection_range(at, at);
                    text.set(ta.value());
                }
                true
            }
            _ => false,
        };
        if handled {
            ev.prevent_default();
        }
    });
    // Caret moved without typing: update it and close the popup.
    let on_caret_moved = Callback::new(move |_: ()| {
        sync_caret();
        suggest_open.set(false);
    });
    let on_keyup = Callback::new(move |ev: web_sys::KeyboardEvent| {
        if matches!(ev.key().as_str(), "ArrowLeft" | "ArrowRight" | "Home" | "End" | "PageUp" | "PageDown")
            || (!suggest_open.get_untracked() && matches!(ev.key().as_str(), "ArrowUp" | "ArrowDown"))
        {
            on_caret_moved.run(());
        }
    });
    let on_scroll = Callback::new(move |_: ()| {
        if let (Some(ta), Some(pre)) = (textarea.get_untracked(), overlay.get_untracked()) {
            pre.set_scroll_top(ta.scroll_top());
            pre.set_scroll_left(ta.scroll_left());
        }
        suggest_open.set(false);
    });
    let on_pick = Callback::new(move |index: usize| accept(index));

    // ── Derived view data ──
    let highlighted = Signal::derive(move || format!("{}\n", text.get()));
    let result_columns = Signal::derive(move || {
        outcome.with(|o| o.as_ref().and_then(|o| o.result.as_ref()).map(|r| r.columns.clone()).unwrap_or_default())
    });
    let result_total =
        Signal::derive(move || outcome.with(|o| o.as_ref().and_then(|o| o.result.as_ref()).map_or(0, |r| r.rows.len())));
    let cell_at = Callback::new(move |(i, c): (usize, usize)| -> Option<Cell> {
        outcome.with(|o| o.as_ref().and_then(|o| o.result.as_ref()).and_then(|r| r.rows.get(i)).and_then(|row| row.get(c)).cloned())
    });
    // (is_error, message)
    let status = Signal::derive(move || {
        outcome.with(|o| {
            o.as_ref().map(|o| match &o.error {
                Some(e) => (true, e.clone()),
                None => {
                    let rows = o.result.as_ref().map(|r| {
                        let more = if r.truncated { " (first rows only)" } else { "" };
                        format!(" · {} returned{more}", format::count(r.rows.len() as u64, "row"))
                    });
                    let summary = format!(
                        "{} · {} changed{} · {} ms",
                        format::count(o.statements as u64, "statement"),
                        format::count(o.affected, "row"),
                        rows.unwrap_or_default(),
                        o.millis
                    );
                    (false, summary)
                }
            })
        })
    });
    let history_labels = Signal::derive(move || {
        history.with(|h| {
            h.iter()
                .map(|q| {
                    let one_line = q.split_whitespace().collect::<Vec<_>>().join(" ");
                    if one_line.chars().count() > 80 { format!("{}…", one_line.chars().take(80).collect::<String>()) } else { one_line }
                })
                .collect::<Vec<_>>()
        })
    });

    // Read-only inspector for the focused result cell.
    let focus = Memo::new(move |_| selection.with(|s| s.focus()));
    let inspected = Memo::new(move |_| focus.get().and_then(|at| cell_at.run(at)));
    let inspected_column =
        Signal::derive(move || focus.get().and_then(|(_, c)| result_columns.with(|cols| cols.get(c).cloned())).unwrap_or_default());
    let on_toggle_inspector = Callback::new(move |_: ()| inspector_open.update(|o| *o = !*o));

    sql_editor_view(
        highlighted,
        running.into(),
        status,
        history_labels,
        suggestions,
        suggest_index.into(),
        popup_at,
        result_columns,
        result_total,
        cell_at,
        selection,
        result_table,
        textarea,
        overlay,
        measure,
        on_run_all,
        on_run_current,
        on_explain,
        on_stop,
        on_history,
        on_input,
        on_keydown,
        on_keyup,
        on_caret_moved,
        on_scroll,
        on_pick,
        on_format,
        on_open_snippets,
        snippets_open,
        snippet_sql.into(),
        on_insert_snippet,
        inspector_open.into(),
        inspected_column,
        Signal::derive(move || inspected.get()),
        on_toggle_inspector,
    )
}
