//! CipherDialog — SQLCipher password / raw key entry.
//!
//! Serves three flows (see `CipherMode`): unlock an encrypted file, optionally
//! encrypt a brand-new file, and set / change / remove the key of the open
//! database. Only SQLCipher 4 defaults are supported; they're shown read-only.
//!
//! Author: Keval Thumar

use leptos::prelude::*;
use shared::KeyInput;

use super::cipher_dialog_view::cipher_dialog_view;
use crate::database::database_structure::CipherMode;

#[component]
pub fn CipherDialog(
    open: RwSignal<bool>,
    #[prop(into)] mode: Signal<CipherMode>,
    #[prop(into)] file_name: Signal<String>,
    /// Inline error (wrong key, mismatch, …) — owned by the caller so it can set backend errors.
    error: RwSignal<Option<String>>,
    #[prop(into)] busy: Signal<bool>,
    /// `Some(key)` to unlock / encrypt; `None` = "don't encrypt" (Create) or "remove encryption" (Change).
    on_submit: Callback<Option<KeyInput>>,
    on_cancel: Callback<()>,
) -> impl IntoView {
    let password = RwSignal::new(String::new());
    let confirm = RwSignal::new(String::new());
    let raw_key = RwSignal::new(false);

    // Fresh fields every time the dialog opens.
    Effect::new(move |_| {
        if open.get() {
            password.set(String::new());
            confirm.set(String::new());
            raw_key.set(false);
        }
    });

    // A new passphrase is typed twice; unlocking and raw keys aren't.
    let needs_confirm = Signal::derive(move || !raw_key.get() && mode.get() != CipherMode::Unlock);
    let title = Signal::derive(move || match mode.get() {
        CipherMode::Unlock => format!("Unlock {}", file_name.get()),
        CipherMode::Create => "Encrypt the new database?".to_string(),
        CipherMode::Change { encrypted: true } => "Change encryption".to_string(),
        CipherMode::Change { encrypted: false } => "Encrypt database".to_string(),
    });
    let intro = Signal::derive(move || match mode.get() {
        CipherMode::Unlock => "This database is encrypted. Enter the key used to encrypt it.",
        CipherMode::Create => "Set a password to create it as a SQLCipher database, or continue without encryption.",
        CipherMode::Change { encrypted: true } => "Enter a new key, or remove encryption. The database is rewritten to a verified copy first.",
        CipherMode::Change { encrypted: false } => "Enter a key to encrypt this database. It is rewritten to a verified copy first.",
    });

    let on_confirm = Callback::new(move |_: ()| {
        let secret = password.get_untracked();
        let key = if raw_key.get_untracked() {
            KeyInput::RawHex(secret.trim().to_string())
        } else {
            KeyInput::Passphrase(secret.clone())
        };
        if let Err(message) = key.validate() {
            error.set(Some(message));
            return;
        }
        if needs_confirm.get_untracked() && confirm.get_untracked() != secret {
            error.set(Some("The passwords don't match.".to_string()));
            return;
        }
        error.set(None);
        on_submit.run(Some(key));
    });
    let on_without_key = Callback::new(move |_: ()| {
        error.set(None);
        on_submit.run(None);
    });

    cipher_dialog_view(
        open,
        mode,
        title,
        intro,
        password,
        confirm,
        raw_key,
        needs_confirm,
        error.into(),
        busy,
        on_confirm,
        on_without_key,
        on_cancel,
    )
}
