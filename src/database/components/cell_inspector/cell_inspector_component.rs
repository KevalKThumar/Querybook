//! CellInspector — side panel for the selected grid cell: the full text
//! (editable when the grid is), pretty-printed JSON, or a hex dump.
//!
//! The parent passes the focused cell and, for blobs, the bytes it fetched;
//! "Apply" / "Set NULL" go back through `on_apply`.
//!
//! Author: Keval Thumar

use leptos::prelude::*;
use leptos::task::spawn_local;
use shared::format::{count, hex_dump, json_pretty};
use shared::Cell;

use super::cell_inspector_view::cell_inspector_view;
use crate::database::database_constant::INSPECT_MAX_BYTES;
use crate::database::database_structure::InspectMode;
use crate::shared::services::clipboard_service::ClipboardService;
use crate::shared::services::toast_service::ToastService;

/// Cell as editable text; NULL becomes "".
fn cell_text(cell: &Cell) -> String {
    match cell {
        Cell::Null => String::new(),
        other => other.display(),
    }
}

// Keval Thumar
#[component]
pub fn CellInspector(
    /// Column of the focused cell.
    #[prop(into)]
    column: Signal<String>,
    /// The focused cell; None when nothing is selected (or it isn't loaded).
    #[prop(into)]
    cell: Signal<Option<Cell>>,
    /// Blob bytes (up to INSPECT_MAX_BYTES) and the full length; None while
    /// loading, Err(why) when they can't be fetched. Leave out for SQL results.
    #[prop(optional, into)]
    blob: Option<Signal<Option<Result<(Vec<u8>, usize), String>>>>,
    #[prop(into)] editable: Signal<bool>,
    #[prop(optional)] on_apply: Option<Callback<Cell>>,
    on_close: Callback<()>,
) -> impl IntoView {
    let toast = expect_context::<ToastService>();
    let mode = RwSignal::new(InspectMode::Text);
    let draft = RwSignal::new(String::new());

    // Only a different cell (or a changed value) resets the draft; reloads of the same value don't.
    let current = Memo::new(move |_| cell.get());
    Effect::new(move |_| {
        let Some(value) = current.get() else { return };
        let text = cell_text(&value);
        mode.set(match value {
            Cell::Blob(_) => InspectMode::Hex,
            _ if json_pretty(&text).is_some() => InspectMode::Json,
            _ => InspectMode::Text,
        });
        draft.set(text);
    });

    let is_blob = Signal::derive(move || matches!(current.get(), Some(Cell::Blob(_))));
    let is_null = Signal::derive(move || matches!(current.get(), Some(Cell::Null)));
    let can_edit = Signal::derive(move || editable.get() && on_apply.is_some() && !is_blob.get());
    let changed = Signal::derive(move || {
        current.with(|c| c.as_ref().is_some_and(|c| draft.with(|d| *d != cell_text(c) || (*c == Cell::Null && !d.is_empty()))))
    });
    let kind = Signal::derive(move || match current.get() {
        Some(Cell::Null) => "NULL",
        Some(Cell::Int(_)) => "INTEGER",
        Some(Cell::Real(_)) => "REAL",
        Some(Cell::Text(_)) => "TEXT",
        Some(Cell::Blob(_)) => "BLOB",
        None => "",
    });
    let size = Signal::derive(move || match current.get() {
        Some(Cell::Blob(n)) => count(n as u64, "byte"),
        Some(Cell::Null) | None => String::new(),
        Some(_) => draft.with(|d| count(d.chars().count() as u64, "char")),
    });
    let json = Signal::derive(move || draft.with(|d| json_pretty(d)));
    // Blobs: the fetched bytes. Text: its UTF-8 bytes (shows hidden characters).
    let hex = Signal::derive(move || -> Result<String, String> {
        if is_blob.get() {
            let Some(blob) = blob else { return Err("Binary data. Browse its table to see the bytes.".into()) };
            let (bytes, len) = blob.get().unwrap_or_else(|| Err("Loading…".into()))?;
            let more = if len > bytes.len() { format!("\n… first {} of {len} bytes", bytes.len()) } else { String::new() };
            return Ok(format!("{}{more}", hex_dump(&bytes)));
        }
        Ok(draft.with(|d| hex_dump(&d.as_bytes()[..d.len().min(INSPECT_MAX_BYTES)])))
    });

    let on_mode = Callback::new(move |m: InspectMode| mode.set(m));
    let on_input = Callback::new(move |text: String| draft.set(text));
    let on_apply_draft = Callback::new(move |_: ()| {
        if let (Some(apply), true, true) = (on_apply, can_edit.get_untracked(), changed.get_untracked()) {
            apply.run(Cell::Text(draft.get_untracked()));
        }
    });
    let on_set_null = Callback::new(move |_: ()| {
        if let (Some(apply), true) = (on_apply, can_edit.get_untracked()) {
            apply.run(Cell::Null);
        }
    });
    let on_copy = Callback::new(move |_: ()| {
        let text = match mode.get_untracked() {
            InspectMode::Text => draft.get_untracked(),
            InspectMode::Json => json.get_untracked().unwrap_or_else(|| draft.get_untracked()),
            InspectMode::Hex => hex.get_untracked().unwrap_or_default(),
        };
        spawn_local(async move {
            if ClipboardService::write_text(&text).await {
                toast.success("Copied.");
            } else {
                toast.error("Couldn't write to the clipboard.");
            }
        });
    });

    cell_inspector_view(
        column,
        Signal::derive(move || current.with(Option::is_some)),
        kind,
        size,
        mode.into(),
        draft.into(),
        json,
        hex,
        is_null,
        is_blob,
        can_edit,
        changed,
        on_mode,
        on_input,
        on_apply_draft,
        on_set_null,
        on_copy,
        on_close,
    )
}
