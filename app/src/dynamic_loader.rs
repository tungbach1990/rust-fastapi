use std::collections::HashMap;
use libloading::Library;
use admin::load_settings;
use std::path::Path;
use walkdir::WalkDir;
use crate::types::{MethodSet, RawHandler, RawHandlerWithBody, RawRoutePath, RawStr};
use crate::util::path_to_route;
use tracing::{info, warn};
use serde_json::Value;

pub struct DynamicModules {
    pub routes: HashMap<String, MethodSet>,
}

impl DynamicModules {
    pub fn load(base: &str) -> Self {
        let mut routes = HashMap::new();
        let settings = load_settings();

        // Map from built lib stem/package name -> module folder name
        let mut name_to_folder: HashMap<String, String> = HashMap::new();
        if let Ok(rd) = std::fs::read_dir("./modules") {
            for e in rd.flatten() {
                let dir = e.path();
                if !dir.join("Cargo.toml").exists() { continue; }
                let folder = dir.file_name().unwrap().to_string_lossy().to_string();
                if let Some(pkg) = read_package_name(&dir) { name_to_folder.insert(pkg.clone(), folder.clone()); }
                name_to_folder.insert(folder.clone(), folder);
            }
        }

        for entry in WalkDir::new(base).min_depth(1).into_iter().flatten() {
            let p = entry.path();
            // Kiểm tra cả file .so (Linux) và .dll (Windows)
            if p.extension().map(|e| e == "so" || e == "dll").unwrap_or(false) {
                // Skip disabled modules by folder name (resolve from stem or package)
                if let Some(stem) = p.file_stem().map(|s| s.to_string_lossy().to_string()) {
                    let folder = name_to_folder.get(&stem).cloned().unwrap_or(stem);
                    if settings.disabled_modules.iter().any(|m| m == &folder) { continue; }
                }
                // Ưu tiên dùng manifest nhiều route nếu có; fallback sang single-route kiểu cũ
                unsafe {
                    match Library::new(p) {
                        Ok(lib) => {
                            // Try routes_manifest first
                            let manifest_json: Option<String> = lib
                                .get::<RawStr>(b"routes_manifest")
                                .ok()
                                .map(|sym| {
                                    let ptr = (*sym)();
                                    let s = std::ffi::CString::from_raw(ptr);
                                    s.to_string_lossy().to_string()
                                });

                            if let Some(mjson) = manifest_json {
                                info!("📋 Manifest JSON: {}", mjson);
                                match serde_json::from_str::<Value>(&mjson) {
                                    Ok(Value::Array(items)) => {
                                        // Group routes by path and accumulate methods
                                        let mut path_methods: HashMap<String, MethodSet> = HashMap::new();
                                        
                                        for item in items {
                                            let path = item.get("path").and_then(|v| v.as_str()).unwrap_or("");
                                            let method = item.get("method").and_then(|v| v.as_str()).unwrap_or("get");
                                            // Map method to the appropriate manifest key name
                                            let sym_key = match method {
                                                "get" => "get",
                                                "post" => "post_bytes",
                                                "put" => "put_bytes",
                                                "delete" => "delete",
                                                _ => "get",
                                            };
                                            let handler_sym = item.get(sym_key).and_then(|v| v.as_str());
                                            if path.is_empty() || handler_sym.is_none() { continue; }
                                            let handler_sym = handler_sym.unwrap();

                                            // Load handler by symbol name
                                            let sym_bytes = handler_sym.as_bytes().to_vec();
                                            
                                            // Get or create MethodSet for this path
                                            let method_set = path_methods.entry(path.to_string()).or_insert(MethodSet {
                                                get: None,
                                                post: None,
                                                put: None,
                                                delete: None,
                                            });
                                            
                                            // Load the appropriate handler based on method
                                            match method {
                                                "get" => {
                                                    if let Ok(sym) = lib.get::<RawHandler>(&sym_bytes) {
                                                        method_set.get = Some(*sym);
                                                        info!("🧩 Loaded {} ({}) - method: GET", path, handler_sym);
                                                    }
                                                },
                                                "post" => {
                                                    if let Ok(sym) = lib.get::<RawHandlerWithBody>(&sym_bytes) {
                                                        method_set.post = Some(*sym);
                                                        info!("🧩 Loaded {} ({}) - method: POST", path, handler_sym);
                                                    }
                                                },
                                                "put" => {
                                                    if let Ok(sym) = lib.get::<RawHandlerWithBody>(&sym_bytes) {
                                                        method_set.put = Some(*sym);
                                                        info!("🧩 Loaded {} ({}) - method: PUT", path, handler_sym);
                                                    }
                                                },
                                                "delete" => {
                                                    if let Ok(sym) = lib.get::<RawHandler>(&sym_bytes) {
                                                        method_set.delete = Some(*sym);
                                                        info!("🧩 Loaded {} ({}) - method: DELETE", path, handler_sym);
                                                    }
                                                },
                                                _ => {}
                                            }
                                        }
                                        
                                        // Add all accumulated routes, skipping disabled ones
                                        for (path, methods) in path_methods {
                                            if settings.disabled_routes.iter().any(|r| r == &path) { continue; }
                                            if methods.get.is_some() || methods.post.is_some() || methods.put.is_some() || methods.delete.is_some() {
                                                routes.insert(path, methods);
                                            }
                                        }
                                        let _keep_alive: &'static Library = Box::leak(Box::new(lib));
                                        continue; // manifest handled for this lib
                                    }
                                    Ok(_) | Err(_) => {
                                        warn!("⚠️ Invalid routes_manifest in {:?}", p);
                                    }
                                }
                            }

                            // Fallback: single-route symbols
                            let route: Option<String> = lib
                                .get::<RawRoutePath>(b"route_path")
                                .ok()
                                .map(|sym| {
                                    let ptr = (*sym)();
                                    let s = std::ffi::CString::from_raw(ptr);
                                    s.to_string_lossy().to_string()
                                })
                                .or_else(|| path_to_route(base, p));
                            if let Some(route) = route {
                                if settings.disabled_routes.iter().any(|r| r == &route) { continue; }
                                let get: Option<RawHandler> = lib.get::<RawHandler>(b"get").ok().map(|sym| *sym);
                                let post: Option<RawHandlerWithBody> = lib.get::<RawHandlerWithBody>(b"post_bytes").ok().map(|sym| *sym);
                                let put: Option<RawHandlerWithBody> = lib.get::<RawHandlerWithBody>(b"put_bytes").ok().map(|sym| *sym);
                                let delete: Option<RawHandler> = lib.get::<RawHandler>(b"delete").ok().map(|sym| *sym);

                                let methods = MethodSet { get, post, put, delete };
                                if methods.get.is_some() || methods.post.is_some() || methods.put.is_some() || methods.delete.is_some() {
                                    info!("🧩 Loaded {}", route);
                                    routes.insert(route.clone(), methods);
                                    let _keep_alive: &'static Library = Box::leak(Box::new(lib));
                                }
                            }
                        }
                        Err(err) => {
                            warn!("⚠️ Skip plugin {:?}: load error: {}", p, err);
                        }
                    }
                }
            }
        }

        Self { routes }
    }
}

// Đọc tên package từ Cargo.toml trong thư mục plugin
fn read_package_name(plugin_dir: &Path) -> Option<String> {
    let cargo_toml = plugin_dir.join("Cargo.toml");
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
