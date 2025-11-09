#![allow(non_snake_case)]
use plugin_macro::{def_get, declare_routes};

#[def_get("/assets/data.xml")]
fn xml_asset() -> String {
    format!("xml:{}", module_utils::read_asset!("src/data.xml"))
}

declare_routes!();