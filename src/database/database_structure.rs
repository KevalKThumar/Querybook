//! Module-level data types for the database module. Plain data, no signals.
//!
//! Author: Keval Thumar

use std::collections::BTreeSet;
use std::ops::Range;

use serde::Serialize;
use shared::sql::RowKey;
use shared::{Cell, KeyInput};

/// Stateless Tauri-command layer for the module (impl in `database_service.rs`).
#[derive(Clone, Copy)]
pub struct DatabaseService;

/// One row of `sqlite_schema`.
#[derive(Clone, Debug, PartialEq)]
pub struct SchemaObject {
    /// table, view, index or trigger.
    pub kind: String,
    pub name: String,
    /// Table an index/trigger belongs to (same as `name` for tables).
    pub table: String,
    pub sql: String,
}

impl SchemaObject {
    pub fn is_browsable(&self) -> bool {
        self.kind == "table" || self.kind == "view"
    }

    /// How rows are identified for edits; views are read-only.
    pub fn row_key(&self, columns: &[ColumnInfo]) -> RowKey {
        if self.kind != "table" {
            return RowKey::ReadOnly;
        }
        if self.sql.to_ascii_uppercase().contains("WITHOUT ROWID") {
            let mut pk: Vec<&ColumnInfo> = columns.iter().filter(|c| c.pk > 0).collect();
            pk.sort_by_key(|c| c.pk);
            return RowKey::Pk(pk.into_iter().map(|c| c.name.clone()).collect());
        }
        RowKey::Rowid
    }

    /// Bootstrap Icons class for the sidebar / tabs.
    pub fn icon(&self) -> &'static str {
        match self.kind.as_str() {
            "table" => "bi bi-table",
            "view" => "bi bi-eye",
            "index" => "bi bi-sort-down",
            _ => "bi bi-lightning-charge",
        }
    }
}

/// One row of `pragma_table_xinfo`.
#[derive(Clone, Debug, PartialEq)]
pub struct ColumnInfo {
    pub name: String,
    pub ty: String,
    pub not_null: bool,
    pub default: Option<String>,
    /// 1-based position in the primary key, 0 if not part of it.
    pub pk: u32,
    /// 0 normal, 1 hidden (virtual table), 2/3 generated.
    pub hidden: u32,
}

/// What a workspace tab shows.
#[derive(Clone, Debug, PartialEq)]
pub enum TabKind {
    Browse(String),
    Structure(String),
    Sql(u32),
    /// Database info + maintenance (integrity check, VACUUM, …).
    Info,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Tab {
    pub id: u32,
    pub kind: TabKind,
}

impl Tab {
    pub fn title(&self) -> String {
        match &self.kind {
            TabKind::Browse(name) | TabKind::Structure(name) => name.clone(),
            TabKind::Sql(n) => format!("SQL {n}"),
            TabKind::Info => "Database".to_string(),
        }
    }

    pub fn icon(&self) -> &'static str {
        match self.kind {
            TabKind::Browse(_) => "bi bi-table",
            TabKind::Structure(_) => "bi bi-diagram-3",
            TabKind::Sql(_) => "bi bi-terminal",
            TabKind::Info => "bi bi-info-square",
        }
    }
}

/// Which flow the cipher dialog is serving.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CipherMode {
    /// Opening an encrypted file.
    Unlock,
    /// A brand-new file: encryption is optional.
    Create,
    /// Set, change or remove the key of the open database.
    Change { encrypted: bool },
}

/// One edit in the Create Table column list.
#[derive(Clone, Debug, PartialEq)]
pub enum ColumnField {
    Name(String),
    Type(String),
    Pk(bool),
    AutoInc(bool),
    NotNull(bool),
    Unique(bool),
    Default(String),
}

/// What the structure view's rename dialog is renaming.
#[derive(Clone, Debug, PartialEq)]
pub enum RenameTarget {
    Object,
    Column(String),
}

/// What the structure view's confirmation dialog will drop.
#[derive(Clone, Debug, PartialEq)]
pub enum DropTarget {
    Object,
    Column(String),
    Index(String),
}

/// What is selected in a `<Grid/>`. Row indexes are display positions.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum GridSelection {
    #[default]
    None,
    /// The rectangle between `anchor` and `focus` (row, column); one cell when equal.
    Cells { anchor: (usize, usize), focus: (usize, usize) },
    /// Whole rows picked in the row-number gutter.
    Rows { rows: BTreeSet<usize>, anchor: usize },
}

impl GridSelection {
    pub fn cell(at: (usize, usize)) -> Self {
        GridSelection::Cells { anchor: at, focus: at }
    }

    pub fn row(r: usize) -> Self {
        GridSelection::Rows { rows: BTreeSet::from([r]), anchor: r }
    }

    /// The keyboard-focused cell.
    pub fn focus(&self) -> Option<(usize, usize)> {
        match self {
            GridSelection::Cells { focus, .. } => Some(*focus),
            GridSelection::Rows { anchor, .. } => Some((*anchor, 0)),
            GridSelection::None => None,
        }
    }

    /// (rows, columns) covered by a cell selection.
    pub fn rect(&self) -> Option<(Range<usize>, Range<usize>)> {
        match self {
            GridSelection::Cells { anchor, focus } => Some((
                anchor.0.min(focus.0)..anchor.0.max(focus.0) + 1,
                anchor.1.min(focus.1)..anchor.1.max(focus.1) + 1,
            )),
            _ => None,
        }
    }

    pub fn has_cell(&self, r: usize, c: usize) -> bool {
        match self {
            GridSelection::Rows { rows, .. } => rows.contains(&r),
            _ => self.rect().is_some_and(|(rs, cs)| rs.contains(&r) && cs.contains(&c)),
        }
    }

    pub fn has_row(&self, r: usize) -> bool {
        matches!(self, GridSelection::Rows { rows, .. } if rows.contains(&r))
    }

    /// Every row touched by the selection, ascending.
    pub fn row_list(&self) -> Vec<usize> {
        match self {
            GridSelection::Rows { rows, .. } => rows.iter().copied().collect(),
            _ => self.rect().map(|(rs, _)| rs.collect()).unwrap_or_default(),
        }
    }
}

/// What a mouse drag in the grid is extending.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GridDrag {
    Cells,
    Rows,
}

/// Which context menu the grid shows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GridMenu {
    Rows,
    Cells,
}

/// "Go to referenced row": open `table` filtered to `column = value`.
#[derive(Clone, Debug, PartialEq)]
pub struct FkJump {
    pub table: String,
    pub column: String,
    pub value: String,
}

/// One single-column foreign key of a table: `column` → `table`.`target`.
#[derive(Clone, Debug, PartialEq)]
pub struct ForeignKey {
    pub column: String,
    pub table: String,
    pub target: String,
}

/// How the cell inspector shows a value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum InspectMode {
    Text,
    Json,
    Hex,
}

/// Clipboard formats offered by the grid.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CopyKind {
    Plain,
    WithHeaders,
    Sql,
}

// ── IPC request bodies (field names = Tauri command argument names) ──

#[derive(Serialize)]
pub struct OpenDialogRequest {
    pub create: bool,
}

#[derive(Serialize)]
pub struct PathRequest<'a> {
    pub path: &'a str,
}

#[derive(Serialize)]
pub struct KeyRequest {
    pub key: Option<KeyInput>,
}

#[derive(Serialize)]
pub struct SqlRequest<'a> {
    pub sql: &'a str,
    pub params: &'a [Cell],
}

#[derive(Serialize)]
pub struct ScriptRequest<'a> {
    pub sql: &'a str,
}

#[derive(Serialize)]
pub struct PickFileRequest<'a> {
    pub kind: &'a str,
}

#[derive(Serialize)]
pub struct CsvPreviewRequest<'a> {
    pub path: &'a str,
    pub sep: char,
    pub header: bool,
}

#[derive(Serialize)]
pub struct ImportCsvRequest<'a> {
    pub path: &'a str,
    pub table: &'a str,
    pub sep: char,
    pub header: bool,
}

#[derive(Serialize)]
pub struct ExportCsvRequest<'a> {
    pub sql: &'a str,
    pub name: &'a str,
}
