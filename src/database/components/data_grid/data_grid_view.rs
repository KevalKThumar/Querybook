//! Template for the data grid: toolbar, sticky header + filter row, virtual body.
//!
//! Author: Keval Thumar

use leptos::html::Input;
use leptos::prelude::*;
use shared::Cell;

use crate::database::database_constant::{COLUMN_WIDTH, ROWNUM_WIDTH, ROW_HEIGHT};

fn px(v: f64) -> String {
    format!("{v}px")
}

#[allow(clippy::too_many_arguments)]
pub fn data_grid_view(
    columns: Signal<Vec<String>>,
    filters: Signal<Vec<String>>,
    sort: Signal<Option<(String, bool)>>,
    total: Signal<usize>,
    visible: Memo<Vec<usize>>,
    cell_at: Callback<(usize, usize), Option<Cell>>,
    selected: Signal<Option<(usize, usize)>>,
    editing: Signal<Option<(usize, usize)>>,
    edit_value: RwSignal<String>,
    editable: Signal<bool>,
    on_scroll: Callback<(f64, f64)>,
    on_sort: Callback<usize>,
    on_filter: Callback<(usize, String)>,
    on_select: Callback<(usize, usize)>,
    on_start_edit: Callback<(usize, usize)>,
    on_commit_edit: Callback<()>,
    on_cancel_edit: Callback<()>,
    on_set_null: Callback<()>,
    on_add_row: Callback<()>,
    on_delete_row: Callback<()>,
    on_refresh: Callback<()>,
    on_export: Callback<()>,
) -> impl IntoView {
    let width = move || px(ROWNUM_WIDTH + columns.with(|c| c.len()) as f64 * COLUMN_WIDTH);
    let indexed_columns = move || columns.get().into_iter().enumerate().collect::<Vec<_>>();
    let nothing_selected = move || selected.get().is_none();

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
                    <button class="btn btn-sm grid-tool" title="Delete the selected row" disabled=nothing_selected on:click=move |_| on_delete_row.run(())>
                        <i class="bi bi-dash-lg"></i>" Row"
                    </button>
                    <button class="btn btn-sm grid-tool" title="Set the selected cell to NULL" disabled=nothing_selected on:click=move |_| on_set_null.run(())>
                        "NULL"
                    </button>
                </Show>
                <button class="btn btn-sm grid-tool" title="Reload" on:click=move |_| on_refresh.run(())>
                    <i class="bi bi-arrow-clockwise"></i>
                </button>
                <button class="btn btn-sm grid-tool" title="Export to CSV" on:click=move |_| on_export.run(())>
                    <i class="bi bi-filetype-csv"></i>" Export"
                </button>
                <span class="ms-auto grid-count">{move || format!("{} rows", total.get())}</span>
            </div>

            <div
                class="grid-scroll flex-grow-1"
                on:scroll=move |ev| {
                    let el = event_target::<web_sys::HtmlElement>(&ev);
                    on_scroll.run((el.scroll_top() as f64, el.client_height() as f64));
                }
            >
                // Header + filters stay pinned while the body scrolls.
                <div class="grid-sticky" style:width=width>
                    <div class="grid-head">
                        <div class="grid-rownum">"#"</div>
                        <For
                            each=indexed_columns
                            key=|(i, n)| (*i, n.clone())
                            children=move |(c, name)| {
                                let column = name.clone();
                                let arrow = move || match sort.get() {
                                    Some((n, false)) if n == column => "bi bi-arrow-up",
                                    Some((n, true)) if n == column => "bi bi-arrow-down",
                                    _ => "",
                                };
                                view! {
                                    <div class="grid-th" title=name.clone() on:click=move |_| on_sort.run(c)>
                                        <span class="text-truncate">{name.clone()}</span>
                                        <i class=arrow></i>
                                    </div>
                                }
                            }
                        />
                    </div>
                    <div class="grid-filters">
                        <div class="grid-rownum"><i class="bi bi-funnel"></i></div>
                        <For
                            each=indexed_columns
                            key=|(i, n)| (*i, n.clone())
                            children=move |(c, _)| view! {
                                <div class="grid-filter">
                                    <input
                                        class="form-control form-control-sm"
                                        placeholder="Filter"
                                        title="Text = contains · prefix with = <> > < >= <= to compare"
                                        prop:value=move || filters.with(|f| f.get(c).cloned().unwrap_or_default())
                                        on:change=move |ev| on_filter.run((c, event_target_value(&ev)))
                                    />
                                </div>
                            }
                        />
                    </div>
                </div>

                <div class="grid-body" style:height=move || px(total.get() as f64 * ROW_HEIGHT) style:width=width>
                    <For
                        each=move || visible.get()
                        key=|i| *i
                        children=move |i| view! {
                            <div
                                class="grid-row"
                                class:selected=move || selected.get().is_some_and(|(r, _)| r == i)
                                style:top=px(i as f64 * ROW_HEIGHT)
                            >
                                <div class="grid-rownum">{i + 1}</div>
                                <For
                                    each={move || (0..columns.with(|c| c.len())).collect::<Vec<_>>()}
                                    key=|c| *c
                                    children=move |c| {
                                        let cell = move || cell_at.run((i, c));
                                        let is_editing = move || editing.get() == Some((i, c));
                                        view! {
                                            <div
                                                class="grid-cell"
                                                class:active=move || selected.get() == Some((i, c))
                                                class:null=move || matches!(cell(), Some(Cell::Null))
                                                class:blob=move || matches!(cell(), Some(Cell::Blob(_)))
                                                class:numeric=move || matches!(cell(), Some(Cell::Int(_) | Cell::Real(_)))
                                                on:mousedown=move |_| on_select.run((i, c))
                                                on:dblclick=move |_| on_start_edit.run((i, c))
                                            >
                                                <Show
                                                    when=is_editing
                                                    fallback=move || view! {
                                                        <span class="text-truncate">{move || cell().map(|v| v.display()).unwrap_or_default()}</span>
                                                    }
                                                >
                                                    {
                                                        let input = NodeRef::<Input>::new();
                                                        input.on_load(|el| {
                                                            let _ = el.focus();
                                                            el.select();
                                                        });
                                                        view! {
                                                            <input
                                                                node_ref=input
                                                                class="grid-edit"
                                                                prop:value=move || edit_value.get_untracked()
                                                                on:input=move |ev| edit_value.set(event_target_value(&ev))
                                                                on:keydown=move |ev| {
                                                                    ev.stop_propagation();
                                                                    match ev.key().as_str() {
                                                                        "Enter" => on_commit_edit.run(()),
                                                                        "Escape" => on_cancel_edit.run(()),
                                                                        _ => {}
                                                                    }
                                                                }
                                                                on:blur=move |_| on_commit_edit.run(())
                                                            />
                                                        }
                                                    }
                                                </Show>
                                            </div>
                                        }
                                    }
                                />
                            </div>
                        }
                    />
                </div>
            </div>
        </div>
    }
}
