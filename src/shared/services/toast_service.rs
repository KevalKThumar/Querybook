//! ToastService
//!
//! Root service driving the globally mounted `<ToastContainer/>`.
//! Holds a reactive list of in-flight toasts; each one auto-dismisses
//! (errors stay a little longer so SQL messages can be read).
//!
//! Usage (in a component):
//! ```
//! let toast = expect_context::<ToastService>();
//! toast.success("Saved!");
//! ```
//!
//! Author: Keval Thumar

use gloo_timers::future::sleep;
use leptos::prelude::*;
use leptos::task::spawn_local;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

static NEXT_ID: AtomicU32 = AtomicU32::new(1);

/// Visual kind of a toast — controls colour and icon.
#[derive(Clone, PartialEq, Debug)]
pub enum ToastKind {
    Success,
    Error,
}

/// A single toast entry held in the service's reactive list.
#[derive(Clone, PartialEq, Debug)]
pub struct Toast {
    pub id: u32,
    pub message: String,
    pub kind: ToastKind,
}

/// Global service providing reactive toast state.
/// `Copy` so it can be passed into `move` closures freely.
#[derive(Clone, Copy)]
pub struct ToastService {
    toasts: RwSignal<Vec<Toast>>,
}

impl Default for ToastService {
    fn default() -> Self {
        Self::new()
    }
}

impl ToastService {
    pub fn new() -> Self {
        Self { toasts: RwSignal::new(vec![]) }
    }

    /// Show a green success toast.
    pub fn success(&self, msg: &str) {
        self.push(msg, ToastKind::Success, 3_000);
    }

    /// Show a red error toast.
    pub fn error(&self, msg: &str) {
        self.push(msg, ToastKind::Error, 6_000);
    }

    /// Immediately remove a toast by id (e.g. user clicked ×).
    pub fn dismiss(&self, id: u32) {
        self.toasts.update(|list| list.retain(|t| t.id != id));
    }

    /// Reactive read-only view of the current toast list for the container component.
    pub fn toasts(&self) -> Signal<Vec<Toast>> {
        Signal::from(self.toasts)
    }

    fn push(&self, msg: &str, kind: ToastKind, millis: u64) {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let toasts = self.toasts;
        toasts.update(|list| list.push(Toast { id, message: msg.to_string(), kind }));
        spawn_local(async move {
            sleep(Duration::from_millis(millis)).await;
            toasts.update(|list| list.retain(|t| t.id != id));
        });
    }
}
