//! Template for the database window.
//!
//! Author: Keval Thumar

use leptos::prelude::*;
use shared::KeyInput;

use crate::database::components::data_grid::data_grid_component::DataGrid;
use crate::database::components::database_info::database_info_component::DatabaseInfo;
use crate::database::components::schema_sidebar::schema_sidebar_component::SchemaSidebar;
use crate::database::components::sql_editor::sql_editor_component::SqlEditor;
use crate::database::components::structure_view::structure_view_component::StructureView;
use crate::database::database_structure::{CipherMode, FkJump, SchemaObject, Tab, TabKind};
use crate::database::dialogs::cipher_dialog::cipher_dialog_component::CipherDialog;
use crate::database::dialogs::create_table_dialog::create_table_dialog_component::CreateTableDialog;
use crate::database::dialogs::import_csv_dialog::import_csv_dialog_component::ImportCsvDialog;
use crate::shared::common_structure::ThemeMode;
use crate::shared::services::platform_service::PlatformService;

#[allow(clippy::too_many_arguments)]
pub fn browser_view(
    connected: Signal<bool>,
    encrypted: Signal<bool>,
    locked: Signal<bool>,
    file_name: Signal<String>,
    file_path: Signal<String>,
    schema: RwSignal<Vec<SchemaObject>>,
    tabs: RwSignal<Vec<Tab>>,
    active_tab: Signal<Option<u32>>,
    in_txn: RwSignal<bool>,
    data_version: Signal<u32>,
    jump: RwSignal<Option<FkJump>>,
    saving: Signal<bool>,
    theme_mode: Signal<ThemeMode>,
    cipher_open: RwSignal<bool>,
    cipher_mode: Signal<CipherMode>,
    cipher_error: RwSignal<Option<String>>,
    cipher_busy: Signal<bool>,
    create_table_open: RwSignal<bool>,
    import_csv_open: RwSignal<bool>,
    import_csv_path: Signal<Option<String>>,
    on_open_db: Callback<bool>,
    open_tab: Callback<TabKind>,
    new_sql_tab: Callback<()>,
    select_tab: Callback<u32>,
    close_tab: Callback<u32>,
    on_changed: Callback<bool>,
    on_write: Callback<()>,
    on_revert: Callback<()>,
    on_unlock: Callback<()>,
    on_encryption: Callback<()>,
    on_new_table: Callback<()>,
    on_import_csv: Callback<()>,
    on_import_sql: Callback<()>,
    on_export_sql: Callback<()>,
    on_theme: Callback<()>,
    on_cipher_submit: Callback<Option<KeyInput>>,
    on_cipher_cancel: Callback<()>,
) -> impl IntoView {
    let disconnected = move || !connected.get();
    view! {
        <div class="browser d-flex flex-column h-100">
            // ── Toolbar ──
            <div class="browser-toolbar d-flex align-items-center gap-1 px-2">
                <button class="btn btn-sm browser-tool" title={PlatformService::keys("Open database (⌘O)")} on:click=move |_| on_open_db.run(false)>
                    <i class="bi bi-folder2-open"></i><span>"Open"</span>
                </button>
                <button class="btn btn-sm browser-tool" title={PlatformService::keys("New database (⌘N)")} on:click=move |_| on_open_db.run(true)>
                    <i class="bi bi-file-earmark-plus"></i><span>"New"</span>
                </button>
                <div class="vr mx-1"></div>
                <button class="btn btn-sm browser-tool" title="Create a table" disabled=disconnected on:click=move |_| on_new_table.run(())>
                    <i class="bi bi-table"></i><span>"New Table"</span>
                </button>
                <button class="btn btn-sm browser-tool" title={PlatformService::keys("New SQL tab (⌘T)")} disabled=disconnected on:click=move |_| new_sql_tab.run(())>
                    <i class="bi bi-terminal"></i><span>"SQL"</span>
                </button>
                <div class="vr mx-1"></div>
                <button class="btn btn-sm browser-tool" title="Import a CSV file into a table" disabled=disconnected on:click=move |_| on_import_csv.run(())>
                    <i class="bi bi-filetype-csv"></i><span>"Import CSV"</span>
                </button>
                <button class="btn btn-sm browser-tool" title="Run a .sql script" disabled=disconnected on:click=move |_| on_import_sql.run(())>
                    <i class="bi bi-file-earmark-code"></i><span>"Import SQL"</span>
                </button>
                <button class="btn btn-sm browser-tool" title="Export the whole database as SQL" disabled=disconnected on:click=move |_| on_export_sql.run(())>
                    <i class="bi bi-box-arrow-up"></i><span>"Export SQL"</span>
                </button>
                <div class="vr mx-1"></div>
                <button class="btn btn-sm browser-tool" title="Database info and maintenance (integrity check, VACUUM…)" disabled=disconnected on:click=move |_| open_tab.run(TabKind::Info)>
                    <i class="bi bi-info-square"></i><span>"Info"</span>
                </button>

                <div class="ms-auto"></div>

                <Show when=move || in_txn.get() fallback=|| ()>
                    <span class="browser-pending me-1"><i class="bi bi-circle-fill"></i>" Unsaved changes"</span>
                    <button class="btn btn-sm btn-outline-secondary" disabled=move || saving.get() on:click=move |_| on_revert.run(())>
                        <i class="bi bi-arrow-counterclockwise"></i>" Revert"
                    </button>
                    <button class="btn btn-sm btn-primary" title={PlatformService::keys("⌘S")} disabled=move || saving.get() on:click=move |_| on_write.run(())>
                        <i class="bi bi-check2"></i>" Write Changes"
                    </button>
                    <div class="vr mx-1"></div>
                </Show>
                <button
                    class="btn btn-sm browser-tool"
                    title="Set, change or remove the SQLCipher password"
                    disabled=disconnected
                    on:click=move |_| on_encryption.run(())
                >
                    <i class=move || if encrypted.get() { "bi bi-lock-fill" } else { "bi bi-unlock" }></i>
                    <span>"Encryption"</span>
                </button>
                <button class="btn btn-sm browser-tool" title="Theme" on:click=move |_| on_theme.run(())>
                    <i class=move || theme_mode.get().icon()></i>
                    <span>{move || theme_mode.get().label()}</span>
                </button>
            </div>

            // ── Sidebar + tabs ──
            <div class="d-flex flex-grow-1 overflow-hidden">
                <Show
                    when=move || connected.get()
                    fallback=move || view! {
                        <div class="empty-state flex-grow-1">
                            <Show
                                when=move || locked.get()
                                fallback=|| view! { <div class="spinner-border spinner-border-sm"></div><div>"Opening…"</div> }
                            >
                                <i class="bi bi-lock"></i>
                                <div>{move || format!("{} is locked.", file_name.get())}</div>
                                <button class="btn btn-sm btn-primary" on:click=move |_| on_unlock.run(())>"Unlock…"</button>
                            </Show>
                        </div>
                    }
                >
                    <SchemaSidebar schema=schema on_open=open_tab />
                    <div class="browser-main d-flex flex-column flex-grow-1 overflow-hidden">
                        <div class="browser-tabs d-flex align-items-stretch">
                            <For
                                each=move || tabs.get()
                                key=|tab| tab.id
                                children=move |tab| {
                                    let id = tab.id;
                                    view! {
                                        <div
                                            class="browser-tab d-flex align-items-center gap-2"
                                            class:active=move || active_tab.get() == Some(id)
                                            title=tab.title()
                                            on:click=move |_| select_tab.run(id)
                                        >
                                            <i class=tab.icon()></i>
                                            <span class="text-truncate">{tab.title()}</span>
                                            <button
                                                class="icon-btn browser-tab-close"
                                                title="Close tab"
                                                on:click=move |ev| {
                                                    ev.stop_propagation();
                                                    close_tab.run(id);
                                                }
                                            >
                                                <i class="bi bi-x"></i>
                                            </button>
                                        </div>
                                    }
                                }
                            />
                            <button class="icon-btn browser-tab-add" title={PlatformService::keys("New SQL tab (⌘T)")} on:click=move |_| new_sql_tab.run(())>
                                <i class="bi bi-plus-lg"></i>
                            </button>
                        </div>

                        <div class="browser-panes flex-grow-1 position-relative">
                            <Show when=move || tabs.with(|t| t.is_empty()) fallback=|| ()>
                                <div class="empty-state">
                                    <i class="bi bi-layout-sidebar-inset"></i>
                                    <div>"Pick a table on the left, or open a SQL tab."</div>
                                    <button class="btn btn-sm btn-outline-secondary" on:click=move |_| new_sql_tab.run(())>
                                        <i class="bi bi-terminal"></i>" New SQL tab"
                                    </button>
                                </div>
                            </Show>
                            // Every tab stays mounted (hidden when inactive) so editors keep their text and grids their scroll.
                            <For
                                each=move || tabs.get()
                                key=|tab| tab.id
                                children=move |tab| {
                                    let id = tab.id;
                                    let content = match tab.kind {
                                        TabKind::Browse(name) => view! {
                                            <DataGrid name=name schema=schema in_txn=in_txn data_version=data_version on_changed=on_changed on_open=open_tab jump=jump />
                                        }.into_any(),
                                        TabKind::Structure(name) => view! {
                                            <StructureView name=name schema=schema in_txn=in_txn data_version=data_version on_changed=on_changed on_open=open_tab />
                                        }.into_any(),
                                        TabKind::Sql(_) => view! {
                                            <SqlEditor in_txn=in_txn on_changed=on_changed schema=schema data_version=data_version db_path=file_path />
                                        }.into_any(),
                                        TabKind::Info => view! {
                                            <DatabaseInfo in_txn=in_txn data_version=data_version file_path=file_path encrypted=encrypted schema=schema on_changed=on_changed />
                                        }.into_any(),
                                    };
                                    view! {
                                        <div class="browser-pane" class=("d-none", move || active_tab.get() != Some(id))>
                                            {content}
                                        </div>
                                    }
                                }
                            />
                        </div>
                    </div>
                </Show>
            </div>

            // ── Status bar ──
            <div class="browser-status d-flex align-items-center gap-3 px-2">
                <span class="text-truncate" title=move || file_path.get()>
                    <i class="bi bi-database"></i>" "{move || file_path.get()}
                </span>
                <span class="ms-auto text-nowrap">
                    {move || if encrypted.get() {
                        view! { <i class="bi bi-lock-fill"></i>" SQLCipher 4" }.into_any()
                    } else {
                        view! { <i class="bi bi-unlock"></i>" Not encrypted" }.into_any()
                    }}
                </span>
                <span class="text-nowrap">
                    {move || format!("{} tables", schema.with(|s| s.iter().filter(|o| o.kind == "table").count()))}
                </span>
            </div>

            <CipherDialog
                open=cipher_open
                mode=cipher_mode
                file_name=file_name
                error=cipher_error
                busy=cipher_busy
                on_submit=on_cipher_submit
                on_cancel=on_cipher_cancel
            />
            <CreateTableDialog open=create_table_open in_txn=in_txn on_changed=on_changed on_open=open_tab />
            <ImportCsvDialog open=import_csv_open path=import_csv_path in_txn=in_txn on_changed=on_changed on_open=open_tab />
        </div>
    }
}
