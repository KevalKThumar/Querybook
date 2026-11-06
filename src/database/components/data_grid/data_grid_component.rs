//! DataGrid — browse and edit one table or view.
//!
//! Virtual scroll: only the visible rows (+ overscan) are in the DOM, and rows
//! are fetched in `CHUNK_ROWS` pages with `LIMIT/OFFSET`, ordered by the sort
//! column then the row key so pages stay stable. Edits run inside the pending
//! savepoint and are written/reverted from the browser toolbar.
//!
//! Author: Keval Thumar

use std::collections::{HashMap, HashSet};

use leptos::prelude::*;
use leptos::task::spawn_local;
use shared::sql::{self, quote_ident, RowKey};
use shared::Cell;

use super::data_grid_view::data_grid_view;
use crate::database::database_constant::{CHUNK_ROWS, OVERSCAN_ROWS, ROW_HEIGHT};
use crate::database::database_structure::{DatabaseService, SchemaObject};
use crate::shared::services::toast_service::ToastService;

// Keval Thumar
/// Grid for the object `name`. Reloads whenever `data_version` changes.
#[component]
pub fn DataGrid(
    name: String,
    #[prop(into)] schema: Signal<Vec<SchemaObject>>,
    in_txn: RwSignal<bool>,
    #[prop(into)] data_version: Signal<u32>,
    on_changed: Callback<bool>,
) -> impl IntoView {
    let toast = expect_context::<ToastService>();
    let name = StoredValue::new(name);

    let key = RwSignal::new(RowKey::ReadOnly);
    let columns = RwSignal::new(Vec::<String>::new());
    let filters = RwSignal::new(Vec::<String>::new());
    let sort = RwSignal::new(None::<(String, bool)>);
    let total = RwSignal::new(0usize);
    let chunks = RwSignal::new(HashMap::<usize, Vec<Vec<Cell>>>::new());
    let in_flight = StoredValue::new(HashSet::<usize>::new());
    // Bumped on every reset so late responses for stale pages are dropped.
    let generation = StoredValue::new(0u32);
    let ready = RwSignal::new(false);
    let scroll_top = RwSignal::new(0.0f64);
    let viewport = RwSignal::new(900.0f64);
    let selected = RwSignal::new(None::<(usize, usize)>);
    let editing = RwSignal::new(None::<(usize, usize)>);
    let edit_value = RwSignal::new(String::new());
    let edit_original = StoredValue::new(String::new());

    let editable = Signal::derive(move || key.with(|k| *k != RowKey::ReadOnly));
    let key_width = move || key.with_untracked(RowKey::width);
    let active_filters = move || {
        columns.with_untracked(|c| c.iter().cloned().zip(filters.get_untracked()).collect::<Vec<(String, String)>>())
    };
    let row_untracked = move |i: usize| {
        chunks.with_untracked(|m| m.get(&(i / CHUNK_ROWS)).and_then(|rows| rows.get(i % CHUNK_ROWS)).cloned())
    };

    // Rows currently worth rendering.
    let visible = Memo::new(move |_| {
        let first = ((scroll_top.get() / ROW_HEIGHT) as usize).saturating_sub(OVERSCAN_ROWS);
        let count = (viewport.get() / ROW_HEIGHT) as usize + OVERSCAN_ROWS * 2;
        (first..(first + count).min(total.get())).collect::<Vec<usize>>()
    });

    let fetch_chunk = move |chunk: usize| {
        if chunks.with_untracked(|m| m.contains_key(&chunk)) || in_flight.with_value(|s| s.contains(&chunk)) {
            return;
        }
        in_flight.update_value(|s| {
            s.insert(chunk);
        });
        let generation_at_start = generation.get_value();
        let (query, params) = sql::select_page(
            &name.get_value(),
            &key.get_untracked(),
            sort.get_untracked().as_ref(),
            &active_filters(),
            CHUNK_ROWS,
            chunk * CHUNK_ROWS,
        );
        spawn_local(async move {
            let result = DatabaseService::query(&query, &params, toast).await;
            if generation.get_value() != generation_at_start {
                return;
            }
            in_flight.update_value(|s| {
                s.remove(&chunk);
            });
            if let Some(rs) = result {
                let names: Vec<String> = rs.columns.iter().skip(key_width()).cloned().collect();
                if columns.with_untracked(|c| *c != names) {
                    filters.set(vec![String::new(); names.len()]);
                    columns.set(names);
                }
                chunks.update(|m| {
                    m.insert(chunk, rs.rows);
                });
            }
        });
    };

    // Drop cached pages and refetch the count + whatever is on screen.
    let reset = move || {
        generation.update_value(|g| *g += 1);
        in_flight.update_value(|s| s.clear());
        chunks.set(HashMap::new());
        editing.set(None);
        let generation_at_start = generation.get_value();
        let (query, params) = sql::count_rows(&name.get_value(), &active_filters());
        spawn_local(async move {
            if let Some(rs) = DatabaseService::query(&query, &params, toast).await {
                if generation.get_value() == generation_at_start {
                    let n = rs.rows.first().and_then(|r| r.first()).map(Cell::display).unwrap_or_default();
                    total.set(n.parse().unwrap_or(0));
                }
            }
        });
        // Chunk 0 always: it also brings the column names for empty tables.
        fetch_chunk(0);
        let mut on_screen: Vec<usize> = visible.get_untracked().iter().map(|i| i / CHUNK_ROWS).collect();
        on_screen.dedup();
        on_screen.into_iter().for_each(fetch_chunk);
    };

    // (Re)load columns + row key, then data. Runs on mount and on every data_version bump.
    Effect::new(move |_| {
        data_version.track();
        ready.set(false);
        spawn_local(async move {
            let table = name.get_value();
            let cols = DatabaseService::load_columns(&table, toast).await;
            let object = schema.with_untracked(|s| s.iter().find(|o| o.name == table).cloned());
            key.set(object.map_or(RowKey::ReadOnly, |o| o.row_key(&cols)));
            ready.set(true);
            reset();
        });
    });

    // Fetch pages as they scroll into view.
    Effect::new(move |_| {
        if !ready.get() {
            return;
        }
        let mut needed: Vec<usize> = visible.get().iter().map(|i| i / CHUNK_ROWS).collect();
        needed.dedup();
        needed.into_iter().for_each(fetch_chunk);
    });

    let cell_at = Callback::new(move |(i, c): (usize, usize)| -> Option<Cell> {
        let kw = key.with(RowKey::width);
        chunks.with(|m| m.get(&(i / CHUNK_ROWS)).and_then(|rows| rows.get(i % CHUNK_ROWS)).and_then(|r| r.get(kw + c)).cloned())
    });

    // Run one data edit inside the pending savepoint, then let every grid reload.
    let run_edit = move |query: String, params: Vec<Cell>| {
        spawn_local(async move {
            if !DatabaseService::begin_edit(in_txn, toast).await {
                return;
            }
            if let Some(out) = DatabaseService::execute(&query, &params, toast).await {
                on_changed.run(out.in_txn);
            }
        });
    };
    let save_cell = move |i: usize, c: usize, value: Cell| {
        let Some(row) = row_untracked(i) else { return };
        let Some(column) = columns.with_untracked(|cols| cols.get(c).cloned()) else { return };
        let mut params = vec![value];
        params.extend(row.into_iter().take(key_width()));
        run_edit(sql::update_cell(&name.get_value(), &key.get_untracked(), &column), params);
    };

    let on_scroll = Callback::new(move |(top, height): (f64, f64)| {
        scroll_top.set(top);
        viewport.set(height);
    });
    let on_sort = Callback::new(move |c: usize| {
        let Some(column) = columns.with_untracked(|cols| cols.get(c).cloned()) else { return };
        sort.update(|s| {
            *s = match s.take() {
                Some((n, false)) if n == column => Some((n, true)),
                Some((n, true)) if n == column => None,
                _ => Some((column, false)),
            }
        });
        reset();
    });
    let on_filter = Callback::new(move |(c, text): (usize, String)| {
        filters.update(|f| {
            if let Some(slot) = f.get_mut(c) {
                *slot = text;
            }
        });
        reset();
    });
    let on_select = Callback::new(move |pos: (usize, usize)| selected.set(Some(pos)));
    let on_start_edit = Callback::new(move |pos: (usize, usize)| {
        if !editable.get_untracked() {
            return;
        }
        match cell_at.run(pos) {
            Some(Cell::Blob(_)) => toast.error("Binary cells can't be edited yet."),
            Some(cell) => {
                let text = if cell == Cell::Null { String::new() } else { cell.display() };
                edit_original.set_value(text.clone());
                edit_value.set(text);
                selected.set(Some(pos));
                editing.set(Some(pos));
            }
            None => {}
        }
    });
    let on_commit_edit = Callback::new(move |_: ()| {
        let Some((i, c)) = editing.get_untracked() else { return };
        editing.set(None);
        let value = edit_value.get_untracked();
        let was_null = matches!(cell_at.run((i, c)), Some(Cell::Null));
        if value != edit_original.get_value() || (was_null && !value.is_empty()) {
            save_cell(i, c, Cell::Text(value));
        }
    });
    let on_cancel_edit = Callback::new(move |_: ()| editing.set(None));
    let on_set_null = Callback::new(move |_: ()| {
        if let Some((i, c)) = selected.get_untracked() {
            save_cell(i, c, Cell::Null);
        }
    });
    let on_add_row = Callback::new(move |_: ()| run_edit(sql::insert_default(&name.get_value()), vec![]));
    let on_delete_row = Callback::new(move |_: ()| {
        let Some((i, _)) = selected.get_untracked() else { return };
        let Some(row) = row_untracked(i) else { return };
        selected.set(None);
        run_edit(sql::delete_row(&name.get_value(), &key.get_untracked()), row.into_iter().take(key_width()).collect());
    });
    let on_refresh = Callback::new(move |_: ()| reset());
    let on_export = Callback::new(move |_: ()| {
        let table = name.get_value();
        spawn_local(async move {
            DatabaseService::export_csv(&format!("SELECT * FROM {}", quote_ident(&table)), &table, toast).await;
        });
    });

    data_grid_view(
        columns.into(),
        filters.into(),
        sort.into(),
        total.into(),
        visible,
        cell_at,
        selected.into(),
        editing.into(),
        edit_value,
        editable,
        on_scroll,
        on_sort,
        on_filter,
        on_select,
        on_start_edit,
        on_commit_edit,
        on_cancel_edit,
        on_set_null,
        on_add_row,
        on_delete_row,
        on_refresh,
        on_export,
    )
}
