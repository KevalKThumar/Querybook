//! Template for the Welcome screen.
//!
//! Author: Keval Thumar

use leptos::prelude::*;

use crate::shared::common_structure::ThemeMode;

pub fn welcome_view(
    recent: Signal<Vec<(String, String, String)>>,
    theme_mode: Signal<ThemeMode>,
    on_open: Callback<bool>,
    on_open_recent: Callback<String>,
    on_remove_recent: Callback<String>,
    on_theme: Callback<()>,
) -> impl IntoView {
    view! {
        <div class="welcome d-flex align-items-center justify-content-center h-100 position-relative">
            <button
                class="btn btn-sm btn-outline-secondary welcome-theme"
                title="Theme"
                on:click=move |_| on_theme.run(())
            >
                <i class=move || theme_mode.get().icon()></i>
                " "
                {move || theme_mode.get().label()}
            </button>

            <div class="welcome-card">
                <div class="welcome-brand d-flex align-items-center gap-3 mb-4">
                    <i class="bi bi-database-fill-lock"></i>
                    <div>
                        <h1 class="welcome-title mb-0">"Querybook"</h1>
                        <div class="text-body-secondary">"SQLite & SQLCipher browser"</div>
                    </div>
                </div>

                <div class="d-grid gap-2">
                    <button class="btn btn-primary welcome-action" on:click=move |_| on_open.run(false)>
                        <i class="bi bi-folder2-open"></i>
                        <span class="flex-grow-1 text-start">"Open Database…"</span>
                        <kbd>"⌘O"</kbd>
                    </button>
                    <button class="btn btn-outline-secondary welcome-action" on:click=move |_| on_open.run(true)>
                        <i class="bi bi-file-earmark-plus"></i>
                        <span class="flex-grow-1 text-start">"New Database…"</span>
                        <kbd>"⌘N"</kbd>
                    </button>
                </div>

                <Show when=move || !recent.with(|r| r.is_empty()) fallback=|| ()>
                    <div class="welcome-recent mt-4">
                        <div class="welcome-recent-title">"Recent"</div>
                        <For
                            each=move || recent.get()
                            key=|(path, _, _)| path.clone()
                            children=move |(path, name, dir)| {
                                let (open_path, tooltip) = (path.clone(), path.clone());
                                view! {
                                    <div
                                        class="welcome-recent-item d-flex align-items-center gap-2"
                                        title=tooltip
                                        on:click=move |_| on_open_recent.run(open_path.clone())
                                    >
                                        <i class="bi bi-database"></i>
                                        <div class="flex-grow-1 overflow-hidden">
                                            <div class="fw-medium text-truncate">{name}</div>
                                            <div class="small text-body-secondary text-truncate">{dir}</div>
                                        </div>
                                        <button
                                            class="icon-btn"
                                            title="Remove from list"
                                            on:click=move |ev| {
                                                ev.stop_propagation();
                                                on_remove_recent.run(path.clone());
                                            }
                                        >
                                            <i class="bi bi-x"></i>
                                        </button>
                                    </div>
                                }
                            }
                        />
                    </div>
                </Show>
            </div>
        </div>
    }
}
