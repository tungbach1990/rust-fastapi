use plugin_macro::{def_get, declare_routes};

#[def_get("/assets/app.js")]
fn js_asset() -> String {
    format!("js:{}", module_utils::read_asset!("src/app.js"))
}

declare_routes!();