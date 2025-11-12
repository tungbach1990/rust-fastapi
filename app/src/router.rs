use axum::{
    routing::{get, post, put, delete},
    Router,
    body::Bytes,
    http::{Request, StatusCode},
};
use axum::body::Body;
use axum::middleware::{from_fn, Next};
use axum::response::Response;
use crate::{dynamic_loader::DynamicModules, types::{call_no_body_async, call_with_body_async}};
use admin::load_settings;
// loại bỏ phụ thuộc compile-time vào crates feature; dùng helper nội bộ dựa trên cấu hình
use tower_http::cors::{CorsLayer, Any};
use http::{HeaderValue, Method};
use std::time::Duration;
use std::time::Instant;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

fn normalize_path(p: &str) -> String {
    let mut s = p.trim().to_string();
    if !s.starts_with('/') { s = format!("/{}", s); }
    while s.contains("//") { s = s.replace("//", "/"); }
    if s.len() > 1 && s.ends_with('/') { s.pop(); }
    s
}

fn route_matches(pattern: &str, path: &str) -> bool {
    let path = normalize_path(path);
    let patt = normalize_path(pattern);
    if let Some(prefix) = patt.strip_suffix("/*") {
        path.starts_with(&normalize_path(prefix))
    } else {
        patt == path
    }
}

fn route_requires_auth(path: &str, protected_routes: &[String]) -> bool {
    protected_routes.iter().any(|p| route_matches(p, path))
}

fn has_bearer(auth: Option<&str>) -> bool {
    match auth { Some(s) => s.trim().to_ascii_lowercase().starts_with("bearer "), None => false }
}

fn waf_is_malicious(uri: &str, ua: Option<&str>, patterns: &[String]) -> bool {
    let ual = ua.unwrap_or("").to_ascii_lowercase();
    let uril = uri.to_ascii_lowercase();
    patterns.iter().any(|p| {
        let q = p.to_ascii_lowercase();
        uril.contains(&q) || (!ual.is_empty() && ual.contains(&q))
    })
}

static RATE_LIMITERS: OnceLock<Mutex<HashMap<(String, String), (usize, Instant)>>> = OnceLock::new();
fn rate_limit_check_allow(ip: &str, path: &str, limit: usize) -> bool {
    let mtx = RATE_LIMITERS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut map = mtx.lock().unwrap();
    let key = (ip.to_string(), normalize_path(path));
    let now = Instant::now();
    let entry = map.entry(key).or_insert((0, now));
    if now.duration_since(entry.1).as_secs() >= 1 {
        entry.0 = 0;
        entry.1 = now;
    }
    entry.0 += 1;
    entry.0 <= limit
}

// WAF guard delegates decision to the waf feature crate
async fn waf_guard(req: Request<Body>, next: Next) -> Result<Response, StatusCode> {
    let settings = load_settings();
    let uri = req.uri().to_string();
    let ua = req.headers().get("user-agent").and_then(|v| v.to_str().ok());
    let patterns: Vec<String> = settings
        .feature_extras
        .get("waf")
        .and_then(|v| v.as_object())
        .and_then(|o| o.get("patterns"))
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect())
        .unwrap_or_default();
    if waf_is_malicious(&uri, ua, &patterns) { return Err(StatusCode::FORBIDDEN); }
    Ok(next.run(req).await)
}

// OAuth2 guard delegates decision to the oauth2 feature crate
async fn oauth2_guard(req: Request<Body>, next: Next) -> Result<Response, StatusCode> {
    let settings = load_settings();
    let path = req.uri().path().to_string();
    let protected_routes: Vec<String> = if let Some(obj) = settings.feature_extras.get("oauth2").and_then(|v| v.as_object()) {
        obj.get("protected_routes")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect())
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    if !route_requires_auth(&path, &protected_routes) { return Ok(next.run(req).await); }
    let auth = req.headers().get("authorization").and_then(|v| v.to_str().ok());
    if !has_bearer(auth) { return Err(StatusCode::UNAUTHORIZED); }
    Ok(next.run(req).await)
}

pub fn build_router_from(base: &str) -> Router {
    let mods = DynamicModules::load(base);
    let mut r = Router::new();

    for (path, m) in mods.routes {
        // Xây sub-router cho từng path để có thể áp lớp CORS riêng
        let mut sub = Router::new();
        if let Some(g) = m.get {
            sub = sub.route(&path, get(move || async move { call_no_body_async(g).await }));
        }
        if let Some(p) = m.post {
            sub = sub.route(&path, post(move |body: Bytes| async move {
                call_with_body_async(p, body).await
            }));
        }
        if let Some(u) = m.put {
            sub = sub.route(&path, put(move |body: Bytes| async move {
                call_with_body_async(u, body).await
            }));
        }
        if let Some(d) = m.delete {
            sub = sub.route(&path, delete(move || async move { call_no_body_async(d).await }));
        }

        // Áp dụng CORS theo route nếu bật cors_enabled và có cấu hình
        let s_for_cors = load_settings();
        // Apply CORS only if plugin exists AND not disabled in settings
        if crate::features_loader::has_feature("./build", "cors") && !s_for_cors.disabled_features.contains(&"cors".to_string()) {
            if let Some(layer) = route_cors_layer(&path, &s_for_cors) {
                sub = sub.layer(layer);
            }
        }
        r = r.merge(sub);
    }

    // Áp dụng middleware theo FeaturesSettings (logic nằm trong crates ở thư mục features)
    let s = load_settings();
    if s.waf_enabled {
        r = r.layer(from_fn(waf_guard));
    }
    if s.oauth2_enabled { r = r.layer(from_fn(oauth2_guard)); }
    if s.rate_limit_enabled {
        r = r.layer(from_fn(rate_limit_guard));
    }

    r
}

async fn rate_limit_guard(req: Request<Body>, next: Next) -> Result<Response, StatusCode> {
    // Lấy IP client (best-effort)
    let ip = req.headers().get("x-forwarded-for").and_then(|v| v.to_str().ok()).unwrap_or("local");
    let path = req.uri().path().to_string();
    // Lấy cấu hình limit theo route nếu có, fallback về global
    let settings = load_settings();
    let path_norm = normalize_path(&path);
    // Đọc per-route từ feature_extras nếu có, fallback sang trường cũ route_rate_limits
    let per_route_limit = if let Some(obj) = settings.feature_extras.get("rate_limit").and_then(|v| v.as_object()) {
        obj.get("route_limits")
            .and_then(|v| v.as_object())
            .and_then(|m| m.get(&path_norm))
            .and_then(|v| v.as_u64())
            .map(|n| n as u32)
            .or_else(|| settings.route_rate_limits.get(&path_norm).copied())
    } else {
        settings.route_rate_limits.get(&path_norm).copied()
    };
    let global_default = if let Some(obj) = settings.feature_extras.get("rate_limit").and_then(|v| v.as_object()) {
        obj.get("rps").and_then(|v| v.as_u64()).map(|n| n as u32).unwrap_or(settings.rate_limit_per_second)
    } else {
        settings.rate_limit_per_second
    };
    let limit = per_route_limit.unwrap_or(global_default) as usize;
    if !rate_limit_check_allow(ip, &path_norm, limit) { return Err(StatusCode::TOO_MANY_REQUESTS); }
    Ok(next.run(req).await)
}

fn route_cors_layer(path: &str, s: &admin::FeaturesSettings) -> Option<CorsLayer> {
    let cors_obj = s.feature_extras.get("cors")?.as_object()?;
    let path_norm = normalize_path(path);

    // Nếu có danh sách bật theo route, chỉ áp dụng khi path nằm trong danh sách
    if let Some(enabled) = cors_obj.get("enabled_routes").and_then(|v| v.as_array()) {
        let enabled_list: Vec<String> = enabled.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect();
        let is_enabled = enabled_list.iter().any(|item| {
            if let Some(prefix) = item.strip_suffix("/*") {
                path_norm.starts_with(&normalize_path(prefix))
            } else {
                normalize_path(item) == path_norm
            }
        });
        if !is_enabled { return None; }
    }

    let mut layer = CorsLayer::new();

    // origins
    if let Some(origins) = cors_obj.get("origins").and_then(|v| v.as_array()) {
        if origins.iter().any(|o| o.as_str() == Some("*")) {
            layer = layer.allow_origin(Any);
        } else {
            let list: Vec<HeaderValue> = origins
                .iter()
                .filter_map(|o| o.as_str())
                .filter_map(|s| s.parse::<HeaderValue>().ok())
                .collect();
            if !list.is_empty() { layer = layer.allow_origin(list); }
        }
    }
    // methods
    if let Some(methods) = cors_obj.get("methods").and_then(|v| v.as_array()) {
        let list: Vec<Method> = methods
            .iter()
            .filter_map(|m| m.as_str())
            .filter_map(|s| s.parse::<Method>().ok())
            .collect();
        if !list.is_empty() { layer = layer.allow_methods(list); }
    }
    // headers
    if let Some(headers) = cors_obj.get("headers").and_then(|v| v.as_array()) {
        if headers.iter().any(|h| h.as_str() == Some("*")) {
            layer = layer.allow_headers(Any);
        } else {
            let list: Vec<http::HeaderName> = headers
                .iter()
                .filter_map(|h| h.as_str())
                .filter_map(|s| s.parse::<http::HeaderName>().ok())
                .collect();
            if !list.is_empty() { layer = layer.allow_headers(list); }
        }
    }
    // expose_headers
    if let Some(headers) = cors_obj.get("expose_headers").and_then(|v| v.as_array()) {
        let list: Vec<http::HeaderName> = headers
            .iter()
            .filter_map(|h| h.as_str())
            .filter_map(|s| s.parse::<http::HeaderName>().ok())
            .collect();
        if !list.is_empty() { layer = layer.expose_headers(list); }
    }
    // allow_credentials (nhận bool hoặc 0/1)
    if let Some(cred_val) = cors_obj.get("allow_credentials") {
        let cred = cred_val.as_bool().unwrap_or_else(|| cred_val.as_u64().map(|n| n != 0).unwrap_or(false));
        if cred { layer = layer.allow_credentials(true); }
    }
    // max_age
    if let Some(age) = cors_obj.get("max_age").and_then(|v| v.as_u64()) {
        layer = layer.max_age(Duration::from_secs(age));
    }

    Some(layer)
}
