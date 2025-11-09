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
use rate_limit as rl_feat;
use waf as waf_feat;
use oauth2 as oauth2_feat;

// WAF guard delegates decision to the waf feature crate
async fn waf_guard(req: Request<Body>, next: Next) -> Result<Response, StatusCode> {
    let uri = req.uri().to_string();
    let ua = req.headers().get("user-agent").and_then(|v| v.to_str().ok());
    if waf_feat::is_malicious(&uri, ua) { return Err(StatusCode::FORBIDDEN); }
    Ok(next.run(req).await)
}

// OAuth2 guard delegates decision to the oauth2 feature crate
async fn oauth2_guard(req: Request<Body>, next: Next) -> Result<Response, StatusCode> {
    let settings = load_settings();
    let path = req.uri().path().to_string();
    // Ưu tiên đọc cấu hình từ feature_extras nếu có
    let protected_routes: Vec<String> = if let Some(obj) = settings.feature_extras.get("oauth2").and_then(|v| v.as_object()) {
        obj.get("protected_routes")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect())
            .unwrap_or_else(|| settings.oauth2_protected_routes.clone())
    } else {
        settings.oauth2_protected_routes.clone()
    };
    let must_protect = oauth2_feat::requires_auth(&protected_routes, &path);
    if !must_protect { return Ok(next.run(req).await); }
    let auth = req.headers().get("authorization").and_then(|v| v.to_str().ok());
    if !oauth2_feat::has_bearer(auth) { return Err(StatusCode::UNAUTHORIZED); }
    Ok(next.run(req).await)
}

pub fn build_router_from(base: &str) -> Router {
    let mods = DynamicModules::load(base);
    let mut r = Router::new();

    for (path, m) in mods.routes {
        if let Some(g) = m.get {
            r = r.route(&path, get(move || async move { call_no_body_async(g).await }));
        }
        if let Some(p) = m.post {
            r = r.route(&path, post(move |body: Bytes| async move {
                call_with_body_async(p, body).await
            }));
        }
        if let Some(u) = m.put {
            r = r.route(&path, put(move |body: Bytes| async move {
                call_with_body_async(u, body).await
            }));
        }
        if let Some(d) = m.delete {
            r = r.route(&path, delete(move || async move { call_no_body_async(d).await }));
        }
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
    let path_norm = rl_feat::normalize_path(&path);
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
    if !rl_feat::check_allow(ip, &path_norm, limit) { return Err(StatusCode::TOO_MANY_REQUESTS); }
    Ok(next.run(req).await)
}
