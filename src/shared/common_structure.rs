//! Types used across modules, plus root service structs (impls live in
//! `shared/services/`).
//!
//! Author: Keval Thumar

use leptos::prelude::*;

/// App theme. `System` follows the OS light/dark setting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeMode {
    System,
    Light,
    Dark,
}

impl ThemeMode {
    pub fn parse(value: Option<&str>) -> Self {
        match value {
            Some("light") => ThemeMode::Light,
            Some("dark") => ThemeMode::Dark,
            _ => ThemeMode::System,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            ThemeMode::System => "system",
            ThemeMode::Light => "light",
            ThemeMode::Dark => "dark",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            ThemeMode::System => "System",
            ThemeMode::Light => "Light",
            ThemeMode::Dark => "Dark",
        }
    }

    /// Bootstrap Icons class for the theme switch.
    pub fn icon(self) -> &'static str {
        match self {
            ThemeMode::System => "bi bi-circle-half",
            ThemeMode::Light => "bi bi-sun",
            ThemeMode::Dark => "bi bi-moon-stars",
        }
    }

    pub fn next(self) -> Self {
        match self {
            ThemeMode::System => ThemeMode::Light,
            ThemeMode::Light => ThemeMode::Dark,
            ThemeMode::Dark => ThemeMode::System,
        }
    }
}

/// Root service for the theme. Provided once in `App`.
#[derive(Clone, Copy)]
pub struct ThemeService {
    pub mode: RwSignal<ThemeMode>,
}

/// One entry of a `<ContextMenu/>`. Built fresh each time the menu opens.
#[derive(Clone)]
pub enum MenuItem {
    Action {
        label: String,
        /// Bootstrap Icons class.
        icon: &'static str,
        /// Keyboard hint shown on the right, e.g. "⌘C".
        shortcut: &'static str,
        enabled: bool,
        /// Some(_) renders a check mark column (toggle items).
        checked: Option<bool>,
        danger: bool,
        on_select: Callback<()>,
    },
    Divider,
}

impl MenuItem {
    pub fn action(label: impl Into<String>, icon: &'static str, on_select: Callback<()>) -> Self {
        MenuItem::Action { label: label.into(), icon, shortcut: "", enabled: true, checked: None, danger: false, on_select }
    }

    pub fn shortcut(mut self, keys: &'static str) -> Self {
        if let MenuItem::Action { shortcut, .. } = &mut self {
            *shortcut = keys;
        }
        self
    }

    pub fn enabled(mut self, on: bool) -> Self {
        if let MenuItem::Action { enabled, .. } = &mut self {
            *enabled = on;
        }
        self
    }

    pub fn checked(mut self, on: bool) -> Self {
        if let MenuItem::Action { checked, .. } = &mut self {
            *checked = Some(on);
        }
        self
    }

    pub fn danger(mut self) -> Self {
        if let MenuItem::Action { danger, .. } = &mut self {
            *danger = true;
        }
        self
    }
}
