//! Template for a SQL tab.
//!
//! Author: Keval Thumar

use leptos::html::{Pre, Textarea};
use leptos::prelude::*;
use shared::ResultSet;

use crate::database::components::result_grid::result_grid_component::ResultGrid;
use crate::database::components::sql_code::sql_code_component::SqlCode;

#[allow(clippy::too_many_arguments)]
pub fn sql_editor_view(
    highlighted: Signal<String>,
    running: Signal<bool>,
    status: Signal<Option<(bool, String)>>,
    result: Signal<Option<ResultSet>>,
    textarea: NodeRef<Textarea>,
    overlay: NodeRef<Pre>,
    on_run: Callback<()>,
    on_stop: Callback<()>,
    on_input: Callback<String>,
    on_keydown: Callback<web_sys::KeyboardEvent>,
    on_scroll: Callback<()>,
) -> impl IntoView {
    view! {
        <div class="sqled d-flex flex-column h-100">
            <div class="sqled-toolbar d-flex align-items-center gap-2 px-2">
                <button class="btn btn-sm btn-primary" title="Run all, or the selection (⌘↵)" disabled=move || running.get() on:click=move |_| on_run.run(())>
                    <i class="bi bi-play-fill"></i>" Run"
                </button>
                <Show when=move || running.get() fallback=|| ()>
                    <button class="btn btn-sm btn-outline-danger" title="Cancel the running query" on:click=move |_| on_stop.run(())>
                        <i class="bi bi-stop-fill"></i>" Stop"
                    </button>
                    <span class="spinner-border spinner-border-sm text-secondary"></span>
                </Show>
                <span class="sqled-hint ms-auto">"⌘↵ run · select text to run only that"</span>
            </div>

            <div class="sqled-input">
                <SqlCode sql=highlighted class="sqled-layer sqled-highlight" pre_ref=overlay />
                <textarea
                    node_ref=textarea
                    class="sqled-layer sqled-textarea"
                    spellcheck="false"
                    autocomplete="off"
                    placeholder="-- Write SQL here"
                    on:input=move |ev| on_input.run(event_target_value(&ev))
                    on:keydown=move |ev| on_keydown.run(ev)
                    on:scroll=move |_| on_scroll.run(())
                ></textarea>
            </div>

            <div class="sqled-status px-2" class:text-danger=move || status.get().is_some_and(|(err, _)| err)>
                {move || status.get().map(|(_, msg)| msg).unwrap_or_else(|| "Ready".to_string())}
            </div>

            <div class="sqled-results flex-grow-1">
                <ResultGrid result=result />
            </div>
        </div>
    }
}
