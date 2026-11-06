//! Global toast container — mount once in `App`, reads from `ToastService` context.
//!
//! Author: Keval Thumar

use leptos::prelude::*;

use super::toast_view::toast_view;
use crate::shared::services::toast_service::ToastService;

#[component]
pub fn ToastContainer() -> impl IntoView {
    let service = expect_context::<ToastService>();
    let on_dismiss = Callback::new(move |id: u32| service.dismiss(id));
    toast_view(service.toasts(), on_dismiss)
}
