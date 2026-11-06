//! Template for the shared grid: sticky header (+ optional filter row),
//! virtual body, row-number gutter and the context menu.
//!
//! Author: Keval Thumar

use leptos::html::{Div, Input};
use leptos::prelude::*;
use shared::Cell;

use crate::database::database_constant::{COLUMN_WIDTH, ROWNUM_WIDTH};
use crate::database::database_structure::{GridMenu, GridSelection};
use crate::shared::common_structure::MenuItem;
use crate::shared::components::context_menu::context_menu_component::ContextMenu;

fn px(v: f64) -> String {
    format!("{v}px")
}

#[allow(clippy::too_many_arguments)]
pub fn grid_view(
    columns: Signal<Vec<String>>,
    total: Signal<usize>,
    visible: Memo<Vec<usize>>,
    cell_at: Callback<(usize, usize), Option<Cell>>,
    selection: RwSignal<GridSelection>,
    filters: Signal<Vec<String>>,
    has_filters: bool,
    sort: Signal<Option<(String, bool)>>,
    on_sort: Option<Callback<usize>>,
    on_filter: Option<Callback<(usize, String)>>,
    row_height: Signal<f64>,
    wrap: Signal<bool>,
    editing: Signal<Option<(usize, usize)>>,
    edit_value: RwSignal<String>,
    scroller: NodeRef<Div>,
    menu_position: RwSignal<Option<(f64, f64)>>,
    menu_items: Signal<Vec<MenuItem>>,
    on_scroll: Callback<(f64, f64)>,
    on_keydown: Callback<web_sys::KeyboardEvent>,
    on_cell_down: Callback<(usize, usize, bool, bool)>,
    on_cell_enter: Callback<(usize, usize)>,
    on_row_down: Callback<(usize, bool, bool, bool)>,
    on_row_enter: Callback<usize>,
    on_select_all: Callback<()>,
    on_context: Callback<(GridMenu, f64, f64)>,
    on_start_edit: Callback<(usize, usize)>,
    on_commit_edit: Callback<bool>,
    on_cancel_edit: Callback<()>,
) -> impl IntoView {
    let width = move || px(ROWNUM_WIDTH + columns.with(|c| c.len()) as f64 * COLUMN_WIDTH);
    let indexed_columns = move || columns.get().into_iter().enumerate().collect::<Vec<_>>();
    let column_indexes = move || (0..columns.with(|c| c.len())).collect::<Vec<_>>();

    view! {
        <div class="qgrid h-100" class:wrap=move || wrap.get()>
            <div
                class="qgrid-scroll h-100"
                node_ref=scroller
                tabindex="0"
                on:scroll=move |ev| {
                    let el = event_target::<web_sys::HtmlElement>(&ev);
                    on_scroll.run((el.scroll_top() as f64, el.client_height() as f64));
                }
                on:keydown=move |ev| on_keydown.run(ev)
            >
                // Header (+ filters) stay pinned while the body scrolls.
                <div class="qgrid-sticky" style:width=width>
                    <div class="qgrid-head">
                        <div class="qgrid-corner" title="Select all rows (⌘A)" on:click=move |_| on_select_all.run(())>"#"</div>
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
                                    <div
                                        class="qgrid-th"
                                        class:sortable=on_sort.is_some()
                                        title=name.clone()
                                        on:click=move |_| {
                                            if let Some(on_sort) = on_sort {
                                                on_sort.run(c);
                                            }
                                        }
                                    >
                                        <span class="text-truncate">{name.clone()}</span>
                                        <i class=arrow></i>
                                    </div>
                                }
                            }
                        />
                    </div>
                    <Show when=move || has_filters fallback=|| ()>
                        <div class="qgrid-filters">
                            <div class="qgrid-rownum"><i class="bi bi-funnel"></i></div>
                            <For
                                each=indexed_columns
                                key=|(i, n)| (*i, n.clone())
                                children=move |(c, _)| view! {
                                    <div class="qgrid-filter">
                                        <input
                                            class="form-control form-control-sm"
                                            placeholder="Filter"
                                            title="Text = contains · prefix with = <> > < >= <= to compare"
                                            prop:value=move || filters.with(|f| f.get(c).cloned().unwrap_or_default())
                                            on:keydown=move |ev| ev.stop_propagation()
                                            on:change=move |ev| {
                                                if let Some(on_filter) = on_filter {
                                                    on_filter.run((c, event_target_value(&ev)));
                                                }
                                            }
                                        />
                                    </div>
                                }
                            />
                        </div>
                    </Show>
                </div>

                <div
                    class="qgrid-body"
                    style:height=move || px(total.get() as f64 * row_height.get())
                    style:width=width
                >
                    <For
                        each=move || visible.get()
                        key=|i| *i
                        children=move |i| view! {
                            <div
                                class="qgrid-row"
                                class:picked=move || selection.with(|s| s.has_row(i))
                                style:top=move || px(i as f64 * row_height.get())
                                style:height=move || px(row_height.get())
                            >
                                <div
                                    class="qgrid-rownum"
                                    on:mousedown=move |ev| on_row_down.run((i, ev.shift_key(), ev.meta_key() || ev.ctrl_key(), ev.button() == 2))
                                    on:mouseenter=move |_| on_row_enter.run(i)
                                    on:contextmenu=move |ev| {
                                        ev.prevent_default();
                                        on_context.run((GridMenu::Rows, ev.client_x() as f64, ev.client_y() as f64));
                                    }
                                >
                                    {i + 1}
                                </div>
                                <For
                                    each=column_indexes
                                    key=|c| *c
                                    children=move |c| {
                                        let cell = move || cell_at.run((i, c));
                                        let is_editing = move || editing.get() == Some((i, c));
                                        view! {
                                            <div
                                                class="qgrid-cell"
                                                class:selected=move || selection.with(|s| s.has_cell(i, c) && !s.has_row(i))
                                                class:focus=move || selection.with(|s| matches!(s, GridSelection::Cells { focus, .. } if *focus == (i, c)))
                                                class:null=move || matches!(cell(), Some(Cell::Null))
                                                class:blob=move || matches!(cell(), Some(Cell::Blob(_)))
                                                class:numeric=move || matches!(cell(), Some(Cell::Int(_) | Cell::Real(_)))
                                                on:mousedown=move |ev| on_cell_down.run((i, c, ev.shift_key(), ev.button() == 2))
                                                on:mouseenter=move |_| on_cell_enter.run((i, c))
                                                on:dblclick=move |_| on_start_edit.run((i, c))
                                                on:contextmenu=move |ev| {
                                                    ev.prevent_default();
                                                    on_context.run((GridMenu::Cells, ev.client_x() as f64, ev.client_y() as f64));
                                                }
                                            >
                                                <Show
                                                    when=is_editing
                                                    fallback=move || view! {
                                                        <span class="qgrid-text">{move || cell().map(|v| v.display()).unwrap_or_default()}</span>
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
                                                                class="qgrid-edit"
                                                                prop:value=move || edit_value.get_untracked()
                                                                on:input=move |ev| edit_value.set(event_target_value(&ev))
                                                                on:mousedown=move |ev| ev.stop_propagation()
                                                                on:keydown=move |ev| {
                                                                    ev.stop_propagation();
                                                                    match ev.key().as_str() {
                                                                        "Enter" => on_commit_edit.run(true),
                                                                        "Escape" => on_cancel_edit.run(()),
                                                                        _ => {}
                                                                    }
                                                                }
                                                                on:blur=move |_| on_commit_edit.run(false)
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
            <ContextMenu position=menu_position items=menu_items />
        </div>
    }
}
