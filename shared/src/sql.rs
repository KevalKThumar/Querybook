//! SQL text builders. Identifiers are always quoted; values always go as `?` params.

use crate::Cell;
use crate::lexer::{Tok, tokenize};

// Pending changes ("Write Changes" / "Revert Changes", like DB Browser).
// Every change runs inside this savepoint, so nothing reaches the file until
// the user writes it. COMMIT / ROLLBACK end whatever transaction is open.
pub const BEGIN_EDIT: &str = "SAVEPOINT querybook";
pub const WRITE_CHANGES: &str = "COMMIT";
pub const REVERT_CHANGES: &str = "ROLLBACK";

/// True for VACUUM and PRAGMA statements. They run outside the pending
/// savepoint: VACUUM fails inside a transaction, and pragmas such as
/// foreign_keys and journal_mode are ignored or refused there.
pub fn runs_outside_transaction(statement: &str) -> bool {
    starts_with_keyword(statement, &["VACUUM", "PRAGMA"])
}

/// `BEGIN [DEFERRED | IMMEDIATE | EXCLUSIVE] [TRANSACTION]`.
pub fn is_txn_begin(statement: &str) -> bool {
    starts_with_keyword(statement, &["BEGIN"])
}

/// `COMMIT` or its alias `END [TRANSACTION]`.
pub fn is_txn_end(statement: &str) -> bool {
    starts_with_keyword(statement, &["COMMIT", "END"])
}

/// `COMMIT`, `END` or `RELEASE`: statements that can write the open transaction to the file.
pub fn may_end_txn(statement: &str) -> bool {
    starts_with_keyword(statement, &["COMMIT", "END", "RELEASE"])
}

/// Tokens that aren't comments or whitespace, with the byte offset each one ends at.
fn significant_tokens(src: &str) -> Vec<(usize, Tok, &str)> {
    let mut end = 0;
    let mut out = Vec::new();
    for (t, s) in tokenize(src) {
        end += s.len();
        if t != Tok::Comment && !s.trim().is_empty() {
            out.push((end, t, s));
        }
    }
    out
}

/// True if the statement's first token is one of the keywords `words`.
fn starts_with_keyword(statement: &str, words: &[&str]) -> bool {
    significant_tokens(statement)
        .first()
        .is_some_and(|(_, t, s)| *t == Tok::Keyword && words.iter().any(|w| s.eq_ignore_ascii_case(w)))
}

pub fn quote_ident(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}

pub fn quote_str(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

pub fn blob_literal(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2 + 3);
    s.push_str("X'");
    for b in bytes {
        s.push_str(&format!("{b:02X}"));
    }
    s.push('\'');
    s
}

/// How rows of a browsed object are identified for edits.
#[derive(Clone, Debug, PartialEq)]
pub enum RowKey {
    Rowid,
    /// WITHOUT ROWID table: its primary-key columns.
    Pk(Vec<String>),
    /// Views: no edits.
    ReadOnly,
}

impl RowKey {
    /// Number of leading key cells `select_page` puts in front of each row.
    pub fn width(&self) -> usize {
        match self {
            RowKey::Rowid => 1,
            RowKey::Pk(cols) => cols.len(),
            RowKey::ReadOnly => 0,
        }
    }

    fn exprs(&self) -> Vec<String> {
        match self {
            // ponytail: shadowed if a table has its own column named _rowid_; fall back to Pk then.
            RowKey::Rowid => vec!["_rowid_".into()],
            RowKey::Pk(cols) => cols.iter().map(|c| quote_ident(c)).collect(),
            RowKey::ReadOnly => vec![],
        }
    }

    fn condition(&self) -> String {
        self.exprs().iter().map(|e| format!("{e} = ?")).collect::<Vec<_>>().join(" AND ")
    }
}

/// Per-column filter text, DB Browser style: `=x`, `<>x`, `>x`, `<=x`… compare;
/// anything else is a case-insensitive "contains".
fn filter_sql(column: &str, text: &str, params: &mut Vec<Cell>) -> Option<String> {
    let t = text.trim();
    if t.is_empty() {
        return None;
    }
    let col = quote_ident(column);
    for op in ["<>", "!=", ">=", "<=", "=", ">", "<"] {
        if let Some(rest) = t.strip_prefix(op) {
            params.push(Cell::Text(rest.trim().into()));
            return Some(format!("{col} {op} ?"));
        }
    }
    let escaped = t.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_");
    params.push(Cell::Text(format!("%{escaped}%")));
    Some(format!("CAST({col} AS TEXT) LIKE ? ESCAPE '\\'"))
}

fn where_sql(filters: &[(String, String)], params: &mut Vec<Cell>) -> String {
    let parts: Vec<String> = filters.iter().filter_map(|(c, t)| filter_sql(c, t, params)).collect();
    if parts.is_empty() { String::new() } else { format!(" WHERE {}", parts.join(" AND ")) }
}

/// `SELECT <key cells>, * ...` page for the data grid. Rows are ordered by the sort
/// column, then the key, so LIMIT/OFFSET pages are stable.
pub fn select_page(
    table: &str,
    key: &RowKey,
    sort: Option<&(String, bool)>,
    filters: &[(String, String)],
    limit: usize,
    offset: usize,
) -> (String, Vec<Cell>) {
    let mut params = Vec::new();
    let mut select: Vec<String> = key.exprs();
    select.push("*".into());
    let mut sql = format!("SELECT {} FROM {}", select.join(", "), quote_ident(table));
    sql += &where_sql(filters, &mut params);
    let mut order: Vec<String> = Vec::new();
    if let Some((col, desc)) = sort {
        order.push(format!("{} {}", quote_ident(col), if *desc { "DESC" } else { "ASC" }));
    }
    order.extend(key.exprs());
    if !order.is_empty() {
        sql += &format!(" ORDER BY {}", order.join(", "));
    }
    sql += " LIMIT ? OFFSET ?";
    params.push(Cell::Int(limit.to_string()));
    params.push(Cell::Int(offset.to_string()));
    (sql, params)
}

pub fn count_rows(table: &str, filters: &[(String, String)]) -> (String, Vec<Cell>) {
    let mut params = Vec::new();
    let sql = format!("SELECT count(*) FROM {}{}", quote_ident(table), where_sql(filters, &mut params));
    (sql, params)
}

/// Params: new value, then the row's key cells.
pub fn update_cell(table: &str, key: &RowKey, column: &str) -> String {
    format!("UPDATE {} SET {} = ? WHERE {}", quote_ident(table), quote_ident(column), key.condition())
}

/// Params: the row's key cells.
pub fn delete_row(table: &str, key: &RowKey) -> String {
    format!("DELETE FROM {} WHERE {}", quote_ident(table), key.condition())
}

/// The first `max_bytes` of a value as hex, plus its full byte length.
/// Params: the row's key cells. CAST makes TEXT count bytes like a BLOB.
pub fn blob_hex(table: &str, key: &RowKey, column: &str, max_bytes: usize) -> String {
    let bytes = format!("CAST({} AS BLOB)", quote_ident(column));
    format!("SELECT hex(substr({bytes}, 1, {max_bytes})), length({bytes}) FROM {} WHERE {}", quote_ident(table), key.condition())
}

/// Copy one row into a new row; params: the row's key cells. `columns` should
/// be the writable columns minus the rowid alias, so the copy gets a fresh key.
pub fn duplicate_row(table: &str, key: &RowKey, columns: &[String]) -> String {
    if columns.is_empty() {
        return insert_default(table);
    }
    let cols: Vec<String> = columns.iter().map(|c| quote_ident(c)).collect();
    let cols = cols.join(", ");
    format!("INSERT INTO {t} ({cols}) SELECT {cols} FROM {t} WHERE {}", key.condition(), t = quote_ident(table))
}

pub fn insert_default(table: &str) -> String {
    format!("INSERT INTO {} DEFAULT VALUES", quote_ident(table))
}

/// One column in the Create Table / Add Column forms.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ColumnDef {
    pub name: String,
    /// Raw type text (INTEGER, TEXT, VARCHAR(20)…); empty = no type.
    pub ty: String,
    pub pk: bool,
    pub autoinc: bool,
    pub not_null: bool,
    pub unique: bool,
    /// Raw default; plain words are quoted, numbers/quoted/`(expr)`/keywords kept as typed.
    pub default: String,
}

fn default_expr(d: &str) -> String {
    const KEEP: [&str; 6] = ["NULL", "TRUE", "FALSE", "CURRENT_TIME", "CURRENT_DATE", "CURRENT_TIMESTAMP"];
    let keep = d.parse::<f64>().is_ok()
        || d.starts_with('\'')
        || d.starts_with('(')
        || KEEP.iter().any(|k| k.eq_ignore_ascii_case(d));
    if keep { d.to_string() } else { quote_str(d) }
}

fn column_sql(c: &ColumnDef, inline_pk: bool) -> String {
    let mut s = quote_ident(c.name.trim());
    if !c.ty.trim().is_empty() {
        s += " ";
        s += c.ty.trim();
    }
    if c.not_null {
        s += " NOT NULL";
    }
    if inline_pk {
        s += " PRIMARY KEY";
        if c.autoinc {
            s += " AUTOINCREMENT";
        }
    }
    if c.unique {
        s += " UNIQUE";
    }
    if !c.default.trim().is_empty() {
        s += " DEFAULT ";
        s += &default_expr(c.default.trim());
    }
    s
}

pub fn create_table(name: &str, cols: &[ColumnDef], without_rowid: bool) -> String {
    let pks: Vec<&ColumnDef> = cols.iter().filter(|c| c.pk).collect();
    let mut parts: Vec<String> = cols.iter().map(|c| column_sql(c, c.pk && pks.len() == 1)).collect();
    if pks.len() > 1 {
        let names: Vec<String> = pks.iter().map(|c| quote_ident(c.name.trim())).collect();
        parts.push(format!("PRIMARY KEY ({})", names.join(", ")));
    }
    format!(
        "CREATE TABLE {} (\n\t{}\n){}",
        quote_ident(name.trim()),
        parts.join(",\n\t"),
        if without_rowid { " WITHOUT ROWID" } else { "" }
    )
}

pub fn add_column(table: &str, col: &ColumnDef) -> String {
    format!("ALTER TABLE {} ADD COLUMN {}", quote_ident(table), column_sql(col, false))
}

pub fn rename_table(old: &str, new: &str) -> String {
    format!("ALTER TABLE {} RENAME TO {}", quote_ident(old), quote_ident(new.trim()))
}

pub fn rename_column(table: &str, old: &str, new: &str) -> String {
    format!("ALTER TABLE {} RENAME COLUMN {} TO {}", quote_ident(table), quote_ident(old), quote_ident(new.trim()))
}

pub fn drop_column(table: &str, column: &str) -> String {
    format!("ALTER TABLE {} DROP COLUMN {}", quote_ident(table), quote_ident(column))
}

/// `kind` is the sqlite_schema type: table, view, index or trigger.
pub fn drop_object(kind: &str, name: &str) -> String {
    format!("DROP {} {}", kind.to_uppercase(), quote_ident(name))
}

pub fn drop_if_exists(kind: &str, name: &str) -> String {
    format!("DROP {} IF EXISTS {}", kind.to_uppercase(), quote_ident(name))
}

/// Tables (virtual ones included) and views the SQL export offers, as (name,
/// type). FTS shadow tables are left out: their virtual table recreates them.
pub const EXPORTABLE_OBJECTS: &str = "SELECT name, type FROM pragma_table_list \
     WHERE schema = 'main' AND type IN ('table', 'virtual', 'view') AND name NOT LIKE 'sqlite_%' \
     ORDER BY name COLLATE NOCASE";

/// Rows in each INSERT when the export packs several together.
pub const ROWS_PER_INSERT: usize = 100;

/// `CREATE [UNIQUE | VIRTUAL] TABLE|INDEX|VIEW|TRIGGER` statement (as stored in
/// sqlite_schema) rewritten to `CREATE … IF NOT EXISTS`. Anything else comes
/// back unchanged.
pub fn create_if_not_exists(ddl: &str) -> String {
    let words = significant_tokens(ddl);
    let kw = |i: usize, options: &[&str]| {
        words.get(i).is_some_and(|(_, t, s)| *t == Tok::Keyword && options.iter().any(|w| s.eq_ignore_ascii_case(w)))
    };
    if !kw(0, &["CREATE"]) {
        return ddl.to_string();
    }
    let mut i = 1;
    while kw(i, &["TEMP", "TEMPORARY", "UNIQUE", "VIRTUAL"]) {
        i += 1;
    }
    if !kw(i, &["TABLE", "INDEX", "VIEW", "TRIGGER"]) || kw(i + 1, &["IF"]) {
        return ddl.to_string();
    }
    let at = words[i].0;
    format!("{} IF NOT EXISTS{}", &ddl[..at], &ddl[at..])
}

/// `statement;`, putting the `;` on its own line when the statement ends in a
/// `--` comment (sqlite_schema keeps comments typed before the semicolon).
pub fn terminate(statement: &str) -> String {
    let statement = statement.trim_end();
    match tokenize(statement).last() {
        Some((Tok::Comment, s)) if s.starts_with("--") => format!("{statement}\n;"),
        _ => format!("{statement};"),
    }
}

pub fn create_index(name: &str, table: &str, columns: &[String], unique: bool) -> String {
    let cols: Vec<String> = columns.iter().map(|c| quote_ident(c)).collect();
    format!(
        "CREATE {}INDEX {} ON {} ({})",
        if unique { "UNIQUE " } else { "" },
        quote_ident(name.trim()),
        quote_ident(table),
        cols.join(", ")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statements_that_skip_the_pending_savepoint() {
        assert!(runs_outside_transaction("VACUUM"));
        assert!(runs_outside_transaction("  -- tidy up\n vacuum;"));
        assert!(runs_outside_transaction("/* fk */ PRAGMA foreign_keys = ON"));
        assert!(!runs_outside_transaction("UPDATE t SET pragma = 1"));
        assert!(!runs_outside_transaction("SELECT 'VACUUM'"));
        assert!(!runs_outside_transaction("INSERT INTO t VALUES (1)"));
        assert!(!runs_outside_transaction(""));
    }

    #[test]
    fn transaction_statements() {
        assert!(is_txn_begin("BEGIN TRANSACTION"));
        assert!(is_txn_begin("-- load\nbegin immediate"));
        assert!(is_txn_end("COMMIT"));
        assert!(is_txn_end("end transaction"));
        assert!(!is_txn_begin("CREATE TRIGGER tr AFTER INSERT ON t BEGIN SELECT 1; END"));
        assert!(!is_txn_end("SELECT 'COMMIT'"));
        assert!(!is_txn_end("ROLLBACK"));
        assert!(may_end_txn("COMMIT") && may_end_txn("end") && may_end_txn("RELEASE SAVEPOINT querybook"));
        assert!(!may_end_txn("ROLLBACK") && !may_end_txn("ROLLBACK TO querybook") && !may_end_txn("UPDATE t SET x = 1"));
    }

    #[test]
    fn if_not_exists_and_if_exists() {
        assert_eq!(create_if_not_exists("CREATE TABLE t(a)"), "CREATE TABLE IF NOT EXISTS t(a)");
        assert_eq!(create_if_not_exists("CREATE TABLE \"my t\" (a)"), "CREATE TABLE IF NOT EXISTS \"my t\" (a)");
        assert_eq!(
            create_if_not_exists("CREATE UNIQUE INDEX ix ON t(a)"),
            "CREATE UNIQUE INDEX IF NOT EXISTS ix ON t(a)"
        );
        assert_eq!(
            create_if_not_exists("CREATE VIRTUAL TABLE f USING fts5(x)"),
            "CREATE VIRTUAL TABLE IF NOT EXISTS f USING fts5(x)"
        );
        assert_eq!(
            create_if_not_exists("CREATE TRIGGER tr AFTER INSERT ON t BEGIN SELECT 1; END"),
            "CREATE TRIGGER IF NOT EXISTS tr AFTER INSERT ON t BEGIN SELECT 1; END"
        );
        assert_eq!(create_if_not_exists("/* v */ create view v as select 1"), "/* v */ create view IF NOT EXISTS v as select 1");
        assert_eq!(create_if_not_exists("CREATE TABLE IF NOT EXISTS t(a)"), "CREATE TABLE IF NOT EXISTS t(a)");
        assert_eq!(create_if_not_exists("INSERT INTO t VALUES (1)"), "INSERT INTO t VALUES (1)");
        assert_eq!(drop_if_exists("table", "a\"b"), r#"DROP TABLE IF EXISTS "a""b""#);
        assert_eq!(drop_if_exists("view", "v"), r#"DROP VIEW IF EXISTS "v""#);
    }

    #[test]
    fn statement_terminator() {
        assert_eq!(terminate("CREATE TABLE t(a)"), "CREATE TABLE t(a);");
        assert_eq!(terminate("CREATE TABLE t(a) -- note"), "CREATE TABLE t(a) -- note\n;");
        assert_eq!(terminate("CREATE TABLE t(a /* x */)  "), "CREATE TABLE t(a /* x */);");
    }

    #[test]
    fn quoting() {
        assert_eq!(quote_ident(r#"a"b"#), r#""a""b""#);
        assert_eq!(quote_str("it's"), "'it''s'");
        assert_eq!(blob_literal(&[0x00, 0xAB, 0xff]), "X'00ABFF'");
    }

    #[test]
    fn page_with_sort_and_filters() {
        let filters = vec![("name".into(), "50%".into()), ("age".into(), ">= 18".into()), ("x".into(), "  ".into())];
        let (sql, params) = select_page("users", &RowKey::Rowid, Some(&("age".into(), true)), &filters, 200, 400);
        assert_eq!(
            sql,
            r#"SELECT _rowid_, * FROM "users" WHERE CAST("name" AS TEXT) LIKE ? ESCAPE '\' AND "age" >= ? ORDER BY "age" DESC, _rowid_ LIMIT ? OFFSET ?"#
        );
        assert_eq!(
            params,
            vec![Cell::Text("%50\\%%".into()), Cell::Text("18".into()), Cell::Int("200".into()), Cell::Int("400".into())]
        );
    }

    #[test]
    fn page_for_view_and_pk_table() {
        let (sql, _) = select_page("v", &RowKey::ReadOnly, None, &[], 10, 0);
        assert_eq!(sql, r#"SELECT * FROM "v" LIMIT ? OFFSET ?"#);
        let key = RowKey::Pk(vec!["a".into(), "b".into()]);
        assert_eq!(update_cell("t", &key, "c"), r#"UPDATE "t" SET "c" = ? WHERE "a" = ? AND "b" = ?"#);
        assert_eq!(delete_row("t", &RowKey::Rowid), r#"DELETE FROM "t" WHERE _rowid_ = ?"#);
        assert_eq!(key.width(), 2);
    }

    #[test]
    fn create_table_sql() {
        let cols = vec![
            ColumnDef { name: "id".into(), ty: "INTEGER".into(), pk: true, autoinc: true, ..Default::default() },
            ColumnDef { name: "name".into(), ty: "TEXT".into(), not_null: true, default: "anon".into(), ..Default::default() },
            ColumnDef { name: "n".into(), ty: "REAL".into(), default: "-1.5".into(), unique: true, ..Default::default() },
        ];
        assert_eq!(
            create_table("t", &cols, false),
            "CREATE TABLE \"t\" (\n\t\"id\" INTEGER PRIMARY KEY AUTOINCREMENT,\n\t\"name\" TEXT NOT NULL DEFAULT 'anon',\n\t\"n\" REAL UNIQUE DEFAULT -1.5\n)"
        );
        let pk2 = vec![
            ColumnDef { name: "a".into(), pk: true, ..Default::default() },
            ColumnDef { name: "b".into(), pk: true, default: "current_timestamp".into(), ..Default::default() },
        ];
        assert_eq!(
            create_table("t", &pk2, true),
            "CREATE TABLE \"t\" (\n\t\"a\",\n\t\"b\" DEFAULT current_timestamp,\n\tPRIMARY KEY (\"a\", \"b\")\n) WITHOUT ROWID"
        );
    }

    #[test]
    fn duplicate() {
        assert_eq!(
            duplicate_row("t", &RowKey::Rowid, &["a".into(), "b c".into()]),
            r#"INSERT INTO "t" ("a", "b c") SELECT "a", "b c" FROM "t" WHERE _rowid_ = ?"#
        );
        assert_eq!(duplicate_row("t", &RowKey::Rowid, &[]), r#"INSERT INTO "t" DEFAULT VALUES"#);
    }

    #[test]
    fn blob_hex_by_key() {
        assert_eq!(
            blob_hex("t", &RowKey::Pk(vec!["k".into()]), "b", 16),
            r#"SELECT hex(substr(CAST("b" AS BLOB), 1, 16)), length(CAST("b" AS BLOB)) FROM "t" WHERE "k" = ?"#
        );
    }

    #[test]
    fn alter_and_index() {
        assert_eq!(rename_column("t", "a", " b "), r#"ALTER TABLE "t" RENAME COLUMN "a" TO "b""#);
        assert_eq!(drop_object("index", "ix"), r#"DROP INDEX "ix""#);
        assert_eq!(
            create_index("ix", "t", &["a".into(), "b".into()], true),
            r#"CREATE UNIQUE INDEX "ix" ON "t" ("a", "b")"#
        );
    }
}
