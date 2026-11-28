//! Template for the About dialog.
//!
//! Author: Keval Thumar

use leptos::prelude::*;
use shared::support::{AppInfo, SupportLink};

use crate::shared::dialogs::dialog_shell::dialog_shell_component::DialogShell;

pub fn about_dialog_view(
    open: RwSignal<bool>,
    info: Signal<Option<AppInfo>>,
    on_close: Callback<()>,
    on_link: Callback<SupportLink>,
    on_copy: Callback<()>,
) -> impl IntoView {
    let field = move |f: fn(&AppInfo) -> String| move || info.get().map(|i| f(&i)).unwrap_or_else(|| "…".into());
    view! {
        <DialogShell open on_close class="about-dialog">
            <div class="shell-body about-body">
                <img class="about-logo" src="/querybook.png" alt="" />
                <div class="about-name">"Querybook"</div>
                <div class="about-version">{field(|i| format!("Version {}", i.version))}</div>
                <div class="about-tagline">"A SQLite & SQLCipher browser. Every database in its own window."</div>
                <dl class="about-facts">
                    <dt>"System"</dt><dd>{field(|i| format!("{} ({})", i.os, i.arch))}</dd>
                    <dt>"WebView"</dt><dd>{field(|i| i.webview.clone())}</dd>
                    <dt>"Logs"</dt><dd class="text-break">{field(|i| i.logs_dir.clone())}</dd>
                </dl>
                <p class="about-privacy">
                    <i class="bi bi-shield-lock"></i>
                    " Your databases never leave this computer. The only network request Querybook makes is the update check, and only if you allow it."
                </p>
                <div class="about-links">
                    <button class="btn btn-link btn-sm" on:click=move |_| on_link.run(SupportLink::Home)>"GitHub"</button>
                    <button class="btn btn-link btn-sm" on:click=move |_| on_link.run(SupportLink::Releases)>"Release notes"</button>
                    <button class="btn btn-link btn-sm" on:click=move |_| on_copy.run(())>"Copy version details"</button>
                </div>
                <div class="about-license">"© Keval Thumar · MIT or Apache-2.0"</div>
            </div>
            <div class="shell-footer">
                <button class="btn btn-sm btn-primary" on:click=move |_| on_close.run(())>"Done"</button>
            </div>
        </DialogShell>
    }
}
