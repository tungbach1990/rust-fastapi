use axum::{Router, Json, response::Html};
use axum::http::{Request, StatusCode};
use axum::response::Response;
use axum::middleware::{from_fn, Next};
use axum::body::Body;
use axum::extract::ConnectInfo;
use std::net::{SocketAddr, IpAddr, Ipv4Addr};
use axum::extract::Json as AxumJson;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use serde_json::{Value, Map, json};
use std::sync::Arc;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct FeaturesSettings {
    pub rate_limit_enabled: bool,
    pub rate_limit_per_second: u32,
    pub waf_enabled: bool,
    pub oauth2_enabled: bool,
    pub cors_enabled: bool,
    pub admin_console_enabled: bool,
    pub disabled_modules: Vec<String>,
    pub disabled_routes: Vec<String>,
    pub disabled_features: Vec<String>,
    pub route_rate_limits: HashMap<String, u32>,
    pub feature_extras: Map<String, Value>,
}

impl Default for FeaturesSettings {
    fn default() -> Self {
        Self {
            rate_limit_enabled: false,
            rate_limit_per_second: 1,
            waf_enabled: false,
            oauth2_enabled: false,
            cors_enabled: false,
            admin_console_enabled: true,
            disabled_modules: Vec::new(),
            disabled_routes: Vec::new(),
            disabled_features: Vec::new(),
            route_rate_limits: HashMap::new(),
            feature_extras: Map::new(),
        }
    }
}

pub fn load_settings() -> FeaturesSettings {
    let path = std::path::Path::new("./admin/config/features.json");
    if let Ok(text) = std::fs::read_to_string(path) {
        serde_json::from_str(&text).unwrap_or_default()
    } else {
        FeaturesSettings::default()
    }
}

pub fn save_settings(s: &FeaturesSettings) -> std::io::Result<()> {
    let path = std::path::Path::new("./admin/config/features.json");
    if let Some(parent) = path.parent() { std::fs::create_dir_all(parent).ok(); }
    let text = serde_json::to_string_pretty(s).unwrap_or_else(|_| "{}".to_string());
    std::fs::write(path, text)
}

// Middleware chặn truy cập /admin và /admin/settings chỉ cho phép từ 127.0.0.1 và Host=localhost
async fn admin_access_guard(req: Request<Body>, next: Next) -> Result<Response, StatusCode> {
    let path = req.uri().path();
    // Chỉ áp dụng guard cho đúng hai endpoint yêu cầu
    let must_guard = path == "/admin" || path == "/admin/" || path == "/admin/settings";
    if !must_guard {
        return Ok(next.run(req).await);
    }

    // Kiểm tra Host header
    let host = req
        .headers()
        .get("host")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let host_base = host.split(':').next().unwrap_or("");
    let host_ok = host_base.eq_ignore_ascii_case("localhost");

    // Lấy IP client từ ConnectInfo (cần into_make_service_with_connect_info ở app)
    let ip_ok = if let Some(ci) = req.extensions().get::<ConnectInfo<SocketAddr>>() {
        match ci.0.ip() {
            IpAddr::V4(v4) => v4 == Ipv4Addr::LOCALHOST,
            IpAddr::V6(v6) => v6.is_loopback(),
        }
    } else {
        false
    };

    if host_ok && ip_ok {
        Ok(next.run(req).await)
    } else {
        Err(StatusCode::FORBIDDEN)
    }
}

// Build admin router to be nested under "/admin"
pub fn build_router(
    live_spec: Arc<RwLock<Value>>,
    reload_fn: Arc<dyn Fn() + Send + Sync + 'static>,
) -> Router {
    Router::new()
        .route("/", axum::routing::get({
            move || async move {
                // Read HTML from assets
                let html = module_utils::read_asset!("assets/index.html");
                Html(html)
            }
        }))
        .route("/assets/styles.css", axum::routing::get({
            move || async move {
                let css = module_utils::read_asset!("assets/styles.css");
                (
                    [(axum::http::header::CONTENT_TYPE, "text/css")],
                    css
                )
            }
        }))
        .route("/assets/app.js", axum::routing::get({
            move || async move {
                let js = module_utils::read_asset!("assets/app.js");
                (
                    [(axum::http::header::CONTENT_TYPE, "application/javascript")],
                    js
                )
            }
        }))
        .route("/settings", axum::routing::get({
            move || async move {
                let s = load_settings();
                let mut modules: Vec<String> = Vec::new();
                if let Ok(rd) = std::fs::read_dir("./modules") {
                    for e in rd.flatten() {
                        let p = e.path();
                        if p.join("Cargo.toml").exists() {
                            if let Some(name) = p.file_name().map(|s| s.to_string_lossy().to_string()) {
                                modules.push(name);
                            }
                        }
                    }
                    modules.sort();
                }
                Json(json!({ "settings": s, "modules": modules }))
            }
        }))
        .route("/settings", axum::routing::post({
            let reload_fn = reload_fn.clone();
            move |AxumJson(body): AxumJson<serde_json::Value>| async move {
                let mut s = load_settings();
                if let Some(v) = body.get("rate_limit_enabled").and_then(|v| v.as_bool()) { s.rate_limit_enabled = v; }
                if let Some(v) = body.get("rate_limit_per_second").and_then(|v| v.as_u64()) { s.rate_limit_per_second = v as u32; }
                if let Some(v) = body.get("waf_enabled").and_then(|v| v.as_bool()) { s.waf_enabled = v; }
                if let Some(v) = body.get("oauth2_enabled").and_then(|v| v.as_bool()) { s.oauth2_enabled = v; }
                if let Some(v) = body.get("admin_console_enabled").and_then(|v| v.as_bool()) { s.admin_console_enabled = v; }
                if let Some(v) = body.get("disabled_modules").and_then(|v| v.as_array()) {
                    s.disabled_modules = v.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect();
                }
                if let Some(v) = body.get("disabled_features").and_then(|v| v.as_array()) {
                    s.disabled_features = v.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect();
                    // Tự đồng bộ các cờ dựa theo tên phổ biến
                    s.waf_enabled = !s.disabled_features.iter().any(|f| f == "waf") && s.waf_enabled;
                    s.oauth2_enabled = !s.disabled_features.iter().any(|f| f == "oauth2") && s.oauth2_enabled;
                    s.rate_limit_enabled = !s.disabled_features.iter().any(|f| f == "rate_limit") && s.rate_limit_enabled;
                }
                if let Some(obj) = body.get("route_rate_limits").and_then(|v| v.as_object()) {
                    let mut map = std::collections::HashMap::new();
                    for (k, vv) in obj {
                        if let Some(n) = vv.as_u64() { map.insert(k.to_string(), n as u32); }
                    }
                    s.route_rate_limits = map;
                }
                if let Some(obj) = body.get("feature_extras").and_then(|v| v.as_object()) {
                    s.feature_extras = obj.clone();
                }
                let _ = save_settings(&s);
                // Trigger reload via provided closure
                (reload_fn)();
                Json(json!({"ok":true}))
            }
        }))
        .route("/reload", axum::routing::post({
            let reload_fn = reload_fn.clone();
            move || async move {
                (reload_fn)();
                Json(json!({"ok":true}))
            }
        }))
        .route("/routes", axum::routing::get({
            let live_spec = live_spec.clone();
            move || async move {
                let spec = live_spec.read().clone();
                let mut groups: Vec<serde_json::Value> = Vec::new();
                use std::collections::{BTreeMap, BTreeSet};
                let mut map: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();

                // 1) Lấy các route từ OpenAPI (chỉ các route đang bật)
                if let Some(paths) = spec.get("paths").and_then(|v| v.as_object()) {
                    for (route, obj) in paths.iter() {
                        let prefix = route.trim_start_matches('/').split('/').next().unwrap_or("");
                        let module = obj.get("x-module").and_then(|v| v.as_str()).unwrap_or(prefix).to_string();
                        map.entry(module).or_default().insert(route.to_string());
                    }
                }

                // 2) Bổ sung các route đang bị disable để vẫn hiển thị dạng bật/tắt
                let s = load_settings();
                for route in s.disabled_routes.iter() {
                    let prefix = route.trim_start_matches('/').split('/').next().unwrap_or("");
                    let module = prefix.to_string();
                    map.entry(module).or_default().insert(route.clone());
                }

                // 3) Chuyển về dạng mảng và sắp xếp
                for (module, set) in map.into_iter() {
                    let mut routes: Vec<String> = set.into_iter().collect();
                    routes.sort();
                    groups.push(json!({"module": module, "routes": routes}));
                }

                Json(json!({"groups": groups}))
            }
        }))
        .route("/routes", axum::routing::post({
            let reload_fn = reload_fn.clone();
            move |AxumJson(body): AxumJson<serde_json::Value>| async move {
                let mut s = load_settings();
                if let Some(v) = body.get("disabled_routes").and_then(|v| v.as_array()) {
                    s.disabled_routes = v.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect();
                }
                let _ = save_settings(&s);
                // Áp dụng ngay: reload OpenAPI và router
                (reload_fn)();
                Json(json!({"ok": true}))
            }
        }))
        // Áp dụng guard IP/Host cho một số endpoint nhạy cảm
        .layer(from_fn(admin_access_guard))
}