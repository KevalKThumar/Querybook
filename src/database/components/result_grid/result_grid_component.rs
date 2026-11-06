//! ResultGrid — read-only, virtualized grid for an in-memory `ResultSet`
//! (SQL editor results). Same look as the data grid.
//!
//! Author: Keval Thumar

use leptos::prelude::*;
use shared::{Cell, ResultSet};

use super::result_grid_view::result_grid_view;
use crate::database::database_constant::{OVERSCAN_ROWS, ROW_HEIGHT};

#[component]
pub fn ResultGrid(#[prop(into)] result: Signal<Option<ResultSet>>) -> impl IntoView {
    let scroll_top = RwSignal::new(0.0f64);
    let viewport = RwSignal::new(600.0f64);

    let columns = Signal::derive(move || result.with(|r| r.as_ref().map(|r| r.columns.clone()).unwrap_or_default()));
    let row_count = Signal::derive(move || result.with(|r| r.as_ref().map_or(0, |r| r.rows.len())));
    let visible = Memo::new(move |_| {
        let first = ((scroll_top.get() / ROW_HEIGHT) as usize).saturating_sub(OVERSCAN_ROWS);
        let count = (viewport.get() / ROW_HEIGHT) as usize + OVERSCAN_ROWS * 2;
        (first..(first + count).min(row_count.get())).collect::<Vec<usize>>()
    });
    let cell_at = Callback::new(move |(i, c): (usize, usize)| -> Option<Cell> {
        result.with(|r| r.as_ref().and_then(|r| r.rows.get(i)).and_then(|row| row.get(c)).cloned())
    });
    let on_scroll = Callback::new(move |(top, height): (f64, f64)| {
        scroll_top.set(top);
        viewport.set(height);
    });

    // New result: back to the top.
    Effect::new(move |_| {
        result.track();
        scroll_top.set(0.0);
    });

    result_grid_view(columns, row_count, visible, cell_at, on_scroll)
}
