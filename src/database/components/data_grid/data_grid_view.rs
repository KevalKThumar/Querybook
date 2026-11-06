//! Template for the data grid: toolbar + shared `<Grid/>` + delete confirmation.
//!
//! Author: Keval Thumar

use leptos::prelude::*;
use shared::Cell;

use crate::database::components::grid::grid_component::Grid;
use crate::database::database_structure::GridSelection;
use crate::shared::dialogs::confirmation_dialog::confirmation_dialog_component::ConfirmationDialog;

#[allow(clippy::too_many_arguments)]
pub fn data_grid_view(
    columns: Signal<Vec<String>>,
    filters: Signal<Vec<String>>,
    sort: Signal<Option<(String, bool)>>,
    total: Signal<usize>,
    cell_at: Callback<(usize, usize), Option<Cell>>,
    selection: RwSignal<GridSelection>,
    editable: Signal<bool>,
    table_name: Signal<String>,
    selection_info: Signal<String>,
    delete_open: RwSignal<bool>,
    delete_message: Signal<String>,
    on_range: Callback<(usize, usize)>,
    on_sort: Callback<usize>,
    on_filter: Callback<(usize, String)>,
    on_filter_value: Callback<(usize, Cell)>,
    on_edit: Callback<(usize, usize, Cell)>,
    on_delete_rows: Callback<Vec<usize>>,
    on_duplicate_rows: Callback<Vec<usize>>,
    on_confirm_delete: Callback<()>,
    on_cancel_delete: Callback<()>,
    on_add_row: Callback<()>,
    on_delete_selected: Callback<()>,
    on_null_selected: Callback<()>,
    on_refresh: Callback<()>,
    on_export: Callback<()>,
) -> impl IntoView {
    let nothing_selected = move || selection.with(|s| *s == GridSelection::None);
    view! {
        <div class="grid d-flex flex-column h-100">
            <div class="grid-toolbar d-flex align-items-center gap-1 px-2">
                <Show
                    when=move || editable.get()
                    fallback=|| view! { <span class="badge text-bg-secondary">"Read-only view"</span> }
                >
                    <button class="btn btn-sm grid-tool" title="Insert a row" on:click=move |_| on_add_row.run(())>
                        <i class="bi bi-plus-lg"></i>" Row"
                    </button>
                    <button class="btn btn-sm grid-tool" title="Delete the selected rows (⌫)" disabled=nothing_selected on:click=move |_| on_delete_selected.run(())>
                        <i class="bi bi-dash-lg"></i>" Row"
                    </button>
                    <button class="btn btn-sm grid-tool" title="Set the selected cells to NULL" disabled=nothing_selected on:click=move |_| on_null_selected.run(())>
                        "NULL"
                    </button>
                </Show>
                <button class="btn btn-sm grid-tool" title="Reload" on:click=move |_| on_refresh.run(())>
                    <i class="bi bi-arrow-clockwise"></i>
                </button>
                <button class="btn btn-sm grid-tool" title="Export to CSV" on:click=move |_| on_export.run(())>
                    <i class="bi bi-filetype-csv"></i>" Export"
                </button>
                <span class="ms-auto grid-count">{move || selection_info.get()}</span>
                <span class="grid-count">{move || format!("{} rows", total.get())}</span>
            </div>
            <div class="flex-grow-1 overflow-hidden">
                <Grid
                    columns=columns
                    total=total
                    cell_at=cell_at
                    selection=selection
                    table_name=table_name
                    editable=editable
                    on_range=on_range
                    sort=sort
                    on_sort=on_sort
                    filters=filters
                    on_filter=on_filter
                    on_edit=on_edit
                    on_delete_rows=on_delete_rows
                    on_duplicate_rows=on_duplicate_rows
                    on_filter_value=on_filter_value
                />
            </div>
            <ConfirmationDialog
                open=delete_open
                title="Delete records"
                message=delete_message
                pos_label="Delete"
                danger=true
                on_confirm=on_confirm_delete
                on_cancel=on_cancel_delete
            />
        </div>
    }
}
