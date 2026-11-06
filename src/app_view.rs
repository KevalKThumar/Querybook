use leptos::prelude::*;
use leptos_router::{components::Outlet, hooks::use_navigate, NavigateOptions};

use crate::database::database_constant::ROUTE_BROWSER;

#[component]
pub fn AppShell() -> impl IntoView {
    view! {
        <div class="app d-flex flex-column vh-100 overflow-hidden">
            <Outlet />
        </div>
    }
}

/// Fallback for unmatched routes (a new window loads `/index.html`). Navigates
/// from inside an `Effect` (post-render) so we never call `navigate()`
/// synchronously during the router's own location update.
#[component]
pub fn RedirectToBrowser() -> impl IntoView {
    let navigate = use_navigate();
    Effect::new(move |_| {
        navigate(ROUTE_BROWSER, NavigateOptions::default());
    });
}
