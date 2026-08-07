// Dioxus RSX 在多个业务页面并列时会生成较深的迭代器类型。
#![recursion_limit = "256"]

mod app;
mod components;
mod pages;
mod permissions;
mod router;
mod services;
mod spacetime_bindings;
mod state;

fn main() {
    install_wasm_panic_hook();
    dioxus::launch(app::App);
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
extern "C" {
    #[wasm_bindgen::prelude::wasm_bindgen(js_namespace = console, js_name = warn)]
    fn yizu_console_warn(message: &str);
}

#[cfg(target_arch = "wasm32")]
fn install_wasm_panic_hook() {
    std::panic::set_hook(Box::new(|panic_info| {
        yizu_console_warn(&format!("[YIZU-WASM-PANIC] {panic_info}"));
    }));
}

#[cfg(not(target_arch = "wasm32"))]
fn install_wasm_panic_hook() {}
