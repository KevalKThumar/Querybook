//! Blank-window watchdog. Every page load arms a timer; the page calls
//! `app_ready` once it has drawn. A window still silent when the timer runs out
//! gets a report and a native dialog offering Reload / Report a Problem / Close,
//! since its own UI can't show anything.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use shared::support::{ReportKind, SupportLink};
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind, MessageDialogResult};

use crate::{applog, crash, support};

/// Release builds load in about a second; `trunk serve`'s first compile can take a minute.
const TIMEOUT: Duration = if cfg!(debug_assertions) { Duration::from_secs(90) } else { Duration::from_secs(20) };

#[derive(Default)]
struct Window {
    generation: u64,
    ready: bool,
}

fn windows() -> &'static Mutex<HashMap<String, Window>> {
    static W: OnceLock<Mutex<HashMap<String, Window>>> = OnceLock::new();
    W.get_or_init(Default::default)
}

/// E2E: `QUERYBOOK_E2E_NO_READY=1` ignores `app_ready`, to show the dialog on purpose.
fn ignore_ready() -> bool {
    std::env::var_os("QUERYBOOK_E2E_NO_READY").is_some()
}

/// A page started loading in `label`.
pub fn arm(app: &AppHandle, label: &str) {
    let generation = {
        let Ok(mut w) = windows().lock() else { return };
        let entry = w.entry(label.to_string()).or_default();
        entry.generation += 1;
        entry.ready = false;
        entry.generation
    };
    let (app, label) = (app.clone(), label.to_string());
    std::thread::spawn(move || {
        std::thread::sleep(TIMEOUT);
        let silent = windows()
            .lock()
            .ok()
            .and_then(|w| w.get(&label).map(|e| e.generation == generation && !e.ready))
            .unwrap_or(false);
        if silent && let Some(window) = app.get_webview_window(&label) {
            stalled(&app, &window);
        }
    });
}

/// The page in `label` has drawn.
pub fn ready(label: &str) {
    if ignore_ready() {
        return;
    }
    if let Ok(mut w) = windows().lock() {
        w.entry(label.to_string()).or_default().ready = true;
    }
}

pub fn forget(label: &str) {
    if let Ok(mut w) = windows().lock() {
        w.remove(label);
    }
}

fn stalled(app: &AppHandle, window: &tauri::WebviewWindow) {
    applog::warn("window didn't finish loading");
    let report = crash::new_report(
        ReportKind::BlankWindow,
        format!("The window didn't finish loading within {} seconds.", TIMEOUT.as_secs()),
        None,
        Vec::new(),
        None,
        window.url().ok().map(|u| u.path().to_string()),
    );
    let id = report.id.clone();
    let _ = crash::write_report(&crash::crashes_dir(), &report);
    recovery_dialog(
        app,
        window,
        "This window didn't finish loading",
        "Querybook couldn't show this window. Reloading it usually helps.\n\nIf you had changes waiting to be written, they are still pending: reloading keeps them.",
        id,
    );
}

/// Reload / Report a Problem… / Close, for a window whose own UI can't respond.
/// Reloading keeps the window's database session (and its pending changes);
/// Close goes through the usual Save / Don't Save prompt.
pub fn recovery_dialog(app: &AppHandle, window: &tauri::WebviewWindow, title: &str, message: &str, report_id: String) {
    let (app, w) = (app.clone(), window.clone());
    window
        .dialog()
        .message(message)
        .title(title)
        .kind(MessageDialogKind::Warning)
        .buttons(MessageDialogButtons::YesNoCancelCustom("Reload Window".into(), "Report a Problem…".into(), "Close Window".into()))
        .parent(window)
        .show_with_result(move |choice| match choice {
            MessageDialogResult::Custom(c) if c == "Reload Window" => {
                let _ = w.eval("window.location.replace('/')");
            }
            MessageDialogResult::Custom(c) if c == "Report a Problem…" => {
                let _ = support::open_link(&app, &SupportLink::NewIssue { report_id: Some(report_id) });
            }
            MessageDialogResult::Custom(c) if c == "Close Window" => {
                let _ = w.close();
            }
            _ => {}
        });
}
