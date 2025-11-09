use std::path::Path;
use std::ffi::{CStr, CString};
use libloading::Library;
use tracing::{info, warn};
use admin::load_settings;
use serde_json::Value;

fn read_package_name(dir: &Path) -> Option<String> {
    let cargo_toml = dir.join("Cargo.toml");
    let content = std::fs::read_to_string(cargo_toml).ok()?;
    let mut in_package = false;
    for line in content.lines() {
        let l = line.trim();
        if l.starts_with('[') {
            in_package = l == "[package]";
            continue;
        }
        if in_package && l.starts_with("name") {
            let parts: Vec<&str> = l.split('=').collect();
            if parts.len() >= 2 {
                let val = parts[1].trim().trim_matches('"');
                if !val.is_empty() { return Some(val.to_string()); }
            }
        }
    }
    None
}

fn built_candidates_for(feature_dir: &Path, build_dir: &Path) -> Vec<std::path::PathBuf> {
    let dir_name = feature_dir.file_name().unwrap().to_string_lossy().to_string();
    let pkg_name = read_package_name(feature_dir).unwrap_or_else(|| dir_name.clone());
    vec![
        build_dir.join(format!("lib{}.so", pkg_name)),
        build_dir.join(format!("{}.dll", pkg_name)),
        build_dir.join(format!("lib{}.dll", pkg_name)),
        build_dir.join(format!("lib{}.so", dir_name.clone())),
        build_dir.join(format!("{}.dll", dir_name.clone())),
        build_dir.join(format!("lib{}.dll", dir_name.clone())),
    ]
}

pub fn load_features(_features_src_dir: &str, build_dir: &str) -> Vec<String> {
    // Chỉ nạp feature plugins từ thư mục build, không phụ thuộc vào thư mục source `features/*`
    let settings = load_settings();
    let mut loaded: Vec<String> = Vec::new();
    let build_dir_path = Path::new(build_dir);

    if let Ok(rd) = std::fs::read_dir(build_dir_path) {
        for e in rd.flatten() {
            let p = e.path();
            let fname = p.file_name().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
            // Chỉ thử nạp các thư viện có khả năng là feature: tên chứa oauth2, rate_limit, waf
            if !(fname.contains("oauth2") || fname.contains("rate_limit") || fname.contains("waf")) { continue; }
            if !p.is_file() { continue; }

            // Bỏ qua nếu bị disable trong settings theo tên phổ biến
            let name_hint = if fname.contains("oauth2") { "oauth2" } else if fname.contains("rate_limit") { "rate_limit" } else if fname.contains("waf") { "waf" } else { "" };
            if !name_hint.is_empty() && settings.disabled_features.iter().any(|f| f == name_hint) { continue; }

            unsafe {
                match Library::new(&p) {
                    Ok(lib) => {
                        type RawStr = *mut std::os::raw::c_char;
                        // Try generic symbol name first; fallback to crate-specific names
                        let sym_candidates: &[&[u8]] = &[
                            b"feature_name\0",
                            if fname.contains("oauth2") { b"feature_name_oauth2\0" } else { b"_\0" },
                            if fname.contains("rate_limit") { b"feature_name_rate_limit\0" } else { b"_\0" },
                            if fname.contains("waf") { b"feature_name_waf\0" } else { b"_\0" },
                        ];
                        for &sym_name in sym_candidates {
                            if sym_name == b"_\0" { continue; }
                            if let Ok(sym) = lib.get::<unsafe extern "C" fn() -> RawStr>(sym_name) {
                                let ptr = sym();
                                if !ptr.is_null() {
                                    let cstr = CStr::from_ptr(ptr);
                                    let s = cstr.to_string_lossy().to_string();
                                    let _ = CString::from_raw(ptr);
                                    info!("🧩 Feature loaded: {} from {:?}", s, p);
                                    loaded.push(s.clone());
                                    break;
                                }
                            }
                        }
                    }
                    Err(e) => { warn!("⚠️ Lỗi nạp feature {:?}: {}", p, e); }
                }
            }
        }
    }

    loaded.sort();
    loaded
}

// Thu thập manifest UI của các feature plugins để Admin UI tự động sinh
pub fn collect_manifests(build_dir: &str) -> Vec<Value> {
    let settings = load_settings();
    let mut manifests: Vec<Value> = Vec::new();
    let build_dir_path = Path::new(build_dir);

    if let Ok(rd) = std::fs::read_dir(build_dir_path) {
        for e in rd.flatten() {
            let p = e.path();
            let fname = p.file_name().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
            if !(fname.contains("oauth2") || fname.contains("rate_limit") || fname.contains("waf")) { continue; }
            if !p.is_file() { continue; }

            let name_hint = if fname.contains("oauth2") { "oauth2" } else if fname.contains("rate_limit") { "rate_limit" } else if fname.contains("waf") { "waf" } else { "" };
            if !name_hint.is_empty() && settings.disabled_features.iter().any(|f| f == name_hint) { continue; }

            unsafe {
                match Library::new(&p) {
                    Ok(lib) => {
                        type RawStr = *mut std::os::raw::c_char;
                        let sym_candidates: &[&[u8]] = &[
                            b"feature_manifest\0",
                            if fname.contains("oauth2") { b"feature_manifest_oauth2\0" } else { b"_\0" },
                            if fname.contains("rate_limit") { b"feature_manifest_rate_limit\0" } else { b"_\0" },
                            if fname.contains("waf") { b"feature_manifest_waf\0" } else { b"_\0" },
                        ];
                        for &sym_name in sym_candidates {
                            if sym_name == b"_\0" { continue; }
                            if let Ok(sym) = lib.get::<unsafe extern "C" fn() -> RawStr>(sym_name) {
                                let ptr = sym();
                                if !ptr.is_null() {
                                    let cstr = CStr::from_ptr(ptr);
                                    let s = cstr.to_string_lossy().to_string();
                                    let _ = CString::from_raw(ptr);
                                    if let Ok(v) = serde_json::from_str::<Value>(&s) {
                                        manifests.push(v);
                                    } else {
                                        warn!("⚠️ feature_manifest JSON invalid in {:?}", p);
                                    }
                                    let _keep_alive: &'static Library = Box::leak(Box::new(lib));
                                    break;
                                }
                            }
                        }
                    }
                    Err(e) => { warn!("⚠️ Lỗi nạp feature {:?}: {}", p, e); }
                }
            }
        }
    }

    manifests
}