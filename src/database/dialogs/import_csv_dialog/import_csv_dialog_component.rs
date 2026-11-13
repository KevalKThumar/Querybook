//! ImportCsvDialog — preview a CSV/TSV file and import it into a new table
//! (TEXT columns, like DB Browser) or an existing one. Runs inside the
//! pending savepoint; opens the table afterwards.
//!
//! Author: Keval Thumar

use leptos::prelude::*;
use leptos::task::spawn_local;
use shared::format::count;
use shared::CsvPreview;

use super::import_csv_dialog_view::import_csv_dialog_view;
use crate::database::database_structure::{DatabaseService, TabKind};
use crate::shared::services::toast_service::ToastService;

#[component]
pub fn ImportCsvDialog(
    open: RwSignal<bool>,
    /// File picked by the caller before opening the dialog.
    #[prop(into)]
    path: Signal<Option<String>>,
    in_txn: RwSignal<bool>,
    on_changed: Callback<bool>,
    on_open: Callback<TabKind>,
) -> impl IntoView {
    let toast = expect_context::<ToastService>();

    let table_name = RwSignal::new(String::new());
    let separator = RwSignal::new(',');
    let header = RwSignal::new(true);
    let preview = RwSignal::new(None::<CsvPreview>);
    let importing = RwSignal::new(false);
    let file_label = Signal::derive(move || path.get().unwrap_or_default());

    // New file: default the table name to the file stem, TSV → tab.
    Effect::new(move |_| {
        if let Some(p) = path.get() {
            let file = p.rsplit(['/', '\\']).next().unwrap_or(&p);
            let (stem, ext) = file.rsplit_once('.').unwrap_or((file, ""));
            table_name.set(stem.to_string());
            separator.set(if ext.eq_ignore_ascii_case("tsv") { '\t' } else { ',' });
        }
    });

    // Re-read the preview when the file or parsing options change.
    Effect::new(move |_| {
        let (Some(p), true) = (path.get(), open.get()) else { return };
        let (sep, has_header) = (separator.get(), header.get());
        spawn_local(async move {
            preview.set(DatabaseService::csv_preview(&p, sep, has_header, toast).await);
        });
    });

    let on_separator = Callback::new(move |value: String| separator.set(value.chars().next().unwrap_or(',')));
    let on_cancel = Callback::new(move |_: ()| open.set(false));
    let on_import = Callback::new(move |_: ()| {
        let Some(p) = path.get_untracked() else { return };
        let table = table_name.get_untracked().trim().to_string();
        if table.is_empty() {
            toast.error("Enter a table name.");
            return;
        }
        let (sep, has_header) = (separator.get_untracked(), header.get_untracked());
        importing.set(true);
        spawn_local(async move {
            if DatabaseService::begin_edit(in_txn, toast).await {
                if let Some(out) = DatabaseService::import_csv(&p, &table, sep, has_header, toast).await {
                    toast.success(&format!("Imported {} into “{table}”.", count(out.affected, "row")));
                    on_changed.run(out.in_txn);
                    on_open.run(TabKind::Browse(table));
                    open.set(false);
                }
            }
            importing.set(false);
        });
    });

    import_csv_dialog_view(
        open,
        file_label,
        table_name,
        separator.into(),
        header,
        preview.into(),
        importing.into(),
        on_separator,
        on_import,
        on_cancel,
    )
}
