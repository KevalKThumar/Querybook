//! Help & support commands: About facts, the Crash Reports viewer, and the few
//! links Querybook opens in the browser or mail app.
//!
//! Links are built here from a fixed list ([`SupportLink`]); the page can't ask
//! the backend to open any other address.

use std::collections::HashMap;
use std::fs;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

use shared::support::{
    AppInfo, CrashReport, CrashSummary, FrontendError, REPO_URL, ReportKind, SUPPORT_EMAIL, SupportLink, issue_url,
    mailto_url, redact, redact_home,
};
use shared::AppError;
use tauri::{AppHandle, WebviewWindow};
use tauri_plugin_opener::OpenerExt;

use crate::applog::{self, home_dir, now_secs};
use crate::db::{invalid, io_err};
use crate::{Res, crash, save_path, watchdog};

pub fn app_info_now() -> AppInfo {
    AppInfo {
        version: crash::version(),
        os: crash::os_name(),
        arch: crash::arch().into(),
        webview: crash::webview().unwrap_or_else(|| "unknown".into()),
        logs_dir: crash::logs_dir().map(|d| redact_home(&d.display().to_string(), &home_dir())).unwrap_or_default(),
    }
}

fn load(id: &str) -> Res<CrashReport> {
    let path = crash::report_path(id).ok_or_else(|| invalid("Unknown report."))?;
    let bytes = fs::read(path).map_err(|_| invalid("That report no longer exists."))?;
    serde_json::from_slice(&bytes).map_err(|_| invalid("That report can't be read."))
}

pub fn open_link(app: &AppHandle, link: &SupportLink) -> Res<()> {
    let info = app_info_now();
    let report = |id: &Option<String>| id.as_deref().and_then(|id| load(id).ok());
    let url = match link {
        SupportLink::NewIssue { report_id } => issue_url(report(report_id).as_ref(), &info),
        SupportLink::Email { report_id } => {
            let to = SUPPORT_EMAIL.ok_or_else(|| invalid("No support email address is set up yet."))?;
            mailto_url(to, report(report_id).as_ref(), &info)
        }
        SupportLink::Releases => format!("{REPO_URL}/releases"),
        SupportLink::Home => REPO_URL.to_string(),
    };
    app.opener().open_url(url, None::<&str>).map_err(|e| invalid(e.to_string()))
}

#[tauri::command]
pub async fn app_info() -> Res<AppInfo> {
    Ok(app_info_now())
}

/// The page has drawn (see the watchdog).
#[tauri::command]
pub async fn app_ready(window: WebviewWindow) -> Res<()> {
    watchdog::ready(window.label());
    Ok(())
}

/// A window's code failed. Recorded as a report; a fatal one also gets the recovery dialog.
#[tauri::command]
pub async fn report_frontend_error(app: AppHandle, window: WebviewWindow, report: FrontendError) -> Res<()> {
    // A page stuck in an error loop writes at most a few reports a minute.
    static RECENT: OnceLock<Mutex<HashMap<String, Vec<u64>>>> = OnceLock::new();
    let now = now_secs();
    {
        let mut recent = RECENT.get_or_init(Default::default).lock().map_err(|_| invalid("busy"))?;
        let times = recent.entry(window.label().to_string()).or_default();
        times.retain(|t| now - t < 60);
        if times.len() >= 5 && !report.fatal {
            return Ok(());
        }
        times.push(now);
    }
    let home = home_dir();
    let kind = if report.kind == ReportKind::FrontendPanic { ReportKind::FrontendPanic } else { ReportKind::FrontendError };
    applog::warn(&format!("window error ({kind:?})"));
    let stack: Vec<String> = report.stack.lines().take(60).map(|l| redact(l.trim(), &home)).filter(|l| !l.is_empty()).collect();
    let r = crash::new_report(
        kind,
        redact(&report.message, &home),
        report.location.map(|l| redact(&l, &home)),
        stack,
        None,
        Some(report.route.chars().take(200).collect()),
    );
    let id = r.id.clone();
    crash::write_report(&crash::crashes_dir(), &r).map_err(io_err)?;
    if report.fatal {
        watchdog::recovery_dialog(
            &app,
            &window,
            "This window stopped working",
            "Querybook ran into a problem in this window and it can't respond any more.\n\nIf you had changes waiting to be written, they are still pending: Reload Window keeps them, and Close Window asks whether to save them.",
            id,
        );
    }
    Ok(())
}

#[tauri::command]
pub async fn list_crash_reports() -> Res<Vec<CrashSummary>> {
    Ok(crash::reports(&crash::crashes_dir()).iter().map(CrashReport::summary).collect())
}

#[tauri::command]
pub async fn crash_report(id: String) -> Res<CrashReport> {
    load(&id)
}

/// `None` deletes them all.
#[tauri::command]
pub async fn delete_crash_reports(ids: Option<Vec<String>>) -> Res<()> {
    let ids = ids.unwrap_or_else(|| crash::reports(&crash::crashes_dir()).into_iter().map(|r| r.id).collect());
    for id in ids {
        if let Some(path) = crash::report_path(&id) {
            let _ = fs::remove_file(path);
        }
    }
    Ok(())
}

/// The report to ask about at launch ("Querybook quit unexpectedly"): the newest
/// one from an earlier run that hasn't been shown yet. Answered once per launch.
#[tauri::command]
pub async fn pending_crash_prompt() -> Res<Option<CrashSummary>> {
    static ASKED: AtomicBool = AtomicBool::new(false);
    if ASKED.swap(true, Ordering::SeqCst) {
        return Ok(None);
    }
    let dir = crash::crashes_dir();
    let seen_file = dir.join("seen.json");
    let mut seen: Vec<String> = fs::read(&seen_file).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
    let all = crash::reports(&dir);
    let fresh: Vec<&CrashReport> =
        all.iter().filter(|r| r.session_id != crash::session_id() && !seen.contains(&r.id)).collect();
    let pick = fresh.iter().find(|r| r.kind.prompts()).map(|r| r.summary());
    if !fresh.is_empty() {
        seen.extend(fresh.iter().map(|r| r.id.clone()));
        seen.retain(|id| all.iter().any(|r| &r.id == id));
        let _ = fs::write(&seen_file, serde_json::to_vec(&seen).unwrap_or_default());
    }
    Ok(pick)
}

/// Save a report where the user chooses, to attach to an issue or email.
#[tauri::command]
pub async fn save_crash_report(app: AppHandle, window: WebviewWindow, id: String) -> Res<Option<String>> {
    let report = load(&id)?;
    let name = format!("querybook-report-{}.json", report.time.get(..10).unwrap_or("report"));
    let Some(out) = save_path(&app, &window, &name, ("Crash report", &["json"]))? else { return Ok(None) };
    fs::write(&out, serde_json::to_vec_pretty(&report).map_err(|e| invalid(e.to_string()))?).map_err(io_err)?;
    Ok(Some(out.display().to_string()))
}

#[tauri::command]
pub async fn open_support_link(app: AppHandle, link: SupportLink) -> Res<()> {
    open_link(&app, &link)
}

#[tauri::command]
pub async fn reveal_crash_report(app: AppHandle, id: String) -> Res<()> {
    let path = crash::report_path(&id).filter(|p| p.exists()).ok_or_else(|| invalid("That report no longer exists."))?;
    app.opener().reveal_item_in_dir(path).map_err(|e| invalid(e.to_string()))
}

#[tauri::command]
pub async fn open_logs_folder(app: AppHandle) -> Res<()> {
    let dir = crash::logs_dir().ok_or_else(|| AppError::new(shared::ErrorKind::Io, "The logs folder isn't available."))?;
    let _ = fs::create_dir_all(&dir);
    app.opener().open_path(dir.display().to_string(), None::<&str>).map_err(|e| invalid(e.to_string()))
}
