//! DataGrid — browse and edit one table or view, on top of the shared `<Grid/>`.
//!
//! This component owns the data side: rows are fetched in `CHUNK_ROWS` pages
//! with LIMIT/OFFSET (ordered by the sort column then the row key, so pages are
//! stable) as they scroll into view, plus sort/filters and the SQL behind each
//! edit / duplicate / delete. Every write runs inside the pending savepoint.
//! It also follows foreign keys (opening the parent table filtered to the
//! referenced row) and feeds the cell inspector, fetching blob bytes by key.
//!
//! Author: Keval Thumar

use std::collections::{HashMap, HashSet};

use leptos::prelude::*;
use leptos::task::spawn_local;
use shared::format::{count, from_hex};
use shared::sql::{self, quote_ident, RowKey};
use shared::Cell;

use super::data_grid_view::data_grid_view;
use crate::database::database_constant::{CHUNK_ROWS, INSPECT_MAX_BYTES};
use crate::database::database_structure::{
    ColumnInfo, DatabaseService, FkJump, ForeignKey, GridSelection, SchemaObject, TabKind,
};
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
    /// Opens (or focuses) a tab — used to follow foreign keys.
    on_open: Callback<TabKind>,
    /// Pending "go to referenced row", shared by every browse tab; the tab
    /// showing `jump.table` applies it and clears it.
    jump: RwSignal<Option<FkJump>>,
    /// Opens the Export SQL dialog with this table ticked.
    on_export_sql: Callback<Option<String>>,
) -> impl IntoView {
    let toast = expect_context::<ToastService>();
    let name = StoredValue::new(name);

    let key = RwSignal::new(RowKey::ReadOnly);
    let column_info = StoredValue::new(Vec::<ColumnInfo>::new());
    let foreign_keys = RwSignal::new(Vec::<ForeignKey>::new());
    let columns = RwSignal::new(Vec::<String>::new());
    let filters = RwSignal::new(Vec::<String>::new());
    let sort = RwSignal::new(None::<(String, bool)>);
    let total = RwSignal::new(0usize);
    let chunks = RwSignal::new(HashMap::<usize, Vec<Vec<Cell>>>::new());
    let in_flight = StoredValue::new(HashSet::<usize>::new());
    // Bumped on every reset so late responses for stale pages are dropped.
    let generation = StoredValue::new(0u32);
    let ready = RwSignal::new(false);
    let range = RwSignal::new((0usize, 0usize));
    let selection = RwSignal::new(GridSelection::None);
    let delete_open = RwSignal::new(false);
    let pending_delete = RwSignal::new(Vec::<usize>::new());
    let inspector_open = RwSignal::new(false);

    let editable = Signal::derive(move || key.with(|k| *k != RowKey::ReadOnly));
    let key_width = move || key.with_untracked(RowKey::width);
    let active_filters = move || {
        columns.with_untracked(|c| c.iter().cloned().zip(filters.get_untracked()).collect::<Vec<(String, String)>>())
    };
    let row_untracked = move |i: usize| {
        chunks.with_untracked(|m| m.get(&(i / CHUNK_ROWS)).and_then(|rows| rows.get(i % CHUNK_ROWS)).cloned())
    };
    let key_cells = move |i: usize| row_untracked(i).map(|row| row.into_iter().take(key_width()).collect::<Vec<_>>());

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
    let fetch_range = move |(first, end): (usize, usize)| {
        let last = end.max(first + 1) - 1;
        (first / CHUNK_ROWS..=last / CHUNK_ROWS).for_each(fetch_chunk);
    };

    // Drop cached pages; refetch the count + whatever is on screen.
    let reset = move || {
        generation.update_value(|g| *g += 1);
        in_flight.update_value(|s| s.clear());
        chunks.set(HashMap::new());
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
        fetch_range(range.get_untracked());
    };

    // (Re)load columns + row key, then data. Runs on mount and on every data_version bump.
    Effect::new(move |_| {
        data_version.track();
        ready.set(false);
        spawn_local(async move {
            let table = name.get_value();
            let cols = DatabaseService::load_columns(&table, toast).await;
            foreign_keys.set(DatabaseService::load_foreign_keys(&table, toast).await);
            let object = schema.with_untracked(|s| s.iter().find(|o| o.name == table).cloned());
            key.set(object.map_or(RowKey::ReadOnly, |o| o.row_key(&cols)));
            column_info.set_value(cols);
            ready.set(true);
            reset();
        });
    });
    // Fetch pages as they scroll into view.
    Effect::new(move |_| {
        let visible = range.get();
        if ready.get() {
            fetch_range(visible);
        }
    });

    let cell_at = Callback::new(move |(i, c): (usize, usize)| -> Option<Cell> {
        let kw = key.with(RowKey::width);
        chunks.with(|m| m.get(&(i / CHUNK_ROWS)).and_then(|rows| rows.get(i % CHUNK_ROWS)).and_then(|r| r.get(kw + c)).cloned())
    });

    // Run statements (sql, params) in order inside the pending savepoint, then reload.
    let run_writes = move |writes: Vec<(String, Vec<Cell>)>, done: Option<String>| {
        if writes.is_empty() {
            return;
        }
        spawn_local(async move {
            if !DatabaseService::begin_edit(in_txn, toast).await {
                return;
            }
            let mut last_txn = None;
            for (query, params) in &writes {
                match DatabaseService::execute(query, params, toast).await {
                    Some(out) => last_txn = Some(out.in_txn),
                    None => break,
                }
            }
            if let Some(txn) = last_txn {
                if let Some(message) = done {
                    toast.success(&message);
                }
                on_changed.run(txn);
            }
        });
    };
    let missing_rows = move || toast.error("Some selected rows aren't loaded yet — scroll through them first.");
    let delete_rows = move |rows: Vec<usize>| {
        let statement = sql::delete_row(&name.get_value(), &key.get_untracked());
        let Some(writes) = rows.iter().map(|&r| key_cells(r).map(|k| (statement.clone(), k))).collect::<Option<Vec<_>>>() else {
            return missing_rows();
        };
        let n = writes.len();
        selection.set(GridSelection::None);
        run_writes(writes, Some(format!("Deleted {n} row{}.", if n == 1 { "" } else { "s" })));
    };

    let on_range = Callback::new(move |visible: (usize, usize)| range.set(visible));
    let on_sort = Callback::new(move |c: usize| {
        let Some(column) = columns.with_untracked(|cols| cols.get(c).cloned()) else { return };
        sort.update(|s| {
            *s = match s.take() {
                Some((n, false)) if n == column => Some((n, true)),
                Some((n, true)) if n == column => None,
                _ => Some((column, false)),
            }
        });
        selection.set(GridSelection::None);
        reset();
    });
    let on_filter = Callback::new(move |(c, text): (usize, String)| {
        filters.update(|f| {
            if let Some(slot) = f.get_mut(c) {
                *slot = text;
            }
        });
        selection.set(GridSelection::None);
        reset();
    });
    let on_filter_value = Callback::new(move |(c, cell): (usize, Cell)| match cell {
        Cell::Null | Cell::Blob(_) => toast.error("Use a SQL tab to filter on NULL or binary values."),
        value => on_filter.run((c, format!("={}", value.display()))),
    });
    // ── Foreign keys ──
    let fk_of = move |column: &str| {
        foreign_keys.with_untracked(|fks| fks.iter().find(|fk| fk.column.eq_ignore_ascii_case(column)).cloned())
    };
    let fk_targets = Signal::derive(move || {
        columns.with(|cols| {
            foreign_keys.with(|fks| {
                cols.iter()
                    .map(|c| fks.iter().find(|fk| fk.column.eq_ignore_ascii_case(c)).map(|fk| format!("{}.{}", fk.table, fk.target)))
                    .collect::<Vec<_>>()
            })
        })
    });
    let on_follow_fk = Callback::new(move |(r, c): (usize, usize)| {
        let Some(fk) = columns.with_untracked(|cols| cols.get(c).and_then(|col| fk_of(col))) else { return };
        let value = match cell_at.run((r, c)) {
            Some(Cell::Null) => return toast.error("NULL doesn't reference a row."),
            Some(Cell::Blob(_)) | None => return toast.error("Can't follow a binary value."),
            Some(value) => value.display(),
        };
        // REFERENCES may spell the table differently: use the schema's name so the tab is reused.
        let table = schema.with_untracked(|s| {
            s.iter().find(|o| o.kind == "table" && o.name.eq_ignore_ascii_case(&fk.table)).map(|o| o.name.clone())
        });
        let Some(table) = table else { return toast.error(&format!("Table “{}” doesn't exist.", fk.table)) };
        jump.set(Some(FkJump { table: table.clone(), column: fk.target, value }));
        on_open.run(TabKind::Browse(table));
    });
    // Apply a jump aimed at this table once its columns are known (a new tab
    // loads them first, and loading resets the filters).
    Effect::new(move |_| {
        let Some(target) = jump.get().filter(|j| name.with_value(|n| *n == j.table)) else { return };
        let Some(c) = columns.with(|cols| cols.iter().position(|n| n.eq_ignore_ascii_case(&target.column))) else { return };
        jump.set(None);
        filters.update(|f| {
            f.iter_mut().for_each(String::clear);
            if let Some(slot) = f.get_mut(c) {
                *slot = format!("={}", target.value);
            }
        });
        selection.set(GridSelection::cell((0, c)));
        reset();
    });

    let on_edit = Callback::new(move |(i, c, value): (usize, usize, Cell)| {
        let (Some(keys), Some(column)) = (key_cells(i), columns.with_untracked(|cols| cols.get(c).cloned())) else {
            return missing_rows();
        };
        let mut params = vec![value];
        params.extend(keys);
        run_writes(vec![(sql::update_cell(&name.get_value(), &key.get_untracked(), &column), params)], None);
    });
    let on_delete_rows = Callback::new(move |rows: Vec<usize>| {
        if rows.len() > 1 {
            pending_delete.set(rows);
            delete_open.set(true);
        } else {
            delete_rows(rows);
        }
    });
    let on_confirm_delete = Callback::new(move |_: ()| {
        delete_open.set(false);
        delete_rows(pending_delete.get_untracked());
    });
    let on_cancel_delete = Callback::new(move |_: ()| delete_open.set(false));
    let on_duplicate_rows = Callback::new(move |rows: Vec<usize>| {
        // Copy every writable column except the rowid alias, so each copy gets a new id.
        let cols = column_info.with_value(|info| {
            let pk: Vec<&ColumnInfo> = info.iter().filter(|c| c.pk > 0).collect();
            let rowid_alias = (key.get_untracked() == RowKey::Rowid
                && pk.len() == 1
                && pk[0].ty.eq_ignore_ascii_case("INTEGER"))
            .then(|| pk[0].name.clone());
            info.iter()
                .filter(|c| c.hidden == 0 && Some(&c.name) != rowid_alias.as_ref())
                .map(|c| c.name.clone())
                .collect::<Vec<_>>()
        });
        let statement = sql::duplicate_row(&name.get_value(), &key.get_untracked(), &cols);
        let Some(writes) = rows.iter().map(|&r| key_cells(r).map(|k| (statement.clone(), k))).collect::<Option<Vec<_>>>() else {
            return missing_rows();
        };
        let n = writes.len();
        run_writes(writes, Some(format!("Duplicated {n} row{}.", if n == 1 { "" } else { "s" })));
    });

    // Toolbar
    let on_add_row = Callback::new(move |_: ()| run_writes(vec![(sql::insert_default(&name.get_value()), vec![])], None));
    let on_delete_selected = Callback::new(move |_: ()| on_delete_rows.run(selection.with_untracked(|s| s.row_list())));
    let on_null_selected = Callback::new(move |_: ()| {
        let sel = selection.get_untracked();
        let cols = sel.rect().map_or(0..columns.with_untracked(|c| c.len()), |(_, cs)| cs);
        for r in sel.row_list() {
            for c in cols.clone() {
                on_edit.run((r, c, Cell::Null));
            }
        }
    });
    let on_refresh = Callback::new(move |_: ()| reset());
    let on_export = Callback::new(move |_: ()| {
        let table = name.get_value();
        spawn_local(async move {
            DatabaseService::export_csv(&format!("SELECT * FROM {}", quote_ident(&table)), &table, toast).await;
        });
    });
    let on_export_table_sql = Callback::new(move |_: ()| on_export_sql.run(Some(name.get_value())));

    let selection_info = Signal::derive(move || {
        selection.with(|s| match s {
            GridSelection::None => String::new(),
            GridSelection::Rows { rows, .. } => format!("{} selected", rows.len()),
            GridSelection::Cells { .. } => {
                let (rs, cs) = s.rect().unwrap_or((0..0, 0..0));
                if rs.len() * cs.len() > 1 { format!("{} × {} cells", rs.len(), cs.len()) } else { String::new() }
            }
        })
    });
    // ── Cell inspector ──
    let focus = Memo::new(move |_| selection.with(|s| s.focus()));
    let inspected = Memo::new(move |_| focus.get().and_then(|at| cell_at.run(at)));
    let inspected_column =
        Signal::derive(move || focus.get().and_then(|(_, c)| columns.with(|cols| cols.get(c).cloned())).unwrap_or_default());
    let blob = RwSignal::new(None::<Result<(Vec<u8>, usize), String>>);
    Effect::new(move |_| {
        blob.set(None);
        let (true, Some(Cell::Blob(_)), Some((r, c))) = (inspector_open.get(), inspected.get(), focus.get()) else { return };
        if key.with_untracked(|k| *k == RowKey::ReadOnly) {
            return blob.set(Some(Err("Views have no row key, so their bytes can't be fetched.".into())));
        }
        let (Some(keys), Some(column)) = (key_cells(r), columns.with_untracked(|cols| cols.get(c).cloned())) else { return };
        let query = sql::blob_hex(&name.get_value(), &key.get_untracked(), &column, INSPECT_MAX_BYTES);
        spawn_local(async move {
            let Some(rs) = DatabaseService::query(&query, &keys, toast).await else { return };
            if focus.get_untracked() != Some((r, c)) {
                return;
            }
            let row = rs.rows.into_iter().next().unwrap_or_default();
            let bytes = row.first().and_then(|h| from_hex(&h.display()));
            let len = row.get(1).and_then(|n| n.display().parse().ok()).unwrap_or(0);
            blob.set(Some(bytes.map(|b| (b, len)).ok_or_else(|| "The row is gone.".to_string())));
        });
    });
    let on_inspect_apply = Callback::new(move |value: Cell| {
        if let Some((r, c)) = focus.get_untracked() {
            on_edit.run((r, c, value));
        }
    });
    let on_toggle_inspector = Callback::new(move |_: ()| inspector_open.update(|o| *o = !*o));

    let delete_message = Signal::derive(move || format!("Delete {}? You can still revert until you write changes.", count(pending_delete.with(|p| p.len()) as u64, "row")));
    let table_name = Signal::derive(move || name.get_value());

    data_grid_view(
        columns.into(),
        filters.into(),
        sort.into(),
        total.into(),
        cell_at,
        selection,
        editable,
        table_name,
        selection_info,
        delete_open,
        delete_message,
        on_range,
        on_sort,
        on_filter,
        on_filter_value,
        on_edit,
        on_delete_rows,
        on_duplicate_rows,
        on_confirm_delete,
        on_cancel_delete,
        on_add_row,
        on_delete_selected,
        on_null_selected,
        on_refresh,
        on_export,
        on_export_table_sql,
        fk_targets,
        on_follow_fk,
        inspector_open.into(),
        inspected_column,
        Signal::derive(move || inspected.get()),
        blob.into(),
        on_inspect_apply,
        on_toggle_inspector,
    )
}
