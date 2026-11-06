use leptos::prelude::*;
use leptos_router::components::{ParentRoute, Redirect, Route, Router, Routes};
use leptos_router::path;

use crate::app_view::{AppShell, RedirectToBrowser};
use crate::database::database_constant::ROUTE_BROWSER;
use crate::database::database_routes::DatabaseRoutes;

#[component]
pub fn AppRoutes() -> impl IntoView {
    view! {
        // Every window loads index.html; the browser feature sends empty windows to the welcome screen.
        <Router>
            <Routes fallback=|| view! { <RedirectToBrowser /> }>
                <ParentRoute path=path!("") view=AppShell>
                    <Route path=path!("") view=|| view! { <Redirect path=ROUTE_BROWSER /> } />
                    <DatabaseRoutes />
                </ParentRoute>
            </Routes>
        </Router>
    }
}
