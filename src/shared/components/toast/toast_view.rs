//! Template for the toast stack — fixed bottom-right cards with icon, message and ×.
//!
//! Author: Keval Thumar

use leptos::prelude::*;

use crate::shared::services::toast_service::{Toast, ToastKind};

pub fn toast_view(toasts: Signal<Vec<Toast>>, on_dismiss: Callback<u32>) -> impl IntoView {
    view! {
        <div class="toast-stack">
            <For
                each=move || toasts.get()
                key=|t| t.id
                children=move |t| {
                    let id = t.id;
                    let (kind_class, icon_class) = match t.kind {
                        ToastKind::Success => ("toast-card toast-card--success", "bi bi-check-circle-fill"),
                        ToastKind::Error => ("toast-card toast-card--error", "bi bi-x-circle-fill"),
                    };
                    view! {
                        <div class=kind_class>
                            <i class=icon_class></i>
                            <span class="toast-message">{t.message.clone()}</span>
                            <button class="icon-btn" title="Dismiss" on:click=move |_| on_dismiss.run(id)>
                                <i class="bi bi-x"></i>
                            </button>
                        </div>
                    }
                }
            />
        </div>
    }
}
