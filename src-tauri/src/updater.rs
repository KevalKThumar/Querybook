//! In-app updates, run from Rust only (the page gets no updater permissions).
//!
//! - Asks once: until the user answers "Check for updates automatically?", nothing
//!   goes over the network. After a yes, it checks at launch and then every 24 hours;
//!   a manual Help › Check for Updates… always works.
//! - The check fetches `latest.json` from the GitHub release (the endpoint in
//!   tauri.conf.json); nothing about the user or their databases is sent.
//! - Installs only on installs that can replace themselves (Windows installer, the
//!   macOS app, a writable AppImage). deb/rpm installs are told about the update and
//!   pointed at their package manager or the download page.
//! - Never installs while a window has changes waiting to be written: the Windows
//!   installer closes the app, and Querybook never saves without asking.
//!
//! Prefs: `updater.json` in the app config folder.

use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use shared::support::{UpdateInfo, UpdateProgress, UpdateStatus, is_newer};
use shared::{AppError, ErrorKind};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::applog::{self, now_secs};
use crate::db::{Registry, invalid};
use crate::{Res, crash};

const DAY: u64 = 24 * 60 * 60;
/// Event every window listens to: the status changed (update found, dismissed, answered).
pub const STATUS_EVENT: &str = "update-status";
pub const PROGRESS_EVENT: &str = "update-progress";

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct Prefs {
    /// None = not asked yet.
    auto_check: Option<bool>,
    skipped_version: Option<String>,
    last_check: u64,
}

#[derive(Default)]
pub struct UpdaterState {
    /// The updater plugin is set up in this build (tauri.conf.json has a public key).
    enabled: AtomicBool,
    prefs: Mutex<Prefs>,
    /// The update the user was shown, kept so Install installs exactly that one.
    pending: Mutex<Option<Update>>,
    available: Mutex<Option<UpdateInfo>>,
    /// Hidden until next launch ("Later").
    later: AtomicBool,
    busy: AtomicBool,
}

fn prefs_path(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_config_dir().ok().map(|d| d.join("updater.json"))
}

fn save(app: &AppHandle, prefs: &Prefs) {
    if let Some(path) = prefs_path(app) {
        if let Some(dir) = path.parent() {
            let _ = fs::create_dir_all(dir);
        }
        let _ = fs::write(path, serde_json::to_vec_pretty(prefs).unwrap_or_default());
    }
}

/// How this copy was installed decides whether it can replace itself.
fn install_kind() -> (&'static str, bool) {
    use tauri::utils::config::BundleType;
    use tauri::utils::platform::bundle_type;
    match bundle_type() {
        Some(BundleType::Nsis) => ("installer", true),
        Some(BundleType::App) => ("app", true),
        Some(BundleType::AppImage) => {
            let writable = std::env::var_os("APPIMAGE")
                .and_then(|p| fs::metadata(p).ok())
                .is_some_and(|m| !m.permissions().readonly());
            ("AppImage", writable)
        }
        Some(BundleType::Deb) => ("deb", false),
        Some(BundleType::Rpm) => ("rpm", false),
        Some(BundleType::Msi) => ("msi", false),
        _ => ("development build", false),
    }
}

fn status(app: &AppHandle, s: &UpdaterState) -> UpdateStatus {
    let prefs = s.prefs.lock().map(|p| p.clone()).unwrap_or_default();
    let (kind, can_install) = install_kind();
    UpdateStatus {
        enabled: s.enabled.load(Ordering::Relaxed),
        auto_check: prefs.auto_check,
        current_version: app.package_info().version.to_string(),
        install_kind: kind.into(),
        can_install,
        available: if s.later.load(Ordering::Relaxed) { None } else { s.available.lock().ok().and_then(|a| a.clone()) },
    }
}

fn broadcast(app: &AppHandle) {
    let s = app.state::<UpdaterState>();
    let _ = app.emit(STATUS_EVENT, status(app, &s));
}

/// In setup: read the prefs and start the daily check.
pub fn init(app: &AppHandle, enabled: bool) {
    let s = app.state::<UpdaterState>();
    s.enabled.store(enabled, Ordering::Relaxed);
    if let Some(prefs) = prefs_path(app).and_then(|p| fs::read(p).ok()).and_then(|b| serde_json::from_slice(&b).ok())
        && let Ok(mut p) = s.prefs.lock()
    {
        *p = prefs;
    }
    if !enabled {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        // Give the first window time to start listening.
        std::thread::sleep(Duration::from_secs(8));
        loop {
            let due = {
                let s = app.state::<UpdaterState>();
                let p = s.prefs.lock().map(|p| p.clone()).unwrap_or_default();
                p.auto_check == Some(true) && now_secs().saturating_sub(p.last_check) >= DAY
            };
            if due {
                let _ = tauri::async_runtime::block_on(check(&app, false));
            }
            // Hourly, so a laptop that slept through the 24 hours checks soon after waking.
            std::thread::sleep(Duration::from_secs(60 * 60));
        }
    });
}

/// Ask GitHub for latest.json. `manual` shows skipped versions and reports errors.
async fn check(app: &AppHandle, manual: bool) -> Res<Option<UpdateInfo>> {
    let s = app.state::<UpdaterState>();
    if !s.enabled.load(Ordering::Relaxed) {
        return Err(invalid("This copy of Querybook wasn't built with updates. Download new versions from the website."));
    }
    let found = app.updater().map_err(|e| invalid(e.to_string()))?.check().await;
    if let Ok(mut p) = s.prefs.lock() {
        p.last_check = now_secs();
        save(app, &p);
    }
    let update = match found {
        Ok(u) => u,
        Err(e) => {
            applog::warn("update check failed");
            return Err(AppError::new(ErrorKind::Io, format!("Couldn't check for updates. Check your internet connection and try again.\n\n{e}")));
        }
    };
    applog::info(if update.is_some() { "update check: newer version" } else { "update check: up to date" });
    let Some(update) = update.filter(|u| is_newer(&u.version, &crash::version())) else {
        return Ok(None);
    };
    let skipped = s.prefs.lock().ok().and_then(|p| p.skipped_version.clone());
    let (_, can_install) = install_kind();
    let info = UpdateInfo {
        version: update.version.clone(),
        current_version: update.current_version.clone(),
        notes: update.body.clone().unwrap_or_default(),
        date: update.date.map(|d| d.to_string()),
        can_install,
    };
    if let Ok(mut p) = s.pending.lock() {
        *p = Some(update);
    }
    if manual || skipped.as_deref() != Some(info.version.as_str()) {
        if let Ok(mut a) = s.available.lock() {
            *a = Some(info.clone());
        }
        if manual {
            s.later.store(false, Ordering::Relaxed);
        }
        broadcast(app);
    }
    Ok(Some(info))
}

#[tauri::command]
pub async fn update_status(app: AppHandle, state: State<'_, UpdaterState>) -> Res<UpdateStatus> {
    Ok(status(&app, &state))
}

/// The answer to "Check for updates automatically?" (also the Settings switch).
#[tauri::command]
pub async fn set_auto_update(app: AppHandle, state: State<'_, UpdaterState>, enabled: bool) -> Res<()> {
    if let Ok(mut p) = state.prefs.lock() {
        p.auto_check = Some(enabled);
        if enabled {
            // Check now rather than waiting for the hourly pass.
            p.last_check = 0;
        }
        save(&app, &p);
    }
    applog::info(if enabled { "automatic update checks on" } else { "automatic update checks off" });
    broadcast(&app);
    if enabled && state.enabled.load(Ordering::Relaxed) {
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            let _ = check(&app, false).await;
        });
    }
    Ok(())
}

/// Help › Check for Updates…: None when this is the newest version.
#[tauri::command]
pub async fn check_update(app: AppHandle) -> Res<Option<UpdateInfo>> {
    check(&app, true).await
}

/// "Later" hides the update until the next launch; "Skip This Version" until a newer one.
#[tauri::command]
pub async fn dismiss_update(app: AppHandle, state: State<'_, UpdaterState>, skip: bool) -> Res<()> {
    if skip {
        let version = state.available.lock().ok().and_then(|a| a.as_ref().map(|i| i.version.clone()));
        if let Ok(mut p) = state.prefs.lock() {
            p.skipped_version = version;
            save(&app, &p);
        }
        if let Ok(mut a) = state.available.lock() {
            *a = None;
        }
    } else {
        state.later.store(true, Ordering::Relaxed);
    }
    broadcast(&app);
    Ok(())
}

/// Download, install and restart. Progress goes out as `update-progress` events.
#[tauri::command]
pub async fn install_update(app: AppHandle, state: State<'_, UpdaterState>, reg: State<'_, Registry>) -> Res<()> {
    if let Some(name) = reg.pending_name() {
        return Err(invalid(format!(
            "{name} has changes that haven't been written. Write or revert them first, then install the update."
        )));
    }
    let (_, can_install) = install_kind();
    if !can_install {
        return Err(invalid("This copy of Querybook is updated by your package manager, or from the download page."));
    }
    let Some(update) = state.pending.lock().ok().and_then(|mut p| p.take()) else {
        return Err(invalid("Check for updates first."));
    };
    if state.busy.swap(true, Ordering::SeqCst) {
        return Err(invalid("The update is already being installed."));
    }
    applog::info("installing update");
    let (progress_app, done_app) = (app.clone(), app.clone());
    let mut downloaded: u64 = 0;
    let result = update
        .download_and_install(
            move |chunk, total| {
                downloaded += chunk as u64;
                let _ = progress_app.emit(PROGRESS_EVENT, UpdateProgress::Downloading { downloaded, total });
            },
            move || {
                let _ = done_app.emit(PROGRESS_EVENT, UpdateProgress::Installing);
                // On Windows the installer ends this process from inside the install step, so close
                // the session now; otherwise the next launch would list an "unclean exit".
                applog::info("update downloaded; installing");
                crash::end_session();
            },
        )
        .await;
    state.busy.store(false, Ordering::SeqCst);
    match result {
        Ok(()) => {
            applog::info("update installed; restarting");
            crash::end_session();
            app.restart();
        }
        Err(e) => {
            applog::warn("update install failed");
            let _ = app.emit(PROGRESS_EVENT, UpdateProgress::Failed);
            Err(AppError::new(ErrorKind::Io, format!("The update couldn't be installed: {e}")))
        }
    }
}

/// deb / rpm installs: open the release page instead.
#[tauri::command]
pub async fn open_release_page(app: AppHandle, version: String) -> Res<()> {
    use tauri_plugin_opener::OpenerExt;
    let ok = version.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-');
    let url = if ok && !version.is_empty() {
        format!("{}/releases/tag/v{version}", shared::support::REPO_URL)
    } else {
        format!("{}/releases/latest", shared::support::REPO_URL)
    };
    app.opener().open_url(url, None::<&str>).map_err(|e| invalid(e.to_string()))
}
