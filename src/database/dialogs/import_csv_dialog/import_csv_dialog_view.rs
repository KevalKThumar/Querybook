//! Template for the Import CSV dialog.
//!
//! Author: Keval Thumar

use leptos::prelude::*;
use shared::CsvPreview;

use crate::database::database_constant::CSV_SEPARATORS;
use crate::shared::dialogs::dialog_shell::dialog_shell_component::DialogShell;

#[allow(clippy::too_many_arguments)]
pub fn import_csv_dialog_view(
    open: RwSignal<bool>,
    file_label: Signal<String>,
    table_name: RwSignal<String>,
    separator: Signal<char>,
    header: RwSignal<bool>,
    preview: Signal<Option<CsvPreview>>,
    importing: Signal<bool>,
    on_separator: Callback<String>,
    on_import: Callback<()>,
    on_cancel: Callback<()>,
) -> impl IntoView {
    view! {
        <DialogShell open on_close=on_cancel class="shell-dialog--wide">
            <div class="shell-header">"Import CSV"</div>
            <div class="shell-body">
                <div class="csvimp-file text-truncate mb-3" title=move || file_label.get()>
                    <i class="bi bi-filetype-csv"></i>" "{move || file_label.get()}
                </div>
                <div class="d-flex align-items-end gap-3 mb-3">
                    <div class="flex-grow-1">
                        <label class="form-label csvimp-label">"Table (new or existing)"</label>
                        <input
                            class="form-control form-control-sm"
                            prop:value=move || table_name.get()
                            on:input=move |ev| table_name.set(event_target_value(&ev))
                        />
                    </div>
                    <div>
                        <label class="form-label csvimp-label">"Separator"</label>
                        <select class="form-select form-select-sm" on:change=move |ev| on_separator.run(event_target_value(&ev))>
                            {CSV_SEPARATORS
                                .iter()
                                .map(|(label, sep)| {
                                    let sep = *sep;
                                    view! { <option value=sep.to_string() selected=move || separator.get() == sep>{*label}</option> }
                                })
                                .collect_view()}
                        </select>
                    </div>
                    <label class="csvimp-check d-flex align-items-center gap-1 mb-1">
                        <input
                            class="form-check-input mt-0"
                            type="checkbox"
                            prop:checked=move || header.get()
                            on:change=move |ev| header.set(event_target_checked(&ev))
                        />
                        "First row is the header"
                    </label>
                </div>

                <div class="csvimp-preview">
                    <Show
                        when=move || preview.with(|p| p.is_some())
                        fallback=|| view! { <div class="p-3 text-body-secondary small">"Reading file…"</div> }
                    >
                        <table class="table table-sm mb-0">
                            <thead>
                                <tr>
                                    {move || preview.with(|p| {
                                        p.as_ref().map(|p| p.columns.iter().map(|c| view! { <th>{c.clone()}</th> }).collect_view())
                                    })}
                                </tr>
                            </thead>
                            <tbody>
                                {move || preview.with(|p| {
                                    p.as_ref().map(|p| {
                                        p.rows
                                            .iter()
                                            .map(|row| view! {
                                                <tr>{row.iter().map(|v| view! { <td>{v.clone()}</td> }).collect_view()}</tr>
                                            })
                                            .collect_view()
                                    })
                                })}
                            </tbody>
                        </table>
                    </Show>
                </div>
                <div class="csvimp-note mt-2">"New tables get TEXT columns; existing tables are filled column by column."</div>
            </div>
            <div class="shell-footer">
                <button class="btn btn-sm btn-outline-secondary" on:click=move |_| on_cancel.run(())>"Cancel"</button>
                <button class="btn btn-sm btn-primary" disabled=move || importing.get() on:click=move |_| on_import.run(())>
                    "Import"
                </button>
            </div>
        </DialogShell>
    }
}
