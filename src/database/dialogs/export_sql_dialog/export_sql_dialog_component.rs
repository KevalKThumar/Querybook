//! ExportSqlDialog — tick tables and views and save them as one SQL script:
//! DROP / CREATE (or CREATE … IF NOT EXISTS) and INSERTs for the rows they
//! have now, unwritten changes included. Opened from the toolbar (everything
//! ticked) or a Browse tab (just that table).
//!
//! Author: Keval Thumar

use std::collections::BTreeSet;

use leptos::prelude::*;
use leptos::task::spawn_local;
use shared::SqlExportOptions;

use super::export_sql_dialog_view::export_sql_dialog_view;
use crate::database::database_structure::{DatabaseService, ExportItem};
use crate::shared::services::toast_service::ToastService;

#[component]
pub fn ExportSqlDialog(
    open: RwSignal<bool>,
    /// The table to tick when the dialog opens; None ticks everything.
    #[prop(into)]
    preselect: Signal<Option<String>>,
    /// Shows a note that the exported file isn't encrypted.
    #[prop(into)]
    encrypted: Signal<bool>,
) -> impl IntoView {
    let toast = expect_context::<ToastService>();

    // None while the list loads.
    let items = RwSignal::new(None::<Vec<ExportItem>>);
    let selected = RwSignal::new(BTreeSet::<String>::new());
    // Kept between openings, for the life of the window.
    let options = RwSignal::new(SqlExportOptions::default());
    let exporting = RwSignal::new(false);

    // Each opening re-reads the list (tables may have changed) and ticks the
    // preselected table, or everything.
    Effect::new(move |_| {
        if !open.get() {
            return;
        }
        let only = preselect.get_untracked();
        items.set(None);
        spawn_local(async move {
            let list = DatabaseService::load_exportable(toast).await;
            selected.set(match only {
                Some(name) if list.iter().any(|i| i.name == name) => BTreeSet::from([name]),
                _ => list.iter().map(|i| i.name.clone()).collect(),
            });
            items.set(Some(list));
        });
    });

    // Ticked objects that will produce SQL: views have no data to export.
    let to_export = Signal::derive(move || {
        let schema = options.with(|o| o.content.schema());
        items.with(|list| {
            list.iter()
                .flatten()
                .filter(|i| (schema || !i.is_view) && selected.with(|s| s.contains(&i.name)))
                .map(|i| i.name.clone())
                .collect::<Vec<_>>()
        })
    });
    let total = Signal::derive(move || items.with(|l| l.as_ref().map_or(0, Vec::len)));
    let ticked = Signal::derive(move || selected.with(BTreeSet::len));

    let on_toggle = Callback::new(move |name: String| {
        selected.update(|s| {
            if !s.remove(&name) {
                s.insert(name);
            }
        });
    });
    let on_toggle_all = Callback::new(move |all: bool| {
        let names: BTreeSet<String> =
            if all { items.with_untracked(|l| l.iter().flatten().map(|i| i.name.clone()).collect()) } else { BTreeSet::new() };
        selected.set(names);
    });
    let on_cancel = Callback::new(move |_: ()| open.set(false));
    let on_export = Callback::new(move |_: ()| {
        let names = to_export.get_untracked();
        if names.is_empty() {
            return;
        }
        let opts = options.get_untracked();
        exporting.set(true);
        spawn_local(async move {
            if DatabaseService::export_sql(&names, &opts, toast).await {
                open.set(false);
            }
            exporting.set(false);
        });
    });

    export_sql_dialog_view(
        open,
        items.into(),
        selected.into(),
        options,
        total,
        ticked,
        Signal::derive(move || to_export.with(Vec::is_empty)),
        exporting.into(),
        encrypted,
        on_toggle,
        on_toggle_all,
        on_export,
        on_cancel,
    )
}
