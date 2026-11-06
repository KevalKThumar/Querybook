//! Template for the structure tab.
//!
//! Author: Keval Thumar

use leptos::prelude::*;
use shared::sql::ColumnDef;

use crate::database::components::sql_code::sql_code_component::SqlCode;
use crate::database::database_constant::COLUMN_TYPES;
use crate::database::database_structure::{ColumnInfo, SchemaObject};
use crate::shared::dialogs::confirmation_dialog::confirmation_dialog_component::ConfirmationDialog;
use crate::shared::dialogs::prompt_dialog::prompt_dialog_component::PromptDialog;

#[allow(clippy::too_many_arguments)]
pub fn structure_view_view(
    object: Memo<Option<SchemaObject>>,
    is_table: Signal<bool>,
    create_sql: Signal<String>,
    columns: Signal<Vec<ColumnInfo>>,
    indexes: Memo<Vec<SchemaObject>>,
    new_column: RwSignal<ColumnDef>,
    index_name: RwSignal<String>,
    index_columns: Signal<Vec<String>>,
    index_unique: RwSignal<bool>,
    rename_open: RwSignal<bool>,
    rename_title: Signal<String>,
    rename_value: RwSignal<String>,
    drop_open: RwSignal<bool>,
    drop_message: Signal<String>,
    on_browse: Callback<()>,
    on_rename_object: Callback<()>,
    on_rename_column: Callback<String>,
    on_rename_submit: Callback<String>,
    on_rename_cancel: Callback<()>,
    on_drop_object: Callback<()>,
    on_drop_column: Callback<String>,
    on_drop_index: Callback<String>,
    on_drop_confirm: Callback<()>,
    on_drop_cancel: Callback<()>,
    on_add_column: Callback<()>,
    on_toggle_index_column: Callback<String>,
    on_create_index: Callback<()>,
) -> impl IntoView {
    let field = move |f: fn(&ColumnDef) -> String| move || new_column.with(f);
    view! {
        <div class="struct h-100 overflow-auto">
            <div class="struct-inner">
                <div class="struct-header d-flex align-items-center gap-2 mb-3">
                    <i class=move || object.with(|o| o.as_ref().map_or("bi bi-question-circle", |o| o.icon()))></i>
                    <h2 class="struct-title mb-0 text-truncate">{move || object.with(|o| o.as_ref().map(|o| o.name.clone()).unwrap_or_default())}</h2>
                    <span class="badge text-bg-secondary">{move || object.with(|o| o.as_ref().map(|o| o.kind.clone()).unwrap_or_default())}</span>
                    <div class="ms-auto d-flex gap-1">
                        <Show when=move || object.with(|o| o.as_ref().is_some_and(|o| o.is_browsable())) fallback=|| ()>
                            <button class="btn btn-sm btn-outline-secondary" on:click=move |_| on_browse.run(())>
                                <i class="bi bi-table"></i>" Browse data"
                            </button>
                        </Show>
                        <Show when=move || is_table.get() fallback=|| ()>
                            <button class="btn btn-sm btn-outline-secondary" on:click=move |_| on_rename_object.run(())>
                                <i class="bi bi-pencil"></i>" Rename"
                            </button>
                        </Show>
                        <button class="btn btn-sm btn-outline-danger" on:click=move |_| on_drop_object.run(())>
                            <i class="bi bi-trash"></i>" Drop"
                        </button>
                    </div>
                </div>

                <Show when=move || !columns.with(|c| c.is_empty()) fallback=|| ()>
                    <h3 class="struct-section">"Columns"</h3>
                    <table class="table table-sm table-hover align-middle struct-table">
                        <thead>
                            <tr>
                                <th class="struct-num">"#"</th>
                                <th>"Name"</th>
                                <th>"Type"</th>
                                <th class="text-center">"Not null"</th>
                                <th class="text-center">"PK"</th>
                                <th>"Default"</th>
                                <th></th>
                            </tr>
                        </thead>
                        <tbody>
                            <For
                                each={move || columns.get().into_iter().enumerate().collect::<Vec<_>>()}
                                key=|(_, c)| c.name.clone()
                                children=move |(i, col)| {
                                    let (rename, drop) = (col.name.clone(), col.name.clone());
                                    view! {
                                        <tr>
                                            <td class="struct-num">{i + 1}</td>
                                            <td class="fw-medium">
                                                {col.name.clone()}
                                                {(col.hidden >= 2).then(|| view! { <span class="badge text-bg-light ms-1">"generated"</span> })}
                                            </td>
                                            <td class="font-monospace">{col.ty.clone()}</td>
                                            <td class="text-center">{col.not_null.then(|| view! { <i class="bi bi-check-lg"></i> })}</td>
                                            <td class="text-center">{(col.pk > 0).then(|| view! { <i class="bi bi-key-fill text-warning"></i> })}</td>
                                            <td class="font-monospace text-body-secondary">{col.default.clone().unwrap_or_default()}</td>
                                            <td class="text-end text-nowrap">
                                                <Show when=move || is_table.get() fallback=|| ()>
                                                    <button class="icon-btn" title="Rename column" on:click={
                                                        let rename = rename.clone();
                                                        move |_| on_rename_column.run(rename.clone())
                                                    }>
                                                        <i class="bi bi-pencil"></i>
                                                    </button>
                                                    <button class="icon-btn text-danger" title="Drop column" on:click={
                                                        let drop = drop.clone();
                                                        move |_| on_drop_column.run(drop.clone())
                                                    }>
                                                        <i class="bi bi-trash"></i>
                                                    </button>
                                                </Show>
                                            </td>
                                        </tr>
                                    }
                                }
                            />
                        </tbody>
                    </table>
                </Show>

                <Show when=move || is_table.get() fallback=|| ()>
                    // ── Add column ──
                    <div class="struct-form d-flex align-items-end gap-2 mb-4">
                        <div class="flex-grow-1">
                            <label class="form-label struct-label">"New column"</label>
                            <input class="form-control form-control-sm" placeholder="name"
                                prop:value=field(|c| c.name.clone())
                                on:input=move |ev| new_column.update(|c| c.name = event_target_value(&ev)) />
                        </div>
                        <div class="struct-type">
                            <label class="form-label struct-label">"Type"</label>
                            <input class="form-control form-control-sm" list="querybook-types" placeholder="TEXT"
                                prop:value=field(|c| c.ty.clone())
                                on:input=move |ev| new_column.update(|c| c.ty = event_target_value(&ev)) />
                        </div>
                        <div class="struct-type">
                            <label class="form-label struct-label">"Default"</label>
                            <input class="form-control form-control-sm" placeholder="none"
                                prop:value=field(|c| c.default.clone())
                                on:input=move |ev| new_column.update(|c| c.default = event_target_value(&ev)) />
                        </div>
                        <label class="struct-check d-flex align-items-center gap-1 mb-1">
                            <input class="form-check-input mt-0" type="checkbox"
                                prop:checked=move || new_column.with(|c| c.not_null)
                                on:change=move |ev| new_column.update(|c| c.not_null = event_target_checked(&ev)) />
                            "Not null"
                        </label>
                        <button class="btn btn-sm btn-primary" on:click=move |_| on_add_column.run(())>
                            <i class="bi bi-plus-lg"></i>" Add column"
                        </button>
                    </div>
                    <datalist id="querybook-types">
                        {COLUMN_TYPES.iter().map(|t| view! { <option value=*t></option> }).collect_view()}
                    </datalist>

                    // ── Indexes ──
                    <h3 class="struct-section">"Indexes"</h3>
                    <Show
                        when=move || !indexes.with(|i| i.is_empty())
                        fallback=|| view! { <p class="text-body-secondary small">"No indexes."</p> }
                    >
                        <ul class="list-group list-group-flush mb-3 struct-indexes">
                            <For
                                each=move || indexes.get()
                                key=|ix| ix.name.clone()
                                children=move |ix| {
                                    let name = ix.name.clone();
                                    view! {
                                        <li class="list-group-item d-flex align-items-center gap-2 px-0">
                                            <i class="bi bi-sort-down text-body-secondary"></i>
                                            <span class="fw-medium">{ix.name.clone()}</span>
                                            <code class="text-truncate small">{ix.sql.clone()}</code>
                                            <button class="icon-btn text-danger ms-auto" title="Drop index" on:click=move |_| on_drop_index.run(name.clone())>
                                                <i class="bi bi-trash"></i>
                                            </button>
                                        </li>
                                    }
                                }
                            />
                        </ul>
                    </Show>
                    <div class="struct-form mb-4">
                        <div class="d-flex align-items-end gap-2 mb-2">
                            <div class="flex-grow-1">
                                <label class="form-label struct-label">"New index"</label>
                                <input class="form-control form-control-sm" placeholder="index name"
                                    prop:value=move || index_name.get()
                                    on:input=move |ev| index_name.set(event_target_value(&ev)) />
                            </div>
                            <label class="struct-check d-flex align-items-center gap-1 mb-1">
                                <input class="form-check-input mt-0" type="checkbox"
                                    prop:checked=move || index_unique.get()
                                    on:change=move |ev| index_unique.set(event_target_checked(&ev)) />
                                "Unique"
                            </label>
                            <button class="btn btn-sm btn-primary" on:click=move |_| on_create_index.run(())>
                                <i class="bi bi-plus-lg"></i>" Create index"
                            </button>
                        </div>
                        <div class="d-flex flex-wrap gap-1">
                            <For
                                each=move || columns.get()
                                key=|c| c.name.clone()
                                children=move |c| {
                                    let (check, toggle) = (c.name.clone(), c.name.clone());
                                    view! {
                                        <button
                                            class="btn btn-sm struct-chip"
                                            class:active=move || index_columns.with(|cols| cols.contains(&check))
                                            on:click=move |_| on_toggle_index_column.run(toggle.clone())
                                        >
                                            {c.name.clone()}
                                        </button>
                                    }
                                }
                            />
                        </div>
                    </div>
                </Show>

                <h3 class="struct-section">"SQL"</h3>
                <SqlCode sql=create_sql class="struct-sql" />
            </div>

            <PromptDialog
                open=rename_open
                title=rename_title
                label="New name"
                value=rename_value
                pos_label="Rename"
                on_submit=on_rename_submit
                on_cancel=on_rename_cancel
            />
            <ConfirmationDialog
                open=drop_open
                title="Drop"
                message=drop_message
                pos_label="Drop"
                danger=true
                on_confirm=on_drop_confirm
                on_cancel=on_drop_cancel
            />
        </div>
    }
}
