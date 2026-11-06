//! CSV / SQL import and export. Pure functions over a connection + file path;
//! the file dialogs live in the Tauri commands.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

use rusqlite::types::ValueRef;
use rusqlite::{Connection, params_from_iter};
use shared::sql::{blob_literal, quote_ident, quote_str};
use shared::{AppError, CsvPreview, ExecOutcome};

use crate::db::{invalid, io_err, sql_err};

const PREVIEW_ROWS: usize = 20;

fn reader(path: &Path, sep: u8) -> Result<csv::Reader<File>, AppError> {
    csv::ReaderBuilder::new().delimiter(sep).has_headers(false).flexible(true).from_path(path).map_err(io_err)
}

/// Header names, with blanks filled and duplicates suffixed so they work as column names.
fn column_names(header: Option<&csv::StringRecord>, width: usize) -> Vec<String> {
    let mut names: Vec<String> = Vec::with_capacity(width);
    for i in 0..width {
        let raw = header.and_then(|h| h.get(i)).map(str::trim).unwrap_or("");
        let base = if raw.is_empty() { format!("field{}", i + 1) } else { raw.to_string() };
        let mut name = base.clone();
        let mut n = 2;
        while names.iter().any(|x| x.eq_ignore_ascii_case(&name)) {
            name = format!("{base}_{n}");
            n += 1;
        }
        names.push(name);
    }
    names
}

pub fn csv_preview(path: &Path, sep: u8, header: bool) -> Result<CsvPreview, AppError> {
    let mut records: Vec<csv::StringRecord> = Vec::new();
    for rec in reader(path, sep)?.records().take(PREVIEW_ROWS + 1) {
        records.push(rec.map_err(io_err)?);
    }
    let width = records.iter().map(|r| r.len()).max().unwrap_or(0);
    let head = if header && !records.is_empty() { Some(records.remove(0)) } else { None };
    records.truncate(PREVIEW_ROWS);
    Ok(CsvPreview {
        columns: column_names(head.as_ref(), width),
        rows: records.iter().map(|r| r.iter().map(String::from).collect()).collect(),
    })
}

/// Columns a plain INSERT can write (generated/hidden columns excluded).
fn writable_columns(conn: &Connection, table: &str) -> Result<Vec<String>, AppError> {
    let mut stmt = conn.prepare("SELECT name FROM pragma_table_xinfo(?1) WHERE hidden = 0").map_err(sql_err)?;
    let cols = stmt.query_map([table], |r| r.get(0)).map_err(sql_err)?.collect::<Result<_, _>>().map_err(sql_err)?;
    Ok(cols)
}

/// Import into `table` (created with TEXT columns if missing) inside its own savepoint.
pub fn import_csv(conn: &Connection, path: &Path, table: &str, sep: u8, header: bool) -> Result<ExecOutcome, AppError> {
    if table.trim().is_empty() {
        return Err(invalid("Enter a table name."));
    }
    let mut records = reader(path, sep)?.into_records();
    let first = match records.next() {
        Some(r) => r.map_err(io_err)?,
        None => return Err(invalid("The file is empty.")),
    };
    let width = first.len();
    let (head, first_row) = if header { (Some(first), None) } else { (None, Some(first)) };

    conn.execute_batch("SAVEPOINT qb_import").map_err(sql_err)?;
    let result = (|| -> Result<u64, AppError> {
        let existing = writable_columns(conn, table)?;
        let columns = if existing.is_empty() {
            let names = column_names(head.as_ref(), width);
            let defs: Vec<String> = names.iter().map(|n| format!("{} TEXT", quote_ident(n))).collect();
            conn.execute_batch(&format!("CREATE TABLE {} ({})", quote_ident(table), defs.join(", ")))
                .map_err(sql_err)?;
            names
        } else if existing.len() < width {
            return Err(invalid(format!("The file has {width} columns but \"{table}\" has only {}.", existing.len())));
        } else {
            existing.into_iter().take(width).collect()
        };
        let cols: Vec<String> = columns.iter().map(|c| quote_ident(c)).collect();
        let marks = vec!["?"; width].join(", ");
        let mut stmt = conn
            .prepare(&format!("INSERT INTO {} ({}) VALUES ({marks})", quote_ident(table), cols.join(", ")))
            .map_err(sql_err)?;
        let mut n = 0;
        for rec in first_row.into_iter().map(Ok).chain(records) {
            let rec = rec.map_err(io_err)?;
            stmt.execute(params_from_iter((0..width).map(|i| rec.get(i).unwrap_or("")))).map_err(sql_err)?;
            n += 1;
        }
        Ok(n)
    })();
    let end = if result.is_ok() { "RELEASE qb_import" } else { "ROLLBACK TO qb_import; RELEASE qb_import" };
    conn.execute_batch(end).map_err(sql_err)?;
    Ok(ExecOutcome { affected: result?, in_txn: !conn.is_autocommit() })
}

fn csv_field(v: ValueRef<'_>) -> String {
    match v {
        ValueRef::Null => String::new(),
        ValueRef::Integer(i) => i.to_string(),
        ValueRef::Real(f) => f.to_string(),
        ValueRef::Text(t) => String::from_utf8_lossy(t).into_owned(),
        // ponytail: blobs exported as hex text; raw bytes in CSV aren't portable.
        ValueRef::Blob(b) => b.iter().map(|x| format!("{x:02X}")).collect(),
    }
}

/// Write the full result of `sql` (no row cap) as CSV with a header row.
pub fn export_csv(conn: &Connection, sql: &str, out: &Path) -> Result<u64, AppError> {
    let mut stmt = conn.prepare(sql).map_err(sql_err)?;
    let n = stmt.column_count();
    let mut w = csv::Writer::from_path(out).map_err(io_err)?;
    w.write_record(stmt.column_names()).map_err(io_err)?;
    let mut rows = stmt.query([]).map_err(sql_err)?;
    let mut count = 0;
    while let Some(row) = rows.next().map_err(sql_err)? {
        let fields: Vec<String> = (0..n).map(|i| row.get_ref(i).map(csv_field)).collect::<Result<_, _>>().map_err(sql_err)?;
        w.write_record(&fields).map_err(io_err)?;
        count += 1;
    }
    w.flush().map_err(io_err)?;
    Ok(count)
}

pub fn sql_literal(v: ValueRef<'_>) -> String {
    match v {
        ValueRef::Null => "NULL".into(),
        ValueRef::Integer(i) => i.to_string(),
        ValueRef::Real(f) if f.is_finite() => format!("{f:?}"),
        ValueRef::Real(f) => if f > 0.0 { "9e999".into() } else { "-9e999".into() },
        ValueRef::Text(t) => quote_str(&String::from_utf8_lossy(t)),
        ValueRef::Blob(b) => blob_literal(b),
    }
}

/// `.dump`-style script: tables + rows, then views, indexes, triggers, in one transaction.
// ponytail: virtual tables (FTS etc.) and their shadow tables are dumped as plain DDL and will
// conflict on re-import; handle them like sqlite3's .dump (writable_schema) if anyone needs it.
pub fn export_sql_dump(conn: &Connection, out: &Path) -> Result<(), AppError> {
    let mut w = BufWriter::new(File::create(out).map_err(io_err)?);
    let mut stmt = conn
        .prepare(
            "SELECT type, name, sql FROM sqlite_schema WHERE sql IS NOT NULL AND name NOT LIKE 'sqlite_%' \
             ORDER BY CASE type WHEN 'table' THEN 0 WHEN 'view' THEN 1 WHEN 'index' THEN 2 ELSE 3 END, rowid",
        )
        .map_err(sql_err)?;
    let objects: Vec<(String, String, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .map_err(sql_err)?
        .collect::<Result<_, _>>()
        .map_err(sql_err)?;

    writeln!(w, "BEGIN TRANSACTION;").map_err(io_err)?;
    let mut has_sequence = false;
    for (kind, name, sql) in &objects {
        writeln!(w, "{sql};").map_err(io_err)?;
        if kind != "table" {
            continue;
        }
        has_sequence |= sql.to_ascii_uppercase().contains("AUTOINCREMENT");
        dump_rows(conn, &mut w, name)?;
    }
    if has_sequence {
        writeln!(w, "DELETE FROM sqlite_sequence;").map_err(io_err)?;
        dump_rows(conn, &mut w, "sqlite_sequence")?;
    }
    writeln!(w, "COMMIT;").map_err(io_err)?;
    w.flush().map_err(io_err)
}

fn dump_rows(conn: &Connection, w: &mut impl Write, table: &str) -> Result<(), AppError> {
    let cols: Vec<String> = writable_columns(conn, table)?.iter().map(|c| quote_ident(c)).collect();
    if cols.is_empty() {
        return Ok(());
    }
    let list = cols.join(",");
    let mut stmt = conn.prepare(&format!("SELECT {list} FROM {}", quote_ident(table))).map_err(sql_err)?;
    let mut rows = stmt.query([]).map_err(sql_err)?;
    while let Some(row) = rows.next().map_err(sql_err)? {
        let vals: Vec<String> =
            (0..cols.len()).map(|i| row.get_ref(i).map(sql_literal)).collect::<Result<_, _>>().map_err(sql_err)?;
        writeln!(w, "INSERT INTO {}({list}) VALUES({});", quote_ident(table), vals.join(",")).map_err(io_err)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::tests::tmp_dir;

    const SETUP: &str = "
        CREATE TABLE t(id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT, n REAL, b BLOB, g AS (id * 2));
        INSERT INTO t(name, n, b) VALUES ('it''s', 1.5, x'00ff'), (NULL, -0.25, NULL), ('a,\"b\"\nc', 1e300, NULL);
        CREATE INDEX ix ON t(name);
        CREATE VIEW v AS SELECT name FROM t;";

    fn rows(conn: &Connection, sql: &str) -> Vec<Vec<String>> {
        let mut stmt = conn.prepare(sql).unwrap();
        let n = stmt.column_count();
        stmt.query_map([], |r| (0..n).map(|i| r.get_ref(i).map(sql_literal)).collect()).unwrap().map(Result::unwrap).collect()
    }

    #[test]
    fn sql_dump_round_trips() {
        let dir = tmp_dir("dump");
        let src = Connection::open_in_memory().unwrap();
        src.execute_batch(SETUP).unwrap();
        export_sql_dump(&src, &dir.join("d.sql")).unwrap();

        let dst = Connection::open_in_memory().unwrap();
        dst.execute_batch(&std::fs::read_to_string(dir.join("d.sql")).unwrap()).unwrap();
        let q = "SELECT * FROM t ORDER BY id";
        assert_eq!(rows(&src, q), rows(&dst, q));
        assert_eq!(rows(&dst, "SELECT seq FROM sqlite_sequence"), vec![vec!["3".to_string()]]);
        assert_eq!(rows(&dst, "SELECT count(*) FROM sqlite_schema WHERE name IN ('ix','v')"), vec![vec!["2".to_string()]]);
    }

    #[test]
    fn csv_round_trips() {
        let dir = tmp_dir("csv");
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(SETUP).unwrap();
        let out = dir.join("t.csv");
        assert_eq!(export_csv(&conn, "SELECT id, name, n FROM t", &out).unwrap(), 3);

        let preview = csv_preview(&out, b',', true).unwrap();
        assert_eq!(preview.columns, vec!["id", "name", "n"]);
        assert_eq!(preview.rows[2][1], "a,\"b\"\nc");

        let done = import_csv(&conn, &out, "copy", b',', true).unwrap();
        assert_eq!(done.affected, 3);
        assert!(!done.in_txn);
        assert_eq!(rows(&conn, "SELECT name FROM copy WHERE id = '3'"), vec![vec!["'a,\"b\"\nc'".to_string()]]);
        // Too-wide file into an existing narrower table rolls back cleanly.
        conn.execute_batch("CREATE TABLE narrow(a)").unwrap();
        assert!(import_csv(&conn, &out, "narrow", b',', true).is_err());
        assert_eq!(rows(&conn, "SELECT count(*) FROM narrow"), vec![vec!["0".to_string()]]);
    }
}
