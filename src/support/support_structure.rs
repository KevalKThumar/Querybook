//! Types for the Help & support module.
//!
//! Author: Keval Thumar

use leptos::prelude::*;
use serde::Serialize;
use shared::support::{CrashSummary, SupportLink, UpdateProgress, UpdateStatus};

/// Root service for Help & support: which dialog is open. Provided once in `App`,
/// so the Help menu, keyboard shortcuts and the launch prompt all drive the same dialogs.
#[derive(Clone, Copy)]
pub struct SupportService {
    /// "Querybook quit unexpectedly" at launch.
    pub crash_prompt: RwSignal<Option<CrashSummary>>,
    pub reports_open: RwSignal<bool>,
    /// The report the viewer opens on.
    pub reports_select: RwSignal<Option<String>>,
    pub about_open: RwSignal<bool>,
    pub shortcuts_open: RwSignal<bool>,
    pub settings_open: RwSignal<bool>,
    /// The updater's state, kept current by the backend's `update-status` events.
    pub update: RwSignal<UpdateStatus>,
    /// Update Now's progress; None when not installing.
    pub update_progress: RwSignal<Option<UpdateProgress>>,
    /// The "Querybook X is available" dialog (release notes, Update Now).
    pub update_open: RwSignal<bool>,
    pub update_checking: RwSignal<bool>,
}

#[derive(Serialize)]
pub struct AutoUpdateRequest {
    pub enabled: bool,
}

#[derive(Serialize)]
pub struct DismissUpdateRequest {
    pub skip: bool,
}

#[derive(Serialize)]
pub struct VersionRequest {
    pub version: String,
}

#[derive(Serialize)]
pub struct ReportIdRequest<'a> {
    pub id: &'a str,
}

#[derive(Serialize)]
pub struct DeleteReportsRequest {
    pub ids: Option<Vec<String>>,
}

#[derive(Serialize)]
pub struct LinkRequest {
    pub link: SupportLink,
}
