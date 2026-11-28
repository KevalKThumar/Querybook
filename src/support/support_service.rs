//! SupportService — the Help & support module's backend calls, plus the dialog
//! signals (see `support_structure`).
//!
//! Static async fns over `OsBridge::invoke`; failures show a toast and return
//! plain data.
//!
//! Author: Keval Thumar

use leptos::logging::error;
use leptos::prelude::*;
use leptos::task::spawn_local;
use serde::Serialize;
use serde::de::DeserializeOwned;
use shared::support::{AppInfo, CrashReport, CrashSummary, SupportLink, UpdateInfo, UpdateProgress, UpdateStatus};

use crate::shared::services::os_bridge::{NoArgs, OsBridge};
use crate::shared::services::toast_service::ToastService;
use crate::support::support_constant::{
    CMD_APP_INFO, CMD_APP_READY, CMD_CHECK_UPDATE, CMD_CRASH_REPORT, CMD_DELETE_CRASH_REPORTS, CMD_DISMISS_UPDATE,
    CMD_INSTALL_UPDATE, CMD_LIST_CRASH_REPORTS, CMD_OPEN_LOGS_FOLDER, CMD_OPEN_RELEASE_PAGE, CMD_OPEN_SUPPORT_LINK,
    CMD_PENDING_CRASH_PROMPT, CMD_REVEAL_CRASH_REPORT, CMD_SAVE_CRASH_REPORT, CMD_SET_AUTO_UPDATE, CMD_UPDATE_STATUS,
    EVENT_UPDATE_PROGRESS, EVENT_UPDATE_STATUS,
};
use crate::support::support_structure::{
    AutoUpdateRequest, DeleteReportsRequest, DismissUpdateRequest, LinkRequest, ReportIdRequest, SupportService,
    VersionRequest,
};

impl Default for SupportService {
    fn default() -> Self {
        Self::new()
    }
}

impl SupportService {
    pub fn new() -> Self {
        Self {
            crash_prompt: RwSignal::new(None),
            reports_open: RwSignal::new(false),
            reports_select: RwSignal::new(None),
            about_open: RwSignal::new(false),
            shortcuts_open: RwSignal::new(false),
            settings_open: RwSignal::new(false),
            update: RwSignal::new(UpdateStatus::default()),
            update_progress: RwSignal::new(None),
            update_open: RwSignal::new(false),
            update_checking: RwSignal::new(false),
        }
    }

    /// Follow the updater: its current status now, and every change the backend sends.
    pub fn watch_updates(&self) {
        let (update, progress) = (self.update, self.update_progress);
        OsBridge::listen::<UpdateStatus>(EVENT_UPDATE_STATUS, move |s| update.set(s));
        OsBridge::listen::<UpdateProgress>(EVENT_UPDATE_PROGRESS, move |p| progress.set(Some(p)));
        spawn_local(async move {
            if let Ok(s) = OsBridge::invoke::<_, UpdateStatus>(CMD_UPDATE_STATUS, &NoArgs {}).await {
                update.set(s);
            }
        });
    }

    /// The answer to "Check for updates automatically?" (and the Settings switch).
    pub fn set_auto_update(enabled: bool, toast: ToastService) {
        spawn_local(async move {
            let _: Option<()> = call(CMD_SET_AUTO_UPDATE, &AutoUpdateRequest { enabled }, toast).await;
        });
    }

    /// Help › Check for Updates…: opens the update dialog, or says this is the newest version.
    pub fn check_now(&self, toast: ToastService) {
        let (checking, open, update) = (self.update_checking, self.update_open, self.update);
        if checking.get_untracked() {
            return;
        }
        checking.set(true);
        spawn_local(async move {
            match OsBridge::invoke::<_, Option<UpdateInfo>>(CMD_CHECK_UPDATE, &NoArgs {}).await {
                Ok(Some(info)) => {
                    update.update(|s| s.available = Some(info));
                    open.set(true);
                }
                Ok(None) => {
                    let version = update.with_untracked(|s| s.current_version.clone());
                    toast.success(&format!("Querybook {version} is the newest version."));
                }
                Err(e) => toast.error(&e.message),
            }
            checking.set(false);
        });
    }

    /// Later (`skip = false`) or Skip This Version.
    pub fn dismiss_update(&self, skip: bool, toast: ToastService) {
        self.update_open.set(false);
        spawn_local(async move {
            let _: Option<()> = call(CMD_DISMISS_UPDATE, &DismissUpdateRequest { skip }, toast).await;
        });
    }

    /// Update Now. On success the app restarts; on failure the dialog shows why.
    pub fn install_update(&self, toast: ToastService) {
        let progress = self.update_progress;
        progress.set(Some(UpdateProgress::Downloading { downloaded: 0, total: None }));
        spawn_local(async move {
            if let Err(e) = OsBridge::invoke::<_, ()>(CMD_INSTALL_UPDATE, &NoArgs {}).await {
                progress.set(None);
                toast.error(&e.message);
            }
        });
    }

    pub fn open_release_page(version: String, toast: ToastService) {
        spawn_local(async move {
            let _: Option<()> = call(CMD_OPEN_RELEASE_PAGE, &VersionRequest { version }, toast).await;
        });
    }

    /// Open the Crash Reports viewer, optionally on one report.
    pub fn show_reports(&self, id: Option<String>) {
        self.reports_select.set(id);
        self.reports_open.set(true);
    }

    /// Once the window has drawn: tell the watchdog, then ask whether an earlier run left a report.
    pub fn on_mounted(&self, toast: ToastService) {
        let prompt = self.crash_prompt;
        spawn_local(async move {
            let _ = OsBridge::invoke::<_, ()>(CMD_APP_READY, &NoArgs {}).await;
            if let Some(Some(summary)) = call::<_, Option<CrashSummary>>(CMD_PENDING_CRASH_PROMPT, &NoArgs {}, toast).await {
                prompt.set(Some(summary));
            }
        });
    }

    pub async fn app_info(toast: ToastService) -> Option<AppInfo> {
        call(CMD_APP_INFO, &NoArgs {}, toast).await
    }

    pub async fn list_reports(toast: ToastService) -> Vec<CrashSummary> {
        call(CMD_LIST_CRASH_REPORTS, &NoArgs {}, toast).await.unwrap_or_default()
    }

    pub async fn report(id: &str, toast: ToastService) -> Option<CrashReport> {
        call(CMD_CRASH_REPORT, &ReportIdRequest { id }, toast).await
    }

    /// `None` deletes every report.
    pub async fn delete_reports(ids: Option<Vec<String>>, toast: ToastService) {
        let _: Option<()> = call(CMD_DELETE_CRASH_REPORTS, &DeleteReportsRequest { ids }, toast).await;
    }

    /// Save dialog; the saved path, or None if cancelled.
    pub async fn save_report(id: &str, toast: ToastService) -> Option<String> {
        call::<_, Option<String>>(CMD_SAVE_CRASH_REPORT, &ReportIdRequest { id }, toast).await.flatten()
    }

    pub async fn reveal_report(id: &str, toast: ToastService) {
        let _: Option<()> = call(CMD_REVEAL_CRASH_REPORT, &ReportIdRequest { id }, toast).await;
    }

    pub fn open_link(link: SupportLink, toast: ToastService) {
        spawn_local(async move {
            let _: Option<()> = call(CMD_OPEN_SUPPORT_LINK, &LinkRequest { link }, toast).await;
        });
    }

    pub fn open_logs_folder(toast: ToastService) {
        spawn_local(async move {
            let _: Option<()> = call(CMD_OPEN_LOGS_FOLDER, &NoArgs {}, toast).await;
        });
    }
}

async fn call<A: Serialize, R: DeserializeOwned>(cmd: &str, args: &A, toast: ToastService) -> Option<R> {
    match OsBridge::invoke(cmd, args).await {
        Ok(value) => Some(value),
        Err(e) => {
            toast.error(&e.message);
            error!("{cmd}: {e}");
            None
        }
    }
}
