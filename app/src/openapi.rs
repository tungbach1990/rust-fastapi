use serde_json::{json, Value};
use libloading::Library;
use std::path::{Path, PathBuf};
use tracing::{info, warn};
use admin::load_settings as load_settings_alias;

// Local symbol type aliases (mirroring app::types)
use std::os::raw::{c_char, c_uchar};
type RawHandler = unsafe extern "C" fn() -> *mut c_char;
type RawHandlerWithBody = unsafe extern "C" fn(*const c_uchar, usize) -> *mut c_char;
type RawRoutePath = unsafe extern "C" fn() -> *mut c_char;
type RawStr = RawRoutePath;

// Kiểm tra route có nằm trong danh sách bảo vệ OAuth2 hay không
fn route_is_protected(route: &str, list: &[String]) -> bool {
    fn normalize(p: &str) -> String {
        if p == "/" { return "/".to_string(); }
        p.trim_end_matches('/').to_string()
    }
    let path_norm = normalize(route);
    if list.is_empty() { return false; }
    list.iter().any(|item| {
        let s = item.as_str();
        if let Some(prefix) = s.strip_suffix("/*") {
            let prefix_norm = normalize(prefix);
            path_norm.starts_with(&prefix_norm)
        } else {
            normalize(s) == path_norm
        }
    })
}

// Read package name from a module directory's Cargo.toml
fn read_package_name(module_dir: &Path) -> Option<String> {
    let cargo_toml = module_dir.join("Cargo.toml");
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

fn built_candidates_for(module_dir: &Path, build_dir: &Path) -> Vec<PathBuf> {
    let dir_name = module_dir.file_name().unwrap().to_string_lossy().to_string();
    let pkg_name = read_package_name(module_dir).unwrap_or_else(|| dir_name.clone());
    vec![
        build_dir.join(format!("lib{}.so", pkg_name)),
        build_dir.join(format!("{}.dll", pkg_name)),
        build_dir.join(format!("lib{}.dll", pkg_name)),
        // Fallback theo tên thư mục nếu khác name
        build_dir.join(format!("lib{}.so", dir_name.clone())),
        build_dir.join(format!("{}.dll", dir_name.clone())),
        build_dir.join(format!("lib{}.dll", dir_name.clone())),
    ]
}

// Sinh OpenAPI tối thiểu bằng cách đọc thư mục modules và các DLL đã build
// - modules_dir: thư mục chứa source các module (ví dụ: "./modules")
// - build_dir: thư mục chứa các thư viện đã build được copy (ví dụ: "./build")
pub fn build_openapi_from_modules(modules_dir: &str, build_dir: &str) -> Value {
    let settings = load_settings_alias();
    let build_dir_path = Path::new(build_dir);
    let mut paths = serde_json::Map::new();

    // Duyệt từng thư mục module trong modules_dir
    if let Ok(rd) = std::fs::read_dir(modules_dir) {
        for entry in rd.flatten() {
            let module_dir = entry.path();
            if !module_dir.join("Cargo.toml").exists() { continue; }
            let folder_name = module_dir.file_name().unwrap().to_string_lossy().to_string();

            // Bỏ qua theo tên thư mục nếu bị disable
            if settings.disabled_modules.iter().any(|m| m == &folder_name) { continue; }

            // Tìm file thư viện đã build tương ứng trong build_dir
            let built = built_candidates_for(&module_dir, build_dir_path)
                .into_iter()
                .find(|p| p.exists());

            let Some(lib_path) = built else {
                warn!("⚠️ Không tìm thấy thư viện build cho module {:?}", folder_name);
                continue;
            };

            unsafe {
                match Library::new(&lib_path) {
                    Ok(lib) => {
                        // Ưu tiên đọc routes_manifest để hỗ trợ nhiều route trong một module
                        let manifest_json: Option<String> = lib
                            .get::<RawStr>(b"routes_manifest")
                            .ok()
                            .map(|sym| {
                                let ptr = (*sym)();
                                let s = std::ffi::CString::from_raw(ptr);
                                s.to_string_lossy().to_string()
                            });

                        if let Some(mjson) = manifest_json {
                            if let Ok(Value::Array(items)) = serde_json::from_str::<Value>(&mjson) {
                                for item in items {
                                    let route = item.get("path").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                    let method = item.get("method").and_then(|v| v.as_str()).unwrap_or("get");
                                    let ct = item.get("content_type").and_then(|v| v.as_str()).unwrap_or("application/json");
                                    if route.is_empty() { continue; }
                                    if settings.disabled_routes.iter().any(|r| r == &route) { continue; }
                                    let entry = paths.entry(route.clone()).or_insert_with(|| Value::Object(serde_json::Map::new()));
                                    if let Value::Object(ref mut map) = entry {
                                        // gắn nhãn module theo tên thư mục
                                        map.insert("x-module".to_string(), Value::String(folder_name.clone()));
                                        // security cho operation: chỉ thêm bearerAuth nếu route đã tick
                                        let security_vec = if settings.oauth2_enabled && route_is_protected(&route, &settings.oauth2_protected_routes) {
                                            vec![json!({"apiKeyAuth": []}), json!({"bearerAuth": []})]
                                        } else {
                                            vec![json!({"apiKeyAuth": []})]
                                        };
                                        // ví dụ request body theo route
                                        let example = if route == "/greet/user" {
                                            json!({"name":"John","age":25})
                                        } else if route == "/greet/message" {
                                            json!({"message":"Hello","language":"vi"})
                                        } else {
                                            json!({"example":"value"})
                                        };
                                        let rb_content = json!({"application/json": {"schema": {"type": "object"}, "example": example}});
                                        match method {
                                            "get" => { map.insert("get".to_string(), json!({
                                                "summary":"GET",
                                                "parameters": [
                                                    {"name":"X-Custom-Header","in":"header","schema":{"type":"string"},"required":false}
                                                ],
                                                "security": security_vec,
                                                "responses":{"200":{"description":"OK","content":{ct:{}}}}
                                            })); },
                                            "post" => { map.insert("post".to_string(), json!({
                                                "summary":"POST",
                                                "parameters": [
                                                    {"name":"X-Custom-Header","in":"header","schema":{"type":"string"},"required":false}
                                                ],
                                                "security": security_vec,
                                                "requestBody": {"content": rb_content},
                                                "responses":{"200":{"description":"OK","content":{ct:{}}}}
                                            })); },
                                            "put" => { map.insert("put".to_string(), json!({
                                                "summary":"PUT",
                                                "parameters": [
                                                    {"name":"X-Custom-Header","in":"header","schema":{"type":"string"},"required":false}
                                                ],
                                                "security": security_vec,
                                                "requestBody": {"content": rb_content},
                                                "responses":{"200":{"description":"OK","content":{ct:{}}}}
                                            })); },
                                            "delete" => { map.insert("delete".to_string(), json!({
                                                "summary":"DELETE",
                                                "parameters": [
                                                    {"name":"X-Custom-Header","in":"header","schema":{"type":"string"},"required":false}
                                                ],
                                                "security": security_vec,
                                                "responses":{"200":{"description":"OK","content":{ct:{}}}}
                                            })); },
                                            _ => {}
                                        }
                                    }
                                }
                            }
                            let _keep_alive: &'static Library = Box::leak(Box::new(lib));
                            continue;
                        }

                        // Fallback: single route per module
                        let route: Option<String> = lib
                            .get::<RawRoutePath>(b"route_path")
                            .ok()
                            .map(|sym| {
                                let ptr = (*sym)();
                                let s = std::ffi::CString::from_raw(ptr);
                                s.to_string_lossy().to_string()
                            })
                            .or_else(|| crate::util::path_to_route(build_dir, &lib_path));

                        if let Some(route) = route {
                            if settings.disabled_routes.iter().any(|r| r == &route) { continue; }
                            let mut methods = serde_json::Map::new();

                            let content_type: String = lib
                                .get::<RawStr>(b"content_type")
                                .ok()
                                .map(|sym| {
                                    let ptr = (*sym)();
                                    let s = std::ffi::CString::from_raw(ptr);
                                    s.to_string_lossy().to_string()
                                })
                                .unwrap_or_else(|| "application/json".to_string());

                            // security cho operation: chỉ thêm bearerAuth nếu route đã tick
                            let security_vec = if settings.oauth2_enabled && route_is_protected(&route, &settings.oauth2_protected_routes) {
                                vec![json!({"apiKeyAuth": []}), json!({"bearerAuth": []})]
                            } else {
                                vec![json!({"apiKeyAuth": []})]
                            };
                            // ví dụ request body theo route
                            let example = if route == "/greet/user" {
                                json!({"name":"John","age":25})
                            } else if route == "/greet/message" {
                                json!({"message":"Hello","language":"vi"})
                            } else {
                                json!({"example":"value"})
                            };
                            let rb_content = json!({"application/json": {"schema": {"type": "object"}, "example": example}});

                            if lib.get::<RawHandler>(b"get").is_ok() {
                                methods.insert("get".to_string(), json!({
                                    "summary": "GET",
                                    "parameters": [
                                        {"name":"X-Custom-Header","in":"header","schema":{"type":"string"},"required":false}
                                    ],
                                    "security": security_vec,
                                    "responses": {"200": {"description": "OK", "content": {content_type.clone(): {}}}}
                                }));
                            }
                            if lib.get::<RawHandlerWithBody>(b"post_bytes").is_ok() {
                                methods.insert("post".to_string(), json!({
                                    "summary": "POST",
                                    "parameters": [
                                        {"name":"X-Custom-Header","in":"header","schema":{"type":"string"},"required":false}
                                    ],
                                    "security": security_vec,
                                    "requestBody": {"content": rb_content},
                                    "responses": {"200": {"description": "OK", "content": {content_type.clone(): {}}}}
                                }));
                            }
                            if lib.get::<RawHandlerWithBody>(b"put_bytes").is_ok() {
                                methods.insert("put".to_string(), json!({
                                    "summary": "PUT",
                                    "parameters": [
                                        {"name":"X-Custom-Header","in":"header","schema":{"type":"string"},"required":false}
                                    ],
                                    "security": security_vec,
                                    "requestBody": {"content": rb_content},
                                    "responses": {"200": {"description": "OK", "content": {content_type.clone(): {}}}}
                                }));
                            }
                            if lib.get::<RawHandler>(b"delete").is_ok() {
                                methods.insert("delete".to_string(), json!({
                                    "summary": "DELETE",
                                    "parameters": [
                                        {"name":"X-Custom-Header","in":"header","schema":{"type":"string"},"required":false}
                                    ],
                                    "security": security_vec,
                                    "responses": {"200": {"description": "OK", "content": {content_type.clone(): {}}}}
                                }));
                            }

                            let entry = Value::Object(methods);
                            paths.insert(route.clone(), entry);

                            info!("📄 OpenAPI: {} (ct={})", route, content_type);
                        }

                        // Keep the library alive for the duration to avoid unloading issues
                        let _keep_alive: &'static Library = Box::leak(Box::new(lib));
                    }
                    Err(e) => {
                        warn!("⚠️ Lỗi nạp thư viện {:?}: {}", lib_path, e);
                    }
                }
            }
        }
    }

    // Base skeleton of the OpenAPI document
    let mut doc = serde_json::Map::new();
    doc.insert("openapi".to_string(), Value::String("3.0.0".to_string()));
    doc.insert("info".to_string(), json!({
        "title": "WebApp FastAPI RS",
        "version": "1.0.0"
    }));
    doc.insert("components".to_string(), json!({
        "securitySchemes": {
            "apiKeyAuth": {
                "type": "apiKey",
                "in": "header",
                "name": "Authorization"
            },
            "bearerAuth": {
                "type": "http",
                "scheme": "bearer",
                "bearerFormat": "JWT"
            }
        }
    }));
    doc.insert("paths".to_string(), Value::Object(paths));
    Value::Object(doc)
}