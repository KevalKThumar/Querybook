//! ContextMenu — a right-click menu opened at the cursor.
//!
//! The caller owns `position` (Some((client_x, client_y)) = open) and builds
//! the `items`. Choosing an item closes the menu, then runs its callback
//! (still inside the click, so clipboard writes keep their user gesture).
//! Closes on outside mouse-down, Escape, or scroll.
//!
//! ```rust
//! let menu = RwSignal::new(None::<(f64, f64)>);
//! view! {
//!     <div on:contextmenu=move |ev| { ev.prevent_default(); menu.set(Some((ev.client_x() as f64, ev.client_y() as f64))) }>…</div>
//!     <ContextMenu position=menu items=items />
//! }
//! ```
//!
//! Author: Keval Thumar

use leptos::ev;
use leptos::prelude::*;

use super::context_menu_view::context_menu_view;
use crate::shared::common_structure::MenuItem;

const MENU_WIDTH: f64 = 230.0;
const ITEM_HEIGHT: f64 = 28.0;

#[component]
pub fn ContextMenu(
    /// Where the menu is open, in viewport pixels; None = closed.
    position: RwSignal<Option<(f64, f64)>>,
    #[prop(into)] items: Signal<Vec<MenuItem>>,
) -> impl IntoView {
    // Keep the menu inside the window.
    let placement = Signal::derive(move || {
        position.get().map(|(x, y)| {
            let w = window().inner_width().ok().and_then(|v| v.as_f64()).unwrap_or(1200.0);
            let h = window().inner_height().ok().and_then(|v| v.as_f64()).unwrap_or(800.0);
            let height = items.with(|i| i.len()) as f64 * ITEM_HEIGHT + 12.0;
            ((x.min(w - MENU_WIDTH - 4.0)).max(4.0), (y.min(h - height - 4.0)).max(4.0))
        })
    });

    let on_close = Callback::new(move |_: ()| position.set(None));
    let on_pick = Callback::new(move |action: Callback<()>| {
        position.set(None);
        action.run(());
    });

    let escape = window_event_listener(ev::keydown, move |e| {
        if e.key() == "Escape" && position.get_untracked().is_some() {
            position.set(None);
        }
    });
    on_cleanup(move || escape.remove());

    context_menu_view(placement, items, on_close, on_pick)
}
