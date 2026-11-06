//! Template for the context menu: invisible backdrop + positioned list.
//!
//! Author: Keval Thumar

use leptos::prelude::*;

use crate::shared::common_structure::MenuItem;

pub fn context_menu_view(
    placement: Signal<Option<(f64, f64)>>,
    items: Signal<Vec<MenuItem>>,
    on_close: Callback<()>,
    on_pick: Callback<Callback<()>>,
) -> impl IntoView {
    view! {
        <Show when=move || placement.get().is_some() fallback=|| ()>
            <div
                class="ctx-backdrop"
                on:mousedown=move |_| on_close.run(())
                on:wheel=move |_| on_close.run(())
                on:contextmenu=move |ev| {
                    ev.prevent_default();
                    on_close.run(());
                }
            ></div>
            <div
                class="ctx-menu"
                style:left=move || format!("{}px", placement.get().map_or(0.0, |p| p.0))
                style:top=move || format!("{}px", placement.get().map_or(0.0, |p| p.1))
                on:contextmenu=move |ev| ev.prevent_default()
            >
                {move || {
                    items
                        .get()
                        .into_iter()
                        .map(|item| match item {
                            MenuItem::Divider => view! { <div class="ctx-divider"></div> }.into_any(),
                            MenuItem::Action { label, icon, shortcut, enabled, checked, danger, on_select } => view! {
                                <button
                                    class="ctx-item"
                                    class:danger=danger
                                    disabled=!enabled
                                    on:mousedown=move |ev| ev.prevent_default()
                                    on:click=move |_| on_pick.run(on_select)
                                >
                                    <span class="ctx-icon">
                                        {match checked {
                                            Some(true) => view! { <i class="bi bi-check2"></i> }.into_any(),
                                            Some(false) => ().into_any(),
                                            None => view! { <i class=icon></i> }.into_any(),
                                        }}
                                    </span>
                                    <span class="ctx-label">{label}</span>
                                    <span class="ctx-shortcut">{shortcut}</span>
                                </button>
                            }
                            .into_any(),
                        })
                        .collect_view()
                }}
            </div>
        </Show>
    }
}
