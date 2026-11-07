//! DatabaseService — stateless Tauri-command layer for the database module.
//!
//! Static async fns over `OsBridge::invoke`. Each one shows a toast and logs on
//! failure and returns plain data — except `connect` / `set_encryption`, whose
//! caller renders the error inline in the cipher dialog. All reactive state
//! lives in the features/components (see `browser_component`).
//!
//! Author: Keval Thumar

use leptos::logging::*;
use leptos::prelude::*;
use serde::de::DeserializeOwned;
use serde::Serialize;
use shared::{
    AppError, Cell, CsvPreview, ExecOutcome, KeyInput, OpenOutcome, ResultSet, ScriptOutcome, SessionInfo,
    SqlExportOptions, sql,
};

use crate::database::database_constant::{
    CMD_CANCEL, CMD_CONNECT, CMD_CSV_PREVIEW, CMD_EXECUTE, CMD_EXEC_SCRIPT, CMD_EXPORT_CSV, CMD_EXPORT_SQL,
    CMD_IMPORT_CSV, CMD_IMPORT_SQL, CMD_OPEN_DIALOG, CMD_OPEN_PATH, CMD_PICK_IMPORT_FILE, CMD_QUERY,
    CMD_SESSION_INFO, CMD_SET_ENCRYPTION, HISTORY_SEPARATOR, MAX_HISTORY, MAX_RECENT_FILES, SNIPPET_NAME_SEPARATOR,
    SQL_BEGIN_EDIT, SQL_FOREIGN_KEYS, SQL_LOAD_SCHEMA, SQL_REVERT_CHANGES, SQL_TABLE_COLUMNS, SQL_WRITE_CHANGES,
    STORAGE_HISTORY_PREFIX, STORAGE_RECENT_FILES, STORAGE_SNIPPETS,
};
use crate::database::database_structure::{
    ColumnInfo, CsvPreviewRequest, DatabaseService, ExportCsvRequest, ExportItem, ExportSqlRequest, ForeignKey,
    ImportCsvRequest, KeyRequest,
    OpenDialogRequest, PathRequest, PickFileRequest, SchemaObject, ScriptRequest, SqlRequest,
};
use crate::shared::services::local_storage_service::LocalStorageService;
use crate::shared::services::os_bridge::{NoArgs, OsBridge};
use crate::shared::services::toast_service::ToastService;

impl Default for DatabaseService {
    fn default() -> Self {
        Self::new()
    }
}

impl DatabaseService {
    pub fn new() -> Self {
        Self
    }

    // Invoke `cmd`; on failure toast the backend's message, log, and return None.
    async fn call<A: Serialize, R: DeserializeOwned>(cmd: &str, args: &A, toast: ToastService) -> Option<R> {
        match OsBridge::invoke(cmd, args).await {
            Ok(value) => Some(value),
            Err(e) => {
                toast.error(&e.message);
                error!("{cmd}: {e}");
                None
            }
        }
    }

    // Keval Thumar
    // Which database (if any) this window holds. An empty window on error.
    pub async fn session_info(toast: ToastService) -> SessionInfo {
        Self::call(CMD_SESSION_INFO, &NoArgs {}, toast).await.unwrap_or_default()
    }

    // Keval Thumar
    // Native Open… / New… picker. The backend binds the file to this window if
    // it's empty, otherwise opens (or focuses) another window.
    pub async fn open_dialog(create: bool, toast: ToastService) -> OpenOutcome {
        Self::call(CMD_OPEN_DIALOG, &OpenDialogRequest { create }, toast).await.unwrap_or(OpenOutcome::Cancelled)
    }

    // Keval Thumar
    // Open a recent file. A missing file is dropped from the recent list.
    pub async fn open_path(path: &str, toast: ToastService) -> OpenOutcome {
        match Self::call(CMD_OPEN_PATH, &PathRequest { path }, toast).await {
            Some(outcome) => outcome,
            None => {
                Self::remove_recent(path);
                OpenOutcome::Cancelled
            }
        }
    }

    // Keval Thumar
    // Connect this window's file, with an optional SQLCipher key. Returns the
    // error so the cipher dialog can show NeedsKey / WrongKey inline.
    pub async fn connect(key: Option<KeyInput>) -> Result<SessionInfo, AppError> {
        OsBridge::invoke(CMD_CONNECT, &KeyRequest { key }).await
    }

    // Keval Thumar
    // Encrypt, re-key, or (key = None) decrypt the open database.
    pub async fn set_encryption(key: Option<KeyInput>) -> Result<SessionInfo, AppError> {
        OsBridge::invoke(CMD_SET_ENCRYPTION, &KeyRequest { key }).await
    }

    pub async fn query(sql: &str, params: &[Cell], toast: ToastService) -> Option<ResultSet> {
        Self::call(CMD_QUERY, &SqlRequest { sql, params }, toast).await
    }

    pub async fn execute(sql: &str, params: &[Cell], toast: ToastService) -> Option<ExecOutcome> {
        Self::call(CMD_EXECUTE, &SqlRequest { sql, params }, toast).await
    }

    // Keval Thumar
    // Run a whole script. Statement errors come back inside the outcome (shown
    // in the editor); only bridge failures toast.
    pub async fn exec_script(sql: &str, toast: ToastService) -> Option<ScriptOutcome> {
        Self::call(CMD_EXEC_SCRIPT, &ScriptRequest { sql }, toast).await
    }

    pub async fn cancel() {
        if let Err(e) = OsBridge::invoke::<_, ()>(CMD_CANCEL, &NoArgs {}).await {
            error!("cancel: {e}");
        }
    }

    // Keval Thumar
    // Tables, views, indexes and triggers (internal sqlite_* objects excluded).
    pub async fn load_schema(toast: ToastService) -> Vec<SchemaObject> {
        let Some(rs) = Self::query(SQL_LOAD_SCHEMA, &[], toast).await else { return vec![] };
        rs.rows
            .iter()
            .map(|r| SchemaObject { kind: text(&r[0]), name: text(&r[1]), table: text(&r[2]), sql: text(&r[3]) })
            .collect()
    }

    // Keval Thumar
    // Tables and views the SQL export offers (FTS shadow tables left out), by name.
    pub async fn load_exportable(toast: ToastService) -> Vec<ExportItem> {
        let Some(rs) = Self::query(sql::EXPORTABLE_OBJECTS, &[], toast).await else { return vec![] };
        rs.rows.iter().map(|r| ExportItem { name: text(&r[0]), is_view: text(&r[1]) == "view" }).collect()
    }

    // Keval Thumar
    // Columns of a table or view, in declaration order.
    pub async fn load_columns(table: &str, toast: ToastService) -> Vec<ColumnInfo> {
        let params = [Cell::Text(table.to_string())];
        let Some(rs) = Self::query(SQL_TABLE_COLUMNS, &params, toast).await else { return vec![] };
        rs.rows
            .iter()
            .map(|r| ColumnInfo {
                name: text(&r[0]),
                ty: text(&r[1]),
                not_null: text(&r[2]) == "1",
                default: if r[3] == Cell::Null { None } else { Some(text(&r[3])) },
                pk: text(&r[4]).parse().unwrap_or(0),
                hidden: text(&r[5]).parse().unwrap_or(0),
            })
            .collect()
    }

    // Keval Thumar
    // Single-column foreign keys of a table (empty for views).
    pub async fn load_foreign_keys(table: &str, toast: ToastService) -> Vec<ForeignKey> {
        let params = [Cell::Text(table.to_string())];
        let Some(rs) = Self::query(SQL_FOREIGN_KEYS, &params, toast).await else { return vec![] };
        rs.rows
            .iter()
            .map(|r| ForeignKey { column: text(&r[0]), table: text(&r[1]), target: text(&r[2]) })
            .collect()
    }

    // Keval Thumar
    // Open the pending-changes savepoint before a grid/schema edit (no-op if a
    // transaction is already open). Returns false if it couldn't be started.
    pub async fn begin_edit(in_txn: RwSignal<bool>, toast: ToastService) -> bool {
        if in_txn.get_untracked() {
            return true;
        }
        match Self::execute(SQL_BEGIN_EDIT, &[], toast).await {
            Some(out) => {
                in_txn.set(out.in_txn);
                true
            }
            None => false,
        }
    }

    pub async fn write_changes(toast: ToastService) -> bool {
        Self::execute(SQL_WRITE_CHANGES, &[], toast).await.is_some()
    }

    pub async fn revert_changes(toast: ToastService) -> bool {
        Self::execute(SQL_REVERT_CHANGES, &[], toast).await.is_some()
    }

    /// `kind` is "csv" or "sql".
    pub async fn pick_import_file(kind: &str, toast: ToastService) -> Option<String> {
        Self::call::<_, Option<String>>(CMD_PICK_IMPORT_FILE, &PickFileRequest { kind }, toast).await.flatten()
    }

    pub async fn csv_preview(path: &str, sep: char, header: bool, toast: ToastService) -> Option<CsvPreview> {
        Self::call(CMD_CSV_PREVIEW, &CsvPreviewRequest { path, sep, header }, toast).await
    }

    pub async fn import_csv(path: &str, table: &str, sep: char, header: bool, toast: ToastService) -> Option<ExecOutcome> {
        Self::call(CMD_IMPORT_CSV, &ImportCsvRequest { path, table, sep, header }, toast).await
    }

    pub async fn import_sql(path: &str, toast: ToastService) -> Option<ScriptOutcome> {
        Self::call(CMD_IMPORT_SQL, &PathRequest { path }, toast).await
    }

    // Keval Thumar
    // Export the full result of `sql` to a CSV the user picks.
    pub async fn export_csv(sql: &str, name: &str, toast: ToastService) {
        if let Some(Some(path)) = Self::call::<_, Option<String>>(CMD_EXPORT_CSV, &ExportCsvRequest { sql, name }, toast).await {
            toast.success(&format!("Exported to {path}"));
        }
    }

    // Keval Thumar
    // Export the ticked tables and views as one SQL script the user saves.
    // False when the save was cancelled or failed (the dialog stays open).
    pub async fn export_sql(tables: &[String], options: &SqlExportOptions, toast: ToastService) -> bool {
        let request = ExportSqlRequest { tables, options };
        match Self::call::<_, Option<String>>(CMD_EXPORT_SQL, &request, toast).await {
            Some(Some(path)) => {
                toast.success(&format!("SQL saved to {path}"));
                true
            }
            _ => false,
        }
    }

    /// Recent files, most recent first (shared by every window via localStorage).
    pub fn recent_files() -> Vec<String> {
        LocalStorageService::get(STORAGE_RECENT_FILES)
            .map(|s| s.lines().filter(|l| !l.is_empty()).map(String::from).collect())
            .unwrap_or_default()
    }

    pub fn push_recent(path: &str) {
        let mut list = Self::recent_files();
        list.retain(|p| p != path);
        list.insert(0, path.to_string());
        list.truncate(MAX_RECENT_FILES);
        LocalStorageService::set(STORAGE_RECENT_FILES, &list.join("\n"));
    }

    /// SQL run in this database, most recent first.
    pub fn history(db_path: &str) -> Vec<String> {
        LocalStorageService::get(&format!("{STORAGE_HISTORY_PREFIX}{db_path}"))
            .map(|s| s.split(HISTORY_SEPARATOR).filter(|q| !q.is_empty()).map(String::from).collect())
            .unwrap_or_default()
    }

    pub fn push_history(db_path: &str, sql: &str) {
        let sql = sql.trim();
        if sql.is_empty() {
            return;
        }
        let mut list = Self::history(db_path);
        list.retain(|q| q != sql);
        list.insert(0, sql.to_string());
        list.truncate(MAX_HISTORY);
        LocalStorageService::set(&format!("{STORAGE_HISTORY_PREFIX}{db_path}"), &list.join(&HISTORY_SEPARATOR.to_string()));
    }

    /// Saved snippets as (name, SQL), in the order they were saved.
    pub fn snippets() -> Vec<(String, String)> {
        LocalStorageService::get(STORAGE_SNIPPETS)
            .map(|s| {
                s.split(HISTORY_SEPARATOR)
                    .filter_map(|e| e.split_once(SNIPPET_NAME_SEPARATOR))
                    .map(|(name, sql)| (name.to_string(), sql.to_string()))
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn set_snippets(list: &[(String, String)]) {
        let clean = |s: &str| s.replace([HISTORY_SEPARATOR, SNIPPET_NAME_SEPARATOR], "");
        let entries: Vec<String> =
            list.iter().map(|(name, sql)| format!("{}{SNIPPET_NAME_SEPARATOR}{}", clean(name), clean(sql))).collect();
        LocalStorageService::set(STORAGE_SNIPPETS, &entries.join(&HISTORY_SEPARATOR.to_string()));
    }

    pub fn remove_recent(path: &str) {
        let mut list = Self::recent_files();
        list.retain(|p| p != path);
        LocalStorageService::set(STORAGE_RECENT_FILES, &list.join("\n"));
    }
}

/// Cell as plain text; NULL becomes "".
fn text(cell: &Cell) -> String {
    match cell {
        Cell::Null => String::new(),
        other => other.display(),
    }
}
