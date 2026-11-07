//! Change history: records what each save writes and keeps it on this device.
//!
//! A preupdate hook remembers how every row looked before the pending changes
//! first touched it. When the changes are written (COMMIT), each remembered row
//! is compared with how it looks then, so edits that were changed back, rolled
//! back to a savepoint, or inserted and deleted again drop out on their own.
//!
//! Each database gets its own history file in the app data folder. For an
//! encrypted database that file is a SQLCipher database with the same key, so
//! only someone who can unlock the database can read its history.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::ffi::{CStr, c_char, c_int, c_void};
use std::fs;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::types::{Value, ValueRef};
use rusqlite::{Connection, OptionalExtension, ffi, params_from_iter};
use shared::history::{
    ColumnChange, HistoryEntry, HistoryPage, RowChange, RowChangeKind, SchemaChange, SchemaChangeKind, TableChanges,
    clip, show,
};
use shared::sql::{self, quote_ident};
use shared::{AppError, Cell, ErrorKind, KeyInput};

use crate::db::{export_copy, invalid, io_err, lock, open_conn, sql_err, to_cell};

/// Rows per save kept with their values; past this the history only counts them.
pub const ROW_LIMIT: usize = 1_000;

// ── Recording ──

/// A value as first seen: what the history shows, and a digest of the full value.
#[derive(Clone)]
struct Captured {
    cell: Cell,
    digest: u64,
}

fn capture(v: ValueRef<'_>) -> Captured {
    let mut h = DefaultHasher::new();
    match v {
        ValueRef::Null => 0u8.hash(&mut h),
        ValueRef::Integer(i) => (1u8, i).hash(&mut h),
        // A REAL column hands back 5.0 for a stored 5: compare those as equal.
        ValueRef::Real(f) if f.fract() == 0.0 && f.abs() < 9.0e18 => (1u8, f as i64).hash(&mut h),
        ValueRef::Real(f) => (2u8, f.to_bits()).hash(&mut h),
        ValueRef::Text(t) => (3u8, t).hash(&mut h),
        ValueRef::Blob(b) => (4u8, b).hash(&mut h),
    }
    Captured { cell: clip(to_cell(v)), digest: h.finish() }
}

/// How a table's rows are read back; built from `pragma_table_xinfo`.
#[derive(Default)]
struct Layout {
    /// (name, plain column) in declaration order, the order the hook's values use.
    /// Generated and hidden columns aren't compared.
    columns: Vec<(String, bool)>,
    /// Primary-key column indexes in key order, used for WITHOUT ROWID tables.
    pk: Vec<usize>,
    without_rowid: bool,
    /// The INTEGER PRIMARY KEY column, which is the rowid.
    alias: Option<String>,
    /// An FTS (or other virtual table's) shadow table: never recorded.
    shadow: bool,
}

#[derive(Clone, PartialEq, Eq, Hash)]
enum KeyId {
    Rowid(i64),
    Pk(Vec<u64>),
}

/// A row the pending changes touched, and how it looked before.
struct Tracked {
    table: String,
    id: KeyId,
    /// Values that find the row again (rowid, or the primary key).
    bind: Vec<Value>,
    /// Shown key of a WITHOUT ROWID row; rowid keys are labelled at commit.
    pk_label: Option<String>,
    /// None: the row didn't exist before.
    original: Option<Vec<(String, Captured)>>,
}

#[derive(Clone, PartialEq)]
struct SchemaRow {
    kind: String,
    name: String,
    sql: String,
}

#[derive(Default)]
struct Pending {
    /// A write transaction is open and being recorded.
    active: bool,
    schema: Vec<SchemaRow>,
    schema_version: i64,
    layouts: HashMap<String, Arc<Layout>>,
    rows: Vec<Tracked>,
    seen: HashSet<(String, KeyId)>,
    /// Changes past ROW_LIMIT: (inserted, updated, deleted) per table.
    overflow: BTreeMap<String, [u64; 3]>,
    /// Saves recorded but not yet stored.
    finished: Vec<HistoryEntry>,
}

impl Pending {
    fn clear(&mut self) {
        let finished = std::mem::take(&mut self.finished);
        *self = Pending { finished, ..Default::default() };
    }

    /// The preupdate hook. Must not use the connection.
    fn on_change(&mut self, db: &str, table: &str, row: &RowEvent) {
        if !self.active || db != "main" || table.get(..7).is_some_and(|p| p.eq_ignore_ascii_case("sqlite_")) {
            return;
        }
        let layout = self.layouts.get(table).cloned();
        if layout.as_ref().is_some_and(|l| l.shadow) {
            return;
        }
        let layout = layout.as_deref();
        let get_old = |i| row.old_value(i);
        let get_new = |i| row.new_value(i);
        match row.op {
            ffi::SQLITE_INSERT => {
                let key = row_key(layout, row.new_rowid, get_new);
                self.touch(table, key, 0, || None);
            }
            ffi::SQLITE_DELETE => {
                let key = row_key(layout, row.old_rowid, get_old);
                self.touch(table, key, 2, || Some(named(layout, row.count(), get_old)));
            }
            ffi::SQLITE_UPDATE => {
                let old_key = row_key(layout, row.old_rowid, get_old);
                let new_key = row_key(layout, row.new_rowid, get_new);
                let moved = new_key.0 != old_key.0;
                self.touch(table, old_key, 1, || Some(named(layout, row.count(), get_old)));
                if moved {
                    // The row now lives under another key, which was free before.
                    self.touch(table, new_key, 1, || None);
                }
            }
            _ => {}
        }
    }

    /// Remember a row the first time it's touched. `what`: 0 insert, 1 update, 2 delete.
    fn touch(
        &mut self,
        table: &str,
        (id, bind, pk_label): (KeyId, Vec<Value>, Option<String>),
        what: usize,
        original: impl FnOnce() -> Option<Vec<(String, Captured)>>,
    ) {
        let seen = (table.to_string(), id);
        if self.seen.contains(&seen) {
            return;
        }
        if self.rows.len() >= ROW_LIMIT {
            self.overflow.entry(seen.0).or_default()[what] += 1;
            return;
        }
        let (table, id) = seen.clone();
        self.seen.insert(seen);
        self.rows.push(Tracked { table, id, bind, pk_label, original: original() });
    }
}

// ── The preupdate hook (SQLite C API; needs SQLITE_ENABLE_PREUPDATE_HOOK, see .cargo/config.toml) ──

/// The row SQLite is about to insert, update or delete.
struct RowEvent {
    db: *mut ffi::sqlite3,
    op: c_int,
    old_rowid: i64,
    new_rowid: i64,
}

impl RowEvent {
    fn count(&self) -> i32 {
        // SAFETY: only called from inside the preupdate callback for `db`.
        unsafe { ffi::sqlite3_preupdate_count(self.db) }
    }

    fn old_value(&self, i: i32) -> Option<Captured> {
        self.value(i, ffi::sqlite3_preupdate_old)
    }

    fn new_value(&self, i: i32) -> Option<Captured> {
        self.value(i, ffi::sqlite3_preupdate_new)
    }

    fn value(
        &self,
        i: i32,
        read: unsafe extern "C" fn(*mut ffi::sqlite3, c_int, *mut *mut ffi::sqlite3_value) -> c_int,
    ) -> Option<Captured> {
        let mut v: *mut ffi::sqlite3_value = std::ptr::null_mut();
        // SAFETY: inside the preupdate callback; SQLite checks `i` and the operation.
        if unsafe { read(self.db, i, &mut v) } != ffi::SQLITE_OK || v.is_null() {
            return None;
        }
        // SAFETY: `v` is valid until the callback returns; capture copies what it keeps.
        Some(capture(unsafe { value_ref(v) }))
    }
}

/// Borrow a `sqlite3_value` as a ValueRef. Text/blob before bytes, as SQLite asks.
unsafe fn value_ref<'a>(v: *mut ffi::sqlite3_value) -> ValueRef<'a> {
    unsafe {
        match ffi::sqlite3_value_type(v) {
            ffi::SQLITE_NULL => ValueRef::Null,
            ffi::SQLITE_INTEGER => ValueRef::Integer(ffi::sqlite3_value_int64(v)),
            ffi::SQLITE_FLOAT => ValueRef::Real(ffi::sqlite3_value_double(v)),
            ffi::SQLITE_TEXT => {
                let p = ffi::sqlite3_value_text(v);
                let n = ffi::sqlite3_value_bytes(v) as usize;
                ValueRef::Text(if p.is_null() { &[] } else { std::slice::from_raw_parts(p, n) })
            }
            _ => {
                let p = ffi::sqlite3_value_blob(v).cast::<u8>();
                let n = ffi::sqlite3_value_bytes(v) as usize;
                ValueRef::Blob(if p.is_null() { &[] } else { std::slice::from_raw_parts(p, n) })
            }
        }
    }
}

/// `ctx` is the `Mutex<Pending>` inside a [`Journal`], which outlives the connection.
unsafe extern "C" fn on_preupdate(
    ctx: *mut c_void,
    db: *mut ffi::sqlite3,
    op: c_int,
    db_name: *const c_char,
    table: *const c_char,
    old_rowid: i64,
    new_rowid: i64,
) {
    // A panic must not unwind into SQLite.
    let _ = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: SQLite passes valid NUL-terminated names; see above for `ctx`.
        let (pending, db_name, table) =
            unsafe { (&*ctx.cast::<Mutex<Pending>>(), CStr::from_ptr(db_name), CStr::from_ptr(table)) };
        if let (Ok(db_name), Ok(table)) = (db_name.to_str(), table.to_str()) {
            lock(pending).on_change(db_name, table, &RowEvent { db, op, old_rowid, new_rowid });
        }
    }));
}

/// The key of the row the hook reports: its primary key for a WITHOUT ROWID
/// table, else its rowid.
fn row_key(
    layout: Option<&Layout>,
    rowid: i64,
    get: impl Fn(i32) -> Option<Captured>,
) -> (KeyId, Vec<Value>, Option<String>) {
    if let Some(l) = layout.filter(|l| l.without_rowid && !l.pk.is_empty()) {
        let parts: Vec<Captured> = l.pk.iter().map(|&i| get(i as i32).unwrap_or(Captured { cell: Cell::Null, digest: 0 })).collect();
        let label = l.pk.iter().zip(&parts).map(|(&i, p)| format!("{} {}", l.columns[i].0, show(&p.cell))).collect::<Vec<_>>();
        let bind = parts.iter().map(|p| bind_value(&p.cell)).collect();
        return (KeyId::Pk(parts.iter().map(|p| p.digest).collect()), bind, Some(label.join(", ")));
    }
    (KeyId::Rowid(rowid), vec![Value::Integer(rowid)], None)
}

/// A captured key value as a parameter. Keys are short, so they're never clipped.
fn bind_value(cell: &Cell) -> Value {
    match cell {
        Cell::Null | Cell::Blob(_) => Value::Null,
        Cell::Int(s) => s.parse().map_or(Value::Null, Value::Integer),
        Cell::Real(s) => s.parse().map_or(Value::Null, Value::Real),
        Cell::Text(s) => Value::Text(s.clone()),
    }
}

/// The hook's values with their column names (plain columns only).
fn named(layout: Option<&Layout>, count: i32, get: impl Fn(i32) -> Option<Captured>) -> Vec<(String, Captured)> {
    (0..count)
        .filter_map(|i| {
            let (name, plain) = match layout.and_then(|l| l.columns.get(i as usize)) {
                Some((name, plain)) => (name.clone(), *plain),
                None => (format!("column {}", i + 1), true),
            };
            plain.then(|| get(i).map(|v| (name, v))).flatten()
        })
        .collect()
}

/// Tables, views, indexes and triggers, without internal or shadow objects.
const SCHEMA_SQL: &str = "SELECT type, name, sql FROM sqlite_schema WHERE sql IS NOT NULL AND name NOT LIKE 'sqlite_%' \
     AND name NOT IN (SELECT name FROM pragma_table_list WHERE schema = 'main' AND type = 'shadow')";

fn read_schema(conn: &Connection) -> rusqlite::Result<Vec<SchemaRow>> {
    let mut stmt = conn.prepare(SCHEMA_SQL)?;
    stmt.query_map([], |r| Ok(SchemaRow { kind: r.get(0)?, name: r.get(1)?, sql: r.get(2)? }))?.collect()
}

fn read_layouts(conn: &Connection) -> rusqlite::Result<HashMap<String, Arc<Layout>>> {
    let mut tables = conn.prepare(
        "SELECT name, type, wr FROM pragma_table_list WHERE schema = 'main' AND type IN ('table', 'shadow') \
         AND name NOT LIKE 'sqlite_%'",
    )?;
    let list: Vec<(String, String, bool)> = tables.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?.collect::<Result<_, _>>()?;
    let mut columns = conn.prepare("SELECT name, pk, hidden, upper(type) FROM pragma_table_xinfo(?1) ORDER BY cid")?;
    let mut out = HashMap::new();
    for (name, kind, without_rowid) in list {
        if kind == "shadow" {
            out.insert(name, Arc::new(Layout { shadow: true, ..Default::default() }));
            continue;
        }
        let cols: Vec<(String, i64, i64, String)> =
            columns.query_map([&name], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?.collect::<Result<_, _>>()?;
        let mut pk: Vec<(i64, usize)> = cols.iter().enumerate().filter(|(_, c)| c.1 > 0).map(|(i, c)| (c.1, i)).collect();
        pk.sort();
        let alias = match pk.as_slice() {
            [(_, i)] if !without_rowid && cols[*i].3 == "INTEGER" => Some(cols[*i].0.clone()),
            _ => None,
        };
        let layout = Layout {
            columns: cols.into_iter().map(|(n, _, hidden, _)| (n, hidden == 0)).collect(),
            pk: pk.into_iter().map(|(_, i)| i).collect(),
            without_rowid,
            alias,
            shadow: false,
        };
        out.insert(name, Arc::new(layout));
    }
    Ok(out)
}

fn schema_version(conn: &Connection) -> rusqlite::Result<i64> {
    conn.query_row("PRAGMA schema_version", [], |r| r.get(0))
}

/// The row's plain columns now, or None if it's gone.
fn read_row(conn: &Connection, table: &str, layout: &Layout, bind: &[Value]) -> rusqlite::Result<Option<Vec<(String, Captured)>>> {
    let plain: Vec<&String> = layout.columns.iter().filter(|(_, p)| *p).map(|(n, _)| n).collect();
    let select = if plain.is_empty() { "1".to_string() } else { plain.iter().map(|n| quote_ident(n)).collect::<Vec<_>>().join(", ") };
    let condition = if layout.without_rowid {
        layout.pk.iter().map(|&i| format!("{} = ?", quote_ident(&layout.columns[i].0))).collect::<Vec<_>>().join(" AND ")
    } else {
        "_rowid_ = ?".to_string()
    };
    let query = format!("SELECT {select} FROM {} WHERE {condition}", quote_ident(table));
    conn.query_row(&query, params_from_iter(bind), |r| {
        plain.iter().enumerate().map(|(i, n)| Ok(((*n).clone(), capture(r.get_ref(i)?)))).collect()
    })
    .optional()
}

fn diff_schema(before: &[SchemaRow], after: &[SchemaRow]) -> Vec<SchemaChange> {
    let same = |a: &SchemaRow, b: &SchemaRow| a.kind == b.kind && a.name == b.name;
    let change = |kind, row: &SchemaRow, before: Option<&SchemaRow>, after: Option<&SchemaRow>| SchemaChange {
        kind,
        object: row.kind.clone(),
        name: row.name.clone(),
        before: before.map(|r| r.sql.clone()),
        after: after.map(|r| r.sql.clone()),
    };
    let mut out = Vec::new();
    for a in after {
        match before.iter().find(|b| same(a, b)) {
            None => out.push(change(SchemaChangeKind::Created, a, None, Some(a))),
            Some(b) if b.sql != a.sql => out.push(change(SchemaChangeKind::Altered, a, Some(b), Some(a))),
            Some(_) => {}
        }
    }
    for b in before.iter().filter(|b| !after.iter().any(|a| same(a, b))) {
        out.push(change(SchemaChangeKind::Dropped, b, Some(b), None));
    }
    out
}

/// Compare a row before and after the pending changes.
fn row_change(key: String, before: Option<&Vec<(String, Captured)>>, after: Option<Vec<(String, Captured)>>) -> Option<RowChange> {
    let all = |values: &[(String, Captured)], old: bool| -> Vec<ColumnChange> {
        values
            .iter()
            .map(|(name, v)| ColumnChange {
                name: name.clone(),
                old: old.then(|| v.cell.clone()),
                new: (!old).then(|| v.cell.clone()),
            })
            .collect()
    };
    let (kind, columns) = match (before, after) {
        (None, None) => return None,
        (None, Some(now)) => (RowChangeKind::Insert, all(&now, false)),
        (Some(was), None) => (RowChangeKind::Delete, all(was, true)),
        (Some(was), Some(now)) => {
            let changed: Vec<ColumnChange> = now
                .iter()
                .filter_map(|(name, v)| {
                    let old = &was.iter().find(|(n, _)| n == name)?.1;
                    (old.digest != v.digest).then(|| ColumnChange {
                        name: name.clone(),
                        old: Some(old.cell.clone()),
                        new: Some(v.cell.clone()),
                    })
                })
                .collect();
            if changed.is_empty() {
                return None;
            }
            (RowChangeKind::Update, changed)
        }
    };
    Some(RowChange { kind, key, columns })
}

fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

/// Taken before each statement; [`Journal::after`] uses it.
pub struct Before {
    was_pending: bool,
    draft: Option<HistoryEntry>,
}

/// What the pending changes did so far, shared with the connection's preupdate hook.
#[derive(Clone, Default)]
pub struct Journal(Arc<Mutex<Pending>>);

impl Journal {
    /// Record this connection's changes. Called for every connection a session
    /// installs; the journal must outlive the connection (`Session` drops its
    /// connection first).
    pub fn attach(&self, conn: &Connection) {
        lock(&self.0).clear();
        // SAFETY: the handle is open; the context pointer stays valid as long as
        // this journal, which outlives the connection.
        unsafe {
            ffi::sqlite3_preupdate_hook(conn.handle(), Some(on_preupdate), Arc::as_ptr(&self.0).cast_mut().cast());
        }
    }

    /// A write transaction just opened: snapshot the schema and start recording.
    pub fn start(&self, conn: &Connection) {
        if lock(&self.0).active {
            return;
        }
        let snapshot = (|| Ok::<_, rusqlite::Error>((read_schema(conn)?, read_layouts(conn)?, schema_version(conn)?)))();
        let Ok((schema, layouts, version)) = snapshot else { return };
        let mut p = lock(&self.0);
        let finished = std::mem::take(&mut p.finished);
        *p = Pending { active: true, schema, layouts, schema_version: version, finished, ..Default::default() };
    }

    /// Call before running `statement`. Before anything that can write the
    /// changes to the file, work out what they are while they're still pending.
    pub fn before(&self, conn: &Connection, statement: &str) -> Before {
        let was_pending = !conn.is_autocommit();
        let draft = if was_pending && sql::may_end_txn(statement) { self.draft(conn) } else { None };
        Before { was_pending, draft }
    }

    /// Call after the statement ran (`ok`: it succeeded).
    pub fn after(&self, conn: &Connection, before: Before, ok: bool) {
        match (before.was_pending, !conn.is_autocommit()) {
            (false, true) => self.start(conn),
            (true, false) => {
                let mut p = lock(&self.0);
                if let Some(entry) = before.draft.filter(|e| ok && !e.is_empty()) {
                    p.finished.push(entry);
                }
                p.clear();
            }
            (true, true) => self.refresh_layouts(conn),
            (false, false) => {}
        }
    }

    /// Saves recorded since the last call.
    pub fn take_finished(&self) -> Vec<HistoryEntry> {
        std::mem::take(&mut lock(&self.0).finished)
    }

    /// A statement changed the schema: re-read the table layouts the hook names values with.
    fn refresh_layouts(&self, conn: &Connection) {
        let Ok(version) = schema_version(conn) else { return };
        if lock(&self.0).schema_version == version {
            return;
        }
        if let Ok(layouts) = read_layouts(conn) {
            let mut p = lock(&self.0);
            p.layouts = layouts;
            p.schema_version = version;
        }
    }

    /// What writing the pending changes now would save. Reads only.
    fn draft(&self, conn: &Connection) -> Option<HistoryEntry> {
        self.refresh_layouts(conn);
        let p = lock(&self.0);
        if !p.active {
            return None;
        }
        let now = read_schema(conn).ok()?;
        let mut tables: Vec<TableChanges> = Vec::new();
        fn table_for(tables: &mut Vec<TableChanges>, name: &str) -> usize {
            tables.iter().position(|t| t.table == name).unwrap_or_else(|| {
                tables.push(TableChanges { table: name.to_string(), ..Default::default() });
                tables.len() - 1
            })
        }
        for t in &p.rows {
            // A table dropped since: the schema changes say so.
            let Some(layout) = p.layouts.get(&t.table).filter(|l| !l.shadow) else { continue };
            let Ok(current) = read_row(conn, &t.table, layout, &t.bind) else { continue };
            let key = match (&t.id, &t.pk_label) {
                (_, Some(label)) => label.clone(),
                (KeyId::Rowid(r), None) => format!("{} {r}", layout.alias.as_deref().unwrap_or("rowid")),
                (KeyId::Pk(_), None) => String::new(),
            };
            let Some(change) = row_change(key, t.original.as_ref(), current) else { continue };
            let i = table_for(&mut tables, &t.table);
            match change.kind {
                RowChangeKind::Insert => tables[i].inserted += 1,
                RowChangeKind::Update => tables[i].updated += 1,
                RowChangeKind::Delete => tables[i].deleted += 1,
            }
            tables[i].rows.push(change);
        }
        for (name, [ins, upd, del]) in &p.overflow {
            if p.layouts.get(name).is_none_or(|l| l.shadow) {
                continue;
            }
            let i = table_for(&mut tables, name);
            tables[i].inserted += ins;
            tables[i].updated += upd;
            tables[i].deleted += del;
            tables[i].approx = true;
        }
        Some(HistoryEntry { id: 0, saved_at: now_ms(), schema: diff_schema(&p.schema, &now), tables })
    }
}

// ── Storage ──

/// FNV-1a: a hash that stays the same across Rust versions, for file names.
fn fnv1a(s: &str) -> u64 {
    s.bytes().fold(0xcbf2_9ce4_8422_2325, |h, b| (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3))
}

const STORE_SETUP: &str = "PRAGMA secure_delete = ON;
CREATE TABLE IF NOT EXISTS meta(key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS saves(id INTEGER PRIMARY KEY, saved_at INTEGER NOT NULL, entry TEXT NOT NULL);";

/// The history file of one database, opened when first needed.
pub struct Store {
    dir: PathBuf,
    db_path: String,
    conn: Mutex<Option<Connection>>,
}

impl Store {
    pub fn new(dir: PathBuf, db_path: &Path) -> Self {
        Self { dir, db_path: db_path.display().to_string(), conn: Mutex::new(None) }
    }

    /// `<hash of the database path>.db`, or `-2`, `-3` … if another database has that name.
    fn candidate(&self, n: u32) -> PathBuf {
        let hash = fnv1a(&self.db_path);
        self.dir.join(if n == 1 { format!("{hash:016x}.db") } else { format!("{hash:016x}-{n}.db") })
    }

    /// Open (or, with `create`, start) this database's history file.
    fn open(&self, key: Option<&KeyInput>, create: bool) -> Result<Option<(Connection, PathBuf)>, AppError> {
        for n in 1..=16 {
            let file = self.candidate(n);
            if !file.exists() {
                if !create {
                    return Ok(None);
                }
                fs::create_dir_all(&self.dir).map_err(io_err)?;
            }
            let conn = match open_conn(&file, key) {
                Ok(c) => c,
                // Locked with another key (changed outside Querybook) or damaged:
                // keep it aside and start over.
                Err(e) if matches!(e.kind, ErrorKind::NeedsKey | ErrorKind::WrongKey) => {
                    let aside = file.with_extension(format!("unreadable-{}.db", now_ms()));
                    fs::rename(&file, &aside).map_err(io_err)?;
                    eprintln!("history: {} can't be read with this key; moved to {}", file.display(), aside.display());
                    if !create {
                        return Ok(None);
                    }
                    open_conn(&file, key)?
                }
                Err(e) => return Err(e),
            };
            conn.execute_batch(STORE_SETUP).map_err(sql_err)?;
            let owner: Option<String> =
                conn.query_row("SELECT value FROM meta WHERE key = 'path'", [], |r| r.get(0)).optional().map_err(sql_err)?;
            match owner {
                Some(p) if p != self.db_path => continue,
                Some(_) => return Ok(Some((conn, file))),
                None => {
                    conn.execute("INSERT INTO meta(key, value) VALUES ('path', ?1)", [&self.db_path]).map_err(sql_err)?;
                    conn.execute_batch("PRAGMA user_version = 1").map_err(sql_err)?;
                    return Ok(Some((conn, file)));
                }
            }
        }
        Err(invalid("No free name for the history file."))
    }

    fn with<T>(
        &self,
        key: Option<&KeyInput>,
        create: bool,
        f: impl FnOnce(&Connection) -> Result<T, AppError>,
    ) -> Result<Option<T>, AppError> {
        let mut slot = lock(&self.conn);
        if slot.is_none() {
            *slot = self.open(key, create)?.map(|(c, _)| c);
        }
        slot.as_ref().map(f).transpose()
    }

    /// Close the file; the next use opens it again (with the session's current key).
    pub fn forget(&self) {
        *lock(&self.conn) = None;
    }

    pub fn append(&self, key: Option<&KeyInput>, entry: &HistoryEntry) -> Result<(), AppError> {
        let json = serde_json::to_string(entry).map_err(|e| invalid(e.to_string()))?;
        self.with(key, true, |c| {
            c.execute("INSERT INTO saves(saved_at, entry) VALUES (?1, ?2)", (entry.saved_at, json)).map_err(sql_err)
        })?;
        Ok(())
    }

    /// The latest `limit` saves, newest first.
    pub fn recent(&self, key: Option<&KeyInput>, limit: u32) -> Result<HistoryPage, AppError> {
        let page = self.with(key, false, |c| {
            let total: i64 = c.query_row("SELECT count(*) FROM saves", [], |r| r.get(0)).map_err(sql_err)?;
            let mut stmt = c.prepare("SELECT id, saved_at, entry FROM saves ORDER BY id DESC LIMIT ?1").map_err(sql_err)?;
            let rows = stmt
                .query_map([limit], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, String>(2)?)))
                .map_err(sql_err)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(sql_err)?;
            let entries = rows
                .into_iter()
                .filter_map(|(id, saved_at, json)| {
                    let entry: HistoryEntry = serde_json::from_str(&json).ok()?;
                    Some(HistoryEntry { id, saved_at, ..entry })
                })
                .collect();
            Ok(HistoryPage { entries, total: total as u64 })
        })?;
        Ok(page.unwrap_or_default())
    }

    pub fn clear(&self, key: Option<&KeyInput>) -> Result<(), AppError> {
        self.with(key, false, |c| c.execute_batch("DELETE FROM saves").map_err(sql_err))?;
        Ok(())
    }

    /// The database's key was set, changed or removed: give the history the same key.
    /// If that fails the history is deleted rather than left readable.
    pub fn rekey(&self, old: Option<&KeyInput>, new: Option<&KeyInput>) {
        self.forget();
        let Ok(Some((conn, file))) = self.open(old, false) else { return };
        let tmp = file.with_extension("rekey-tmp");
        let _ = fs::remove_file(&tmp);
        let copied = export_copy(&conn, &tmp, new);
        drop(conn);
        if let Err(e) = copied.and_then(|_| fs::rename(&tmp, &file).map_err(io_err)) {
            eprintln!("history: couldn't re-key {}: {}; deleting it", file.display(), e.message);
            let _ = fs::remove_file(&tmp);
            let _ = fs::remove_file(&file);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::tests::tmp_dir;
    use crate::db::{Session, file_kind};
    use shared::FileKind;

    fn session(name: &str, key: Option<KeyInput>) -> (Session, PathBuf) {
        let dir = tmp_dir(name);
        let s = Session::new(Some(dir.join("d.db"))).with_history(Some(dir.join("history")));
        s.connect(key).unwrap();
        (s, dir)
    }

    fn run(s: &Session, sql: &str) {
        let out = s.exec_script(sql).unwrap();
        assert!(out.error.is_none(), "{sql}: {:?}", out.error);
    }

    fn history(s: &Session) -> HistoryPage {
        s.change_history(10).unwrap()
    }

    fn text(s: &str) -> Option<Cell> {
        Some(Cell::Text(s.into()))
    }

    fn int(i: i64) -> Option<Cell> {
        Some(Cell::Int(i.to_string()))
    }

    const SETUP: &str = "CREATE TABLE t(id INTEGER PRIMARY KEY, name TEXT, city TEXT);
        INSERT INTO t VALUES (1, 'Ann', NULL), (2, 'Bob', 'Surat'), (3, 'Cy', 'Pune');";

    #[test]
    fn a_save_records_rows_and_schema() {
        let (s, _) = session("hist-basic", None);
        run(&s, SETUP);
        assert_eq!(history(&s).total, 0, "nothing until the changes are written");
        s.write_pending().unwrap();

        let page = history(&s);
        assert_eq!(page.total, 1);
        let first = &page.entries[0];
        assert_eq!(first.schema.len(), 1);
        assert_eq!(first.schema[0].label(), "Created table t");
        assert_eq!(first.tables[0].inserted, 3);

        // Grid-style edits: begin_edit, then single statements.
        s.execute(sql::BEGIN_EDIT, &[]).unwrap();
        s.execute("UPDATE t SET name = ?, city = ? WHERE _rowid_ = ?", &[Cell::Text("Anna".into()), Cell::Text("Pune".into()), Cell::Int("1".into())]).unwrap();
        s.execute("UPDATE t SET name = 'Bobby' WHERE _rowid_ = 2", &[]).unwrap();
        s.execute("UPDATE t SET name = 'Bob' WHERE _rowid_ = 2", &[]).unwrap(); // changed back: not a change
        s.execute("INSERT INTO t(name) VALUES ('Dee')", &[]).unwrap();
        s.execute("DELETE FROM t WHERE _rowid_ = 3", &[]).unwrap();
        s.execute("INSERT INTO t(name) VALUES ('gone')", &[]).unwrap();
        s.execute("DELETE FROM t WHERE name = 'gone'", &[]).unwrap(); // inserted and deleted: nothing
        s.write_pending().unwrap();

        let page = history(&s);
        assert_eq!(page.total, 2);
        let e = &page.entries[0];
        assert!(e.schema.is_empty());
        assert_eq!(e.tables.len(), 1);
        let t = &e.tables[0];
        assert_eq!((t.table.as_str(), t.inserted, t.updated, t.deleted, t.approx), ("t", 1, 1, 1, false));
        assert_eq!(e.summary(), "1 table · 3 rows");

        let update = &t.rows[0];
        assert_eq!((update.kind, update.key.as_str()), (RowChangeKind::Update, "id 1"));
        assert_eq!(update.columns, vec![
            ColumnChange { name: "name".into(), old: text("Ann"), new: text("Anna") },
            ColumnChange { name: "city".into(), old: Some(Cell::Null), new: text("Pune") },
        ]);
        let delete = &t.rows[2];
        assert_eq!((delete.kind, delete.key.as_str()), (RowChangeKind::Delete, "id 3"));
        assert_eq!(delete.columns[1], ColumnChange { name: "name".into(), old: text("Cy"), new: None });
        let insert = &t.rows[1];
        assert_eq!((insert.kind, insert.key.as_str()), (RowChangeKind::Insert, "id 4"));
        assert_eq!(insert.columns[0], ColumnChange { name: "id".into(), old: None, new: int(4) });
        assert!(page.entries[0].id > page.entries[1].id && page.entries[0].saved_at > 0);
    }

    #[test]
    fn reverted_and_rolled_back_changes_are_not_recorded() {
        let (s, _) = session("hist-revert", None);
        run(&s, "CREATE TABLE t(id INTEGER PRIMARY KEY, v TEXT UNIQUE); INSERT INTO t VALUES (1, 'a'), (2, 'b'), (3, 'bx');");
        s.write_pending().unwrap();

        // Revert: nothing.
        run(&s, "UPDATE t SET v = 'z' WHERE id = 1");
        s.execute(sql::REVERT_CHANGES, &[]).unwrap();
        assert_eq!(history(&s).total, 1);

        // A statement that fails half way, and a rolled-back savepoint.
        let out = s.exec_script("UPDATE t SET v = v || 'x'").unwrap();
        assert!(out.error.is_some() && out.in_txn);
        run(&s, "SAVEPOINT inner; UPDATE t SET v = 'gone' WHERE id = 2; ROLLBACK TO inner; RELEASE inner;");
        run(&s, "UPDATE t SET v = 'kept' WHERE id = 3");
        s.write_pending().unwrap();

        let page = history(&s);
        assert_eq!(page.total, 2);
        let t = &page.entries[0].tables[0];
        assert_eq!((t.updated, t.rows.len()), (1, 1));
        assert_eq!(t.rows[0].key, "id 3");
        assert_eq!(t.rows[0].columns, vec![ColumnChange { name: "v".into(), old: text("bx"), new: text("kept") }]);
    }

    #[test]
    fn every_way_of_saving_is_recorded() {
        let (s, _) = session("hist-commits", None);
        // A COMMIT typed in the SQL editor.
        run(&s, "BEGIN; CREATE TABLE t(x); INSERT INTO t VALUES (1); COMMIT;");
        assert_eq!(history(&s).total, 1);
        // Two saves in one script.
        run(&s, "INSERT INTO t VALUES (2); COMMIT; INSERT INTO t VALUES (3); END;");
        assert_eq!(history(&s).total, 3);
        // Import SQL: its own COMMIT doesn't save; Write Changes does.
        let out = s.exec_import("BEGIN TRANSACTION; INSERT INTO t VALUES (4); COMMIT;").unwrap();
        assert!(out.error.is_none() && out.in_txn);
        assert_eq!(history(&s).total, 3);
        s.write_pending().unwrap();
        // RELEASE of the pending savepoint writes it too.
        s.execute(sql::BEGIN_EDIT, &[]).unwrap();
        s.execute("DELETE FROM t WHERE x = 1", &[]).unwrap();
        s.execute("RELEASE querybook", &[]).unwrap();
        // Nothing pending: writing is a no-op, and a save that changed nothing isn't kept.
        s.write_pending().unwrap();
        run(&s, "UPDATE t SET x = x");
        s.write_pending().unwrap();

        let page = history(&s);
        assert_eq!(page.total, 5);
        assert_eq!(page.entries[0].tables[0].deleted, 1);
        assert_eq!(page.entries[1].tables[0].rows[0].columns[0], ColumnChange { name: "x".into(), old: None, new: int(4) });
        // The tab asks for the last 3.
        assert_eq!(s.change_history(3).unwrap().entries.len(), 3);
    }

    #[test]
    fn schema_changes_and_keys() {
        let (s, _) = session("hist-schema", None);
        run(&s, "CREATE TABLE p(a INTEGER, b TEXT, v, PRIMARY KEY (a, b)) WITHOUT ROWID; INSERT INTO p VALUES (1, 'x', 'old');
                 CREATE TABLE r(v); INSERT INTO r VALUES ('one'); CREATE TABLE gone(x); CREATE INDEX ix ON r(v);");
        s.write_pending().unwrap();

        run(&s, "UPDATE p SET v = 'new'; ALTER TABLE r ADD COLUMN w DEFAULT 0; UPDATE r SET w = 5; DROP TABLE gone;
                 UPDATE r SET _rowid_ = 10;");
        s.write_pending().unwrap();
        let e = &history(&s).entries[0];
        let labels: Vec<String> = e.schema.iter().map(|c| c.label()).collect();
        assert_eq!(labels, ["Changed table r", "Dropped table gone"]);
        assert!(e.schema[0].after.as_deref().unwrap().contains("w DEFAULT 0"));

        let p = e.tables.iter().find(|t| t.table == "p").unwrap();
        assert_eq!(p.rows[0].key, "a 1, b \"x\"");
        assert_eq!(p.rows[0].columns, vec![ColumnChange { name: "v".into(), old: text("old"), new: text("new") }]);
        // A table without an INTEGER PRIMARY KEY shows rowids; a moved rowid is a delete plus an insert.
        let r = e.tables.iter().find(|t| t.table == "r").unwrap();
        assert_eq!((r.inserted, r.updated, r.deleted), (1, 0, 1));
        assert_eq!(r.rows.iter().map(|c| c.key.as_str()).collect::<Vec<_>>(), ["rowid 1", "rowid 10"]);
        assert_eq!(r.rows[1].columns[1], ColumnChange { name: "w".into(), old: None, new: int(5) });
    }

    #[test]
    fn big_saves_and_big_values() {
        let (s, _) = session("hist-big", None);
        run(&s, "CREATE TABLE t(id INTEGER PRIMARY KEY, v);
                 WITH RECURSIVE c(x) AS (SELECT 1 UNION ALL SELECT x + 1 FROM c WHERE x < 1005) INSERT INTO t(v) SELECT x FROM c;");
        s.write_pending().unwrap();
        let t = &history(&s).entries[0].tables[0];
        assert_eq!((t.inserted, t.rows.len(), t.approx), (1005, ROW_LIMIT, true));
        assert!(t.counts_label().starts_with("about"));

        run(&s, &format!("UPDATE t SET v = '{}' WHERE id = 1; UPDATE t SET v = x'00ff' WHERE id = 2;", "a".repeat(500)));
        s.write_pending().unwrap();
        let rows = &history(&s).entries[0].tables[0].rows;
        let Some(Cell::Text(long)) = &rows[0].columns[0].new else { panic!() };
        assert!(long.ends_with('…') && long.chars().count() == shared::history::MAX_TEXT_CHARS + 1);
        assert_eq!(rows[1].columns[0].new, Some(Cell::Blob(2)));
    }

    #[test]
    fn fts_shadow_tables_are_left_out() {
        let (s, _) = session("hist-fts", None);
        run(&s, "CREATE VIRTUAL TABLE docs USING fts5(body); INSERT INTO docs VALUES ('hello world');");
        s.write_pending().unwrap();
        let e = &history(&s).entries[0];
        assert_eq!(e.schema.iter().map(|c| c.label()).collect::<Vec<_>>(), ["Created table docs"]);
        assert!(e.tables.is_empty(), "{:?}", e.tables);
    }

    #[test]
    fn each_database_has_its_own_history() {
        let dir = tmp_dir("hist-two");
        let open = |name: &str| {
            let s = Session::new(Some(dir.join(name))).with_history(Some(dir.join("history")));
            s.connect(None).unwrap();
            s
        };
        let (a, b) = (open("a.db"), open("b.db"));
        run(&a, "CREATE TABLE t(x)");
        a.write_pending().unwrap();
        assert_eq!((history(&a).total, history(&b).total), (1, 0));
        assert_eq!(fs::read_dir(dir.join("history")).unwrap().count(), 1, "b has no file until it saves");

        a.clear_change_history().unwrap();
        assert_eq!(history(&a).total, 0);
        // Reopening finds the same file.
        run(&a, "INSERT INTO t VALUES (1)");
        a.write_pending().unwrap();
        drop(a);
        assert_eq!(history(&open("a.db")).total, 1);
    }

    #[test]
    fn encrypted_history_follows_the_database_key() {
        let pw = |s: &str| KeyInput::Passphrase(s.into());
        let (s, dir) = session("hist-enc", Some(pw("one")));
        run(&s, "CREATE TABLE secret(v); INSERT INTO secret VALUES ('top secret');");
        s.write_pending().unwrap();
        let files: Vec<PathBuf> = fs::read_dir(dir.join("history")).unwrap().map(|e| e.unwrap().path()).collect();
        let [file] = files.as_slice() else { panic!("{files:?}") };
        assert_eq!(file_kind(file), FileKind::Encrypted);
        assert!(!String::from_utf8_lossy(&fs::read(file).unwrap()).contains("top secret"));
        assert_eq!(open_conn(file, None).unwrap_err().kind, ErrorKind::NeedsKey);
        assert!(open_conn(file, Some(&pw("one"))).is_ok());

        // Change the key: the history moves to the new key.
        s.set_encryption(Some(pw("two"))).unwrap();
        assert_eq!(open_conn(file, Some(&pw("one"))).unwrap_err().kind, ErrorKind::WrongKey);
        assert!(open_conn(file, Some(&pw("two"))).is_ok());
        assert_eq!(history(&s).total, 1);

        // Remove encryption: a plain history, still there.
        s.set_encryption(None).unwrap();
        assert_eq!(file_kind(file), FileKind::Plain);
        assert_eq!(history(&s).total, 1);
        assert_eq!(history(&s).entries[0].tables[0].rows[0].columns[0].new, text("top secret"));

        // A history the current key can't open is moved aside, and a new one starts.
        drop(s);
        let s = Session::new(Some(dir.join("d.db"))).with_history(Some(dir.join("history")));
        fs::write(file, b"not a database at all, but long enough to have a header").unwrap();
        s.connect(None).unwrap();
        assert_eq!(history(&s).total, 0);
        run(&s, "INSERT INTO secret VALUES ('x')");
        s.write_pending().unwrap();
        assert_eq!(history(&s).total, 1);
        let names: Vec<String> = fs::read_dir(dir.join("history")).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        assert!(names.iter().any(|n| n.contains(".unreadable-")), "{names:?}");
    }
}
