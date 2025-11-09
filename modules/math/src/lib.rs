use plugin_macro::{def_get, declare_routes};
use serde_json::{json, Value};

#[def_get("/ping")]
fn ping() -> Value {
    json!({ "pong": true })
}

declare_routes!();
