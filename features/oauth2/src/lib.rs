#![allow(non_snake_case)]
#[cfg(feature = "plugin")] use libc::c_char;
#[cfg(feature = "plugin")] use std::ffi::CString;

// C-ABI symbol used by the dynamic loader to identify the feature
#[cfg(feature = "plugin")]
#[no_mangle]
pub extern "C" fn feature_name_oauth2() -> *mut c_char {
    CString::new("oauth2").unwrap().into_raw()
}

// ---- Pure Rust logic below (used by the main app via rlib) ----

fn normalize(p: &str) -> String {
    if p == "/" { return "/".to_string(); }
    p.trim_end_matches('/').to_string()
}

pub fn requires_auth(protected_routes: &[String], path: &str) -> bool {
    if protected_routes.is_empty() { return false; }
    let path_norm = normalize(path);
    protected_routes.iter().any(|item| {
        let item_str = item.as_str();
        if let Some(prefix) = item_str.strip_suffix("/*") {
            let prefix_norm = normalize(prefix);
            path_norm.starts_with(&prefix_norm)
        } else {
            normalize(item_str) == path_norm
        }
    })
}

pub fn has_bearer(auth_header: Option<&str>) -> bool {
    match auth_header {
        Some(v) => v.to_lowercase().starts_with("bearer "),
        None => false,
    }
}

// Manifest để UI Admin tự động sinh toggle theo code của feature
#[cfg(feature = "plugin")]
#[no_mangle]
pub extern "C" fn feature_manifest_oauth2() -> *mut c_char {
    let json = r#"{
        "name": "oauth2",
        "settings": [
          {"key": "protected_routes", "type": "route_list", "label": "OAuth2 Protected Routes", "default": []}
        ]
    }"#;
    CString::new(json).unwrap().into_raw()
}