//! Querybook's own log: a small text file in the logs folder that crash reports
//! quote from, so a report says what led up to a problem.
//!
//! Privacy rule: log only what happened, never the data. Event names, error
//! kinds, counts and durations are fine. SQL text, cell values, keys, error
//! messages from SQLite (they quote names and values) and file paths are not;
//! a file is named by [`file_tag`] instead. Every line also goes through
//! [`shared::support::redact`] as a safety net.
//!
//! Files: `querybook.log`, rolled over to `querybook.1.log` at 1 MB.

use std::collections::VecDeque;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use shared::encode::fnv1a64;
use shared::support::{redact, rfc3339};

const MAX_BYTES: u64 = 1_000_000;
/// Lines kept in memory for crash reports.
const TAIL: usize = 200;

struct Log {
    dir: Option<PathBuf>,
    file: Option<File>,
    size: u64,
    tail: VecDeque<String>,
    home: String,
}

static LOG: OnceLock<Mutex<Log>> = OnceLock::new();

fn log() -> &'static Mutex<Log> {
    LOG.get_or_init(|| {
        Mutex::new(Log { dir: None, file: None, size: 0, tail: VecDeque::with_capacity(TAIL), home: home_dir() })
    })
}

/// The user's home folder, for showing paths as `~`.
pub fn home_dir() -> String {
    std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")).unwrap_or_default()
}

pub fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// Start writing to `dir/querybook.log`. Lines logged before this are kept in memory only.
pub fn init(dir: &Path) {
    let Ok(mut l) = log().lock() else { return };
    if fs::create_dir_all(dir).is_err() {
        return;
    }
    let path = dir.join("querybook.log");
    l.size = fs::metadata(&path).map_or(0, |m| m.len());
    l.file = OpenOptions::new().create(true).append(true).open(&path).ok();
    l.dir = Some(dir.to_path_buf());
}

/// Log one event. Pass a fixed description plus non-sensitive facts.
pub fn info(event: &str) {
    write("INFO", event);
}

/// Log a problem. Pass an error *kind*, not its message.
pub fn warn(event: &str) {
    write("WARN", event);
}

fn write(level: &str, event: &str) {
    let Ok(mut l) = log().lock() else { return };
    let line = format!("{} {level} {}", rfc3339(now_secs()), redact(event, &l.home).replace(['\r', '\n'], " "));
    if l.tail.len() == TAIL {
        l.tail.pop_front();
    }
    l.tail.push_back(line.clone());
    if l.size + line.len() as u64 + 1 > MAX_BYTES {
        roll(&mut l);
    }
    if let Some(f) = l.file.as_mut()
        && writeln!(f, "{line}").is_ok()
    {
        l.size += line.len() as u64 + 1;
    }
}

fn roll(l: &mut Log) {
    let Some(dir) = l.dir.clone() else { return };
    l.file = None;
    let _ = fs::rename(dir.join("querybook.log"), dir.join("querybook.1.log"));
    l.file = OpenOptions::new().create(true).append(true).open(dir.join("querybook.log")).ok();
    l.size = 0;
}

/// The last lines logged, oldest first.
pub fn tail() -> Vec<String> {
    log().lock().map(|l| l.tail.iter().cloned().collect()).unwrap_or_default()
}

/// A stand-in for a file path in the log: `db:1a2b3c4d.sqlite`.
pub fn file_tag(path: &Path) -> String {
    let ext = path.extension().map(|e| e.to_string_lossy().into_owned()).unwrap_or_default();
    format!("file:{:08x}.{ext}", fnv1a64(&path.display().to_string()) as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_redacted_lines_and_keeps_a_tail() {
        let dir = std::env::temp_dir().join(format!("qb-applog-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        init(&dir);
        info("window opened");
        warn("open failed: no such table \"secret_table\" in /home/x/private/a.db");
        let text = fs::read_to_string(dir.join("querybook.log")).unwrap();
        assert!(text.contains("INFO window opened"));
        assert!(!text.contains("secret_table") && !text.contains("private"), "{text}");
        assert!(tail().iter().any(|l| l.contains("window opened")));
        assert!(file_tag(Path::new("/home/x/a.sqlite")).ends_with(".sqlite"));
    }
}
