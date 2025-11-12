#![allow(non_snake_case)]
#[cfg(feature = "plugin")] use libc::c_char;
#[cfg(feature = "plugin")] use std::ffi::CString;

// C-ABI symbol used by the dynamic loader to identify the feature
#[cfg(feature = "plugin")]
#[no_mangle]
pub extern "C" fn feature_name_waf() -> *mut c_char {
    CString::new("waf").unwrap().into_raw()
}

// ---- Pure Rust logic below (used by the main app via rlib) ----

pub fn is_malicious(uri: &str, user_agent: Option<&str>) -> bool {
    let uri_l = uri.to_lowercase();
    let suspicious = [
        "<script", "%3cscript", "javascript:", "onerror=", "onload=",
        "<img", "<svg", "../", "union select", "select%20", "or 1=1",
        "drop table", "insert%20", "update%20", "delete%20",
    ];
    if suspicious.iter().any(|p| uri_l.contains(p)) { return true; }
    if let Some(ua) = user_agent { if ua.len() > 1024 { return true; } }
    false
}

// Manifest để UI Admin tự động sinh theo code của feature
#[cfg(feature = "plugin")]
#[no_mangle]
pub extern "C" fn feature_manifest_waf() -> *mut c_char {
    let json = r#"{
        "name": "waf",
        "settings": [
          {"key": "patterns", "type": "string_list", "label": "WAF Patterns", "default": []}
        ]
    }"#;
    CString::new(json).unwrap().into_raw()
}