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
use crate::shared::services::platform_service::PlatformService;

fn px(v: f64) -> String {
    format!("{v}px")
}

#[allow(clippy::too_many_arguments)]
pub fn grid_view(
    columns: Signal<Vec<String>>,
    visible: Memo<Vec<usize>>,
    cell_at: Callback<(usize, usize), Option<Cell>>,
    selection: RwSignal<GridSelection>,
    filters: Signal<Vec<String>>,
    has_filters: bool,
    sort: Signal<Option<(String, bool)>>,
    on_sort: Option<Callback<usize>>,
    on_filter: Option<Callback<(usize, String)>>,
    fk_targets: Signal<Vec<Option<String>>>,
    on_follow_fk: Option<Callback<(usize, usize)>>,
    column_widths: Signal<Vec<f64>>,
    top_of: Callback<usize, f64>,
    height_of: Callback<usize, f64>,
    body_height: Signal<f64>,
    wrap: Signal<bool>,
    resizing: Signal<bool>,
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
    on_resize_start: Callback<(usize, f64)>,
    on_fit_column: Callback<usize>,
) -> impl IntoView {
    let width = move || px(ROWNUM_WIDTH + column_widths.with(|w| w.iter().sum::<f64>()));
    // Column widths as CSS variables on the root: a resize drag restyles one element, not every cell.
    let width_vars = move || {
        column_widths.with(|w| w.iter().enumerate().map(|(c, w)| format!("--qw-{c}:{w}px;")).collect::<String>())
    };
    let column_width = |c: usize| format!("var(--qw-{c}, {COLUMN_WIDTH}px)");
    let fk_target = move |c: usize| fk_targets.with(|t| t.get(c).cloned().flatten());
    let indexed_columns = move || columns.get().into_iter().enumerate().collect::<Vec<_>>();
    let column_indexes = move || (0..columns.with(|c| c.len())).collect::<Vec<_>>();

    view! {
        <div class="qgrid h-100" class:wrap=move || wrap.get() class:resizing=move || resizing.get() style=width_vars>
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
                        <div class="qgrid-corner" title={PlatformService::keys("Select all rows (⌘A)")} on:click=move |_| on_select_all.run(())>"#"</div>
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
                                        style:width=column_width(c)
                                        title=name.clone()
                                        on:click=move |_| {
                                            if let Some(on_sort) = on_sort {
                                                on_sort.run(c);
                                            }
                                        }
                                    >
                                        <span class="text-truncate">{name.clone()}</span>
                                        <Show when=move || fk_target(c).is_some() fallback=|| ()>
                                            <i class="bi bi-link-45deg qgrid-fk" title=move || format!("References {}", fk_target(c).unwrap_or_default())></i>
                                        </Show>
                                        <i class=arrow></i>
                                        // Drag to resize, double-click to fit.
                                        <div
                                            class="qgrid-resize"
                                            title="Drag to resize · double-click to fit"
                                            on:mousedown=move |ev| {
                                                ev.prevent_default();
                                                ev.stop_propagation();
                                                on_resize_start.run((c, ev.client_x() as f64));
                                            }
                                            on:click=move |ev| ev.stop_propagation()
                                            on:dblclick=move |ev| {
                                                ev.stop_propagation();
                                                on_fit_column.run(c);
                                            }
                                        ></div>
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
                                    <div class="qgrid-filter" style:width=column_width(c)>
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
                    style:height=move || px(body_height.get())
                    style:width=width
                >
                    <For
                        each=move || visible.get()
                        key=|i| *i
                        children=move |i| view! {
                            <div
                                class="qgrid-row"
                                class:picked=move || selection.with(|s| s.has_row(i))
                                style:top=move || px(top_of.run(i))
                                style:height=move || px(height_of.run(i))
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
                                        // A link button on non-NULL foreign-key values.
                                        let followable = move || {
                                            on_follow_fk.is_some()
                                                && fk_target(c).is_some()
                                                && matches!(cell(), Some(Cell::Int(_) | Cell::Real(_) | Cell::Text(_)))
                                        };
                                        view! {
                                            <div
                                                class="qgrid-cell"
                                                style:width=column_width(c)
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
                                                        <Show when=followable fallback=|| ()>
                                                            <button
                                                                class="qgrid-follow"
                                                                title=move || format!("Go to {}", fk_target(c).unwrap_or_default())
                                                                on:mousedown=move |ev| ev.stop_propagation()
                                                                on:click=move |ev| {
                                                                    ev.stop_propagation();
                                                                    if let Some(on_follow) = on_follow_fk {
                                                                        on_follow.run((i, c));
                                                                    }
                                                                }
                                                            >
                                                                <i class="bi bi-arrow-right-short"></i>
                                                            </button>
                                                        </Show>
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
