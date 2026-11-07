//! Template for the History tab.
//!
//! Author: Keval Thumar

use std::collections::HashSet;

use leptos::prelude::*;
use shared::history::{HistoryEntry, HistoryPage, RowChangeKind, SchemaChange, SchemaChangeKind, TableChanges};

use super::change_history_component::saved_when;
use crate::database::database_constant::{HISTORY_ROWS_PER_TABLE, HISTORY_SHOWN};
use crate::shared::dialogs::confirmation_dialog::confirmation_dialog_component::ConfirmationDialog;
use crate::shared::services::platform_service::PlatformService;

#[allow(clippy::too_many_arguments)]
pub fn change_history_view(
    page: Signal<Option<HistoryPage>>,
    expanded: Signal<HashSet<(i64, String)>>,
    encrypted: Signal<bool>,
    confirm_open: RwSignal<bool>,
    confirm_message: Signal<String>,
    on_expand: Callback<(i64, String)>,
    on_clear: Callback<()>,
    on_confirm_clear: Callback<()>,
    on_cancel_clear: Callback<()>,
) -> impl IntoView {
    let total = move || page.with(|p| p.as_ref().map_or(0, |p| p.total));
    let nothing_saved = move || total() == 0;
    let footer = move || match total() {
        1 => "1 save made on this device.".to_string(),
        n if n > u64::from(HISTORY_SHOWN) => format!("Showing the last {HISTORY_SHOWN} of {n} saves made on this device."),
        n => format!("{n} saves made on this device."),
    };
    let empty_hint = PlatformService::keys("No saved changes yet. Changes show up here after you write them (⌘S).");

    view! {
        <div class="chist h-100 overflow-auto">
            <div class="chist-content">
                <div class="chist-head d-flex align-items-center gap-2">
                    <h6 class="chist-heading">"Change history"</h6>
                    <button
                        class="btn btn-sm btn-outline-secondary ms-auto"
                        title="Delete this database's saved history (the database isn't changed)"
                        disabled=nothing_saved
                        on:click=move |_| on_clear.run(())
                    >
                        <i class="bi bi-trash"></i>" Clear history…"
                    </button>
                </div>
                <div class="chist-note">"What each save wrote to this database, newest first. Kept on this device only."</div>

                {move || match page.get() {
                    None => view! {
                        <div class="chist-empty"><span class="spinner-border spinner-border-sm"></span></div>
                    }.into_any(),
                    Some(p) if p.entries.is_empty() => view! {
                        <div class="chist-empty">
                            <i class="bi bi-clock-history"></i>
                            <div>{empty_hint.clone()}</div>
                        </div>
                    }.into_any(),
                    Some(p) => p.entries.into_iter().map(|e| entry_view(e, expanded, on_expand)).collect_view().into_any(),
                }}

                <Show when=move || !nothing_saved() fallback=|| ()>
                    <div class="chist-footer">{footer}</div>
                </Show>
                <Show when=move || encrypted.get() fallback=|| ()>
                    <div class="chist-footer"><i class="bi bi-lock-fill"></i>" Encrypted with this database's key."</div>
                </Show>
            </div>
            <ConfirmationDialog
                open=confirm_open
                title="Clear change history"
                message=confirm_message
                pos_label="Clear"
                danger=true
                on_confirm=on_confirm_clear
                on_cancel=on_cancel_clear
            />
        </div>
    }
}

/// One save: when, a summary, its schema changes, then the rows of each table.
fn entry_view(e: HistoryEntry, expanded: Signal<HashSet<(i64, String)>>, on_expand: Callback<(i64, String)>) -> impl IntoView {
    let id = e.id;
    let summary = e.summary();
    view! {
        <div class="chist-card">
            <div class="chist-card-head">
                <i class="bi bi-floppy"></i>
                <span class="chist-when">{format!("Saved {}", saved_when(e.saved_at))}</span>
                <span class="chist-summary">{summary}</span>
            </div>
            <ul class="chist-schema">
                {e.schema.into_iter().map(schema_view).collect_view()}
            </ul>
            {e.tables.into_iter().map(|t| table_view(id, t, expanded, on_expand)).collect_view()}
        </div>
    }
}

fn schema_view(c: SchemaChange) -> impl IntoView {
    let icon = match c.kind {
        SchemaChangeKind::Created => "bi bi-plus-square text-success",
        SchemaChangeKind::Dropped => "bi bi-dash-square text-danger",
        SchemaChangeKind::Altered => "bi bi-pencil-square text-primary",
    };
    let sql = match (&c.before, &c.after) {
        (Some(before), Some(after)) => format!("-- Before\n{before}\n\n-- After\n{after}"),
        (Some(sql), None) | (None, Some(sql)) => sql.clone(),
        (None, None) => String::new(),
    };
    view! {
        <li>
            <details>
                <summary><i class=icon></i>" "{c.label()}</summary>
                <pre class="chist-sql">{sql}</pre>
            </details>
        </li>
    }
}

fn table_view(id: i64, t: TableChanges, expanded: Signal<HashSet<(i64, String)>>, on_expand: Callback<(i64, String)>) -> impl IntoView {
    let key = (id, t.table.clone());
    let total = t.rows.len();
    let hidden = total.saturating_sub(HISTORY_ROWS_PER_TABLE);
    let counts = t.counts_label();
    let approx = t.approx;
    let rows = StoredValue::new(t.rows);
    let open = {
        let key = key.clone();
        move || expanded.with(|e| e.contains(&key))
    };
    let shown = {
        let open = open.clone();
        move || if open() { total } else { total.min(HISTORY_ROWS_PER_TABLE) }
    };
    let more_hidden = move || hidden > 0 && !open();
    view! {
        <div class="chist-table">
            <div class="chist-table-head">
                <i class="bi bi-table"></i>
                <strong>{t.table}</strong>
                <span class="chist-counts">{counts}</span>
            </div>
            <ul class="chist-rows">
                {move || rows.with_value(|rows| rows.iter().take(shown()).map(|r| {
                    let (icon, what) = match r.kind {
                        RowChangeKind::Insert => ("bi bi-plus-circle text-success", "Inserted"),
                        RowChangeKind::Update => ("bi bi-pencil text-primary", "Updated"),
                        RowChangeKind::Delete => ("bi bi-dash-circle text-danger", "Deleted"),
                    };
                    let values = r.values_label();
                    view! {
                        <li class="chist-row" title=format!("{what} {}: {values}", r.key)>
                            <i class=icon></i>
                            <span class="chist-key">{r.key.clone()}</span>
                            <span class="chist-values">{values.clone()}</span>
                        </li>
                    }
                }).collect_view())}
            </ul>
            <Show when=more_hidden fallback=|| ()>
                <button class="btn btn-link btn-sm chist-more" on:click={
                    let key = key.clone();
                    move |_| on_expand.run(key.clone())
                }>
                    {format!("Show {hidden} more")}
                </button>
            </Show>
            <Show when=move || approx fallback=|| ()>
                <div class="chist-note">"This save changed more rows than the history keeps; the rest were only counted."</div>
            </Show>
        </div>
    }
}
