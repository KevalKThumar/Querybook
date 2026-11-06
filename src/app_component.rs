use leptos::prelude::*;

use crate::{
    app_route::AppRoutes,
    shared::common_structure::ThemeService,
    shared::components::toast::toast_component::ToastContainer,
    shared::services::toast_service::ToastService,
};

#[component]
pub fn App() -> impl IntoView {
    // Root service for toast notifications.
    provide_context(ToastService::new());
    // Root service for the System / Light / Dark theme (shared by all windows).
    provide_context(ThemeService::new());

    view! {
        <ToastContainer />
        <AppRoutes />
    }
}
