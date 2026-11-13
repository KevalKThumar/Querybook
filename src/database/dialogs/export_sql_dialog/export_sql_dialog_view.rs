//! Template for the Export SQL dialog.
//!
//! Author: Keval Thumar

use std::collections::BTreeSet;

use leptos::prelude::*;
use shared::sql::ROWS_PER_INSERT;
use shared::{ExportContent, SqlExportOptions};

use crate::database::database_constant::EXPORT_CONTENTS;
use crate::database::database_structure::ExportItem;
use crate::shared::dialogs::dialog_shell::dialog_shell_component::DialogShell;

#[allow(clippy::too_many_arguments)]
pub fn export_sql_dialog_view(
    open: RwSignal<bool>,
    items: Signal<Option<Vec<ExportItem>>>,
    selected: Signal<BTreeSet<String>>,
    options: RwSignal<SqlExportOptions>,
    total: Signal<usize>,
    ticked: Signal<usize>,
    nothing_to_export: Signal<bool>,
    exporting: Signal<bool>,
    encrypted: Signal<bool>,
    on_toggle: Callback<String>,
    on_toggle_all: Callback<bool>,
    on_export: Callback<()>,
    on_cancel: Callback<()>,
) -> impl IntoView {
    let content = move || options.with(|o| o.content);
    let data_only = move || content() == ExportContent::DataOnly;
    let schema_only = move || content() == ExportContent::SchemaOnly;
    // Outside the template: `<` and `>` there would end the tag.
    let all_ticked = move || total.get() > 0 && ticked.get() == total.get();
    let some_ticked = move || ticked.get() > 0 && ticked.get() < total.get();
    let empty = move || total.get() == 0;

    view! {
        <DialogShell open on_close=on_cancel class="sqlexp-dialog">
            <div class="shell-header">"Export SQL"</div>
            <div class="shell-body">
                <label class="sqlexp-check sqlexp-all">
                    <input
                        class="form-check-input mt-0"
                        type="checkbox"
                        prop:checked=all_ticked
                        prop:indeterminate=some_ticked
                        disabled=empty
                        on:change=move |ev| on_toggle_all.run(event_target_checked(&ev))
                    />
                    {move || format!("All tables and views ({})", total.get())}
                </label>
                <div class="sqlexp-list">
                    {move || match items.get() {
                        None => view! { <div class="sqlexp-empty">"Reading the schema…"</div> }.into_any(),
                        Some(list) if list.is_empty() => {
                            view! { <div class="sqlexp-empty">"This database has no tables yet."</div> }.into_any()
                        }
                        Some(list) => list
                            .into_iter()
                            .map(|item| {
                                let is_view = item.is_view;
                                let name = item.name.clone();
                                let toggled = item.name.clone();
                                view! {
                                    <label class="sqlexp-check sqlexp-item" class:sqlexp-nodata=move || is_view && data_only()>
                                        <input
                                            class="form-check-input mt-0"
                                            type="checkbox"
                                            prop:checked=move || selected.with(|s| s.contains(&name))
                                            on:change=move |_| on_toggle.run(toggled.clone())
                                        />
                                        <i class=if is_view { "bi bi-eye" } else { "bi bi-table" }></i>
                                        <span class="text-truncate">{item.name}</span>
                                        {is_view.then(|| view! {
                                            <span class="sqlexp-hint">{move || if data_only() { "view, no data" } else { "view" }}</span>
                                        })}
                                    </label>
                                }
                            })
                            .collect_view()
                            .into_any(),
                    }}
                </div>

                <div class="sqlexp-options">
                    <div class="sqlexp-label">"Export"</div>
                    <div class="d-flex flex-wrap column-gap-3 row-gap-1">
                        {EXPORT_CONTENTS
                            .iter()
                            .map(|(label, value)| {
                                let value = *value;
                                radio("sqlexp-content", label, move || content() == value, move || options.update(|o| o.content = value))
                            })
                            .collect_view()}
                    </div>

                    <Show when=move || !data_only() fallback=|| ()>
                        <div class="sqlexp-label">"If a table exists"</div>
                        <div class="d-flex flex-column gap-1">
                            {radio(
                                "sqlexp-existing",
                                "Drop it and create it again",
                                move || options.with(|o| o.drop_existing),
                                move || options.update(|o| o.drop_existing = true),
                            )}
                            {radio(
                                "sqlexp-existing",
                                "Keep it (CREATE … IF NOT EXISTS)",
                                move || options.with(|o| !o.drop_existing),
                                move || options.update(|o| o.drop_existing = false),
                            )}
                        </div>
                    </Show>

                    <div class="sqlexp-label">"Options"</div>
                    <div class="d-flex flex-column gap-1">
                        <Show when=data_only fallback=|| ()>
                            {check(options, "Delete the existing rows first (DELETE FROM)", |o| o.clear_rows, |o, on| o.clear_rows = on)}
                        </Show>
                        <Show when=move || !data_only() fallback=|| ()>
                            {check(options, "Indexes and triggers", |o| o.indexes_triggers, |o, on| o.indexes_triggers = on)}
                        </Show>
                        <Show when=move || !schema_only() fallback=|| ()>
                            {check(options, "Column names in INSERT", |o| o.column_names, |o, on| o.column_names = on)}
                            {check(
                                options,
                                format!("Several rows per INSERT (up to {ROWS_PER_INSERT})"),
                                |o| o.multi_row,
                                |o, on| o.multi_row = on,
                            )}
                        </Show>
                        {check(options, "Wrap in a transaction (BEGIN … COMMIT)", |o| o.transaction, |o, on| o.transaction = on)}
                    </div>
                </div>

                <Show when=move || encrypted.get() fallback=|| ()>
                    <div class="sqlexp-warn">
                        <i class="bi bi-exclamation-triangle"></i>
                        " The SQL file is plain text: it isn't encrypted."
                    </div>
                </Show>
            </div>
            <div class="shell-footer">
                <span class="me-auto sqlexp-count">{move || format!("{} of {} selected", ticked.get(), total.get())}</span>
                <button class="btn btn-sm btn-outline-secondary" on:click=move |_| on_cancel.run(())>"Cancel"</button>
                <button
                    class="btn btn-sm btn-primary"
                    disabled=move || nothing_to_export.get() || exporting.get()
                    on:click=move |_| on_export.run(())
                >
                    "Export"
                </button>
            </div>
        </DialogShell>
    }
}

/// One radio button of the group `name`.
fn radio(
    name: &'static str,
    label: &'static str,
    checked: impl Fn() -> bool + Send + Sync + 'static,
    pick: impl Fn() + Send + Sync + 'static,
) -> impl IntoView {
    view! {
        <label class="sqlexp-check">
            <input class="form-check-input mt-0" type="radio" name=name prop:checked=checked on:change=move |_| pick() />
            {label}
        </label>
    }
}

/// A checkbox bound to one flag of the options.
fn check(
    options: RwSignal<SqlExportOptions>,
    label: impl Into<String>,
    get: fn(&SqlExportOptions) -> bool,
    set: fn(&mut SqlExportOptions, bool),
) -> impl IntoView {
    let label: String = label.into();
    view! {
        <label class="sqlexp-check">
            <input
                class="form-check-input mt-0"
                type="checkbox"
                prop:checked=move || options.with(get)
                on:change=move |ev| {
                    let on = event_target_checked(&ev);
                    options.update(|o| set(o, on));
                }
            />
            {label}
        </label>
    }
}
