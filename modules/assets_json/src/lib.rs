#![allow(non_snake_case)]
use plugin_macro::{def_get, declare_routes};
use serde_json::Value;

#[def_get("/assets/data.json")]
fn json_asset() -> Value {
    let text = module_utils::read_asset!("src/data.json");
    serde_json::from_str(&text).unwrap_or_else(|_| serde_json::json!({"error":"invalid json"}))
}

declare_routes!();