//! CSV / SQL import and export. Pure functions over a connection + file path;
//! the file dialogs live in the Tauri commands.

use std::collections::HashSet;
use std::fs::File;
use std::io::Write;
use std::path::Path;

use rusqlite::types::ValueRef;
use rusqlite::{Connection, OptionalExtension, params_from_iter};
use shared::sql::{
    EXPORTABLE_OBJECTS, ROWS_PER_INSERT, blob_literal, create_if_not_exists, drop_if_exists, quote_ident, quote_str,
    terminate,
};
use shared::{AppError, CsvPreview, ExecOutcome, SqlExportOptions};

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

/// One row of `sqlite_schema`.
struct SchemaRow {
    kind: String,
    name: String,
    table: String,
    sql: String,
}

fn line(w: &mut impl Write, text: &str) -> Result<(), AppError> {
    writeln!(w, "{text}").map_err(io_err)
}

/// SQL script for the tables and views in `names` (see `SqlExportOptions`).
/// Objects keep their creation order. Inside the transaction, foreign keys are
/// checked only at COMMIT, so linked tables load in any order.
// ponytail: a virtual table exports as CREATE VIRTUAL TABLE plus INSERTs through it (its
// shadow tables aren't offered); modules that can't take INSERTs won't round-trip.
pub fn export_sql(conn: &Connection, names: &[String], opts: &SqlExportOptions, w: &mut impl Write) -> Result<(), AppError> {
    let mut stmt = conn.prepare(EXPORTABLE_OBJECTS).map_err(sql_err)?;
    let exportable: HashSet<String> =
        stmt.query_map([], |r| r.get(0)).map_err(sql_err)?.collect::<Result<_, _>>().map_err(sql_err)?;
    if let Some(gone) = names.iter().find(|n| !exportable.contains(*n)) {
        return Err(invalid(format!("“{gone}” can't be exported: it is no longer in the database.")));
    }
    let wanted: HashSet<&str> = names.iter().map(String::as_str).collect();
    let mut stmt = conn
        .prepare("SELECT type, name, tbl_name, sql FROM sqlite_schema WHERE sql IS NOT NULL ORDER BY rowid")
        .map_err(sql_err)?;
    let objects: Vec<SchemaRow> = stmt
        .query_map([], |r| Ok(SchemaRow { kind: r.get(0)?, name: r.get(1)?, table: r.get(2)?, sql: r.get(3)? }))
        .map_err(sql_err)?
        .collect::<Result<_, _>>()
        .map_err(sql_err)?;
    let picked = |kind: &str| -> Vec<&SchemaRow> {
        objects.iter().filter(|o| o.kind == kind && wanted.contains(o.name.as_str())).collect()
    };
    let (schema, data) = (opts.content.schema(), opts.content.data());
    let tables = picked("table");
    let views = if schema { picked("view") } else { vec![] };
    if tables.is_empty() && views.is_empty() {
        return Err(invalid("Nothing to export: tick at least one table."));
    }
    let create = |ddl: &str| terminate(&if opts.drop_existing { ddl.to_string() } else { create_if_not_exists(ddl) });

    line(w, &format!("-- Querybook SQL export: {} table(s), {} view(s)", tables.len(), views.len()))?;
    if opts.transaction {
        line(w, "BEGIN TRANSACTION;")?;
        line(w, "PRAGMA defer_foreign_keys = ON;")?;
    }
    if schema && opts.drop_existing {
        // Views first, then tables newest first, so children go before their parents.
        for o in views.iter().chain(tables.iter().rev()) {
            line(w, &format!("{};", drop_if_exists(&o.kind, &o.name)))?;
        }
    }
    // Every table exists before any rows go in: a table may reference one created after it.
    if schema {
        for t in &tables {
            line(w, &create(&t.sql))?;
        }
    } else if opts.clear_rows {
        for t in tables.iter().rev() {
            line(w, &format!("DELETE FROM {};", quote_ident(&t.name)))?;
        }
    }
    if data {
        for t in &tables {
            write_rows(conn, w, &t.name, opts)?;
        }
        write_sequences(conn, w, &tables)?;
    }
    for v in &views {
        line(w, &create(&v.sql))?;
    }
    if schema && opts.indexes_triggers {
        // After the rows: indexes build once, and the INSERTs above don't fire triggers.
        let owners: HashSet<&str> = tables.iter().chain(&views).map(|o| o.name.as_str()).collect();
        for kind in ["index", "trigger"] {
            for o in objects.iter().filter(|o| o.kind == kind && owners.contains(o.table.as_str())) {
                line(w, &create(&o.sql))?;
            }
        }
    }
    if opts.transaction {
        line(w, "COMMIT;")?;
    }
    w.flush().map_err(io_err)
}

/// INSERTs for every row of `table`. Generated and hidden columns are left out,
/// so the column list is written whenever the table has any.
fn write_rows(conn: &Connection, w: &mut impl Write, table: &str, opts: &SqlExportOptions) -> Result<(), AppError> {
    let writable = writable_columns(conn, table)?;
    if writable.is_empty() {
        return Ok(());
    }
    let all: i64 = conn
        .query_row("SELECT count(*) FROM pragma_table_xinfo(?1)", [table], |r| r.get(0))
        .map_err(sql_err)?;
    let list = writable.iter().map(|c| quote_ident(c)).collect::<Vec<_>>().join(",");
    let target =
        if opts.column_names || all as usize != writable.len() { format!("{}({list})", quote_ident(table)) } else { quote_ident(table) };
    let per_insert = if opts.multi_row { ROWS_PER_INSERT } else { 1 };

    let mut stmt = conn.prepare(&format!("SELECT {list} FROM {}", quote_ident(table))).map_err(sql_err)?;
    let mut rows = stmt.query([]).map_err(sql_err)?;
    let mut batch: Vec<String> = Vec::with_capacity(per_insert);
    while let Some(row) = rows.next().map_err(sql_err)? {
        let vals: Vec<String> =
            (0..writable.len()).map(|i| row.get_ref(i).map(sql_literal)).collect::<Result<_, _>>().map_err(sql_err)?;
        batch.push(format!("({})", vals.join(",")));
        if batch.len() == per_insert {
            line(w, &format!("INSERT INTO {target} VALUES{};", batch.join(",")))?;
            batch.clear();
        }
    }
    if !batch.is_empty() {
        line(w, &format!("INSERT INTO {target} VALUES{};", batch.join(",")))?;
    }
    Ok(())
}

/// AUTOINCREMENT counters of the exported tables, so new rows continue where
/// the source left off. Other tables' counters are left alone.
fn write_sequences(conn: &Connection, w: &mut impl Write, tables: &[&SchemaRow]) -> Result<(), AppError> {
    let has_sequence: bool = conn
        .query_row("SELECT count(*) > 0 FROM sqlite_schema WHERE name = 'sqlite_sequence'", [], |r| r.get(0))
        .map_err(sql_err)?;
    if !has_sequence {
        return Ok(());
    }
    let mut stmt = conn.prepare("SELECT seq FROM sqlite_sequence WHERE name = ?1").map_err(sql_err)?;
    for t in tables {
        let seq = stmt.query_row([&t.name], |r| r.get_ref(0).map(sql_literal)).optional().map_err(sql_err)?;
        if let Some(seq) = seq {
            let name = quote_str(&t.name);
            line(w, &format!("DELETE FROM sqlite_sequence WHERE name = {name};"))?;
            line(w, &format!("INSERT INTO sqlite_sequence(name, seq) VALUES({name}, {seq});"))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::tests::tmp_dir;
    use shared::ExportContent;

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

    fn export(conn: &Connection, names: &[&str], opts: &SqlExportOptions) -> String {
        let names: Vec<String> = names.iter().map(|n| n.to_string()).collect();
        let mut out = Vec::new();
        export_sql(conn, &names, opts, &mut out).unwrap();
        String::from_utf8(out).unwrap()
    }

    fn one(conn: &Connection, sql: &str) -> String {
        rows(conn, sql).remove(0).remove(0)
    }

    #[test]
    fn everything_round_trips() {
        let src = Connection::open_in_memory().unwrap();
        src.execute_batch(SETUP).unwrap();
        src.execute_batch("DELETE FROM t WHERE id = 3; INSERT INTO t(name) VALUES ('x'); DELETE FROM t WHERE id = 4;").unwrap();
        let script = export(&src, &["t", "v"], &SqlExportOptions::default());
        assert!(script.contains("BEGIN TRANSACTION;\nPRAGMA defer_foreign_keys = ON;\n"));
        assert!(script.contains("DROP VIEW IF EXISTS \"v\";\nDROP TABLE IF EXISTS \"t\";\n"));

        let dst = Connection::open_in_memory().unwrap();
        dst.execute_batch(&script).unwrap();
        let q = "SELECT * FROM t ORDER BY id";
        assert_eq!(rows(&src, q), rows(&dst, q));
        // The counter keeps the deleted rows' ids.
        assert_eq!(one(&dst, "SELECT seq FROM sqlite_sequence WHERE name = 't'"), "4");
        assert_eq!(one(&dst, "SELECT count(*) FROM sqlite_schema WHERE name IN ('ix','v')"), "2");
        // Running it again drops and recreates instead of failing.
        dst.execute_batch(&script).unwrap();
        assert_eq!(rows(&src, q), rows(&dst, q));
    }

    #[test]
    fn one_table_replaces_only_that_table() {
        let src = Connection::open_in_memory().unwrap();
        src.execute_batch(
            "CREATE TABLE a(id INTEGER PRIMARY KEY, v TEXT); INSERT INTO a VALUES (1, 'new'), (2, 'rows');
             CREATE INDEX a_v ON a(v);
             CREATE TABLE log(msg); CREATE TRIGGER a_log AFTER INSERT ON a BEGIN INSERT INTO log VALUES ('fired'); END;
             CREATE TABLE b(x); INSERT INTO b VALUES ('source b');",
        )
        .unwrap();
        let script = export(&src, &["a"], &SqlExportOptions::default());
        assert!(!script.contains("\"b\"") && !script.contains("CREATE TABLE log"));

        let dst = Connection::open_in_memory().unwrap();
        dst.execute_batch(
            "CREATE TABLE a(old_column); INSERT INTO a VALUES ('old');
             CREATE TABLE log(msg); CREATE TABLE b(x); INSERT INTO b VALUES ('target b');",
        )
        .unwrap();
        dst.execute_batch(&script).unwrap();
        assert_eq!(rows(&dst, "SELECT id, v FROM a ORDER BY id"), rows(&src, "SELECT id, v FROM a ORDER BY id"));
        assert_eq!(one(&dst, "SELECT count(*) FROM sqlite_schema WHERE name IN ('a_v', 'a_log')"), "2");
        // The trigger is created after the rows, so loading them didn't fire it.
        assert_eq!(one(&dst, "SELECT count(*) FROM log"), "0");
        assert_eq!(one(&dst, "SELECT x FROM b"), "'target b'");
    }

    #[test]
    fn keep_existing_schema_only_runs_twice() {
        let src = Connection::open_in_memory().unwrap();
        src.execute_batch(SETUP).unwrap();
        let opts = SqlExportOptions { content: ExportContent::SchemaOnly, drop_existing: false, ..Default::default() };
        let script = export(&src, &["t", "v"], &opts);
        assert!(script.contains("CREATE TABLE IF NOT EXISTS t(") && script.contains("CREATE INDEX IF NOT EXISTS ix"));
        assert!(!script.contains("INSERT") && !script.contains("DROP"));

        let dst = Connection::open_in_memory().unwrap();
        dst.execute_batch(&script).unwrap();
        dst.execute_batch(&script).unwrap();
        assert_eq!(one(&dst, "SELECT count(*) FROM t"), "0");
    }

    #[test]
    fn data_only_options() {
        let src = Connection::open_in_memory().unwrap();
        src.execute_batch(SETUP).unwrap();
        src.execute_batch("WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < 250) INSERT INTO t(name) SELECT 'row ' || i FROM n;").unwrap();
        let opts = SqlExportOptions {
            content: ExportContent::DataOnly,
            clear_rows: true,
            column_names: false,
            multi_row: true,
            transaction: false,
            ..Default::default()
        };
        // Views have no data: only the table is written.
        let script = export(&src, &["t", "v"], &opts);
        assert!(!script.contains("CREATE") && !script.contains("BEGIN") && !script.contains("\"v\""));
        assert!(script.contains("DELETE FROM \"t\";"));
        // 253 rows in batches of 100, and the generated column forces a column list.
        assert_eq!(script.matches("INSERT INTO \"t\"(\"id\",\"name\",\"n\",\"b\") VALUES(").count(), 3);

        let dst = Connection::open_in_memory().unwrap();
        dst.execute_batch(SETUP).unwrap();
        dst.execute_batch(&script).unwrap();
        let q = "SELECT * FROM t ORDER BY id";
        assert_eq!(rows(&src, q), rows(&dst, q));

        // A plain table without the column-names option.
        let plain = Connection::open_in_memory().unwrap();
        plain.execute_batch("CREATE TABLE p(a, b); INSERT INTO p VALUES (1, 'x');").unwrap();
        let script = export(&plain, &["p"], &SqlExportOptions { column_names: false, ..Default::default() });
        assert!(script.contains("INSERT INTO \"p\" VALUES(1,'x');"));
    }

    #[test]
    fn linked_tables_load_with_foreign_keys_on() {
        let src = Connection::open_in_memory().unwrap();
        src.execute_batch(
            "CREATE TABLE child(id INTEGER PRIMARY KEY, parent_id REFERENCES parent(id));
             CREATE TABLE parent(id INTEGER PRIMARY KEY);
             INSERT INTO parent VALUES (1), (2); INSERT INTO child VALUES (10, 1), (11, 2);",
        )
        .unwrap();
        let script = export(&src, &["child", "parent"], &SqlExportOptions::default());

        let dst = Connection::open_in_memory().unwrap();
        dst.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        dst.execute_batch(&script).unwrap();
        // Again, now that both tables exist and reference each other's rows.
        dst.execute_batch(&script).unwrap();
        assert_eq!(one(&dst, "SELECT count(*) FROM child JOIN parent ON parent.id = child.parent_id"), "2");
    }

    #[test]
    fn virtual_tables_skip_their_shadow_tables() {
        let src = Connection::open_in_memory().unwrap();
        src.execute_batch("CREATE VIRTUAL TABLE docs USING fts5(body); INSERT INTO docs VALUES ('hello world'), ('bye');")
            .unwrap();
        let offered: Vec<String> = rows(&src, EXPORTABLE_OBJECTS).into_iter().map(|r| r[0].clone()).collect();
        assert_eq!(offered, vec!["'docs'"]);
        assert!(export_sql(&src, &["docs_data".into()], &SqlExportOptions::default(), &mut Vec::new()).is_err());

        let script = export(&src, &["docs"], &SqlExportOptions::default());
        let dst = Connection::open_in_memory().unwrap();
        dst.execute_batch(&script).unwrap();
        dst.execute_batch(&script).unwrap();
        assert_eq!(one(&dst, "SELECT body FROM docs WHERE docs MATCH 'hello'"), "'hello world'");
    }

    #[test]
    fn nothing_to_export() {
        let src = Connection::open_in_memory().unwrap();
        src.execute_batch(SETUP).unwrap();
        let data_only = SqlExportOptions { content: ExportContent::DataOnly, ..Default::default() };
        assert!(export_sql(&src, &["v".into()], &data_only, &mut Vec::new()).is_err());
        assert!(export_sql(&src, &[], &SqlExportOptions::default(), &mut Vec::new()).is_err());
        assert!(export_sql(&src, &["missing".into()], &SqlExportOptions::default(), &mut Vec::new()).is_err());
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
