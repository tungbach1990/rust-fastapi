#![allow(non_snake_case)]
use libc::c_char;
use once_cell::sync::Lazy;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use std::ffi::CString;

// C-ABI symbol used by the dynamic loader to identify the feature
#[cfg(feature = "plugin")]
#[no_mangle]
pub extern "C" fn feature_name_rate_limit() -> *mut c_char {
    CString::new("rate_limit").unwrap().into_raw()
}

// ---- Pure Rust logic below (used by the main app via rlib) ----

static RATE_LIMIT_MAP: Lazy<RwLock<HashMap<String, (Instant, usize)>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

pub fn normalize_path(p: &str) -> String {
    if p == "/" { return "/".to_string(); }
    p.trim_end_matches('/').to_string()
}

// Returns true if request is allowed under the provided limit for the ip+path key
pub fn check_allow(ip: &str, path: &str, limit: usize) -> bool {
    let now = Instant::now();
    let key = format!("{}|{}", ip, normalize_path(path));
    let mut map = RATE_LIMIT_MAP.write();
    let entry = map.entry(key).or_insert((now, 0));
    let elapsed = now.duration_since(entry.0);
    if elapsed >= Duration::from_secs(1) {
        entry.0 = now;
        entry.1 = 0;
    }
    entry.1 += 1;
    entry.1 <= limit
}

// Manifest mô tả UI và schema cấu hình cho plugin, phục vụ Admin UI động
#[cfg(feature = "plugin")]
#[no_mangle]
pub extern "C" fn feature_manifest_rate_limit() -> *mut c_char {
    let json = r#"{
        "name": "rate_limit",
        "settings": [
          {"key": "rps", "type": "number", "label": "Rate Limit (req/s)", "default": 1, "scope": "global"},
          {"key": "route_limits", "type": "route_number_map", "label": "Per-Route Rate Limits", "default": {}}
        ]
    }"#;
    CString::new(json).unwrap().into_raw()
}