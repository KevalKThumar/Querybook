//! Per-window SQLite/SQLCipher sessions and the SQL gateway the UI talks to.
//!
//! Only SQLCipher 4 defaults are used: we never touch `cipher_default_*`
//! pragmas (they are process-wide and would leak into other windows).

use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use rusqlite::fallible_iterator::FallibleIterator;
use rusqlite::types::{Value, ValueRef};
use rusqlite::{Batch, Connection, ErrorCode, InterruptHandle, Statement, params_from_iter};
use shared::history::HistoryPage;
use shared::{
    AppError, Cell, ErrorKind, ExecOutcome, FileKind, KeyInput, ResultSet, ScriptOutcome, SessionInfo, sql,
};

use crate::history::{Journal, Store};

/// Max rows one query returns to the UI.
// ponytail: fixed cap; the grid pages with LIMIT/OFFSET so only the SQL editor hits it.
pub const ROW_CAP: usize = 5_000;

pub fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

pub fn sql_err(e: rusqlite::Error) -> AppError {
    match e.sqlite_error_code() {
        Some(ErrorCode::OperationInterrupted) => AppError::new(ErrorKind::Interrupted, "Query cancelled."),
        _ => AppError::new(ErrorKind::Sqlite, e.to_string()),
    }
}

pub fn io_err(e: impl std::fmt::Display) -> AppError {
    AppError::new(ErrorKind::Io, e.to_string())
}

pub fn invalid(msg: impl Into<String>) -> AppError {
    AppError::new(ErrorKind::Invalid, msg)
}

fn no_db() -> AppError {
    AppError::new(ErrorKind::NoDatabase, "No database is open in this window.")
}

/// Classify a file by its first 16 bytes.
pub fn file_kind(path: &Path) -> FileKind {
    let mut header = Vec::with_capacity(16);
    match fs::File::open(path).and_then(|f| f.take(16).read_to_end(&mut header)) {
        Err(_) | Ok(0) => FileKind::New,
        Ok(_) if header == b"SQLite format 3\0" => FileKind::Plain,
        Ok(_) => FileKind::Encrypted,
    }
}

/// Open `path`, apply the key (if any) and prove it decrypts.
pub fn open_conn(path: &Path, key: Option<&KeyInput>) -> Result<Connection, AppError> {
    if let Some(k) = key {
        k.validate().map_err(invalid)?;
    }
    let conn = Connection::open(path).map_err(sql_err)?;
    if let Some(k) = key {
        conn.pragma_update(None, "key", k.pragma_value()).map_err(sql_err)?;
    }
    // PRAGMA key never fails; the first read is what tells us the key was right.
    match conn.query_row("SELECT count(*) FROM sqlite_master", [], |r| r.get::<_, i64>(0)) {
        Ok(_) => {}
        Err(e) if e.sqlite_error_code() == Some(ErrorCode::NotADatabase) => {
            return Err(match key {
                Some(_) => AppError::new(
                    ErrorKind::WrongKey,
                    "Wrong password or key, or the file is not a SQLCipher 4 database.",
                ),
                None => AppError::new(ErrorKind::NeedsKey, "This database is encrypted. Enter its password."),
            });
        }
        Err(e) => return Err(sql_err(e)),
    }
    conn.busy_timeout(Duration::from_secs(5)).map_err(sql_err)?;
    Ok(conn)
}

pub fn to_value(c: &Cell) -> Result<Value, AppError> {
    Ok(match c {
        Cell::Null => Value::Null,
        Cell::Int(s) => s.parse().map(Value::Integer).unwrap_or_else(|_| Value::Text(s.clone())),
        Cell::Real(s) => s.parse().map(Value::Real).unwrap_or_else(|_| Value::Text(s.clone())),
        Cell::Text(s) => Value::Text(s.clone()),
        Cell::Blob(_) => return Err(invalid("Blob values can't be used as parameters.")),
    })
}

pub fn to_cell(v: ValueRef<'_>) -> Cell {
    match v {
        ValueRef::Null => Cell::Null,
        ValueRef::Integer(i) => Cell::Int(i.to_string()),
        ValueRef::Real(f) => Cell::Real(format!("{f:?}")),
        ValueRef::Text(t) => match std::str::from_utf8(t) {
            Ok(s) => Cell::Text(s.to_string()),
            Err(_) => Cell::Blob(t.len()),
        },
        ValueRef::Blob(b) => Cell::Blob(b.len()),
    }
}

fn values(params: &[Cell]) -> Result<Vec<Value>, AppError> {
    params.iter().map(to_value).collect()
}

fn collect(stmt: &mut Statement<'_>, params: Vec<Value>, cap: usize) -> Result<ResultSet, AppError> {
    let columns: Vec<String> = stmt.column_names().map(String::from).collect();
    let n = columns.len();
    let mut out = ResultSet { columns, ..Default::default() };
    let mut rows = stmt.query(params_from_iter(params)).map_err(sql_err)?;
    while let Some(row) = rows.next().map_err(sql_err)? {
        if out.rows.len() == cap {
            out.truncated = true;
            break;
        }
        out.rows.push((0..n).map(|i| row.get_ref(i).map(to_cell)).collect::<Result<_, _>>().map_err(sql_err)?);
    }
    Ok(out)
}

pub fn query(conn: &Connection, sql: &str, params: &[Cell]) -> Result<ResultSet, AppError> {
    let mut stmt = conn.prepare(sql).map_err(sql_err)?;
    collect(&mut stmt, values(params)?, ROW_CAP)
}

pub fn execute(conn: &Connection, sql: &str, params: &[Cell]) -> Result<ExecOutcome, AppError> {
    let affected = conn.execute(sql, params_from_iter(values(params)?)).map_err(sql_err)? as u64;
    Ok(ExecOutcome { affected, in_txn: !conn.is_autocommit() })
}

/// [`Session::exec_script`] on a bare connection, without the change history.
#[cfg(test)]
pub fn exec_script(conn: &Connection, sql: &str) -> ScriptOutcome {
    run_script(conn, sql, false, None)
}

/// [`Session::exec_import`] on a bare connection, without the change history.
#[cfg(test)]
pub fn exec_import(conn: &Connection, sql: &str) -> ScriptOutcome {
    run_script(conn, sql, true, None)
}

/// Run every statement in `sql`; keep the rows of the last one that returns columns.
/// `import`: the script's own BEGIN / COMMIT join the pending changes (Import SQL).
/// `journal` records what each statement changes, for the change history.
fn run_script(conn: &Connection, sql: &str, import: bool, journal: Option<&Journal>) -> ScriptOutcome {
    let start = Instant::now();
    let before = conn.total_changes();
    let mut out = ScriptOutcome::default();
    let mut batch = Batch::new(conn, sql);
    loop {
        let step = batch.next().map_err(sql_err).and_then(|stmt| match stmt {
            None => Ok(false),
            Some(mut st) => {
                let text = st.expanded_sql().unwrap_or_default();
                if import {
                    if sql::is_txn_end(&text) {
                        return Ok(true);
                    }
                    if sql::is_txn_begin(&text) {
                        if conn.is_autocommit() {
                            begin_edit(conn, journal)?;
                        }
                        return Ok(true);
                    }
                }
                begin_pending_for(conn, &st, journal)?;
                let tracked = journal.map(|j| j.before(conn, &text));
                let ran = if st.column_count() > 0 {
                    collect(&mut st, vec![], ROW_CAP).map(|rs| out.result = Some(rs))
                } else {
                    st.execute([]).map(|_| ()).map_err(sql_err)
                };
                if let (Some(j), Some(b)) = (journal, tracked) {
                    j.after(conn, b, ran.is_ok());
                }
                ran.map(|_| true)
            }
        });
        match step {
            Ok(true) => out.statements += 1,
            Ok(false) => break,
            Err(e) => {
                out.error = Some(format!("Statement {}: {}", out.statements + 1, e.message));
                break;
            }
        }
    }
    out.affected = conn.total_changes() - before;
    out.millis = start.elapsed().as_millis() as u64;
    out.in_txn = !conn.is_autocommit();
    out
}

/// Open the pending savepoint before a statement that writes, unless a
/// transaction is already open. SQLite's own read-only flag decides: SELECT,
/// BEGIN / COMMIT / SAVEPOINT / RELEASE and ATTACH / DETACH don't open it.
/// VACUUM and PRAGMA run as they are (see `runs_outside_transaction`).
fn begin_pending_for(conn: &Connection, st: &Statement, journal: Option<&Journal>) -> Result<(), AppError> {
    if !conn.is_autocommit() || st.readonly() || st.expanded_sql().is_some_and(|s| sql::runs_outside_transaction(&s)) {
        return Ok(());
    }
    begin_edit(conn, journal)
}

/// Open the pending savepoint, and start recording before anything is written.
fn begin_edit(conn: &Connection, journal: Option<&Journal>) -> Result<(), AppError> {
    conn.execute_batch(sql::BEGIN_EDIT).map_err(sql_err)?;
    if let Some(j) = journal {
        j.start(conn);
    }
    Ok(())
}

/// Copy the main database of `conn` into a new file at `target`, encrypted
/// with `key` (plain when None). Keeps `user_version`, which
/// `sqlcipher_export` doesn't carry over.
pub fn export_copy(conn: &Connection, target: &Path, key: Option<&KeyInput>) -> Result<(), AppError> {
    let key_value = key.map(KeyInput::pragma_value).unwrap_or_default();
    conn.execute("ATTACH DATABASE ?1 AS qb_target KEY ?2", (target.to_string_lossy(), key_value)).map_err(sql_err)?;
    let copied = conn.query_row("SELECT sqlcipher_export('qb_target')", [], |_| Ok(())).and_then(|_| {
        let v: i64 = conn.query_row("PRAGMA main.user_version", [], |r| r.get(0))?;
        conn.execute_batch(&format!("PRAGMA qb_target.user_version = {v}"))
    });
    conn.execute_batch("DETACH DATABASE qb_target").map_err(sql_err)?;
    copied.map_err(sql_err)
}

/// One database bound to one window.
pub struct Session {
    /// None for an empty (welcome) window.
    pub path: Option<PathBuf>,
    /// Declared before `journal`: its preupdate hook points into the journal,
    /// so the connection must be dropped first.
    conn: Mutex<Option<Connection>>,
    /// Kept outside `conn` so cancel works while a running query holds that lock.
    interrupt: Mutex<Option<InterruptHandle>>,
    /// Current key; needed to reopen the original if an encryption change fails.
    key: Mutex<Option<KeyInput>>,
    /// What the pending changes did, for the change history.
    journal: Journal,
    /// Where this database's saves are kept (None: not recorded).
    history: Option<Store>,
}

impl Session {
    pub fn new(path: Option<PathBuf>) -> Self {
        Self {
            path,
            conn: Mutex::new(None),
            interrupt: Mutex::new(None),
            key: Mutex::new(None),
            journal: Journal::default(),
            history: None,
        }
    }

    /// Keep the change history of this database in `dir`.
    pub fn with_history(mut self, dir: Option<PathBuf>) -> Self {
        self.history = dir.zip(self.path.as_deref()).map(|(dir, path)| Store::new(dir, path));
        self
    }

    pub fn info(&self) -> SessionInfo {
        // A busy lock means a query is running, so it's connected.
        let connected = self.conn.try_lock().map_or(true, |g| g.is_some());
        SessionInfo {
            path: self.path.as_ref().map(|p| p.display().to_string()),
            name: self
                .path
                .as_ref()
                .and_then(|p| p.file_name())
                .map_or("Querybook".into(), |n| n.to_string_lossy().into_owned()),
            kind: self.path.as_deref().map_or(FileKind::New, file_kind),
            connected,
            encrypted: lock(&self.key).is_some(),
        }
    }

    fn install(&self, conn: Connection, key: Option<KeyInput>, slot: &mut Option<Connection>) {
        self.journal.attach(&conn);
        if let Some(h) = &self.history {
            h.forget();
        }
        *lock(&self.interrupt) = Some(conn.get_interrupt_handle());
        *lock(&self.key) = key;
        *slot = Some(conn);
    }

    pub fn connect(&self, key: Option<KeyInput>) -> Result<SessionInfo, AppError> {
        let path = self.path.as_ref().ok_or_else(no_db)?;
        let is_new = file_kind(path) == FileKind::New;
        let conn = open_conn(path, key.as_ref())?;
        if is_new {
            // Write page 1 now so a new file gets its (encrypted) header immediately.
            conn.execute_batch("PRAGMA user_version = 0").map_err(sql_err)?;
        }
        self.install(conn, key, &mut lock(&self.conn));
        Ok(self.info())
    }

    pub fn with_conn<T>(&self, f: impl FnOnce(&Connection) -> Result<T, AppError>) -> Result<T, AppError> {
        let guard = lock(&self.conn);
        f(guard.as_ref().ok_or_else(no_db)?)
    }

    /// One statement from the UI (grid edits, schema changes, Write / Revert).
    pub fn execute(&self, sql: &str, params: &[Cell]) -> Result<ExecOutcome, AppError> {
        let out = self.with_conn(|c| {
            let before = self.journal.before(c, sql);
            let out = execute(c, sql, params);
            self.journal.after(c, before, out.is_ok());
            out
        });
        self.store_saves();
        out
    }

    /// Run a script from the SQL editor. Nothing is saved automatically: the
    /// first statement that writes opens the pending savepoint, so it waits for
    /// Write Changes like any grid edit. A COMMIT typed in the script is a save.
    pub fn exec_script(&self, sql: &str) -> Result<ScriptOutcome, AppError> {
        let out = self.with_conn(|c| Ok(run_script(c, sql, false, Some(&self.journal))));
        self.store_saves();
        out
    }

    /// Run a `.sql` file (Import SQL). The script's own `BEGIN … COMMIT` joins
    /// the pending changes: BEGIN opens the savepoint and COMMIT / END leave it
    /// open, so nothing reaches the file until the user writes the changes.
    pub fn exec_import(&self, sql: &str) -> Result<ScriptOutcome, AppError> {
        let out = self.with_conn(|c| Ok(run_script(c, sql, true, Some(&self.journal))));
        self.store_saves();
        out
    }

    /// Write the pending changes to the file (COMMIT), if there are any:
    /// Write Changes, Ctrl+S, or Save when closing.
    pub fn write_pending(&self) -> Result<(), AppError> {
        let out = self.with_conn(|c| {
            if c.is_autocommit() {
                return Ok(());
            }
            let before = self.journal.before(c, sql::WRITE_CHANGES);
            let out = c.execute_batch(sql::WRITE_CHANGES).map_err(sql_err);
            self.journal.after(c, before, out.is_ok());
            out
        });
        self.store_saves();
        out
    }

    /// Keep the saves the journal recorded. A failure is logged, never fatal:
    /// the changes are already in the file.
    fn store_saves(&self) {
        let saves = self.journal.take_finished();
        let Some(store) = self.history.as_ref().filter(|_| !saves.is_empty()) else { return };
        let key = lock(&self.key).clone();
        for entry in &saves {
            if let Err(e) = store.append(key.as_ref(), entry) {
                eprintln!("history: couldn't keep a save: {}", e.message);
            }
        }
    }

    /// The latest `limit` saves of this database, newest first.
    pub fn change_history(&self, limit: u32) -> Result<HistoryPage, AppError> {
        match &self.history {
            Some(store) => store.recent(lock(&self.key).as_ref(), limit),
            None => Ok(HistoryPage::default()),
        }
    }

    pub fn clear_change_history(&self) -> Result<(), AppError> {
        match &self.history {
            Some(store) => store.clear(lock(&self.key).as_ref()),
            None => Ok(()),
        }
    }

    /// Uncommitted changes exist (a savepoint/transaction is open).
    pub fn has_pending(&self) -> bool {
        // ponytail: a busy (running) connection is not checked; add a flag if that matters.
        self.conn.try_lock().is_ok_and(|g| g.as_ref().is_some_and(|c| !c.is_autocommit()))
    }

    pub fn cancel(&self) {
        if let Some(h) = lock(&self.interrupt).as_ref() {
            h.interrupt();
        }
    }

    /// Encrypt, re-key or decrypt (key = None) via `sqlcipher_export` into a temp
    /// file next to the original, verified, then atomically renamed over it.
    pub fn set_encryption(&self, key: Option<KeyInput>) -> Result<SessionInfo, AppError> {
        let path = self.path.clone().ok_or_else(no_db)?;
        if let Some(k) = &key {
            k.validate().map_err(invalid)?;
        }
        let mut guard = lock(&self.conn);
        let conn = guard.as_ref().ok_or_else(no_db)?;
        if !conn.is_autocommit() {
            return Err(invalid("Write or revert your pending changes first."));
        }
        let file_name = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
        let tmp = path.with_file_name(format!(".{file_name}.querybook-tmp"));
        let _ = fs::remove_file(&tmp);

        let export = || -> Result<(), AppError> {
            export_copy(conn, &tmp, key.as_ref())?;
            // Prove the copy opens with the new key before touching the original.
            let check = open_conn(&tmp, key.as_ref())?;
            let status: String = check.query_row("PRAGMA integrity_check", [], |r| r.get(0)).map_err(sql_err)?;
            if status != "ok" {
                return Err(invalid(format!("Integrity check of the new copy failed: {status}")));
            }
            Ok(())
        };
        if let Err(e) = export() {
            let _ = fs::remove_file(&tmp);
            return Err(e);
        }

        // Close the original; a WAL/journal left behind would be replayed onto the new file.
        let old_key = lock(&self.key).clone();
        let reopen_original = |slot: &mut Option<Connection>, e: AppError| -> Result<SessionInfo, AppError> {
            let _ = fs::remove_file(&tmp);
            if let Ok(c) = open_conn(&path, old_key.as_ref()) {
                self.install(c, old_key.clone(), slot);
            }
            Err(e)
        };
        if let Err((c, e)) = guard.take().expect("checked above").close() {
            *guard = Some(c);
            let _ = fs::remove_file(&tmp);
            return Err(sql_err(e));
        }
        let side_file = |suffix: &str| PathBuf::from(format!("{}{suffix}", path.display())).exists();
        if side_file("-wal") || side_file("-journal") {
            return reopen_original(&mut guard, invalid("Another program has this database open. Close it and try again."));
        }
        if let Err(e) = fs::rename(&tmp, &path) {
            return reopen_original(&mut guard, io_err(e));
        }
        let conn = open_conn(&path, key.as_ref())?;
        self.install(conn, key.clone(), &mut guard);
        drop(guard);
        // The history is readable with exactly the database's key.
        if let Some(h) = &self.history {
            h.rekey(old_key.as_ref(), key.as_ref());
        }
        Ok(self.info())
    }
}

/// All sessions, keyed by window label.
#[derive(Default)]
pub struct Registry(Mutex<HashMap<String, Arc<Session>>>);

impl Registry {
    pub fn get(&self, label: &str) -> Option<Arc<Session>> {
        lock(&self.0).get(label).cloned()
    }

    pub fn session(&self, label: &str) -> Result<Arc<Session>, AppError> {
        self.get(label).filter(|s| s.path.is_some()).ok_or_else(no_db)
    }

    pub fn insert(&self, label: &str, session: Session) {
        lock(&self.0).insert(label.to_string(), Arc::new(session));
    }

    pub fn remove(&self, label: &str) {
        lock(&self.0).remove(label);
    }

    /// Label of the window that already has `path` open.
    pub fn find_path(&self, path: &Path) -> Option<String> {
        lock(&self.0).iter().find(|(_, s)| s.path.as_deref() == Some(path)).map(|(l, _)| l.clone())
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    pub fn tmp_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("querybook-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn pw(s: &str) -> KeyInput {
        KeyInput::Passphrase(s.into())
    }

    fn count(conn: &Connection) -> i64 {
        conn.query_row("SELECT count(*) FROM t", [], |r| r.get(0)).unwrap()
    }

    fn kind_of(r: Result<Connection, AppError>) -> ErrorKind {
        r.err().map(|e| e.kind).expect("expected an error")
    }

    #[test]
    fn encrypted_open_flows() {
        let path = tmp_dir("enc").join("a.db");
        let s = Session::new(Some(path.clone()));
        assert_eq!(s.info().kind, FileKind::New);
        let info = s.connect(Some(pw("it's secret"))).unwrap();
        assert!(info.encrypted && info.connected);
        s.with_conn(|c| c.execute_batch("CREATE TABLE t(x); INSERT INTO t VALUES (1),(2);").map_err(sql_err)).unwrap();
        drop(s);

        assert_eq!(file_kind(&path), FileKind::Encrypted);
        assert_eq!(kind_of(open_conn(&path, None)), ErrorKind::NeedsKey);
        assert_eq!(kind_of(open_conn(&path, Some(&pw("nope")))), ErrorKind::WrongKey);
        assert_eq!(count(&open_conn(&path, Some(&pw("it's secret"))).unwrap()), 2);
    }

    #[test]
    fn raw_key_and_new_plain_file() {
        let dir = tmp_dir("raw");
        let raw = KeyInput::RawHex("2DD29CA851E7B56E4697B0E1F08507293D761A05CE4D1B628663F411A8086D99".into());
        let s = Session::new(Some(dir.join("raw.db")));
        s.connect(Some(raw.clone())).unwrap();
        s.with_conn(|c| c.execute_batch("CREATE TABLE t(x); INSERT INTO t VALUES (1);").map_err(sql_err)).unwrap();
        drop(s);
        assert_eq!(count(&open_conn(&dir.join("raw.db"), Some(&raw)).unwrap()), 1);
        let raw_as_text = pw("2DD29CA851E7B56E4697B0E1F08507293D761A05CE4D1B628663F411A8086D99");
        assert_eq!(kind_of(open_conn(&dir.join("raw.db"), Some(&raw_as_text))), ErrorKind::WrongKey);

        let plain = Session::new(Some(dir.join("plain.db")));
        plain.connect(None).unwrap();
        assert_eq!(file_kind(&dir.join("plain.db")), FileKind::Plain);
    }

    #[test]
    fn set_encryption_round_trip() {
        let path = tmp_dir("rekey").join("r.db");
        let s = Session::new(Some(path.clone()));
        s.connect(None).unwrap();
        s.with_conn(|c| {
            c.execute_batch("CREATE TABLE t(x); INSERT INTO t VALUES (1),(2),(3); PRAGMA user_version = 7;")
                .map_err(sql_err)
        })
        .unwrap();

        // plain -> encrypted
        assert!(s.set_encryption(Some(pw("one"))).unwrap().encrypted);
        assert_eq!(file_kind(&path), FileKind::Encrypted);
        assert_eq!(kind_of(open_conn(&path, None)), ErrorKind::NeedsKey);
        assert_eq!(count(&open_conn(&path, Some(&pw("one"))).unwrap()), 3);

        // change key
        s.set_encryption(Some(pw("two"))).unwrap();
        assert_eq!(kind_of(open_conn(&path, Some(&pw("one")))), ErrorKind::WrongKey);
        let c = open_conn(&path, Some(&pw("two"))).unwrap();
        assert_eq!(count(&c), 3);
        assert_eq!(c.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0)).unwrap(), 7);
        drop(c);

        // encrypted -> plain; the live session still works
        assert!(!s.set_encryption(None).unwrap().encrypted);
        assert_eq!(file_kind(&path), FileKind::Plain);
        assert_eq!(s.with_conn(|c| Ok(count(c))).unwrap(), 3);

        // refuses while changes are pending
        s.with_conn(|c| c.execute_batch("SAVEPOINT querybook; INSERT INTO t VALUES (4);").map_err(sql_err)).unwrap();
        assert!(s.has_pending());
        assert_eq!(s.set_encryption(Some(pw("x"))).unwrap_err().kind, ErrorKind::Invalid);
    }

    /// Opens a file made by another SQLCipher 4 tool (DB4S, sqlcipher CLI):
    /// `QB_INTEROP_DB=path QB_INTEROP_KEY=pw cargo test -p querybook -- --ignored`
    #[test]
    #[ignore]
    fn opens_external_sqlcipher4_file() {
        let path = PathBuf::from(std::env::var("QB_INTEROP_DB").expect("set QB_INTEROP_DB"));
        let key = pw(&std::env::var("QB_INTEROP_KEY").expect("set QB_INTEROP_KEY"));
        assert_eq!(file_kind(&path), FileKind::Encrypted);
        let rs = query(&open_conn(&path, Some(&key)).unwrap(), "SELECT name FROM sqlite_master", &[]).unwrap();
        assert!(!rs.rows.is_empty());
    }

    /// SQL-editor scripts never autocommit: writes wait for Write Changes.
    #[test]
    fn scripts_wait_for_write_changes() {
        let path = tmp_dir("pending").join("p.db");
        let s = Session::new(Some(path.clone()));
        s.connect(None).unwrap();
        let run = |sql: &str| s.with_conn(|c| Ok(exec_script(c, sql))).unwrap();

        // Reads don't open the savepoint.
        let out = run("SELECT 1; PRAGMA user_version;");
        assert!(out.error.is_none() && !out.in_txn && !s.has_pending());

        // Writes do, and another connection can't see them yet.
        let out = run("CREATE TABLE t(x); INSERT INTO t VALUES (1);");
        assert!(out.error.is_none() && out.in_txn && s.has_pending());
        let other = open_conn(&path, None).unwrap();
        let tables = |c: &Connection| c.query_row("SELECT count(*) FROM sqlite_master WHERE name = 't'", [], |r| r.get::<_, i64>(0)).unwrap();
        assert_eq!(tables(&other), 0);

        // VACUUM can't join the pending changes; SQLite refuses instead of committing them.
        let out = run("VACUUM");
        assert!(out.error.is_some() && s.has_pending());

        // Write Changes (Ctrl+S, or Save in the close prompt).
        s.write_pending().unwrap();
        assert!(!s.has_pending());
        assert_eq!(tables(&other), 1);
        assert_eq!(count(&other), 1);

        // With nothing pending, VACUUM runs directly and writing is a no-op.
        let out = run("VACUUM");
        assert!(out.error.is_none() && !out.in_txn);
        s.write_pending().unwrap();
    }

    /// Import SQL: an exported script's own BEGIN … COMMIT doesn't write the file.
    #[test]
    fn imported_scripts_wait_for_write_changes() {
        let path = tmp_dir("import").join("i.db");
        let s = Session::new(Some(path.clone()));
        s.connect(None).unwrap();
        let import = |sql: &str| s.with_conn(|c| Ok(exec_import(c, sql))).unwrap();
        let other = open_conn(&path, None).unwrap();
        let rows = |c: &Connection| c.query_row("SELECT count(*) FROM sqlite_master WHERE name = 't'", [], |r| r.get::<_, i64>(0)).unwrap();

        let script = "BEGIN TRANSACTION;\nPRAGMA defer_foreign_keys = ON;\nCREATE TABLE t(x);\nINSERT INTO t VALUES (1);\nCOMMIT;";
        let out = import(script);
        assert!(out.error.is_none(), "{:?}", out.error);
        assert_eq!(out.statements, 5);
        assert!(out.in_txn && s.has_pending());
        assert_eq!(rows(&other), 0);

        // A second import while changes are pending joins them instead of failing on BEGIN.
        let out = import("BEGIN; INSERT INTO t VALUES (2); END;");
        assert!(out.error.is_none() && out.in_txn);
        s.write_pending().unwrap();
        assert_eq!(rows(&other), 1);
        assert_eq!(count(&other), 2);

        // The SQL editor still runs a typed COMMIT as written.
        let out = s.with_conn(|c| Ok(exec_script(c, "BEGIN; INSERT INTO t VALUES (3); COMMIT;"))).unwrap();
        assert!(out.error.is_none() && !out.in_txn);
        assert_eq!(count(&other), 3);
    }

    #[test]
    fn gateway_keeps_big_integers_and_reports_script_errors() {
        let conn = Connection::open_in_memory().unwrap();
        let out = exec_script(&conn, "CREATE TABLE t(id INTEGER PRIMARY KEY, v); INSERT INTO t VALUES (9007199254740993, 'a'); SELECT * FROM t; SELECT nope;");
        assert_eq!(out.statements, 3);
        assert_eq!(out.affected, 1);
        assert!(out.error.as_deref().unwrap().starts_with("Statement 4:"));
        let rs = out.result.unwrap();
        assert_eq!(rs.rows[0][0], Cell::Int("9007199254740993".into()));
        // The script's writes wait in the pending savepoint; write them.
        assert!(out.in_txn);
        conn.execute_batch(sql::WRITE_CHANGES).unwrap();

        let key = rs.rows[0][0].clone();
        let done = execute(&conn, "UPDATE t SET v = ? WHERE _rowid_ = ?", &[Cell::Text("b".into()), key]).unwrap();
        assert_eq!(done.affected, 1);
        assert!(!done.in_txn);
        let rs = query(&conn, "SELECT v, 1.0, x'00ff' FROM t", &[]).unwrap();
        assert_eq!(rs.rows[0], vec![Cell::Text("b".into()), Cell::Real("1.0".into()), Cell::Blob(2)]);
    }
}
