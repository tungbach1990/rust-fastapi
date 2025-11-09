use plugin_macro::{def_get, declare_routes};

#[def_get("/assets/app.css")]
fn css_asset() -> String {
    format!("css:{}", module_utils::read_asset!("src/app.css"))
}

declare_routes!();