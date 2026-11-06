//! Querybook UI — Leptos CSR app loaded into every Tauri window by Trunk.
//!
//! There is no `main.rs`: this crate is a `cdylib`, and wasm-bindgen runs
//! [`mount_app`] as soon as Trunk's loader instantiates the module.
//!
//! Author: Keval Thumar

mod app_component;
mod app_route;
mod app_view;
mod database;
mod shared;

use leptos::prelude::*;
use wasm_bindgen::prelude::wasm_bindgen;

use app_component::App;

#[wasm_bindgen(start)]
pub fn mount_app() {
    // Surface Rust panics in the webview console.
    console_error_panic_hook::set_once();
    leptos::mount::mount_to_body(|| view! { <App /> });
}
