//! Grid — the one table component, used for browsing tables and for SQL
//! results.
//!
//! - Virtual scroll: only the visible rows (+ overscan) are in the DOM.
//! - Click a cell = that cell only; Shift-click / drag = a cell range;
//!   click / ⌘-click / Shift-click / drag the row numbers = whole rows.
//! - Right-click: row menu (copy, duplicate, delete, adjust rows to contents)
//!   or cell menu (copy, edit, NULL, filter by value).
//! - Keys: arrows (+Shift), ⌘↑/⌘↓, Home/End, PageUp/Down, Enter/F2 edit,
//!   Delete = NULL (cells) or delete (rows), ⌘C, ⇧⌘C, ⌘A, Esc.
//!
//! The parent owns the data (`cell_at`) and the selection. Edit actions are
//! optional callbacks, so a read-only grid simply leaves them out.
//!
//! Author: Keval Thumar

use leptos::ev;
use leptos::html::Div;
use leptos::prelude::*;
use leptos::task::spawn_local;
use shared::copy::{insert_sql, to_tsv};
use shared::Cell;

use super::grid_view::grid_view;
use crate::database::database_constant::{
    COLUMN_WIDTH, GRID_CHAR_WIDTH, GRID_LINE_HEIGHT, GRID_MAX_FIT_LINES, OVERSCAN_ROWS, ROWNUM_WIDTH, ROW_HEIGHT,
};
use crate::database::database_structure::{CopyKind, GridDrag, GridMenu, GridSelection};
use crate::shared::common_structure::MenuItem;
use crate::shared::services::clipboard_service::ClipboardService;
use crate::shared::services::toast_service::ToastService;

/// Most cells "Set to NULL" touches in one go.
const MAX_BULK_CELLS: usize = 1_000;

// Keval Thumar
#[allow(clippy::too_many_arguments)]
#[component]
pub fn Grid(
    #[prop(into)] columns: Signal<Vec<String>>,
    #[prop(into)] total: Signal<usize>,
    /// Value at (row, column); None while that row isn't loaded.
    cell_at: Callback<(usize, usize), Option<Cell>>,
    selection: RwSignal<GridSelection>,
    /// Table name used by "Copy as SQL".
    #[prop(into)]
    table_name: Signal<String>,
    #[prop(optional, into)] editable: Option<Signal<bool>>,
    /// Visible row range changed (first, end) — lazily fetch rows here.
    #[prop(optional)]
    on_range: Option<Callback<(usize, usize)>>,
    #[prop(optional, into)] sort: Option<Signal<Option<(String, bool)>>>,
    #[prop(optional)] on_sort: Option<Callback<usize>>,
    #[prop(optional, into)] filters: Option<Signal<Vec<String>>>,
    #[prop(optional)] on_filter: Option<Callback<(usize, String)>>,
    /// Write one cell (row, column, value).
    #[prop(optional)]
    on_edit: Option<Callback<(usize, usize, Cell)>>,
    #[prop(optional)] on_delete_rows: Option<Callback<Vec<usize>>>,
    #[prop(optional)] on_duplicate_rows: Option<Callback<Vec<usize>>>,
    /// "Filter by this value" (column, value).
    #[prop(optional)]
    on_filter_value: Option<Callback<(usize, Cell)>>,
) -> impl IntoView {
    let toast = expect_context::<ToastService>();
    let editable = Signal::derive(move || on_edit.is_some() && editable.is_some_and(|e| e.get()));
    let has_filters = filters.is_some();
    let filters = Signal::derive(move || filters.map(|f| f.get()).unwrap_or_default());
    let sort = Signal::derive(move || sort.and_then(|s| s.get()));

    let scroller = NodeRef::<Div>::new();
    let scroll_top = RwSignal::new(0.0f64);
    let viewport = RwSignal::new(900.0f64);
    let row_height = RwSignal::new(ROW_HEIGHT);
    let wrap = RwSignal::new(false);
    let editing = RwSignal::new(None::<(usize, usize)>);
    let edit_value = RwSignal::new(String::new());
    let edit_original = StoredValue::new(String::new());
    let drag = RwSignal::new(None::<GridDrag>);
    let menu_position = RwSignal::new(None::<(f64, f64)>);
    let menu_kind = RwSignal::new(GridMenu::Cells);

    let column_count = move || columns.with_untracked(|c| c.len());
    let header_height = if has_filters { ROW_HEIGHT * 2.0 } else { ROW_HEIGHT };

    // Rows worth rendering.
    let visible = Memo::new(move |_| {
        let h = row_height.get();
        let first = ((scroll_top.get() / h) as usize).saturating_sub(OVERSCAN_ROWS);
        let count = (viewport.get() / h) as usize + OVERSCAN_ROWS * 2;
        (first..(first + count).min(total.get())).collect::<Vec<usize>>()
    });
    if let Some(on_range) = on_range {
        Effect::new(move |_| {
            visible.with(|v| on_range.run((v.first().copied().unwrap_or(0), v.last().map_or(0, |l| l + 1))));
        });
    }

    // Scroll so (row, column) is fully visible below the sticky header.
    let reveal = move |(r, c): (usize, usize)| {
        let Some(el) = scroller.get_untracked() else { return };
        let h = row_height.get_untracked();
        let top = r as f64 * h;
        let view_top = el.scroll_top() as f64;
        let body_view = el.client_height() as f64 - header_height;
        if top < view_top {
            el.set_scroll_top(top as i32);
        } else if top + h > view_top + body_view {
            el.set_scroll_top((top + h - body_view) as i32);
        }
        let left = ROWNUM_WIDTH + c as f64 * COLUMN_WIDTH;
        let view_left = el.scroll_left() as f64;
        let view_w = el.client_width() as f64;
        if left - ROWNUM_WIDTH < view_left {
            el.set_scroll_left((left - ROWNUM_WIDTH) as i32);
        } else if left + COLUMN_WIDTH > view_left + view_w {
            el.set_scroll_left((left + COLUMN_WIDTH - view_w) as i32);
        }
    };
    let focus_grid = move || {
        if let Some(el) = scroller.get_untracked() {
            let _ = el.focus();
        }
    };

    // ── Copy ──
    // The selected values, row by row, with their column names.
    let gather = move || -> Result<(Vec<String>, Vec<Vec<Cell>>), String> {
        let names = columns.get_untracked();
        let sel = selection.get_untracked();
        let cols = sel.rect().map_or(0..names.len(), |(_, cs)| cs.start.min(names.len())..cs.end.min(names.len()));
        let rows = sel.row_list();
        if rows.is_empty() {
            return Err("Select cells or rows first.".into());
        }
        let mut out = Vec::with_capacity(rows.len());
        for r in rows {
            let row: Option<Vec<Cell>> = cols.clone().map(|c| cell_at.run((r, c))).collect();
            // ponytail: copies only rows already fetched; fetch the range first if big copies matter.
            out.push(row.ok_or("Some selected rows aren't loaded yet — scroll through them first, or select fewer rows.")?);
        }
        Ok((names[cols].to_vec(), out))
    };
    let copy = Callback::new(move |kind: CopyKind| match gather() {
        Err(message) => toast.error(&message),
        Ok((headers, rows)) => {
            let text = match kind {
                CopyKind::Plain => to_tsv(None, &rows),
                CopyKind::WithHeaders => to_tsv(Some(&headers), &rows),
                CopyKind::Sql => insert_sql(&table_name.get_untracked(), &headers, &rows),
            };
            let n = rows.len();
            spawn_local(async move {
                if ClipboardService::write_text(&text).await {
                    toast.success(&format!("Copied {n} row{}.", if n == 1 { "" } else { "s" }));
                } else {
                    toast.error("Couldn't write to the clipboard.");
                }
            });
        }
    });

    // ── Edit ──
    let start_edit = move |(r, c): (usize, usize)| {
        if !editable.get_untracked() {
            return;
        }
        match cell_at.run((r, c)) {
            Some(Cell::Blob(_)) => toast.error("Binary cells can't be edited yet."),
            Some(cell) => {
                let text = if cell == Cell::Null { String::new() } else { cell.display() };
                edit_original.set_value(text.clone());
                edit_value.set(text);
                selection.set(GridSelection::cell((r, c)));
                editing.set(Some((r, c)));
            }
            None => {}
        }
    };
    // `true` = finished from the keyboard: give focus back to the grid.
    let on_commit_edit = Callback::new(move |refocus: bool| {
        let Some((r, c)) = editing.get_untracked() else { return };
        editing.set(None);
        let value = edit_value.get_untracked();
        let was_null = matches!(cell_at.run((r, c)), Some(Cell::Null));
        if value != edit_original.get_value() || (was_null && !value.is_empty()) {
            if let Some(on_edit) = on_edit {
                on_edit.run((r, c, Cell::Text(value)));
            }
        }
        if refocus {
            focus_grid();
        }
    });
    let on_cancel_edit = Callback::new(move |_: ()| {
        editing.set(None);
        focus_grid();
    });
    let set_null = move || {
        let Some(on_edit) = on_edit.filter(|_| editable.get_untracked()) else { return };
        let sel = selection.get_untracked();
        let cols = sel.rect().map_or(0..column_count(), |(_, cs)| cs);
        let cells: Vec<(usize, usize)> =
            sel.row_list().into_iter().flat_map(|r| cols.clone().map(move |c| (r, c))).collect();
        if cells.len() > MAX_BULK_CELLS {
            toast.error(&format!("Select at most {MAX_BULK_CELLS} cells to set NULL."));
            return;
        }
        for (r, c) in cells {
            if !matches!(cell_at.run((r, c)), Some(Cell::Null) | None) {
                on_edit.run((r, c, Cell::Null));
            }
        }
    };
    let selected_rows = move || selection.with_untracked(|s| s.row_list());
    let delete_rows = move || {
        if let Some(on_delete) = on_delete_rows.filter(|_| editable.get_untracked()) {
            on_delete.run(selected_rows());
        }
    };

    // "Adjust rows to contents": one row height that fits the tallest visible row.
    // ponytail: a single height keeps virtual scroll O(1); per-row heights need a prefix-sum table.
    let fit_rows = move || {
        let per_line = ((COLUMN_WIDTH - 16.0) / GRID_CHAR_WIDTH).max(1.0) as usize;
        let mut lines = 1;
        for r in visible.get_untracked() {
            for c in 0..column_count() {
                if let Some(cell) = cell_at.run((r, c)) {
                    let n: usize = cell.display().split('\n').map(|l| l.chars().count().div_ceil(per_line).max(1)).sum();
                    lines = lines.max(n);
                }
            }
        }
        row_height.set(ROW_HEIGHT.max(lines.min(GRID_MAX_FIT_LINES) as f64 * GRID_LINE_HEIGHT + 10.0));
    };

    // ── Mouse ──
    // (row, column, shift, right button)
    let on_cell_down = Callback::new(move |(r, c, shift, right): (usize, usize, bool, bool)| {
        if right {
            if !selection.with_untracked(|s| s.has_cell(r, c)) {
                selection.set(GridSelection::cell((r, c)));
            }
            return;
        }
        match selection.get_untracked() {
            GridSelection::Cells { anchor, .. } if shift => selection.set(GridSelection::Cells { anchor, focus: (r, c) }),
            _ => selection.set(GridSelection::cell((r, c))),
        }
        drag.set(Some(GridDrag::Cells));
    });
    let on_cell_enter = Callback::new(move |(r, c): (usize, usize)| {
        if drag.get_untracked() == Some(GridDrag::Cells) {
            selection.update(|s| {
                if let GridSelection::Cells { focus, .. } = s {
                    *focus = (r, c);
                }
            });
        }
    });
    // (row, shift, ⌘/ctrl, right button)
    let on_row_down = Callback::new(move |(r, shift, toggle, right): (usize, bool, bool, bool)| {
        if right {
            if !selection.with_untracked(|s| s.has_row(r)) {
                selection.set(GridSelection::row(r));
            }
            return;
        }
        let next = match selection.get_untracked() {
            GridSelection::Rows { mut rows, .. } if toggle => {
                if !rows.remove(&r) {
                    rows.insert(r);
                }
                GridSelection::Rows { rows, anchor: r }
            }
            GridSelection::Rows { anchor, .. } if shift => {
                GridSelection::Rows { rows: (anchor.min(r)..=anchor.max(r)).collect(), anchor }
            }
            GridSelection::Cells { anchor, .. } if shift => {
                GridSelection::Rows { rows: (anchor.0.min(r)..=anchor.0.max(r)).collect(), anchor: anchor.0 }
            }
            _ => GridSelection::row(r),
        };
        selection.set(next);
        drag.set(Some(GridDrag::Rows));
    });
    let on_row_enter = Callback::new(move |r: usize| {
        if drag.get_untracked() == Some(GridDrag::Rows) {
            selection.update(|s| {
                if let GridSelection::Rows { rows, anchor } = s {
                    *rows = ((*anchor).min(r)..=(*anchor).max(r)).collect();
                }
            });
        }
    });
    let mouse_up = window_event_listener(ev::mouseup, move |_| drag.set(None));
    on_cleanup(move || mouse_up.remove());

    let on_select_all = Callback::new(move |_: ()| {
        let n = total.get_untracked();
        if n > 0 {
            selection.set(GridSelection::Rows { rows: (0..n).collect(), anchor: 0 });
        }
    });
    // A right-click inside a row selection acts on those rows.
    let on_context = Callback::new(move |(kind, x, y): (GridMenu, f64, f64)| {
        let rows = selection.with_untracked(|s| matches!(s, GridSelection::Rows { .. }));
        menu_kind.set(if rows { GridMenu::Rows } else { kind });
        menu_position.set(Some((x, y)));
    });
    let on_scroll = Callback::new(move |(top, height): (f64, f64)| {
        scroll_top.set(top);
        viewport.set(height);
    });
    let on_start_edit = Callback::new(start_edit);

    // ── Keyboard ──
    let on_keydown = Callback::new(move |ev: web_sys::KeyboardEvent| {
        if editing.get_untracked().is_some() {
            return;
        }
        let (n_rows, n_cols) = (total.get_untracked(), column_count());
        if n_rows == 0 || n_cols == 0 {
            return;
        }
        let cmd = ev.meta_key() || ev.ctrl_key();
        let key = ev.key();
        let mut handled = true;
        match key.as_str() {
            "ArrowUp" | "ArrowDown" | "ArrowLeft" | "ArrowRight" | "Home" | "End" | "PageUp" | "PageDown" => {
                let page = (viewport.get_untracked() / row_height.get_untracked()).max(1.0) as usize;
                let (r, c) = selection.with_untracked(|s| s.focus()).unwrap_or((0, 0));
                let (r, c) = (r.min(n_rows - 1), c.min(n_cols - 1));
                let to = match key.as_str() {
                    "ArrowUp" if cmd => (0, c),
                    "ArrowDown" if cmd => (n_rows - 1, c),
                    "ArrowUp" => (r.saturating_sub(1), c),
                    "ArrowDown" => ((r + 1).min(n_rows - 1), c),
                    "ArrowLeft" => (r, c.saturating_sub(1)),
                    "ArrowRight" => (r, (c + 1).min(n_cols - 1)),
                    "Home" => (r, 0),
                    "End" => (r, n_cols - 1),
                    "PageUp" => (r.saturating_sub(page), c),
                    _ => ((r + page).min(n_rows - 1), c),
                };
                let next = match selection.get_untracked() {
                    GridSelection::Cells { anchor, .. } if ev.shift_key() => GridSelection::Cells { anchor, focus: to },
                    _ => GridSelection::cell(to),
                };
                selection.set(next);
                reveal(to);
            }
            "Enter" | "F2" => match selection.with_untracked(|s| s.focus()) {
                Some(at) => start_edit(at),
                None => handled = false,
            },
            "Delete" | "Backspace" => {
                if selection.with_untracked(|s| matches!(s, GridSelection::Rows { .. })) {
                    delete_rows();
                } else {
                    set_null();
                }
            }
            "Escape" => selection.set(GridSelection::None),
            "c" | "C" if cmd => copy.run(if ev.shift_key() { CopyKind::WithHeaders } else { CopyKind::Plain }),
            "a" if cmd => on_select_all.run(()),
            _ => handled = false,
        }
        if handled {
            ev.prevent_default();
            ev.stop_propagation();
        }
    });

    // ── Context menu ── (callbacks made once; the item list is rebuilt per open)
    let copy_plain = Callback::new(move |_: ()| copy.run(CopyKind::Plain));
    let copy_headers = Callback::new(move |_: ()| copy.run(CopyKind::WithHeaders));
    let copy_sql = Callback::new(move |_: ()| copy.run(CopyKind::Sql));
    let duplicate = Callback::new(move |_: ()| {
        if let Some(on_duplicate) = on_duplicate_rows {
            on_duplicate.run(selected_rows());
        }
    });
    let delete = Callback::new(move |_: ()| delete_rows());
    let toggle_fit = Callback::new(move |_: ()| {
        if wrap.get_untracked() {
            wrap.set(false);
            row_height.set(ROW_HEIGHT);
        } else {
            wrap.set(true);
            fit_rows();
        }
    });
    let edit_cell = Callback::new(move |_: ()| {
        if let Some(at) = selection.with_untracked(|s| s.focus()) {
            start_edit(at);
        }
    });
    let null_cells = Callback::new(move |_: ()| set_null());
    let filter_value = Callback::new(move |_: ()| {
        let (Some(on_filter_value), Some((r, c))) = (on_filter_value, selection.with_untracked(|s| s.focus())) else {
            return;
        };
        if let Some(cell) = cell_at.run((r, c)) {
            on_filter_value.run((c, cell));
        }
    });
    let menu_items = Signal::derive(move || {
        let edit = editable.get();
        let mut items = vec![
            MenuItem::action("Copy", "bi bi-clipboard", copy_plain).shortcut("⌘C"),
            MenuItem::action("Copy with Headers", "bi bi-clipboard-plus", copy_headers).shortcut("⇧⌘C"),
            MenuItem::action("Copy as SQL", "bi bi-filetype-sql", copy_sql),
        ];
        if menu_kind.get() == GridMenu::Rows {
            if on_duplicate_rows.is_some() || on_delete_rows.is_some() {
                items.push(MenuItem::Divider);
            }
            if on_duplicate_rows.is_some() {
                items.push(MenuItem::action("Duplicate records", "bi bi-copy", duplicate).enabled(edit));
            }
            if on_delete_rows.is_some() {
                items.push(MenuItem::action("Delete records", "bi bi-trash", delete).shortcut("⌫").enabled(edit).danger());
            }
            items.push(MenuItem::Divider);
            items.push(MenuItem::action("Adjust rows to contents", "", toggle_fit).checked(wrap.get()));
        } else {
            let single = selection.with(|s| s.rect().is_some_and(|(rs, cs)| rs.len() == 1 && cs.len() == 1));
            if on_edit.is_some() {
                items.push(MenuItem::Divider);
                items.push(MenuItem::action("Edit cell", "bi bi-pencil", edit_cell).shortcut("↵").enabled(edit && single));
                items.push(MenuItem::action("Set to NULL", "bi bi-slash-circle", null_cells).shortcut("⌫").enabled(edit));
            }
            if on_filter_value.is_some() {
                items.push(MenuItem::action("Filter by this value", "bi bi-funnel", filter_value).enabled(single));
            }
        }
        items
    });

    grid_view(
        columns,
        total,
        visible,
        cell_at,
        selection,
        filters,
        has_filters,
        sort,
        on_sort,
        on_filter,
        row_height.into(),
        wrap.into(),
        editing.into(),
        edit_value,
        scroller,
        menu_position,
        menu_items,
        on_scroll,
        on_keydown,
        on_cell_down,
        on_cell_enter,
        on_row_down,
        on_row_enter,
        on_select_all,
        on_context,
        on_start_edit,
        on_commit_edit,
        on_cancel_edit,
    )
}
