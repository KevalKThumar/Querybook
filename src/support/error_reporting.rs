//! Sends a window's failures to the backend as crash reports.
//!
//! The release build aborts on a panic, and after that the WebAssembly module
//! can't run any more code. So the listeners for script errors are plain
//! JavaScript, and the panic hook hands its report to `invoke` synchronously,
//! before the abort. The backend then offers Reload Window, which keeps the
//! window's database session and its pending changes.
//!
//! Author: Keval Thumar

use wasm_bindgen::prelude::*;

#[wasm_bindgen(inline_js = r#"
let sent = 0, fatalSent = false;
const seen = new Set();
function send(kind, message, stack, loc, fatal) {
    try {
        if (fatalSent && !fatal) return;              // the trap that follows a panic
        const key = kind + "|" + message;
        if (!fatal && (seen.has(key) || sent >= 5)) return;
        seen.add(key); sent++;
        if (fatal) fatalSent = true;
        window.__TAURI__.core.invoke("report_frontend_error", { report: {
            kind, fatal,
            message: String(message || "Unknown error").slice(0, 2000),
            stack: String(stack || "").slice(0, 8000),
            location: loc || null,
            route: window.location.pathname,
        }});
    } catch (_) {}
}
export function __install_error_reporting() {
    window.addEventListener("error", (e) => {
        const loc = e.filename ? `${e.filename}:${e.lineno}:${e.colno}` : null;
        send("FrontendError", e.message || "Script error", e.error && e.error.stack, loc, false);
    });
    window.addEventListener("unhandledrejection", (e) => {
        const r = e.reason;
        send("FrontendError", (r && r.message) || String(r), r && r.stack, null, false);
    });
}
export function __report_panic(message, loc) {
    send("FrontendPanic", message, new Error().stack, loc, true);
}
"#)]
extern "C" {
    fn __install_error_reporting();
    fn __report_panic(message: &str, loc: Option<String>);
}

/// Call first in `mount_app`.
pub fn install() {
    __install_error_reporting();
    std::panic::set_hook(Box::new(|info| {
        // Still shown in the WebView's console for development.
        console_error_panic_hook::hook(info);
        let message = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "panic".into());
        let loc = info.location().map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()));
        __report_panic(&message, loc);
    }));
}
