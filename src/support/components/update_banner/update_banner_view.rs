//! Template for the update card.
//!
//! Author: Keval Thumar

use leptos::prelude::*;

pub fn update_banner_view(
    ask: Signal<bool>,
    available: Signal<Option<String>>,
    on_allow: Callback<()>,
    on_deny: Callback<()>,
    on_details: Callback<()>,
    on_later: Callback<()>,
) -> impl IntoView {
    view! {
        <Show when=move || ask.get() fallback=|| ()>
            <div class="update-card" role="dialog" aria-label="Automatic updates">
                <div class="update-card-title"><i class="bi bi-arrow-repeat"></i>" Check for updates automatically?"</div>
                <div class="update-card-text">
                    "Once a day Querybook asks GitHub whether a newer version exists. "
                    "Nothing about you or your databases is sent. You can change this in Settings."
                </div>
                <div class="update-card-actions">
                    <button class="btn btn-sm btn-outline-secondary" on:click=move |_| on_deny.run(())>"Don't Check"</button>
                    <button class="btn btn-sm btn-primary" on:click=move |_| on_allow.run(())>"Check Automatically"</button>
                </div>
            </div>
        </Show>
        <Show when=move || !ask.get() && available.get().is_some() fallback=|| ()>
            <div class="update-card" role="status">
                <div class="update-card-title">
                    <i class="bi bi-gift"></i>
                    {move || format!(" Querybook {} is available", available.get().unwrap_or_default())}
                </div>
                <div class="update-card-actions">
                    <button class="btn btn-sm btn-outline-secondary" on:click=move |_| on_later.run(())>"Later"</button>
                    <button class="btn btn-sm btn-primary" on:click=move |_| on_details.run(())>"What's New…"</button>
                </div>
            </div>
        </Show>
    }
}
