//! CreateTableDialog — build a CREATE TABLE from a column form with a live SQL
//! preview. Runs inside the pending savepoint and opens the new table.
//!
//! Author: Keval Thumar

use leptos::prelude::*;
use leptos::task::spawn_local;
use shared::sql::{self, ColumnDef};

use super::create_table_dialog_view::create_table_dialog_view;
use crate::database::database_structure::{ColumnField, DatabaseService, TabKind};
use crate::shared::services::toast_service::ToastService;

fn starter_columns() -> Vec<ColumnDef> {
    vec![ColumnDef { name: "id".into(), ty: "INTEGER".into(), pk: true, autoinc: true, ..Default::default() }]
}

#[component]
pub fn CreateTableDialog(
    open: RwSignal<bool>,
    in_txn: RwSignal<bool>,
    on_changed: Callback<bool>,
    on_open: Callback<TabKind>,
) -> impl IntoView {
    let toast = expect_context::<ToastService>();

    let table_name = RwSignal::new(String::new());
    let columns = RwSignal::new(starter_columns());
    let without_rowid = RwSignal::new(false);
    let creating = RwSignal::new(false);
    let preview = Signal::derive(move || {
        columns.with(|cols| sql::create_table(&table_name.get(), cols, without_rowid.get()))
    });

    let reset = move || {
        table_name.set(String::new());
        columns.set(starter_columns());
        without_rowid.set(false);
    };

    let on_add_column = Callback::new(move |_: ()| columns.update(|c| c.push(ColumnDef::default())));
    let on_remove_column = Callback::new(move |i: usize| {
        columns.update(|c| {
            if i < c.len() {
                c.remove(i);
            }
        })
    });
    let on_update = Callback::new(move |(i, field): (usize, ColumnField)| {
        columns.update(|cols| {
            let Some(c) = cols.get_mut(i) else { return };
            match field {
                ColumnField::Name(v) => c.name = v,
                ColumnField::Type(v) => c.ty = v,
                ColumnField::Pk(v) => c.pk = v,
                ColumnField::AutoInc(v) => c.autoinc = v,
                ColumnField::NotNull(v) => c.not_null = v,
                ColumnField::Unique(v) => c.unique = v,
                ColumnField::Default(v) => c.default = v,
            }
        })
    });
    let on_cancel = Callback::new(move |_: ()| {
        open.set(false);
        reset();
    });
    let on_create = Callback::new(move |_: ()| {
        let name = table_name.get_untracked().trim().to_string();
        if name.is_empty() {
            toast.error("Enter a table name.");
            return;
        }
        if columns.with_untracked(|c| c.is_empty() || c.iter().any(|col| col.name.trim().is_empty())) {
            toast.error("Every column needs a name.");
            return;
        }
        let statement = preview.get_untracked();
        creating.set(true);
        spawn_local(async move {
            if DatabaseService::begin_edit(in_txn, toast).await {
                if let Some(out) = DatabaseService::execute(&statement, &[], toast).await {
                    on_changed.run(out.in_txn);
                    on_open.run(TabKind::Browse(name));
                    open.set(false);
                    reset();
                }
            }
            creating.set(false);
        });
    });

    create_table_dialog_view(
        open,
        table_name,
        columns.into(),
        without_rowid,
        preview,
        creating.into(),
        on_add_column,
        on_remove_column,
        on_update,
        on_create,
        on_cancel,
    )
}
