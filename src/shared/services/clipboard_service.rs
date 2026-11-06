//! ClipboardService
//!
//! Writes text to the system clipboard with `navigator.clipboard.writeText`,
//! falling back to `execCommand("copy")` on a hidden textarea. Call it from a
//! click / keydown handler (the browser requires a user gesture).
//!
//! ```
//! spawn_local(async move { ClipboardService::write_text(&text).await; });
//! ```
//!
//! Author: Keval Thumar

use wasm_bindgen::prelude::*;

#[wasm_bindgen(inline_js = r#"
export async function __write_text(text) {
    try { await navigator.clipboard.writeText(text); return true; } catch (_) {}
    const ta = document.createElement("textarea");
    ta.value = text;
    ta.style.position = "fixed";
    ta.style.opacity = "0";
    document.body.appendChild(ta);
    ta.select();
    const ok = document.execCommand("copy");
    ta.remove();
    return ok;
}
"#)]
extern "C" {
    async fn __write_text(text: &str) -> JsValue;
}

pub struct ClipboardService;

impl ClipboardService {
    /// True if the text reached the clipboard.
    pub async fn write_text(text: &str) -> bool {
        __write_text(text).await.as_bool().unwrap_or(false)
    }
}
