//! Crash reports, kept on this computer (like a local Xcode Organizer).
//!
//! - A panic hook writes `crashes/crash-<time>-<pid>.json` before the process
//!   aborts (release builds use `panic = "abort"`; the hook still runs first).
//! - Each run holds a lock on `sessions/<id>.lock`. A lock file nobody holds at
//!   the next launch means that run ended without closing normally, e.g. a
//!   crash inside SQLCipher's C code that no panic hook sees. It is listed as an
//!   "unclean exit" report.
//! - Nothing is sent anywhere. The user copies, saves or attaches a report.
//!
//! Folders (`app_log_dir`): `%LOCALAPPDATA%\com.kevalthumar.querybook\logs` on
//! Windows, `~/Library/Logs/com.kevalthumar.querybook` on macOS,
//! `~/.local/share/com.kevalthumar.querybook/logs` on Linux.

use std::fs::{self, File};
use std::panic::PanicHookInfo;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use shared::support::{CrashReport, ReportKind, compact_time, redact, redact_home, rfc3339, valid_report_id};

use crate::applog::{self, home_dir, now_secs};

/// Reports kept; older ones are deleted.
pub const KEEP: usize = 50;

/// Facts every report carries, set once the app knows them.
struct Ctx {
    logs: PathBuf,
    version: String,
    webview: Option<String>,
}

static CTX: OnceLock<Ctx> = OnceLock::new();
static SESSION_ID: OnceLock<String> = OnceLock::new();
/// This run's lock, held until the process ends.
static SESSION_LOCK: OnceLock<(File, PathBuf)> = OnceLock::new();

pub fn session_id() -> &'static str {
    SESSION_ID.get_or_init(|| format!("{}-{}", compact_time(now_secs()), std::process::id()))
}

/// Before setup the logs folder isn't known yet: reports go to the temp folder
/// and are moved in by [`init`].
fn early_dir() -> PathBuf {
    std::env::temp_dir().join("com.kevalthumar.querybook-crashes")
}

pub fn crashes_dir() -> PathBuf {
    CTX.get().map_or_else(early_dir, |c| c.logs.join("crashes"))
}

pub fn logs_dir() -> Option<PathBuf> {
    CTX.get().map(|c| c.logs.clone())
}

pub fn version() -> String {
    CTX.get().map_or_else(|| env!("CARGO_PKG_VERSION").to_string(), |c| c.version.clone())
}

pub fn webview() -> Option<String> {
    CTX.get().and_then(|c| c.webview.clone())
}

pub fn os_name() -> String {
    let info = os_info::get();
    format!("{} {}", info.os_type(), info.version())
}

pub fn arch() -> &'static str {
    std::env::consts::ARCH
}

/// First thing in `run()`: record panics from here on.
pub fn install_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        // Never panic in here: a second panic aborts without a report.
        let _ = write_panic(info);
        previous(info);
    }));
}

fn write_panic(info: &PanicHookInfo<'_>) -> std::io::Result<PathBuf> {
    let home = home_dir();
    let payload = info
        .payload()
        .downcast_ref::<&str>()
        .map(|s| s.to_string())
        .or_else(|| info.payload().downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "panic".into());
    let backtrace = std::backtrace::Backtrace::force_capture().to_string();
    let report = new_report(
        ReportKind::BackendPanic,
        redact(&payload, &home),
        info.location().map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column())),
        backtrace_lines(&backtrace, &home),
        std::thread::current().name().map(str::to_string),
        None,
    );
    write_report(&crashes_dir(), &report)
}

/// Frames worth showing: drop the panic machinery at the top, cap the length.
pub fn backtrace_lines(bt: &str, home: &str) -> Vec<String> {
    let lines: Vec<&str> = bt.lines().map(str::trim_end).filter(|l| !l.trim().is_empty()).collect();
    let start = lines
        .iter()
        .position(|l| l.contains("rust_begin_unwind") || l.contains("begin_panic"))
        .map_or(0, |i| (i + 1).min(lines.len()));
    lines[start..].iter().take(80).map(|l| redact_home(l, home)).collect()
}

pub fn new_report(
    kind: ReportKind,
    message: String,
    location: Option<String>,
    backtrace: Vec<String>,
    thread: Option<String>,
    route: Option<String>,
) -> CrashReport {
    let now = now_secs();
    let prefix = match kind {
        ReportKind::BackendPanic => "crash",
        ReportKind::FrontendPanic | ReportKind::FrontendError => "window",
        ReportKind::BlankWindow => "blank",
        ReportKind::UncleanExit => "exit",
    };
    // Two reports in the same second (a burst of window errors) still get their own files.
    static SEQ: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    CrashReport {
        id: format!("{prefix}-{}-{}-{seq}", compact_time(now), std::process::id()),
        kind,
        time: rfc3339(now),
        version: version(),
        os: os_name(),
        arch: arch().into(),
        webview: webview(),
        thread,
        message,
        location,
        backtrace,
        route,
        session_id: session_id().into(),
        log_tail: applog::tail(),
    }
}

/// Write a report as `<dir>/<id>.json` (via a temp file, so a crash mid-write leaves no half file).
pub fn write_report(dir: &Path, report: &CrashReport) -> std::io::Result<PathBuf> {
    fs::create_dir_all(dir)?;
    let file = dir.join(format!("{}.json", report.id));
    let tmp = dir.join(format!("{}.tmp", report.id));
    fs::write(&tmp, serde_json::to_vec_pretty(report).map_err(std::io::Error::other)?)?;
    fs::rename(&tmp, &file)?;
    Ok(file)
}

/// In setup: the real folders, the version, the session lock, and last run's leftovers.
pub fn init(logs: PathBuf, version: String) {
    let webview = tauri::webview_version().ok();
    let _ = CTX.set(Ctx { logs: logs.clone(), version, webview });
    applog::init(&logs);
    let crashes = logs.join("crashes");
    // Reports written before setup (or by a run that crashed early).
    if let Ok(entries) = fs::read_dir(early_dir()) {
        let _ = fs::create_dir_all(&crashes);
        for e in entries.flatten() {
            let _ = fs::rename(e.path(), crashes.join(e.file_name()));
        }
    }
    let sessions = logs.join("sessions");
    for stale in stale_sessions(&sessions) {
        if !reports(&crashes).iter().any(|r| r.session_id == stale) {
            let mut r = new_report(
                ReportKind::UncleanExit,
                "Querybook ended without closing normally: it may have crashed in native code, been force-quit, or the computer lost power."
                    .into(),
                None,
                Vec::new(),
                None,
                None,
            );
            r.session_id = stale.clone();
            r.log_tail.clear();
            let _ = write_report(&crashes, &r);
        }
    }
    hold_session_lock(&sessions);
    prune(&crashes);
    applog::info(&format!("start {} on {} ({})", self::version(), os_name(), arch()));
}

/// Lock files no running Querybook holds: their runs ended without closing normally.
/// They are removed as they are found.
fn stale_sessions(dir: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(dir) else { return Vec::new() };
    let mut stale = Vec::new();
    for e in entries.flatten() {
        let path = e.path();
        if path.extension().is_none_or(|x| x != "lock") {
            continue;
        }
        let Ok(file) = File::options().read(true).write(true).open(&path) else { continue };
        if file.try_lock().is_ok() {
            let _ = file.unlock();
            drop(file);
            let _ = fs::remove_file(&path);
            if let Some(stem) = path.file_stem() {
                stale.push(stem.to_string_lossy().into_owned());
            }
        }
    }
    stale
}

fn hold_session_lock(dir: &Path) {
    let _ = fs::create_dir_all(dir);
    let path = dir.join(format!("{}.lock", session_id()));
    if let Ok(file) = File::options().create(true).truncate(true).read(true).write(true).open(&path)
        && file.try_lock().is_ok()
    {
        let _ = SESSION_LOCK.set((file, path));
    }
}

/// Normal exit: this run closed cleanly, so its lock file goes.
pub fn end_session() {
    applog::info("exit");
    if let Some((file, path)) = SESSION_LOCK.get() {
        let _ = file.unlock();
        let _ = fs::remove_file(path);
    }
}

/// Every readable report in `dir`, newest first.
pub fn reports(dir: &Path) -> Vec<CrashReport> {
    let Ok(entries) = fs::read_dir(dir) else { return Vec::new() };
    let mut out: Vec<CrashReport> = entries
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
        .filter_map(|e| serde_json::from_slice(&fs::read(e.path()).ok()?).ok())
        .collect();
    out.sort_by(|a, b| b.time.cmp(&a.time).then_with(|| b.id.cmp(&a.id)));
    out
}

pub fn report_path(id: &str) -> Option<PathBuf> {
    valid_report_id(id).then(|| crashes_dir().join(format!("{id}.json")))
}

fn prune(dir: &Path) {
    for old in reports(dir).into_iter().skip(KEEP) {
        let _ = fs::remove_file(dir.join(format!("{}.json", old.id)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("qb-crash-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn writes_and_lists_reports_newest_first() {
        let dir = tmp("list");
        let mut a = new_report(ReportKind::BackendPanic, "first".into(), None, vec![], None, None);
        a.time = "2026-01-01T00:00:00Z".into();
        let b = new_report(ReportKind::FrontendPanic, "second".into(), None, vec![], None, Some("/".into()));
        write_report(&dir, &a).unwrap();
        write_report(&dir, &b).unwrap();
        let all = reports(&dir);
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].message, "second");
        assert!(!dir.join(format!("{}.tmp", a.id)).exists());
    }

    #[test]
    fn a_held_lock_is_not_stale_but_a_released_one_is() {
        let dir = tmp("locks");
        fs::create_dir_all(&dir).unwrap();
        let held = File::options().create(true).truncate(true).read(true).write(true).open(dir.join("live.lock")).unwrap();
        held.try_lock().unwrap();
        fs::write(dir.join("dead.lock"), b"").unwrap();
        let stale = stale_sessions(&dir);
        assert_eq!(stale, vec!["dead".to_string()]);
        assert!(dir.join("live.lock").exists() && !dir.join("dead.lock").exists());
        drop(held);
    }

    #[test]
    fn backtrace_skips_the_panic_machinery() {
        let bt = "   0: std::backtrace::capture\n   1: rust_begin_unwind\n   2: querybook_lib::db::open\n             at C:\\Users\\Ann\\src\\db.rs:10\n";
        let lines = backtrace_lines(bt, r"C:\Users\Ann");
        assert_eq!(lines[0].trim(), "2: querybook_lib::db::open");
        assert!(lines[1].contains(r"~\src\db.rs"));
    }

    #[test]
    fn report_paths_are_confined() {
        assert!(report_path("../x").is_none());
        assert!(report_path("crash-1-2-0").unwrap().ends_with("crash-1-2-0.json"));
    }
}
