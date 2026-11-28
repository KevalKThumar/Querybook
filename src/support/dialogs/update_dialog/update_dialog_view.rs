//! Template for the update dialog.
//!
//! Author: Keval Thumar

use leptos::prelude::*;
use shared::support::{UpdateProgress, UpdateStatus};

use crate::shared::dialogs::dialog_shell::dialog_shell_component::DialogShell;

fn progress_text(p: &UpdateProgress) -> (String, Option<f64>) {
    match p {
        UpdateProgress::Downloading { downloaded, total: Some(total) } if *total > 0 => (
            format!("Downloading… {:.1} of {:.1} MB", *downloaded as f64 / 1e6, *total as f64 / 1e6),
            Some((*downloaded as f64 / *total as f64 * 100.0).min(100.0)),
        ),
        UpdateProgress::Downloading { downloaded, .. } => (format!("Downloading… {:.1} MB", *downloaded as f64 / 1e6), None),
        UpdateProgress::Installing => ("Installing… Querybook will restart.".into(), Some(100.0)),
        UpdateProgress::Failed => ("The update couldn't be installed.".into(), None),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn update_dialog_view(
    open: RwSignal<bool>,
    status: Signal<UpdateStatus>,
    progress: Signal<Option<UpdateProgress>>,
    busy: Signal<bool>,
    on_close: Callback<()>,
    on_install: Callback<()>,
    on_later: Callback<()>,
    on_skip: Callback<()>,
    on_download: Callback<()>,
) -> impl IntoView {
    let info = move || status.get().available;
    let can_install = move || status.with(|s| s.can_install);
    view! {
        <DialogShell open on_close class="update-dialog">
            <div class="shell-header d-flex align-items-center gap-2">
                <i class="bi bi-gift update-dialog-icon"></i>
                {move || info().map(|i| format!("Querybook {} is available", i.version)).unwrap_or_else(|| "Update".into())}
            </div>
            <div class="shell-body">
                <div class="update-dialog-versions">
                    {move || info().map(|i| format!("You have {}. ", i.current_version)).unwrap_or_default()}
                    {move || info().and_then(|i| i.date).map(|d| format!("Released {}.", d.get(..10).unwrap_or(&d))).unwrap_or_default()}
                </div>
                <div class="update-dialog-section">"What's new"</div>
                <pre class="update-dialog-notes">{move || info().map(|i| i.notes).filter(|n| !n.trim().is_empty()).unwrap_or_else(|| "See the release notes on GitHub.".into())}</pre>
                <Show
                    when=move || can_install()
                    fallback=move || view! {
                        <p class="update-dialog-hint mb-0">
                            {move || format!(
                                "This copy was installed as a {} package: update it with your package manager (apt, dnf, yay…), or download the new version.",
                                status.with(|s| s.install_kind.clone())
                            )}
                        </p>
                    }
                >
                    <p class="update-dialog-hint mb-0">
                        "Querybook downloads the update, installs it and restarts. Write or revert any unsaved changes first."
                    </p>
                </Show>
                {move || progress.get().map(|p| {
                    let (text, pct) = progress_text(&p);
                    view! {
                        <div class="update-dialog-progress">
                            <div class="progress" role="progressbar" aria-label="Update progress">
                                <div
                                    class="progress-bar"
                                    class:progress-bar-striped=pct.is_none()
                                    class:progress-bar-animated=pct.is_none()
                                    style:width=format!("{}%", pct.unwrap_or(100.0))
                                ></div>
                            </div>
                            <div class="update-dialog-progress-text">{text}</div>
                        </div>
                    }
                })}
            </div>
            <div class="shell-footer">
                <button class="btn btn-sm btn-link me-auto" disabled=busy on:click=move |_| on_skip.run(())>"Skip This Version"</button>
                <button class="btn btn-sm btn-outline-secondary" disabled=busy on:click=move |_| on_later.run(())>"Later"</button>
                <Show
                    when=move || can_install()
                    fallback=move || view! {
                        <button class="btn btn-sm btn-primary" on:click=move |_| on_download.run(())>
                            <i class="bi bi-box-arrow-up-right"></i>" Download"
                        </button>
                    }
                >
                    <button class="btn btn-sm btn-primary" disabled=busy on:click=move |_| on_install.run(())>
                        <i class="bi bi-download"></i>" Update Now"
                    </button>
                </Show>
            </div>
        </DialogShell>
    }
}
