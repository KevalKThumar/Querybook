//! StructureView — columns, indexes and CREATE SQL of one object, with the
//! schema edits SQLite's ALTER TABLE supports: rename table, add / rename /
//! drop column, create / drop index, drop object. Every edit runs inside the
//! pending savepoint, so it can be reverted.
//!
//! Author: Keval Thumar

use leptos::prelude::*;
use leptos::task::spawn_local;
use shared::sql::{self, ColumnDef};

use super::structure_view_view::structure_view_view;
use crate::database::database_structure::{ColumnInfo, DatabaseService, DropTarget, RenameTarget, SchemaObject, TabKind};
use crate::shared::services::toast_service::ToastService;

// Keval Thumar
/// Structure tab for the object `name`.
#[component]
pub fn StructureView(
    name: String,
    #[prop(into)] schema: Signal<Vec<SchemaObject>>,
    in_txn: RwSignal<bool>,
    #[prop(into)] data_version: Signal<u32>,
    on_changed: Callback<bool>,
    on_open: Callback<TabKind>,
) -> impl IntoView {
    let toast = expect_context::<ToastService>();
    let name = StoredValue::new(name);

    let object = Memo::new(move |_| schema.with(|s| s.iter().find(|o| name.with_value(|n| o.name == *n)).cloned()));
    let is_table = Signal::derive(move || object.with(|o| o.as_ref().is_some_and(|o| o.kind == "table")));
    let create_sql = Signal::derive(move || object.with(|o| o.as_ref().map(|o| format!("{};", o.sql)).unwrap_or_default()));
    let indexes = Memo::new(move |_| {
        schema.with(|s| {
            s.iter()
                .filter(|o| o.kind == "index" && name.with_value(|n| o.table == *n))
                .cloned()
                .collect::<Vec<_>>()
        })
    });
    let columns = RwSignal::new(Vec::<ColumnInfo>::new());

    // Columns for tables/views; reload after every change.
    Effect::new(move |_| {
        data_version.track();
        spawn_local(async move {
            columns.set(DatabaseService::load_columns(&name.get_value(), toast).await);
        });
    });

    // Add-column and create-index forms.
    let new_column = RwSignal::new(ColumnDef::default());
    let index_name = RwSignal::new(String::new());
    let index_columns = RwSignal::new(Vec::<String>::new());
    let index_unique = RwSignal::new(false);

    // Rename / drop dialogs.
    let rename_open = RwSignal::new(false);
    let rename_value = RwSignal::new(String::new());
    let rename_target = RwSignal::new(None::<RenameTarget>);
    let drop_open = RwSignal::new(false);
    let drop_target = RwSignal::new(None::<DropTarget>);
    let rename_title = Signal::derive(move || match rename_target.get() {
        Some(RenameTarget::Column(c)) => format!("Rename column “{c}”"),
        _ => format!("Rename “{}”", name.get_value()),
    });
    let drop_message = Signal::derive(move || {
        let kind = object.with(|o| o.as_ref().map(|o| o.kind.clone()).unwrap_or_default());
        match drop_target.get() {
            Some(DropTarget::Column(c)) => format!("Drop column “{c}” from “{}”?", name.get_value()),
            Some(DropTarget::Index(i)) => format!("Drop index “{i}”?"),
            _ => format!("Drop {kind} “{}”? Its data is removed when you write changes.", name.get_value()),
        }
    });

    // Run one DDL statement inside the pending savepoint.
    let run_ddl = move |statement: String, then_open: Option<TabKind>| {
        spawn_local(async move {
            if !DatabaseService::begin_edit(in_txn, toast).await {
                return;
            }
            if let Some(out) = DatabaseService::execute(&statement, &[], toast).await {
                on_changed.run(out.in_txn);
                if let Some(tab) = then_open {
                    on_open.run(tab);
                }
            }
        });
    };

    let on_browse = Callback::new(move |_: ()| on_open.run(TabKind::Browse(name.get_value())));
    let on_rename_object = Callback::new(move |_: ()| {
        rename_target.set(Some(RenameTarget::Object));
        rename_value.set(name.get_value());
        rename_open.set(true);
    });
    let on_rename_column = Callback::new(move |column: String| {
        rename_value.set(column.clone());
        rename_target.set(Some(RenameTarget::Column(column)));
        rename_open.set(true);
    });
    let on_rename_submit = Callback::new(move |new_name: String| {
        rename_open.set(false);
        let table = name.get_value();
        match rename_target.get_untracked() {
            Some(RenameTarget::Column(old)) => run_ddl(sql::rename_column(&table, &old, &new_name), None),
            Some(RenameTarget::Object) => {
                run_ddl(sql::rename_table(&table, &new_name), Some(TabKind::Structure(new_name)))
            }
            None => {}
        }
    });
    let on_rename_cancel = Callback::new(move |_: ()| rename_open.set(false));

    let on_drop_object = Callback::new(move |_: ()| {
        drop_target.set(Some(DropTarget::Object));
        drop_open.set(true);
    });
    let on_drop_column = Callback::new(move |column: String| {
        drop_target.set(Some(DropTarget::Column(column)));
        drop_open.set(true);
    });
    let on_drop_index = Callback::new(move |index: String| {
        drop_target.set(Some(DropTarget::Index(index)));
        drop_open.set(true);
    });
    let on_drop_confirm = Callback::new(move |_: ()| {
        drop_open.set(false);
        let table = name.get_value();
        let kind = object.with_untracked(|o| o.as_ref().map(|o| o.kind.clone()).unwrap_or_default());
        match drop_target.get_untracked() {
            Some(DropTarget::Column(c)) => run_ddl(sql::drop_column(&table, &c), None),
            Some(DropTarget::Index(i)) => run_ddl(sql::drop_object("index", &i), None),
            Some(DropTarget::Object) => run_ddl(sql::drop_object(&kind, &table), None),
            None => {}
        }
    });
    let on_drop_cancel = Callback::new(move |_: ()| drop_open.set(false));

    let on_add_column = Callback::new(move |_: ()| {
        let column = new_column.get_untracked();
        if column.name.trim().is_empty() {
            toast.error("Enter a column name.");
            return;
        }
        run_ddl(sql::add_column(&name.get_value(), &column), None);
        new_column.set(ColumnDef::default());
    });
    let on_toggle_index_column = Callback::new(move |column: String| {
        index_columns.update(|cols| match cols.iter().position(|c| *c == column) {
            Some(i) => {
                cols.remove(i);
            }
            None => cols.push(column),
        })
    });
    let on_create_index = Callback::new(move |_: ()| {
        let (index, cols) = (index_name.get_untracked(), index_columns.get_untracked());
        if index.trim().is_empty() || cols.is_empty() {
            toast.error("Enter an index name and pick at least one column.");
            return;
        }
        run_ddl(sql::create_index(&index, &name.get_value(), &cols, index_unique.get_untracked()), None);
        index_name.set(String::new());
        index_columns.set(vec![]);
        index_unique.set(false);
    });

    structure_view_view(
        object,
        is_table,
        create_sql,
        columns.into(),
        indexes,
        new_column,
        index_name,
        index_columns.into(),
        index_unique,
        rename_open,
        rename_title,
        rename_value,
        drop_open,
        drop_message,
        on_browse,
        on_rename_object,
        on_rename_column,
        on_rename_submit,
        on_rename_cancel,
        on_drop_object,
        on_drop_column,
        on_drop_index,
        on_drop_confirm,
        on_drop_cancel,
        on_add_column,
        on_toggle_index_column,
        on_create_index,
    )
}
