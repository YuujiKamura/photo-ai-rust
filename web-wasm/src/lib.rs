//! Photo AI Web App (Leptos + WASM)

mod app;
mod components;
mod api;
mod export;
mod secure_store;

use wasm_bindgen::prelude::*;
use leptos::prelude::*;

#[cfg(not(test))]
#[wasm_bindgen(start)]
pub fn main() {
    console_error_panic_hook::set_once();
    leptos::mount::mount_to_body(app::App);
}
