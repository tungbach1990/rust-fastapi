use plugin_macro::{def_get, declare_routes};
use serde_json::{json, Value};

#[def_get("/api/hello")]
fn hello() -> Value {
    json!({
        "msg": "Hello from plugin!",
        "time": chrono::Utc::now().to_rfc3339()
    })
}

declare_routes!();
