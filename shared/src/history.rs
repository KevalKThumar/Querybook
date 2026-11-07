//! Change history: what each save (Write Changes) wrote to a database.
//!
//! The backend builds one [`HistoryEntry`] when pending changes are committed
//! and keeps it on this device, next to (never inside) the database. The
//! History tab shows the latest ones.

use serde::{Deserialize, Serialize};

use crate::Cell;

/// Longest text value kept in the history, in characters.
pub const MAX_TEXT_CHARS: usize = 200;

/// One save.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub id: i64,
    /// Unix time in milliseconds.
    pub saved_at: i64,
    pub schema: Vec<SchemaChange>,
    /// Tables whose rows changed, in the order they were first changed.
    pub tables: Vec<TableChanges>,
}

impl HistoryEntry {
    pub fn is_empty(&self) -> bool {
        self.schema.is_empty() && self.tables.iter().all(|t| t.row_count() == 0)
    }

    pub fn row_count(&self) -> u64 {
        self.tables.iter().map(TableChanges::row_count).sum()
    }

    /// "2 tables · 6 rows · 1 schema change".
    pub fn summary(&self) -> String {
        let mut parts = Vec::new();
        if !self.tables.is_empty() {
            let about = if self.tables.iter().any(|t| t.approx) { "about " } else { "" };
            parts.push(count(self.tables.len() as u64, "table"));
            parts.push(format!("{about}{}", count(self.row_count(), "row")));
        }
        if !self.schema.is_empty() {
            parts.push(count(self.schema.len() as u64, "schema change"));
        }
        if parts.is_empty() { "No changes".into() } else { parts.join(" · ") }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SchemaChangeKind {
    Created,
    Dropped,
    Altered,
}

/// A table, view, index or trigger that was created, dropped or changed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SchemaChange {
    pub kind: SchemaChangeKind,
    /// table, view, index or trigger.
    pub object: String,
    pub name: String,
    /// Its SQL before and after the save.
    pub before: Option<String>,
    pub after: Option<String>,
}

impl SchemaChange {
    /// "Created table orders".
    pub fn label(&self) -> String {
        let verb = match self.kind {
            SchemaChangeKind::Created => "Created",
            SchemaChangeKind::Dropped => "Dropped",
            SchemaChangeKind::Altered => "Changed",
        };
        format!("{verb} {} {}", self.object, self.name)
    }
}

/// The row changes of one table in one save.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TableChanges {
    pub table: String,
    pub inserted: u64,
    pub updated: u64,
    pub deleted: u64,
    /// True when the save changed more rows than the history keeps: the rest
    /// are only counted, so the totals are approximate.
    pub approx: bool,
    /// The rows kept with their values.
    pub rows: Vec<RowChange>,
}

impl TableChanges {
    pub fn row_count(&self) -> u64 {
        self.inserted + self.updated + self.deleted
    }

    /// "1 inserted · 2 updated · 1 deleted", prefixed "about " when approximate.
    pub fn counts_label(&self) -> String {
        let parts: Vec<String> = [(self.inserted, "inserted"), (self.updated, "updated"), (self.deleted, "deleted")]
            .into_iter()
            .filter(|(n, _)| *n > 0)
            .map(|(n, what)| format!("{n} {what}"))
            .collect();
        let about = if self.approx { "about " } else { "" };
        format!("{about}{}", parts.join(" · "))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RowChangeKind {
    Insert,
    Update,
    Delete,
}

/// One row: every value for an insert or delete, the changed columns for an update.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RowChange {
    pub kind: RowChangeKind,
    /// How the row is identified: "id 12", "rowid 5" or "a 1, b \"x\"".
    pub key: String,
    pub columns: Vec<ColumnChange>,
}

impl RowChange {
    /// `name "Ann" → "Anna", city NULL → "Pune"` for an update; the values for
    /// an insert (new) or delete (old).
    pub fn values_label(&self) -> String {
        self.columns
            .iter()
            .map(|c| match (&c.old, &c.new) {
                (Some(old), Some(new)) => format!("{} {} → {}", c.name, show(old), show(new)),
                (Some(v), None) | (None, Some(v)) => format!("{} {}", c.name, show(v)),
                (None, None) => c.name.clone(),
            })
            .collect::<Vec<_>>()
            .join(", ")
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ColumnChange {
    pub name: String,
    /// None for an inserted row.
    pub old: Option<Cell>,
    /// None for a deleted row.
    pub new: Option<Cell>,
}

/// The latest saves of a database, newest first, and how many are kept.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct HistoryPage {
    pub entries: Vec<HistoryEntry>,
    pub total: u64,
}

/// A value as the history keeps it: text over [`MAX_TEXT_CHARS`] is cut and ends with "…".
pub fn clip(cell: Cell) -> Cell {
    match cell {
        Cell::Text(s) if s.chars().count() > MAX_TEXT_CHARS => {
            Cell::Text(s.chars().take(MAX_TEXT_CHARS).chain(['…']).collect())
        }
        other => other,
    }
}

/// A value in a history line: NULL, 42, 1.5, "text", BLOB (3 bytes).
pub fn show(cell: &Cell) -> String {
    match cell {
        Cell::Text(s) => format!("\"{s}\""),
        other => other.display(),
    }
}

fn count(n: u64, what: &str) -> String {
    format!("{n} {what}{}", if n == 1 { "" } else { "s" })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(name: &str, i: u64, u: u64, d: u64) -> TableChanges {
        TableChanges { table: name.into(), inserted: i, updated: u, deleted: d, ..Default::default() }
    }

    #[test]
    fn labels() {
        let mut t = table("t", 1, 2, 0);
        assert_eq!(t.counts_label(), "1 inserted · 2 updated");
        t.approx = true;
        assert_eq!(t.counts_label(), "about 1 inserted · 2 updated");

        let change = SchemaChange {
            kind: SchemaChangeKind::Created,
            object: "table".into(),
            name: "orders".into(),
            before: None,
            after: Some("CREATE TABLE orders(x)".into()),
        };
        assert_eq!(change.label(), "Created table orders");

        let entry = HistoryEntry { schema: vec![change], tables: vec![table("a", 1, 0, 0), table("b", 0, 3, 2)], ..Default::default() };
        assert_eq!(entry.summary(), "2 tables · 6 rows · 1 schema change");
        assert!(!entry.is_empty());
        assert_eq!(HistoryEntry { tables: vec![table("a", 1, 0, 0)], ..Default::default() }.summary(), "1 table · 1 row");
        assert!(HistoryEntry::default().is_empty());
    }

    #[test]
    fn row_lines() {
        let col = |name: &str, old: Option<Cell>, new: Option<Cell>| ColumnChange { name: name.into(), old, new };
        let update = RowChange {
            kind: RowChangeKind::Update,
            key: "id 12".into(),
            columns: vec![
                col("name", Some(Cell::Text("Ann".into())), Some(Cell::Text("Anna".into()))),
                col("city", Some(Cell::Null), Some(Cell::Text("Pune".into()))),
            ],
        };
        assert_eq!(update.values_label(), "name \"Ann\" → \"Anna\", city NULL → \"Pune\"");
        let insert = RowChange {
            kind: RowChangeKind::Insert,
            key: "id 31".into(),
            columns: vec![col("id", None, Some(Cell::Int("31".into()))), col("pic", None, Some(Cell::Blob(3)))],
        };
        assert_eq!(insert.values_label(), "id 31, pic BLOB (3 bytes)");
        let delete = RowChange { kind: RowChangeKind::Delete, key: "rowid 7".into(), columns: vec![col("x", Some(Cell::Real("1.5".into())), None)] };
        assert_eq!(delete.values_label(), "x 1.5");
    }

    #[test]
    fn values() {
        assert_eq!(show(&Cell::Text("Ann".into())), "\"Ann\"");
        assert_eq!(show(&Cell::Null), "NULL");
        assert_eq!(show(&Cell::Int("42".into())), "42");
        assert_eq!(show(&Cell::Blob(3)), "BLOB (3 bytes)");

        let long = "é".repeat(MAX_TEXT_CHARS + 5);
        let Cell::Text(cut) = clip(Cell::Text(long)) else { panic!() };
        assert_eq!(cut.chars().count(), MAX_TEXT_CHARS + 1);
        assert!(cut.ends_with('…'));
        assert_eq!(clip(Cell::Text("short".into())), Cell::Text("short".into()));
    }
}
