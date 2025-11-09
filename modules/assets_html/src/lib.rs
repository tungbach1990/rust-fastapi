use plugin_macro::{def_get, declare_routes};

#[def_get("/assets/index.html")]
fn html_asset() -> String {
    format!("html:{}", module_utils::read_asset!("src/index.html"))
}

declare_routes!();