//! Template for the schema sidebar.
//!
//! Author: Keval Thumar

use leptos::prelude::*;

use crate::database::database_constant::SCHEMA_GROUPS;
use crate::database::database_structure::SchemaObject;

pub fn schema_sidebar_view(
    filter: RwSignal<String>,
    groups: Memo<Vec<(&'static str, Vec<SchemaObject>)>>,
    collapsed: Signal<Vec<&'static str>>,
    on_toggle: Callback<&'static str>,
    on_item: Callback<SchemaObject>,
    on_structure: Callback<String>,
) -> impl IntoView {
    view! {
        <aside class="sidebar d-flex flex-column">
            <div class="sidebar-search p-2">
                <input
                    class="form-control form-control-sm"
                    type="search"
                    placeholder="Filter"
                    prop:value=move || filter.get()
                    on:input=move |ev| filter.set(event_target_value(&ev))
                />
            </div>
            <div class="sidebar-tree flex-grow-1 overflow-auto pb-2">
                <For
                    each=move || SCHEMA_GROUPS.to_vec()
                    key=|(kind, _)| *kind
                    children=move |(kind, label)| {
                        let items = move || {
                            groups.with(|g| g.iter().find(|(k, _)| *k == kind).map(|(_, v)| v.clone()).unwrap_or_default())
                        };
                        let expanded = move || !collapsed.with(|c| c.contains(&kind));
                        view! {
                            <div class="sidebar-group">
                                <div class="sidebar-group-head d-flex align-items-center gap-1" on:click=move |_| on_toggle.run(kind)>
                                    <i class=move || if expanded() { "bi bi-chevron-down" } else { "bi bi-chevron-right" }></i>
                                    <span>{label}</span>
                                    <span class="sidebar-count ms-auto">{move || items().len()}</span>
                                </div>
                                <Show when=expanded fallback=|| ()>
                                    <For
                                        each=items
                                        key=|o| o.name.clone()
                                        children=move |o| {
                                            let name = o.name.clone();
                                            let obj = o.clone();
                                            view! {
                                                <div
                                                    class="sidebar-item d-flex align-items-center gap-2"
                                                    title=o.name.clone()
                                                    on:click=move |_| on_item.run(obj.clone())
                                                >
                                                    <i class=o.icon()></i>
                                                    <span class="text-truncate flex-grow-1">{o.name.clone()}</span>
                                                    <button
                                                        class="icon-btn sidebar-item-info"
                                                        title="Structure"
                                                        on:click=move |ev| {
                                                            ev.stop_propagation();
                                                            on_structure.run(name.clone());
                                                        }
                                                    >
                                                        <i class="bi bi-info-circle"></i>
                                                    </button>
                                                </div>
                                            }
                                        }
                                    />
                                </Show>
                            </div>
                        }
                    }
                />
            </div>
        </aside>
    }
}
