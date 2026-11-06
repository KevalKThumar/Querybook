//! Wire types shared by the Leptos UI (wasm) and the Tauri backend (native),
//! plus pure SQL helpers that both sides need. No I/O lives here.

use serde::{Deserialize, Serialize};

pub mod complete;
pub mod copy;
pub mod format;
pub mod lexer;
pub mod sql;

/// One SQLite value as it crosses the IPC bridge.
///
/// Integers and reals travel as strings: Tauri's bridge goes through JS
/// numbers, which lose precision above 2^53 and can't carry inf/NaN.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "t", content = "v")]
pub enum Cell {
    Null,
    Int(String),
    Real(String),
    Text(String),
    /// Byte length only; blob contents are not sent to the UI.
    Blob(usize),
}

impl Cell {
    /// Text shown in a grid cell.
    pub fn display(&self) -> String {
        match self {
            Cell::Null => "NULL".into(),
            Cell::Int(s) | Cell::Real(s) | Cell::Text(s) => s.clone(),
            Cell::Blob(n) => format!("BLOB ({n} bytes)"),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ResultSet {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Cell>>,
    /// True when the row cap was hit and more rows exist.
    pub truncated: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ExecOutcome {
    pub affected: u64,
    /// True while a transaction/savepoint is open (unsaved changes).
    pub in_txn: bool,
}

/// Result of running a whole SQL script from the editor.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ScriptOutcome {
    /// Rows of the last statement that returned columns.
    pub result: Option<ResultSet>,
    /// Statements that ran successfully.
    pub statements: usize,
    pub affected: u64,
    pub millis: u64,
    /// Set when a statement failed; earlier statements stay applied.
    pub error: Option<String>,
    pub in_txn: bool,
}

/// SQLCipher key. Only SQLCipher 4 default settings are supported.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "mode", content = "value")]
pub enum KeyInput {
    Passphrase(String),
    /// 64 hex chars (256-bit key) or 96 (key + 128-bit salt).
    RawHex(String),
}

impl KeyInput {
    pub fn validate(&self) -> Result<(), String> {
        match self {
            KeyInput::Passphrase(p) if p.is_empty() => Err("Password must not be empty.".into()),
            KeyInput::Passphrase(_) => Ok(()),
            KeyInput::RawHex(h) => {
                let ok = matches!(h.len(), 64 | 96) && h.bytes().all(|b| b.is_ascii_hexdigit());
                if ok { Ok(()) } else { Err("Raw key must be 64 or 96 hex characters.".into()) }
            }
        }
    }

    /// Value for `PRAGMA key` / `ATTACH ... KEY`. SQLCipher treats `x'..'` as a raw key.
    pub fn pragma_value(&self) -> String {
        match self {
            KeyInput::Passphrase(p) => p.clone(),
            KeyInput::RawHex(h) => format!("x'{h}'"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorKind {
    /// File is encrypted (or not a database) and no key was given.
    NeedsKey,
    /// A key was given but the file didn't decrypt.
    WrongKey,
    Sqlite,
    Interrupted,
    Io,
    Invalid,
    NoDatabase,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppError {
    pub kind: ErrorKind,
    pub message: String,
}

impl AppError {
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self { kind, message: message.into() }
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

/// What the file on disk looks like, from its header.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileKind {
    /// Starts with the plain SQLite header.
    Plain,
    /// Non-empty but no SQLite header: SQLCipher (or not a database at all).
    Encrypted,
    /// Missing or empty: a new database.
    #[default]
    New,
}

/// State of the database bound to the calling window.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SessionInfo {
    /// None for an empty (welcome) window.
    pub path: Option<String>,
    pub name: String,
    pub kind: FileKind,
    pub connected: bool,
    pub encrypted: bool,
}

/// Where `open_dialog` / `open_path` put the database.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum OpenOutcome {
    Cancelled,
    /// Bound to the calling (empty) window — reload its session.
    ThisWindow,
    /// Opened in, or focused, another window.
    OtherWindow,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CsvPreview {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
}
