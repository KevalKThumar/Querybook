//! Template for the Create Table dialog.
//!
//! Author: Keval Thumar

use leptos::prelude::*;
use shared::sql::ColumnDef;

use crate::database::components::sql_code::sql_code_component::SqlCode;
use crate::database::database_constant::COLUMN_TYPES;
use crate::database::database_structure::ColumnField;
use crate::shared::dialogs::dialog_shell::dialog_shell_component::DialogShell;

#[allow(clippy::too_many_arguments)]
pub fn create_table_dialog_view(
    open: RwSignal<bool>,
    table_name: RwSignal<String>,
    columns: Signal<Vec<ColumnDef>>,
    without_rowid: RwSignal<bool>,
    preview: Signal<String>,
    creating: Signal<bool>,
    on_add_column: Callback<()>,
    on_remove_column: Callback<usize>,
    on_update: Callback<(usize, ColumnField)>,
    on_create: Callback<()>,
    on_cancel: Callback<()>,
) -> impl IntoView {
    view! {
        <DialogShell open on_close=on_cancel class="shell-dialog--wide">
            <div class="shell-header">"Create table"</div>
            <div class="shell-body">
                <div class="d-flex align-items-end gap-3 mb-3">
                    <div class="flex-grow-1">
                        <label class="form-label newtable-label">"Table name"</label>
                        <input
                            class="form-control form-control-sm"
                            placeholder="e.g. customers"
                            prop:value=move || table_name.get()
                            on:input=move |ev| table_name.set(event_target_value(&ev))
                        />
                    </div>
                    <label class="newtable-check d-flex align-items-center gap-1 mb-1">
                        <input
                            class="form-check-input mt-0"
                            type="checkbox"
                            prop:checked=move || without_rowid.get()
                            on:change=move |ev| without_rowid.set(event_target_checked(&ev))
                        />
                        "WITHOUT ROWID"
                    </label>
                </div>

                <table class="table table-sm align-middle newtable-columns mb-2">
                    <thead>
                        <tr>
                            <th>"Name"</th>
                            <th>"Type"</th>
                            <th class="text-center" title="Primary key">"PK"</th>
                            <th class="text-center" title="Autoincrement">"AI"</th>
                            <th class="text-center" title="Not null">"NN"</th>
                            <th class="text-center" title="Unique">"U"</th>
                            <th>"Default"</th>
                            <th></th>
                        </tr>
                    </thead>
                    <tbody>
                        <For
                            each={move || (0..columns.with(|c| c.len())).collect::<Vec<_>>()}
                            key=|i| *i
                            children=move |i| {
                                let get = move |f: fn(&ColumnDef) -> String| move || columns.with(|c| c.get(i).map(f).unwrap_or_default());
                                let flag = move |f: fn(&ColumnDef) -> bool| move || columns.with(|c| c.get(i).is_some_and(f));
                                view! {
                                    <tr>
                                        <td>
                                            <input class="form-control form-control-sm" placeholder="column"
                                                prop:value=get(|c| c.name.clone())
                                                on:input=move |ev| on_update.run((i, ColumnField::Name(event_target_value(&ev)))) />
                                        </td>
                                        <td class="newtable-type">
                                            <input class="form-control form-control-sm" list="querybook-new-types" placeholder="TEXT"
                                                prop:value=get(|c| c.ty.clone())
                                                on:input=move |ev| on_update.run((i, ColumnField::Type(event_target_value(&ev)))) />
                                        </td>
                                        <td class="text-center">
                                            <input class="form-check-input" type="checkbox" prop:checked=flag(|c| c.pk)
                                                on:change=move |ev| on_update.run((i, ColumnField::Pk(event_target_checked(&ev)))) />
                                        </td>
                                        <td class="text-center">
                                            <input class="form-check-input" type="checkbox" prop:checked=flag(|c| c.autoinc)
                                                on:change=move |ev| on_update.run((i, ColumnField::AutoInc(event_target_checked(&ev)))) />
                                        </td>
                                        <td class="text-center">
                                            <input class="form-check-input" type="checkbox" prop:checked=flag(|c| c.not_null)
                                                on:change=move |ev| on_update.run((i, ColumnField::NotNull(event_target_checked(&ev)))) />
                                        </td>
                                        <td class="text-center">
                                            <input class="form-check-input" type="checkbox" prop:checked=flag(|c| c.unique)
                                                on:change=move |ev| on_update.run((i, ColumnField::Unique(event_target_checked(&ev)))) />
                                        </td>
                                        <td>
                                            <input class="form-control form-control-sm" placeholder="none"
                                                prop:value=get(|c| c.default.clone())
                                                on:input=move |ev| on_update.run((i, ColumnField::Default(event_target_value(&ev)))) />
                                        </td>
                                        <td class="text-end">
                                            <button class="icon-btn text-danger" title="Remove column" on:click=move |_| on_remove_column.run(i)>
                                                <i class="bi bi-x-lg"></i>
                                            </button>
                                        </td>
                                    </tr>
                                }
                            }
                        />
                    </tbody>
                </table>
                <datalist id="querybook-new-types">
                    {COLUMN_TYPES.iter().map(|t| view! { <option value=*t></option> }).collect_view()}
                </datalist>
                <button class="btn btn-sm btn-outline-secondary mb-3" on:click=move |_| on_add_column.run(())>
                    <i class="bi bi-plus-lg"></i>" Add column"
                </button>

                <label class="form-label newtable-label d-block">"SQL"</label>
                <SqlCode sql=preview class="newtable-preview" />
            </div>
            <div class="shell-footer">
                <button class="btn btn-sm btn-outline-secondary" on:click=move |_| on_cancel.run(())>"Cancel"</button>
                <button class="btn btn-sm btn-primary" disabled=move || creating.get() on:click=move |_| on_create.run(())>
                    "Create table"
                </button>
            </div>
        </DialogShell>
    }
}
