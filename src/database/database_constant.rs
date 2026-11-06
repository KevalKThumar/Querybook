//! Constants for the database module: Tauri command names (the "endpoints"),
//! routes, storage keys, SQL used by the UI, and grid sizing.
//!
//! Author: Keval Thumar

// Tauri commands (registered in src-tauri/src/lib.rs)
pub const CMD_SESSION_INFO: &str = "session_info";
pub const CMD_OPEN_DIALOG: &str = "open_dialog";
pub const CMD_OPEN_PATH: &str = "open_path";
pub const CMD_CONNECT: &str = "connect";
pub const CMD_QUERY: &str = "query";
pub const CMD_EXECUTE: &str = "execute";
pub const CMD_EXEC_SCRIPT: &str = "exec_script";
pub const CMD_CANCEL: &str = "cancel";
pub const CMD_SET_ENCRYPTION: &str = "set_encryption";
pub const CMD_PICK_IMPORT_FILE: &str = "pick_import_file";
pub const CMD_CSV_PREVIEW: &str = "csv_preview";
pub const CMD_IMPORT_CSV: &str = "import_csv";
pub const CMD_IMPORT_SQL: &str = "import_sql";
pub const CMD_EXPORT_CSV: &str = "export_csv";
pub const CMD_EXPORT_SQL_DUMP: &str = "export_sql_dump";

// Routes
pub const ROUTE_BROWSER: &str = "/database/browser";
pub const ROUTE_WELCOME: &str = "/database/welcome";

// Local storage
pub const STORAGE_RECENT_FILES: &str = "querybook.recent";
pub const MAX_RECENT_FILES: usize = 10;

// Pending changes ("Write Changes" / "Revert Changes", like DB Browser).
// Grid edits and schema dialogs open this savepoint; COMMIT / ROLLBACK end
// whatever transaction is open, however it was started.
pub const SQL_BEGIN_EDIT: &str = "SAVEPOINT querybook";
pub const SQL_WRITE_CHANGES: &str = "COMMIT";
pub const SQL_REVERT_CHANGES: &str = "ROLLBACK";

// Schema
pub const SQL_LOAD_SCHEMA: &str = "SELECT type, name, tbl_name, COALESCE(sql, '') FROM sqlite_schema \
     WHERE name NOT LIKE 'sqlite_%' ORDER BY name COLLATE NOCASE";
pub const SQL_TABLE_COLUMNS: &str =
    "SELECT name, type, \"notnull\", dflt_value, pk, hidden FROM pragma_table_xinfo(?1) ORDER BY cid";

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

/// SQL-editor indent inserted by the Tab key.
pub const EDITOR_INDENT: &str = "    ";
