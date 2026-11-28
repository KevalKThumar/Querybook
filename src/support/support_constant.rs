//! Command names and fixed lists for the Help & support module.
//!
//! Author: Keval Thumar

pub const CMD_APP_INFO: &str = "app_info";
pub const CMD_APP_READY: &str = "app_ready";
pub const CMD_LIST_CRASH_REPORTS: &str = "list_crash_reports";
pub const CMD_CRASH_REPORT: &str = "crash_report";
pub const CMD_DELETE_CRASH_REPORTS: &str = "delete_crash_reports";
pub const CMD_PENDING_CRASH_PROMPT: &str = "pending_crash_prompt";
pub const CMD_SAVE_CRASH_REPORT: &str = "save_crash_report";
pub const CMD_OPEN_SUPPORT_LINK: &str = "open_support_link";
pub const CMD_REVEAL_CRASH_REPORT: &str = "reveal_crash_report";
pub const CMD_OPEN_LOGS_FOLDER: &str = "open_logs_folder";
pub const CMD_UPDATE_STATUS: &str = "update_status";
pub const CMD_SET_AUTO_UPDATE: &str = "set_auto_update";
pub const CMD_CHECK_UPDATE: &str = "check_update";
pub const CMD_DISMISS_UPDATE: &str = "dismiss_update";
pub const CMD_INSTALL_UPDATE: &str = "install_update";
pub const CMD_OPEN_RELEASE_PAGE: &str = "open_release_page";

/// Backend events (see `src-tauri/src/updater.rs`).
pub const EVENT_UPDATE_STATUS: &str = "update-status";
pub const EVENT_UPDATE_PROGRESS: &str = "update-progress";

/// The Keyboard Shortcuts sheet: (section, keys in Mac notation, action).
/// Keys go through `PlatformService::keys`, so Windows and Linux read Ctrl / Alt / Shift.
pub const SHORTCUTS: &[(&str, &str, &str)] = &[
    ("Window", "⌘O", "Open a database"),
    ("Window", "⌘N", "New database"),
    ("Window", "⌘T", "New SQL tab"),
    ("Window", "⌘S", "Write changes (when there are any)"),
    ("Window", "⌘/", "Keyboard shortcuts"),
    ("Window", "⌘,", "Settings"),
    ("Window", "Esc", "Close a dialog or menu"),
    ("Grid", "Arrows, ⇧ + arrows", "Move, extend a cell range"),
    ("Grid", "⌘↑ / ⌘↓", "First / last row"),
    ("Grid", "Home / End", "First / last column"),
    ("Grid", "PageUp / PageDown", "Page up / down"),
    ("Grid", "↵ or F2", "Edit the cell (↵ saves, Esc cancels)"),
    ("Grid", "⌫", "Set cells to NULL, or delete the selected rows"),
    ("Grid", "⌘C / ⇧⌘C", "Copy / copy with headers"),
    ("Grid", "⌘A", "Select all rows"),
    ("Inspector", "⌘↵", "Apply the edited value"),
    ("SQL editor", "⇧↵ or ⌘↵", "Run the statement under the cursor, or the selection"),
    ("SQL editor", "⇧⌥F", "Format SQL"),
    ("SQL editor", "⌃Space", "Show suggestions"),
    ("SQL editor", "Tab", "Indent"),
];
