use leptos::prelude::*;
use leptos_router::components::{Outlet, ParentRoute, Route};
use leptos_router::path;

use crate::database::browser::browser_component::Browser;
use crate::database::database_structure::DatabaseService;
use crate::database::welcome::welcome_component::Welcome;

#[component(transparent)]
pub fn DatabaseRoutes() -> impl leptos_router::MatchNestedRoutes + Clone {
    view! {
        <ParentRoute path=path!("database") view=DatabaseShell>
            <Route path=path!("browser") view=Browser />
            <Route path=path!("welcome") view=Welcome />
        </ParentRoute>
    }
    .into_inner()
}

#[component]
pub fn DatabaseShell() -> impl IntoView {
    provide_context(DatabaseService::new());
    view! { <Outlet /> }
}
