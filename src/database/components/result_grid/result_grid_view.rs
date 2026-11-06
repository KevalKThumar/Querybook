//! Template for the SQL result grid.
//!
//! Author: Keval Thumar

use leptos::prelude::*;
use shared::Cell;

use crate::database::database_constant::{COLUMN_WIDTH, ROWNUM_WIDTH, ROW_HEIGHT};

fn px(v: f64) -> String {
    format!("{v}px")
}

pub fn result_grid_view(
    columns: Signal<Vec<String>>,
    row_count: Signal<usize>,
    visible: Memo<Vec<usize>>,
    cell_at: Callback<(usize, usize), Option<Cell>>,
    on_scroll: Callback<(f64, f64)>,
) -> impl IntoView {
    let width = move || px(ROWNUM_WIDTH + columns.with(|c| c.len()) as f64 * COLUMN_WIDTH);
    view! {
        <div
            class="result-scroll h-100"
            on:scroll=move |ev| {
                let el = event_target::<web_sys::HtmlElement>(&ev);
                on_scroll.run((el.scroll_top() as f64, el.client_height() as f64));
            }
        >
            <div class="result-head" style:width=width>
                <div class="result-rownum">"#"</div>
                <For
                    each={move || columns.get().into_iter().enumerate().collect::<Vec<_>>()}
                    key=|(i, n)| (*i, n.clone())
                    children=move |(_, name)| view! {
                        <div class="result-th" title=name.clone()>{name.clone()}</div>
                    }
                />
            </div>
            <div class="result-body" style:height=move || px(row_count.get() as f64 * ROW_HEIGHT) style:width=width>
                <For
                    each=move || visible.get()
                    key=|i| *i
                    children=move |i| view! {
                        <div class="result-row" style:top=px(i as f64 * ROW_HEIGHT)>
                            <div class="result-rownum">{i + 1}</div>
                            <For
                                each={move || (0..columns.with(|c| c.len())).collect::<Vec<_>>()}
                                key=|c| *c
                                children=move |c| {
                                    let cell = move || cell_at.run((i, c));
                                    view! {
                                        <div
                                            class="result-cell"
                                            class:null=move || matches!(cell(), Some(Cell::Null))
                                            class:blob=move || matches!(cell(), Some(Cell::Blob(_)))
                                            class:numeric=move || matches!(cell(), Some(Cell::Int(_) | Cell::Real(_)))
                                        >
                                            {move || cell().map(|v| v.display()).unwrap_or_default()}
                                        </div>
                                    }
                                }
                            />
                        </div>
                    }
                />
            </div>
        </div>
    }
}
