//! Crash reports, Help links and updates: the types both sides share, and the
//! pure helpers that decide what a report may contain.
//!
//! Privacy rule: a report never carries SQL, cell values, keys or the paths of
//! the user's files. [`redact`] runs over every message before it is written,
//! and the user sees the whole report before anything leaves the computer.

use serde::{Deserialize, Serialize};

use crate::encode::{fnv1a64, percent_encode};

/// GitHub repository the Help links point at.
pub const REPO_URL: &str = "https://github.com/KevalKThumar/Querybook";
/// Address for Contact Support. `None` until the owner provides one: the menu then offers GitHub only.
pub const SUPPORT_EMAIL: Option<&str> = None;
/// Prefilled links stay under this length (Windows' ShellExecute and some
/// browsers cut longer URLs).
pub const MAX_LINK_LEN: usize = 1800;

/// What produced a report.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReportKind {
    /// A panic in the native backend (the app quit).
    BackendPanic,
    /// A panic in the window's WebAssembly (the window stopped working).
    FrontendPanic,
    /// An uncaught script error or rejected promise in a window.
    FrontendError,
    /// A window that never finished loading.
    BlankWindow,
    /// Querybook ended without closing normally (crash in native code, killed, power loss).
    UncleanExit,
}

impl ReportKind {
    pub fn label(self) -> &'static str {
        match self {
            ReportKind::BackendPanic => "Querybook quit unexpectedly",
            ReportKind::FrontendPanic => "A window stopped working",
            ReportKind::FrontendError => "A window reported an error",
            ReportKind::BlankWindow => "A window didn't finish loading",
            ReportKind::UncleanExit => "Querybook didn't close normally",
        }
    }

    /// Worth asking about at the next launch. Unclean exits are only listed:
    /// a force quit or a power cut looks the same and would cry wolf.
    pub fn prompts(self) -> bool {
        !matches!(self, ReportKind::UncleanExit | ReportKind::FrontendError)
    }
}

/// One crash report, as stored in `crashes/<id>.json` and shown in the viewer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CrashReport {
    /// File stem, e.g. `crash-20261128T101500Z-1234`.
    pub id: String,
    pub kind: ReportKind,
    /// RFC 3339, UTC.
    pub time: String,
    pub version: String,
    pub os: String,
    pub arch: String,
    #[serde(default)]
    pub webview: Option<String>,
    #[serde(default)]
    pub thread: Option<String>,
    pub message: String,
    #[serde(default)]
    pub location: Option<String>,
    #[serde(default)]
    pub backtrace: Vec<String>,
    /// The page the window was on (`/database/browser`), for frontend reports.
    #[serde(default)]
    pub route: Option<String>,
    /// Groups a run's reports, and links an unclean exit to its panic.
    #[serde(default)]
    pub session_id: String,
    /// The last lines of Querybook's own log (already free of user data).
    #[serde(default)]
    pub log_tail: Vec<String>,
}

/// A row of the Crash Reports list.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CrashSummary {
    pub id: String,
    pub kind: ReportKind,
    pub time: String,
    pub version: String,
    pub title: String,
}

impl CrashReport {
    pub fn summary(&self) -> CrashSummary {
        CrashSummary {
            id: self.id.clone(),
            kind: self.kind,
            time: self.time.clone(),
            version: self.version.clone(),
            title: first_line(&self.message, 120),
        }
    }
}

/// What a window sends when its code fails (from the panic hook or the
/// `error` / `unhandledrejection` listeners).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FrontendError {
    pub kind: ReportKind,
    pub message: String,
    #[serde(default)]
    pub stack: String,
    #[serde(default)]
    pub location: Option<String>,
    #[serde(default)]
    pub route: String,
    /// The window can no longer work (a panic): offer to reload it.
    #[serde(default)]
    pub fatal: bool,
}

/// About / environment facts, also used to prefill a problem report.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AppInfo {
    pub version: String,
    pub os: String,
    pub arch: String,
    pub webview: String,
    /// The logs folder, with the home folder shown as `~`.
    pub logs_dir: String,
}

/// The only links the UI can ask the backend to open.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum SupportLink {
    /// A new GitHub issue, prefilled from a report when one is given.
    NewIssue { report_id: Option<String> },
    /// An email to [`SUPPORT_EMAIL`], prefilled the same way.
    Email { report_id: Option<String> },
    /// The releases page.
    Releases,
    /// The project page.
    Home,
}

// ── Updates ──

/// A newer version found by the update check.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UpdateInfo {
    pub version: String,
    pub current_version: String,
    /// Release notes (Markdown text, shown as plain text).
    pub notes: String,
    pub date: Option<String>,
    /// This install can replace itself; otherwise the user updates through their package manager.
    pub can_install: bool,
}

/// Everything the update UI shows, sent on request and with every change.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct UpdateStatus {
    /// This build has the updater (a release built with the signing key).
    pub enabled: bool,
    /// None until the user answers "Check for updates automatically?".
    pub auto_check: Option<bool>,
    pub current_version: String,
    /// "installer", "app", "AppImage", "deb", "rpm", "development build".
    pub install_kind: String,
    pub can_install: bool,
    /// An update to offer now (not skipped, not put off until the next launch).
    pub available: Option<UpdateInfo>,
}

/// Progress of Update Now.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum UpdateProgress {
    Downloading { downloaded: u64, total: Option<u64> },
    Installing,
    Failed,
}

/// A report id names a file in the crashes folder: letters, digits and dashes only.
pub fn valid_report_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 80 && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

fn first_line(s: &str, max: usize) -> String {
    let line = s.lines().find(|l| !l.trim().is_empty()).unwrap_or("").trim();
    if line.chars().count() <= max {
        line.to_string()
    } else {
        format!("{}…", line.chars().take(max - 1).collect::<String>())
    }
}

// ── Redaction ──

/// Replace the home folder with `~` (any slash style, any case on Windows).
pub fn redact_home(text: &str, home: &str) -> String {
    let home = home.trim_end_matches(['/', '\\']);
    if home.len() < 2 {
        return text.to_string();
    }
    let mut out = text.to_string();
    for variant in [home.to_string(), home.replace('\\', "/"), home.replace('/', "\\")] {
        out = replace_ignore_ascii_case(&out, &variant, "~");
    }
    out
}

fn replace_ignore_ascii_case(text: &str, needle: &str, with: &str) -> String {
    if needle.is_empty() {
        return text.to_string();
    }
    let (lower, needle_lower) = (text.to_ascii_lowercase(), needle.to_ascii_lowercase());
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    for (i, _) in lower.match_indices(&needle_lower) {
        if i < last {
            continue;
        }
        out.push_str(&text[last..i]);
        out.push_str(with);
        last = i + needle.len();
    }
    out.push_str(&text[last..]);
    out
}

/// Strip user data from a message or log line:
/// - the home folder becomes `~`;
/// - quoted text (`'…'`, `"…"`) becomes `'…'`: SQL errors quote names and values;
/// - absolute paths become `<file:1a2b3c4d>.db`, keeping only the extension;
/// - runs of 32+ hex digits (a raw SQLCipher key, a blob) become `<hex>`.
pub fn redact(text: &str, home: &str) -> String {
    let text = redact_home(text, home);
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        // Quoted text.
        if (c == '\'' || c == '"')
            && let Some(end) = chars[i + 1..].iter().position(|&d| d == c)
        {
            out.push(c);
            out.push('…');
            out.push(c);
            i += end + 2;
            continue;
        }
        // An absolute path: C:\… C:/… \\server\… /a/b… ~/…
        let starts_path = at_word_start(&chars, i)
            && ((c.is_ascii_alphabetic()
                && chars.get(i + 1) == Some(&':')
                && matches!(chars.get(i + 2), Some('\\' | '/')))
                || (c == '\\' && chars.get(i + 1) == Some(&'\\'))
                || (c == '~' && matches!(chars.get(i + 1), Some('/' | '\\')))
                || (c == '/' && is_unix_path(&chars[i..])));
        if starts_path {
            let end = (i..chars.len()).find(|&j| ends_path(chars[j])).unwrap_or(chars.len());
            let path: String = chars[i..end].iter().collect();
            out.push_str(&hide_path(&path));
            i = end;
            continue;
        }
        // Long hex runs.
        if c.is_ascii_hexdigit() && at_word_start(&chars, i) {
            let end = (i..chars.len()).find(|&j| !chars[j].is_ascii_hexdigit()).unwrap_or(chars.len());
            if end - i >= 32 {
                out.push_str("<hex>");
                i = end;
                continue;
            }
        }
        out.push(c);
        i += 1;
    }
    out
}

fn at_word_start(chars: &[char], i: usize) -> bool {
    i == 0 || !(chars[i - 1].is_alphanumeric() || chars[i - 1] == '_')
}

/// `/a/b`: at least two segments, so `a/b` ratios and `/` alone stay.
fn is_unix_path(rest: &[char]) -> bool {
    let end = rest.iter().position(|&c| ends_path(c)).unwrap_or(rest.len());
    let seg = &rest[..end];
    seg.iter().filter(|&&c| c == '/').count() >= 2 && seg.get(1).is_some_and(|c| !c.is_whitespace() && *c != '/')
}

fn ends_path(c: char) -> bool {
    c.is_whitespace() || matches!(c, '\'' | '"' | ')' | '(' | ',' | ';' | '<' | '>' | '|' | '`')
}

/// `C:\Users\me\shop.db` → `<file:1a2b3c4d>.db`. The hash tells two files apart without naming them.
fn hide_path(path: &str) -> String {
    let trimmed = path.trim_end_matches(['.', ':']);
    let tail = &path[trimmed.len()..];
    let name = trimmed.rsplit(['/', '\\']).next().unwrap_or("");
    let ext = name
        .rsplit_once('.')
        .map(|(_, e)| e)
        .filter(|e| !e.is_empty() && e.len() <= 8 && e.chars().all(|c| c.is_ascii_alphanumeric()));
    let hash = fnv1a64(trimmed) as u32;
    match ext {
        Some(e) => format!("<file:{hash:08x}>.{e}{tail}"),
        None => format!("<file:{hash:08x}>{tail}"),
    }
}

// ── Time ──

/// Unix seconds → `2026-11-28T10:15:00Z`.
pub fn rfc3339(secs: u64) -> String {
    let (days, rem) = ((secs / 86_400) as i64, secs % 86_400);
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z", rem / 3600, rem % 3600 / 60, rem % 60)
}

/// Unix seconds → `20261128T101500Z`, for file names.
pub fn compact_time(secs: u64) -> String {
    rfc3339(secs).replace(['-', ':'], "")
}

/// Days since 1970-01-01 → (year, month, day), proleptic Gregorian.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

// ── Sharing ──

/// The whole report as plain text, for Copy and for the email body.
pub fn report_text(r: &CrashReport) -> String {
    let mut s = format!(
        "{}\n\nKind: {:?}\nTime: {}\nQuerybook: {}\nSystem: {} ({})\n",
        r.kind.label(),
        r.kind,
        r.time,
        r.version,
        r.os,
        r.arch
    );
    if let Some(w) = &r.webview {
        s.push_str(&format!("WebView: {w}\n"));
    }
    if let Some(t) = &r.thread {
        s.push_str(&format!("Thread: {t}\n"));
    }
    if let Some(route) = &r.route {
        s.push_str(&format!("Page: {route}\n"));
    }
    s.push_str(&format!("\nMessage:\n{}\n", r.message));
    if let Some(l) = &r.location {
        s.push_str(&format!("\nAt: {l}\n"));
    }
    if !r.backtrace.is_empty() {
        s.push_str("\nBacktrace:\n");
        for f in &r.backtrace {
            s.push_str(&format!("  {f}\n"));
        }
    }
    if !r.log_tail.is_empty() {
        s.push_str("\nRecent log:\n");
        for l in &r.log_tail {
            s.push_str(&format!("  {l}\n"));
        }
    }
    s
}

/// Title and body for a problem report: the environment, plus the report's
/// headline and top frames when there is one. The full report is attached by the user.
fn problem_text(report: Option<&CrashReport>, info: &AppInfo, frames: usize) -> (String, String) {
    let env = format!("- Querybook: {}\n- System: {} ({})\n- WebView: {}\n", info.version, info.os, info.arch, info.webview);
    match report {
        None => (
            String::new(),
            format!("**What happened**\n\n\n**Steps to reproduce**\n1.\n\n**Environment**\n{env}"),
        ),
        Some(r) => {
            let mut body = format!(
                "**What I was doing**\n\n\n**Report**\n- {}\n- {}\n{env}\n```\n{}\n",
                r.kind.label(),
                r.time,
                first_line(&r.message, 300)
            );
            if let Some(l) = &r.location {
                body.push_str(&format!("at {l}\n"));
            }
            for f in r.backtrace.iter().take(frames) {
                body.push_str(&format!("  {f}\n"));
            }
            body.push_str("```\n\nPlease attach the saved report: Help › Crash Reports › Save….\n");
            (format!("{}: {}", r.kind.label(), first_line(&r.message, 80)), body)
        }
    }
}

/// A prefilled "new issue" link, at most [`MAX_LINK_LEN`] long: frames are dropped first.
pub fn issue_url(report: Option<&CrashReport>, info: &AppInfo) -> String {
    let label = if report.is_some() { "crash" } else { "bug" };
    fit_link(|frames| {
        let (title, body) = problem_text(report, info, frames);
        format!(
            "{REPO_URL}/issues/new?labels={label}&title={}&body={}",
            percent_encode(&title),
            percent_encode(&body)
        )
    })
}

/// A prefilled email to `to`, at most [`MAX_LINK_LEN`] long.
pub fn mailto_url(to: &str, report: Option<&CrashReport>, info: &AppInfo) -> String {
    fit_link(|frames| {
        let (title, body) = problem_text(report, info, frames);
        let subject = if title.is_empty() { "Querybook support".to_string() } else { format!("Querybook: {title}") };
        format!("mailto:{to}?subject={}&body={}", percent_encode(&subject), percent_encode(&body))
    })
}

fn fit_link(build: impl Fn(usize) -> String) -> String {
    for frames in [8, 4, 2, 0] {
        let url = build(frames);
        if url.len() <= MAX_LINK_LEN {
            return url;
        }
    }
    // Still too long (a huge message): cut at a character boundary that isn't inside a %XX escape.
    let url = build(0);
    let mut cut = MAX_LINK_LEN;
    while cut > 0 && (!url.is_char_boundary(cut) || url[..cut].ends_with('%') || url[..cut.saturating_sub(1)].ends_with('%')) {
        cut -= 1;
    }
    url[..cut].to_string()
}

// ── Versions ──

/// `1.2.0`, `v1.10.3`, `1.2.0-beta.1` → comparable parts. Unparseable parts count as 0.
fn version_key(v: &str) -> ([u64; 3], bool) {
    let v = v.trim().trim_start_matches(['v', 'V']);
    let (core, pre) = match v.split_once(['-', '+']) {
        Some((c, rest)) => (c, !rest.is_empty() && v.contains('-')),
        None => (v, false),
    };
    let mut parts = [0u64; 3];
    for (slot, p) in parts.iter_mut().zip(core.split('.')) {
        *slot = p.parse().unwrap_or(0);
    }
    // A release sorts after its pre-releases.
    (parts, !pre)
}

/// `candidate` is a newer version than `current`.
pub fn is_newer(candidate: &str, current: &str) -> bool {
    version_key(candidate) > version_key(current)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report() -> CrashReport {
        CrashReport {
            id: "crash-20261128T101500Z-42".into(),
            kind: ReportKind::BackendPanic,
            time: "2026-11-28T10:15:00Z".into(),
            version: "1.2.0".into(),
            os: "Windows 11".into(),
            arch: "x86_64".into(),
            webview: Some("131.0".into()),
            thread: Some("main".into()),
            message: "index out of bounds".into(),
            location: Some("src/db.rs:10:5".into()),
            backtrace: (0..30).map(|i| format!("{i}: querybook_lib::db::frame_{i}")).collect(),
            route: None,
            session_id: "s1".into(),
            log_tail: vec!["connect ok".into()],
        }
    }

    #[test]
    fn home_becomes_tilde() {
        assert_eq!(redact_home(r"C:\Users\Ann\x.db and c:/users/ann/y", r"C:\Users\Ann"), r"~\x.db and ~/y");
        assert_eq!(redact_home("/home/ann/a.db", "/home/ann/"), "~/a.db");
        assert_eq!(redact_home("nothing here", ""), "nothing here");
    }

    #[test]
    fn redaction_hides_paths_quotes_and_keys() {
        let home = r"C:\Users\Ann";
        let out = redact(r#"can't open C:\Users\Ann\Shop\orders.sqlite: no such table "customers""#, home);
        assert!(!out.contains("Ann") && !out.contains("orders") && !out.contains("customers"), "{out}");
        assert!(out.contains(">.sqlite:"), "{out}");
        assert!(out.contains("\"…\""), "{out}");

        let out = redact("open /home/bob/data/x.db failed", "/home/bob");
        assert!(out.starts_with("open <file:") && out.ends_with(">.db failed"), "{out}");

        let out = redact("key 2DD29CA851E7B56E4697B0E1F08507293D761A05CE4D1B628663F411A8086D99 rejected", "");
        assert_eq!(out, "key <hex> rejected");

        let out = redact(r"UNC \\server\share\db.sqlite3 and SQL 'DELETE FROM t'", "");
        assert!(!out.contains("server") && !out.contains("DELETE"), "{out}");

        // ordinary text survives
        assert_eq!(redact("index 3 out of range for slice of length 2 at src/db.rs:10", ""), "index 3 out of range for slice of length 2 at src/db.rs:10");
        assert_eq!(redact("ratio a/b and 1/2", ""), "ratio a/b and 1/2");
    }

    #[test]
    fn report_ids() {
        assert!(valid_report_id("crash-20261128T101500Z-42"));
        assert!(!valid_report_id("../secrets"));
        assert!(!valid_report_id("a/b"));
        assert!(!valid_report_id(""));
    }

    #[test]
    fn times() {
        assert_eq!(rfc3339(0), "1970-01-01T00:00:00Z");
        assert_eq!(rfc3339(1_795_860_900), "2026-11-28T10:15:00Z");
        assert_eq!(rfc3339(951_782_400), "2000-02-29T00:00:00Z");
        assert_eq!(compact_time(1_795_860_900), "20261128T101500Z");
    }

    #[test]
    fn links_are_capped_and_encoded() {
        let info = AppInfo { version: "1.2.0".into(), os: "Windows 11".into(), arch: "x86_64".into(), webview: "131".into(), logs_dir: "~".into() };
        let url = issue_url(Some(&report()), &info);
        assert!(url.len() <= MAX_LINK_LEN);
        assert!(url.starts_with("https://github.com/KevalKThumar/Querybook/issues/new?labels=crash&title="));
        assert!(!url.contains(' ') && !url.contains('\n'));
        assert!(url.contains("frame_0"));

        let mut huge = report();
        huge.message = "x".repeat(5000);
        assert!(issue_url(Some(&huge), &info).len() <= MAX_LINK_LEN);

        let plain = issue_url(None, &info);
        assert!(plain.contains("labels=bug") && plain.contains("Querybook%3A%201.2.0"));

        let mail = mailto_url("help@example.com", Some(&report()), &info);
        assert!(mail.starts_with("mailto:help@example.com?subject=Querybook%3A%20") && mail.len() <= MAX_LINK_LEN);
    }

    #[test]
    fn report_text_has_everything() {
        let t = report_text(&report());
        assert!(t.contains("Querybook quit unexpectedly") && t.contains("Querybook: 1.2.0") && t.contains("frame_29") && t.contains("connect ok"));
    }

    #[test]
    fn version_order() {
        assert!(is_newer("1.10.0", "1.9.9"));
        assert!(is_newer("v1.2.1", "1.2.0"));
        assert!(!is_newer("1.2.0", "1.2.0"));
        assert!(!is_newer("1.1.9", "1.2.0"));
        assert!(is_newer("1.2.0", "1.2.0-beta.1"));
        assert!(!is_newer("1.2.0-beta.2", "1.2.0"));
    }
}
