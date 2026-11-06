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
