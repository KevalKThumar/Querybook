//! Template for the cell inspector panel.
//!
//! Author: Keval Thumar

use leptos::prelude::*;

use crate::database::database_structure::InspectMode;

const MODES: [(InspectMode, &str); 3] = [(InspectMode::Text, "Text"), (InspectMode::Json, "JSON"), (InspectMode::Hex, "Hex")];

#[allow(clippy::too_many_arguments)]
pub fn cell_inspector_view(
    column: Signal<String>,
    has_cell: Signal<bool>,
    kind: Signal<&'static str>,
    size: Signal<String>,
    mode: Signal<InspectMode>,
    draft: Signal<String>,
    json: Signal<Option<String>>,
    hex: Signal<Result<String, String>>,
    is_null: Signal<bool>,
    is_blob: Signal<bool>,
    can_edit: Signal<bool>,
    changed: Signal<bool>,
    on_mode: Callback<InspectMode>,
    on_input: Callback<String>,
    on_apply: Callback<()>,
    on_set_null: Callback<()>,
    on_copy: Callback<()>,
    on_close: Callback<()>,
) -> impl IntoView {
    view! {
        <aside class="inspector d-flex flex-column">
            <div class="inspector-head d-flex align-items-center gap-2 px-2">
                <span class="text-truncate fw-semibold" title=move || column.get()>{move || column.get()}</span>
                <Show when=move || has_cell.get() fallback=|| ()>
                    <span class="badge text-bg-secondary">{move || kind.get()}</span>
                </Show>
                <span class="inspector-size ms-auto text-nowrap">{move || size.get()}</span>
                <button class="icon-btn" title="Close the inspector" on:click=move |_| on_close.run(())>
                    <i class="bi bi-x-lg"></i>
                </button>
            </div>

            <Show
                when=move || has_cell.get()
                fallback=|| view! {
                    <div class="empty-state flex-grow-1">
                        <i class="bi bi-cursor"></i>
                        <div>"Select a cell to inspect it."</div>
                    </div>
                }
            >
                <div class="btn-group btn-group-sm m-2">
                    {MODES
                        .into_iter()
                        .map(|(m, label)| view! {
                            <button
                                class="btn btn-outline-secondary"
                                class:active=move || mode.get() == m
                                disabled=move || m != InspectMode::Hex && is_blob.get()
                                on:click=move |_| on_mode.run(m)
                            >
                                {label}
                            </button>
                        })
                        .collect_view()}
                </div>

                <div class="inspector-body flex-grow-1 mx-2">
                    {move || match mode.get() {
                        InspectMode::Text => view! {
                            <textarea
                                class="inspector-text"
                                spellcheck="false"
                                readonly=move || !can_edit.get()
                                placeholder=move || if is_null.get() { "NULL" } else { "" }
                                prop:value=move || draft.get()
                                on:input=move |ev| on_input.run(event_target_value(&ev))
                                on:keydown=move |ev| {
                                    if ev.key() == "Enter" && (ev.meta_key() || ev.ctrl_key()) {
                                        ev.prevent_default();
                                        on_apply.run(());
                                    }
                                }
                            ></textarea>
                        }.into_any(),
                        InspectMode::Json => view! {
                            <pre class="inspector-pre">
                                {move || json.get().unwrap_or_else(|| "Not a JSON object or array.".to_string())}
                            </pre>
                        }.into_any(),
                        InspectMode::Hex => view! {
                            <pre class="inspector-pre">{move || hex.get().unwrap_or_else(|why| why)}</pre>
                        }.into_any(),
                    }}
                </div>

                <div class="inspector-actions d-flex align-items-center gap-2 p-2">
                    <button class="btn btn-sm btn-outline-secondary" title="Copy what's shown" on:click=move |_| on_copy.run(())>
                        <i class="bi bi-clipboard"></i>" Copy"
                    </button>
                    <div class="ms-auto"></div>
                    <Show when=move || can_edit.get() fallback=|| ()>
                        <button class="btn btn-sm btn-outline-secondary" disabled=move || is_null.get() on:click=move |_| on_set_null.run(())>
                            "Set NULL"
                        </button>
                        <button
                            class="btn btn-sm btn-primary"
                            title="Write this value to the cell (⌘↵)"
                            disabled=move || !changed.get() || mode.get() != InspectMode::Text
                            on:click=move |_| on_apply.run(())
                        >
                            <i class="bi bi-check2"></i>" Apply"
                        </button>
                    </Show>
                </div>
            </Show>
        </aside>
    }
}
