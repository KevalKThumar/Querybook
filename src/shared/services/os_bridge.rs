//! Thin wasm_bindgen bridge to Tauri's JS `invoke` (needs `withGlobalTauri`).
//!
//! Errors come back as the backend's `AppError`, so callers can branch on
//! its `kind` (e.g. NeedsKey / WrongKey). Components never call this directly;
//! they go through a module service.

use serde::Serialize;
use serde::de::DeserializeOwned;
use shared::{AppError, ErrorKind};
use wasm_bindgen::prelude::*;

#[wasm_bindgen(inline_js = r#"
export async function __invoke(cmd, args) {
    return await window.__TAURI__.core.invoke(cmd, args);
}
export function __listen(event, callback) {
    window.__TAURI__.event.listen(event, (e) => callback(e.payload));
}
"#)]
extern "C" {
    #[wasm_bindgen(catch)]
    async fn __invoke(cmd: &str, args: JsValue) -> Result<JsValue, JsValue>;
    fn __listen(event: &str, callback: &Closure<dyn FnMut(JsValue)>);
}

/// Command args for commands that take none.
#[derive(Serialize)]
pub struct NoArgs {}

/// Static OS-command bridge: unit struct, static method.
pub struct OsBridge;

impl OsBridge {
    pub async fn invoke<A: Serialize, R: DeserializeOwned>(cmd: &str, args: &A) -> Result<R, AppError> {
        // json_compatible: structs/maps become plain objects and None becomes null, as Tauri expects.
        let js_args = args
            .serialize(&serde_wasm_bindgen::Serializer::json_compatible())
            .map_err(|e| bridge_error(e.to_string()))?;
        match __invoke(cmd, js_args).await {
            Ok(out) => serde_wasm_bindgen::from_value(out).map_err(|e| bridge_error(format!("{cmd}: {e}"))),
            Err(e) => Err(serde_wasm_bindgen::from_value::<AppError>(e.clone())
                .unwrap_or_else(|_| bridge_error(e.as_string().unwrap_or_else(|| format!("{cmd}: {e:?}"))))),
        }
    }

    /// Run `on_event` with each payload of a backend event, for the window's lifetime.
    pub fn listen<T: DeserializeOwned + 'static>(event: &str, mut on_event: impl FnMut(T) + 'static) {
        let callback = Closure::<dyn FnMut(JsValue)>::new(move |payload: JsValue| {
            if let Ok(value) = serde_wasm_bindgen::from_value(payload) {
                on_event(value);
            }
        });
        __listen(event, &callback);
        // ponytail: window-lifetime listener, intentionally never removed.
        callback.forget();
    }
}

fn bridge_error(message: String) -> AppError {
    AppError::new(ErrorKind::Invalid, message)
}
