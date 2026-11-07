//! Constants for the database module: Tauri command names (the "endpoints"),
//! routes, storage keys, SQL used by the UI, and grid sizing.
//!
//! Author: Keval Thumar

use shared::ExportContent;

// Tauri commands (registered in src-tauri/src/lib.rs)
pub const CMD_SESSION_INFO: &str = "session_info";
pub const CMD_OPEN_DIALOG: &str = "open_dialog";
pub const CMD_OPEN_PATH: &str = "open_path";
pub const CMD_CONNECT: &str = "connect";
pub const CMD_QUERY: &str = "query";
pub const CMD_EXECUTE: &str = "execute";
pub const CMD_EXEC_SCRIPT: &str = "exec_script";
pub const CMD_WRITE_CHANGES: &str = "write_changes";
pub const CMD_CHANGE_HISTORY: &str = "change_history";
pub const CMD_CLEAR_CHANGE_HISTORY: &str = "clear_change_history";
pub const CMD_CANCEL: &str = "cancel";
pub const CMD_SET_ENCRYPTION: &str = "set_encryption";
pub const CMD_PICK_IMPORT_FILE: &str = "pick_import_file";
pub const CMD_CSV_PREVIEW: &str = "csv_preview";
pub const CMD_IMPORT_CSV: &str = "import_csv";
pub const CMD_IMPORT_SQL: &str = "import_sql";
pub const CMD_EXPORT_CSV: &str = "export_csv";
pub const CMD_EXPORT_SQL: &str = "export_sql";

// Routes
pub const ROUTE_BROWSER: &str = "/database/browser";
pub const ROUTE_WELCOME: &str = "/database/welcome";

// Local storage
pub const STORAGE_RECENT_FILES: &str = "querybook.recent";
pub const MAX_RECENT_FILES: usize = 10;

// Pending changes ("Write Changes" / "Revert Changes", like DB Browser).
// Grid edits and schema dialogs open this savepoint; the backend opens it for
// SQL-editor and script statements that write. Shared with the backend.
// Writing them is its own command (CMD_WRITE_CHANGES), which records the save.
pub const SQL_BEGIN_EDIT: &str = shared::sql::BEGIN_EDIT;
pub const SQL_REVERT_CHANGES: &str = shared::sql::REVERT_CHANGES;

// Change history (History tab)
/// Saves shown: the latest few. The backend keeps all of them.
pub const HISTORY_SHOWN: u32 = 3;
/// Rows listed per table before "Show N more".
pub const HISTORY_ROWS_PER_TABLE: usize = 20;

// Schema
pub const SQL_LOAD_SCHEMA: &str = "SELECT type, name, tbl_name, COALESCE(sql, '') FROM sqlite_schema \
     WHERE name NOT LIKE 'sqlite_%' ORDER BY name COLLATE NOCASE";
pub const SQL_TABLE_COLUMNS: &str =
    "SELECT name, type, \"notnull\", dflt_value, pk, hidden FROM pragma_table_xinfo(?1) ORDER BY cid";

/// Single-column foreign keys of ?1 as (column, parent table, parent column).
/// A REFERENCES without a column list points at the parent's primary key.
// ponytail: composite foreign keys are skipped; jumping needs every key value.
pub const SQL_FOREIGN_KEYS: &str = "SELECT f.\"from\", f.\"table\", \
     COALESCE(f.\"to\", (SELECT p.name FROM pragma_table_info(f.\"table\") p WHERE p.pk = 1)) \
     FROM pragma_foreign_key_list(?1) f \
     WHERE (SELECT count(*) FROM pragma_foreign_key_list(?1) g WHERE g.id = f.id) = 1";

/// Database facts for the info tab, in one row.
pub const SQL_DATABASE_INFO: &str = "SELECT (SELECT page_size FROM pragma_page_size), \
     (SELECT page_count FROM pragma_page_count), (SELECT freelist_count FROM pragma_freelist_count), \
     (SELECT encoding FROM pragma_encoding), (SELECT journal_mode FROM pragma_journal_mode), \
     (SELECT auto_vacuum FROM pragma_auto_vacuum), (SELECT user_version FROM pragma_user_version), \
     (SELECT application_id FROM pragma_application_id), (SELECT foreign_keys FROM pragma_foreign_keys), \
     sqlite_version()";
pub const SQL_CIPHER_VERSION: &str = "PRAGMA cipher_version";

/// Maintenance actions on the info tab: (label, icon, SQL, needs no pending changes, what it does).
/// VACUUM fails inside a transaction, optimize would write its statistics into
/// the pending changes, and foreign_keys is silently ignored inside one.
pub const MAINTENANCE_ACTIONS: [(&str, &str, &str, bool, &str); 5] = [
    ("Integrity check", "bi bi-shield-check", "PRAGMA integrity_check", false, "Full check of every page, index and constraint."),
    ("Quick check", "bi bi-lightning", "PRAGMA quick_check", false, "Like the integrity check, minus index contents. Faster."),
    ("Foreign key check", "bi bi-link-45deg", "PRAGMA foreign_key_check", false, "Rows whose foreign key points at a missing parent."),
    ("Optimize", "bi bi-speedometer2", "PRAGMA optimize", true, "Refresh the query planner's statistics where useful."),
    ("Vacuum", "bi bi-arrows-collapse", "VACUUM", true, "Rebuild the file: reclaims free pages, defragments. Can renumber rowids."),
];

/// Sidebar groups: (sqlite_schema type, label).
pub const SCHEMA_GROUPS: [(&str, &str); 4] = [("table", "Tables"), ("view", "Views"), ("index", "Indexes"), ("trigger", "Triggers")];

/// Keywords that mean a SQL-editor script may have changed data or schema,
/// so open grids and the sidebar should reload.
pub const MODIFYING_KEYWORDS: [&str; 20] = [
    "INSERT", "UPDATE", "DELETE", "REPLACE", "CREATE", "DROP", "ALTER", "BEGIN", "COMMIT", "END",
    "ROLLBACK", "SAVEPOINT", "RELEASE", "VACUUM", "ATTACH", "DETACH", "REINDEX", "ANALYZE", "PRAGMA", "UPSERT",
];

// Data grid (must match $height-row / $width-column / $width-rownum in styles/variables.scss)
pub const ROW_HEIGHT: f64 = 28.0;
pub const COLUMN_WIDTH: f64 = 180.0;
pub const ROWNUM_WIDTH: f64 = 56.0;
/// Rows fetched per LIMIT/OFFSET page while scrolling.
pub const CHUNK_ROWS: usize = 200;
/// Extra rows rendered above/below the viewport.
pub const OVERSCAN_ROWS: usize = 10;
/// "Adjust rows to contents": text metrics used to estimate wrapped lines.
pub const GRID_LINE_HEIGHT: f64 = 17.0;
pub const GRID_CHAR_WIDTH: f64 = 7.0;
pub const GRID_MAX_FIT_LINES: usize = 12;
/// Column resize: narrowest drag, widest double-click fit, and the room a fit
/// leaves for padding plus the sort / link icons.
pub const MIN_COLUMN_WIDTH: f64 = 48.0;
pub const MAX_FIT_COLUMN_WIDTH: f64 = 640.0;
pub const COLUMN_FIT_PADDING: f64 = 34.0;
/// Cell inspector: bytes of a blob fetched for the hex view.
pub const INSPECT_MAX_BYTES: usize = 65_536;

// SQL editor
/// Autocomplete list length.
pub const SUGGEST_LIMIT: usize = 12;
/// Padding of the editor layers (must match `.sqled-layer` padding: $spacing-md).
pub const EDITOR_PADDING: f64 = 12.0;
/// Query history per database (localStorage key = prefix + db path).
pub const STORAGE_HISTORY_PREFIX: &str = "querybook.history:";
pub const MAX_HISTORY: usize = 50;
/// Separates history entries / snippets in storage (queries may contain newlines).
pub const HISTORY_SEPARATOR: char = '\u{1e}';
/// Saved SQL snippets, shared by every database: name + SNIPPET_NAME_SEPARATOR + SQL.
pub const STORAGE_SNIPPETS: &str = "querybook.snippets";
pub const SNIPPET_NAME_SEPARATOR: char = '\u{1f}';

// SQLCipher 4 defaults — the only settings Querybook uses (shown read-only).
pub const CIPHER_DEFAULTS: [(&str, &str); 5] = [
    ("Page size", "4,096"),
    ("KDF iterations", "256,000"),
    ("HMAC algorithm", "SHA512"),
    ("KDF algorithm", "SHA512"),
    ("Plaintext header size", "0"),
];

// Create Table / Add Column type suggestions
pub const COLUMN_TYPES: [&str; 5] = ["INTEGER", "TEXT", "REAL", "BLOB", "NUMERIC"];

// CSV import separators
pub const CSV_SEPARATORS: [(&str, char); 4] = [("Comma", ','), ("Semicolon", ';'), ("Tab", '\t'), ("Pipe", '|')];

// What the SQL export writes
pub const EXPORT_CONTENTS: [(&str, ExportContent); 3] = [
    ("Schema and data", ExportContent::SchemaAndData),
    ("Schema only", ExportContent::SchemaOnly),
    ("Data only", ExportContent::DataOnly),
];

/// SQL-editor indent inserted by the Tab key.
pub const EDITOR_INDENT: &str = "    ";
