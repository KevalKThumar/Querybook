//! Template for a SQL tab.
//!
//! Author: Keval Thumar

use leptos::html::{Pre, Textarea};
use leptos::prelude::*;
use shared::complete::{SuggestKind, Suggestion};
use shared::Cell;

use crate::database::components::cell_inspector::cell_inspector_component::CellInspector;
use crate::database::components::grid::grid_component::Grid;
use crate::database::components::sql_code::sql_code_component::SqlCode;
use crate::database::database_structure::GridSelection;
use crate::database::dialogs::snippets_dialog::snippets_dialog_component::SnippetsDialog;
use crate::shared::services::platform_service::PlatformService;

fn kind_icon(kind: SuggestKind) -> &'static str {
    match kind {
        SuggestKind::Column => "bi bi-layout-three-columns",
        SuggestKind::Table => "bi bi-table",
        SuggestKind::Function => "bi bi-braces",
        SuggestKind::Keyword => "bi bi-type",
    }
}

#[allow(clippy::too_many_arguments)]
pub fn sql_editor_view(
    highlighted: Signal<String>,
    running: Signal<bool>,
    status: Signal<Option<(bool, String)>>,
    history_labels: Signal<Vec<String>>,
    suggestions: Memo<Vec<Suggestion>>,
    suggest_index: Signal<usize>,
    popup_at: Signal<Option<(f64, f64)>>,
    result_columns: Signal<Vec<String>>,
    result_total: Signal<usize>,
    cell_at: Callback<(usize, usize), Option<Cell>>,
    selection: RwSignal<GridSelection>,
    result_table: Signal<String>,
    textarea: NodeRef<Textarea>,
    overlay: NodeRef<Pre>,
    measure: NodeRef<Pre>,
    on_run_all: Callback<()>,
    on_run_current: Callback<()>,
    on_explain: Callback<()>,
    on_stop: Callback<()>,
    on_history: Callback<usize>,
    on_input: Callback<String>,
    on_keydown: Callback<web_sys::KeyboardEvent>,
    on_keyup: Callback<web_sys::KeyboardEvent>,
    on_caret_moved: Callback<()>,
    on_scroll: Callback<()>,
    on_pick: Callback<usize>,
    on_format: Callback<()>,
    on_open_snippets: Callback<()>,
    snippets_open: RwSignal<bool>,
    snippet_sql: Signal<String>,
    on_insert_snippet: Callback<String>,
    inspector_open: Signal<bool>,
    inspected_column: Signal<String>,
    inspected: Signal<Option<Cell>>,
    on_toggle_inspector: Callback<()>,
) -> impl IntoView {
    view! {
        <div class="sqled d-flex flex-column h-100">
            <div class="sqled-toolbar d-flex align-items-center gap-2 px-2">
                <button class="btn btn-sm btn-primary" title="Run everything, or the selection" disabled=move || running.get() on:click=move |_| on_run_all.run(())>
                    <i class="bi bi-play-fill"></i>" Run all"
                </button>
                <button class="btn btn-sm btn-outline-primary" title={PlatformService::keys("Run the statement under the cursor (⇧↵ or ⌘↵)")} disabled=move || running.get() on:click=move |_| on_run_current.run(())>
                    <i class="bi bi-cursor-text"></i>" Run current"
                </button>
                <button class="btn btn-sm btn-outline-secondary" title="EXPLAIN QUERY PLAN for the current statement" disabled=move || running.get() on:click=move |_| on_explain.run(())>
                    <i class="bi bi-diagram-2"></i>" Explain"
                </button>
                <div class="vr mx-1"></div>
                <button class="btn btn-sm btn-outline-secondary" title={PlatformService::keys("Format the SQL, or the selection (⇧⌥F)")} on:click=move |_| on_format.run(())>
                    <i class="bi bi-text-indent-left"></i>" Format"
                </button>
                <button class="btn btn-sm btn-outline-secondary" title="Saved snippets" on:click=move |_| on_open_snippets.run(())>
                    <i class="bi bi-bookmark"></i>" Snippets"
                </button>
                <Show when=move || running.get() fallback=|| ()>
                    <button class="btn btn-sm btn-outline-danger" title="Cancel the running query" on:click=move |_| on_stop.run(())>
                        <i class="bi bi-stop-fill"></i>" Stop"
                    </button>
                    <span class="spinner-border spinner-border-sm text-secondary"></span>
                </Show>
                <select
                    class="form-select form-select-sm sqled-history"
                    title="Query history for this database"
                    prop:value=""
                    on:change=move |ev| {
                        if let Ok(i) = event_target_value(&ev).parse::<usize>() {
                            on_history.run(i);
                        }
                    }
                >
                    <option value="" selected=true disabled=true>"History"</option>
                    {move || {
                        history_labels
                            .get()
                            .into_iter()
                            .enumerate()
                            .map(|(i, label)| view! { <option value=i.to_string()>{label}</option> })
                            .collect_view()
                    }}
                </select>
                <span class="sqled-hint ms-auto">{PlatformService::keys("⇧↵ run current · ⌃Space suggest")}</span>
                <button
                    class="btn btn-sm btn-outline-secondary"
                    class:active=move || inspector_open.get()
                    title="Cell inspector for the results"
                    on:click=move |_| on_toggle_inspector.run(())
                >
                    <i class="bi bi-layout-sidebar-reverse"></i>
                </button>
            </div>

            <div class="sqled-input">
                <SqlCode sql=highlighted class="sqled-layer sqled-highlight" pre_ref=overlay />
                <textarea
                    node_ref=textarea
                    class="sqled-layer sqled-textarea"
                    spellcheck="false"
                    autocomplete="off"
                    placeholder={PlatformService::keys("-- Write SQL here. ⇧↵ runs the statement under the cursor.")}
                    on:input=move |ev| on_input.run(event_target_value(&ev))
                    on:keydown=move |ev| on_keydown.run(ev)
                    on:keyup=move |ev| on_keyup.run(ev)
                    on:click=move |_| on_caret_moved.run(())
                    on:blur=move |_| on_caret_moved.run(())
                    on:scroll=move |_| on_scroll.run(())
                ></textarea>
                // Hidden 100×10 block of the editor font: char width / line height for the popup.
                <pre class="sqled-measure" node_ref=measure aria-hidden="true">
                    {(0..10).map(|_| "M".repeat(100)).collect::<Vec<_>>().join("\n")}
                </pre>
                <Show when=move || popup_at.get().is_some() fallback=|| ()>
                    <div
                        class="sqled-suggest"
                        style:left=move || format!("{}px", popup_at.get().map_or(0.0, |p| p.0))
                        style:top=move || format!("{}px", popup_at.get().map_or(0.0, |p| p.1))
                    >
                        <For
                            each={move || suggestions.get().into_iter().enumerate().collect::<Vec<_>>()}
                            key=|(i, s)| (*i, s.label.clone(), s.detail.clone())
                            children=move |(i, s)| view! {
                                <div
                                    class="sqled-suggest-item d-flex align-items-center gap-2"
                                    class:active=move || suggest_index.get() == i
                                    on:mousedown=move |ev| {
                                        ev.prevent_default();
                                        on_pick.run(i);
                                    }
                                >
                                    <i class=kind_icon(s.kind)></i>
                                    <span class="flex-grow-1 text-truncate">{s.label.clone()}</span>
                                    <span class="sqled-suggest-detail text-truncate">{s.detail.clone()}</span>
                                </div>
                            }
                        />
                    </div>
                </Show>
            </div>

            <div class="sqled-status px-2" class:text-danger=move || status.get().is_some_and(|(err, _)| err)>
                {move || status.get().map(|(_, msg)| msg).unwrap_or_else(|| "Ready".to_string())}
            </div>

            <div class="sqled-results d-flex flex-grow-1">
                <div class="flex-grow-1 overflow-hidden">
                    <Grid
                        columns=result_columns
                        total=result_total
                        cell_at=cell_at
                        selection=selection
                        table_name=result_table
                    />
                </div>
                <Show when=move || inspector_open.get() fallback=|| ()>
                    <CellInspector
                        column=inspected_column
                        cell=inspected
                        editable=false
                        on_close=on_toggle_inspector
                    />
                </Show>
            </div>
            <SnippetsDialog open=snippets_open current_sql=snippet_sql on_insert=on_insert_snippet />
        </div>
    }
}
