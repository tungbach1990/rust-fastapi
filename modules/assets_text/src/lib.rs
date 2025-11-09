#![allow(non_snake_case)]
use plugin_macro::{def_get, declare_routes};

#[def_get("/assets/text")]
fn text_asset() -> String {
    format!("text:{}", module_utils::read_asset!("src/assets.txt"))
}

declare_routes!();