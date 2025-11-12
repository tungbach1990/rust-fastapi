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

// Tự động phát hiện tên feature từ thư mục features/
fn discover_feature_names(features_dir: &str) -> Vec<String> {
    let mut names = Vec::new();
    if let Ok(rd) = std::fs::read_dir(features_dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() && p.join("Cargo.toml").exists() {
                if let Some(name) = p.file_name().map(|s| s.to_string_lossy().to_string()) {
                    names.push(name);
                }
            }
        }
    }
    names.sort();
    names
}

pub fn load_features(features_src_dir: &str, build_dir: &str) -> Vec<String> {
    // Tự động phát hiện features từ thư mục source
    let feature_names = discover_feature_names(features_src_dir);
    let settings = load_settings();
    let mut loaded: Vec<String> = Vec::new();
    let build_dir_path = Path::new(build_dir);

    // Thử nạp từng feature đã phát hiện
    for feature_name in feature_names {
        // Bỏ qua nếu bị disable
        if settings.disabled_features.iter().any(|f| f == &feature_name) {
            info!("⏭️ Feature {} bị disable, bỏ qua", feature_name);
            continue;
        }

        // Tìm file DLL/SO tương ứng trong build_dir
        let feature_dir = Path::new(features_src_dir).join(&feature_name);
        let candidates = built_candidates_for(&feature_dir, build_dir_path);

        let lib_path = candidates.into_iter().find(|p| p.exists());
        let Some(p) = lib_path else {
            warn!("⚠️ Không tìm thấy thư viện build cho feature {}", feature_name);
            continue;
        };

        if !p.is_file() { continue; }

        unsafe {
            match Library::new(&p) {
                Ok(lib) => {
                    type RawStr = *mut std::os::raw::c_char;
                    // Thử các tên symbol có thể có
                    let sym_name_generic = format!("feature_name_{}\0", feature_name);
                    let sym_candidates: Vec<&[u8]> = vec![
                        b"feature_name\0",
                        sym_name_generic.as_bytes(),
                    ];

                    let mut found = false;
                    for sym_name in sym_candidates {
                        if let Ok(sym) = lib.get::<unsafe extern "C" fn() -> RawStr>(sym_name) {
                            let ptr = sym();
                            if !ptr.is_null() {
                                let cstr = CStr::from_ptr(ptr);
                                let s = cstr.to_string_lossy().to_string();
                                let _ = CString::from_raw(ptr);
                                info!("🧩 Feature loaded: {} from {:?}", s, p);
                                loaded.push(s.clone());
                                found = true;
                                break;
                            }
                        }
                    }

                    if !found {
                        warn!("⚠️ Feature {} không export symbol feature_name", feature_name);
                    }
                }
                Err(e) => { warn!("⚠️ Lỗi nạp feature {:?}: {}", p, e); }
            }
        }
    }

    loaded.sort();
    loaded
}

// Thu thập manifest UI của các feature plugins để Admin UI tự động sinh
pub fn collect_manifests(build_dir: &str) -> Vec<Value> {
    // Tự động phát hiện features từ thư mục features/
    let feature_names = discover_feature_names("./features");
    let settings = load_settings();
    let mut manifests: Vec<Value> = Vec::new();
    let build_dir_path = Path::new(build_dir);

    for feature_name in feature_names {
        // Bỏ qua nếu bị disable (nhưng vẫn hiển thị trong UI để có thể bật lại)
        // if settings.disabled_features.iter().any(|f| f == &feature_name) { continue; }

        // Tìm file DLL/SO tương ứng
        let feature_dir = Path::new("./features").join(&feature_name);
        let candidates = built_candidates_for(&feature_dir, build_dir_path);

        let lib_path = candidates.into_iter().find(|p| p.exists());
        let Some(p) = lib_path else {
            warn!("⚠️ Không tìm thấy thư viện build cho feature {} (manifest)", feature_name);
            continue;
        };

        if !p.is_file() { continue; }

        unsafe {
            match Library::new(&p) {
                Ok(lib) => {
                    type RawStr = *mut std::os::raw::c_char;
                    let sym_name_generic = format!("feature_manifest_{}\0", feature_name);
                    let sym_candidates: Vec<&[u8]> = vec![
                        b"feature_manifest\0",
                        sym_name_generic.as_bytes(),
                    ];

                    for sym_name in sym_candidates {
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

    manifests
}

// Helper: kiểm tra xem một feature có mặt trong build theo tên thường (vd: "cors")
pub fn has_feature(build_dir: &str, name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    let dir = std::path::Path::new(build_dir);
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            if !p.is_file() { continue; }
            if let Some(fname) = p.file_name().and_then(|s| s.to_str()) {
                let f = fname.to_ascii_lowercase();
                if f.contains(&name) { return true; }
            }
        }
    }
    false
}