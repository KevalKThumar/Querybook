//! Template for the Help button and its menu.
//!
//! Author: Keval Thumar

use leptos::prelude::*;

use crate::shared::common_structure::MenuItem;
use crate::shared::components::context_menu::context_menu_component::ContextMenu;

pub fn help_menu_view(
    class: String,
    labelled: bool,
    menu: RwSignal<Option<(f64, f64)>>,
    items: Signal<Vec<MenuItem>>,
    on_toggle: Callback<leptos::ev::MouseEvent>,
) -> impl IntoView {
    view! {
        <button class=format!("btn btn-sm help-menu-button {class}") title="Help" aria-label="Help" on:click=move |ev| on_toggle.run(ev)>
            <i class="bi bi-question-circle"></i>
            {labelled.then(|| view! { <span>"Help"</span> })}
        </button>
        <ContextMenu position=menu items=items />
    }
}
