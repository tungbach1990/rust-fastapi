mod types;
mod util;
mod dynamic_loader;
mod router;
mod watcher;
mod features_loader;
mod openapi;

use axum::{Router, Json, response::Html};
use parking_lot::RwLock;
use std::sync::Arc;
use tower::ServiceExt; // 👈 cần cho .oneshot()
use tokio::net::TcpListener;
use serde_json::Value;
use tracing::info;
use tower_http::trace::TraceLayer;
use dotenvy::dotenv;
use admin::{build_router as build_admin_router, load_settings};
use std::path::PathBuf;
use walkdir::WalkDir;
use reqwest::Client;

// Autoreadme (deepwiki-rs)
// (đã loại bỏ Autoreadme)
// Autoreadme removed

#[tokio::main]
async fn main() {
    // Initialize structured logging via tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    // Load .env if present
    let _ = dotenv();

    let mode = std::env::var("APP_ENV").unwrap_or("dev".into());
    let port = std::env::var("APP_PORT").unwrap_or("3000".into());
    let hot_reload = std::env::var("HOT_RELOAD").unwrap_or("1".into()) == "1";
    let prod_autoload = std::env::var("APP_AUTOLOAD").unwrap_or("0".into()) == "1";
    let module_dir = if mode == "prod" { "./build" } else { "./modules" };
    info!("🚀 Mode: {}, loading modules from {}", mode, module_dir);

    // Khởi tạo router/spec sống ngay lập tức để app có thể phục vụ ngay
    let live_router = Arc::new(RwLock::new(router::build_router_from(module_dir)));
    let live_spec: Arc<RwLock<Value>> = Arc::new(RwLock::new(openapi::build_openapi_from_modules("./modules", "./build")));

    // Load features ngay từ đầu nếu DLL đã có sẵn trong ./build
    let _ = features_loader::load_features("./features", "./build");

    // DEV: Khởi động server trước; build & load modules/features bất đồng bộ
    if mode == "dev" {
        // Build ban đầu chạy trong background, không chặn server
        {
            let live_router_clone = live_router.clone();
            let live_spec_clone = live_spec.clone();
            let module_dir_clone = module_dir.to_string();
            tokio::spawn(async move {
                watcher::build_and_load(&module_dir_clone).await;
                // build features để copy DLL vào ./build
                watcher::build_and_load("./features").await;
                // Sau khi build xong, cập nhật router/spec và nạp feature plugins
                *live_router_clone.write() = router::build_router_from("./build");
                *live_spec_clone.write() = openapi::build_openapi_from_modules("./modules", "./build");
                let _ = features_loader::load_features("./features", "./build");
                tracing::info!("✅ Initial build completed and router/spec updated");
            });
        }
        // Watch source modules; khi thay đổi sẽ rebuild và copy sang ./build
        tokio::spawn({
            let module_dir = module_dir.to_string();
            async move { watcher::watch_dev(&module_dir).await; }
        });
        // Watch ./build để reload router/spec và nạp lại features khi DLL đổi
        if hot_reload {
            let live_router_clone = live_router.clone();
            let live_spec_clone = live_spec.clone();
            tokio::spawn(async move {
                watcher::watch_prod_build("./build", live_router_clone, Some(live_spec_clone)).await;
            });
        }
    }

    // PROD: nếu bật APP_AUTOLOAD, theo dõi ./build để reload khi có DLL mới
    if mode == "prod" && prod_autoload {
        let live_router_clone = live_router.clone();
        let live_spec_clone = live_spec.clone();
        tokio::spawn(async move {
            watcher::watch_prod_build("./build", live_router_clone, Some(live_spec_clone)).await;
        });
    }

    let settings = load_settings();

    // Tự động sinh README.md bằng Autoreadme (deepwiki-rs)
    tokio::spawn(async move {
        // Ưu tiên dùng API ngoài nếu có cấu hình
        if std::env::var("README_API_URL").is_ok() {
            if let Err(e) = generate_readme_via_external_api().await {
                tracing::warn!("generate_readme_via_external_api failed: {}", e);
            } else {
                tracing::info!("README.md generated via external API successfully");
            }
        }
    });
    let mut app = Router::new()
        .route("/openapi.json", axum::routing::get({
            let live_spec = live_spec.clone();
            move || async move {
                let spec = live_spec.read().clone();
                Json(spec)
            }
        }))
        
        // Nest admin router từ crate admin
        .nest("/admin", {
            let live_spec = live_spec.clone();
            let reload_fn = {
                let live_router = live_router.clone();
                let live_spec = live_spec.clone();
                move || {
                    *live_router.write() = router::build_router_from("./build");
                    *live_spec.write() = openapi::build_openapi_from_modules("./modules", "./build");
                    // Nạp lại feature plugins theo cấu hình mới
                    let _ = features_loader::load_features("./features", "./build");
                }
            };
            let reload_fn = std::sync::Arc::new(reload_fn);
            build_admin_router(live_spec, reload_fn)
                .route("/features-manifest", axum::routing::get(|| async move {
                    let v = features_loader::collect_manifests("./build");
                    Json(v)
                }))
        })
        .route("/docs", axum::routing::get(|| async move {
            Html(r#"<!DOCTYPE html>
<html>
  <head>
    <meta charset='utf-8'>
    <title>API Docs</title>
    <link rel="stylesheet" href="https://unpkg.com/swagger-ui-dist@5/swagger-ui.css" />
  </head>
  <body>
    <div id="swagger-ui"></div>
    <script src="https://unpkg.com/swagger-ui-dist@5/swagger-ui-bundle.js"></script>
    <script>
      window.onload = () => {
        const ui = SwaggerUIBundle({
          url: '/openapi.json',
          dom_id: '#swagger-ui',
          presets: [SwaggerUIBundle.presets.apis],
        });
        window.ui = ui;
      };
    </script>
  </body>
</html>"#)
        }))
        .fallback_service(axum::routing::any_service({
            let state = live_router.clone();
            tower::service_fn(move |req| {
                let r = state.read().clone();
                r.oneshot(req)
            })
        }))
        // Global middleware: request tracing
        .layer(TraceLayer::new_for_http());

    // removed global CORS fallback layer: CORS is plugin-only now
    // if !settings.cors_enabled {
    //     app = app.layer(CorsLayer::permissive());
    // }

    info!("🌐 WebApp running at http://0.0.0.0:{}", port);

    let listener = TcpListener::bind(format!("0.0.0.0:{}", port)).await.unwrap();
    // Cung cấp ConnectInfo<SocketAddr> để middleware/admin có thể biết IP source
    let svc = app.into_make_service_with_connect_info::<std::net::SocketAddr>();
    axum::serve(listener, svc).await.unwrap(); // ✅ axum 0.7 style
}

// Sinh README.md bằng Autoreadme
async fn generate_readme_via_autoreadme() -> Result<(), Box<dyn std::error::Error>> {
    // Autoreadme đã bị loại bỏ; stub để tránh lỗi biên dịch
    Ok(())
}

async fn generate_readme_via_external_api() -> Result<(), Box<dyn std::error::Error>> {
    let url = std::env::var("README_API_URL")?;
    let method = std::env::var("README_API_METHOD").unwrap_or_else(|_| "GET".to_string());
    let token = std::env::var("README_API_TOKEN").ok();
    let extra_headers = std::env::var("README_API_HEADERS").ok(); // JSON map
    let body = std::env::var("README_API_BODY").ok(); // raw JSON string

    let client = Client::new();
    let mut req = match method.as_str() {
        "POST" => client.post(&url),
        "PUT" => client.put(&url),
        "PATCH" => client.patch(&url),
        _ => client.get(&url),
    };

    if let Some(t) = token {
        req = req.bearer_auth(t);
    }

    if let Some(hjson) = extra_headers {
        if let Ok(map) = serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&hjson) {
            for (k, v) in map.into_iter() {
                if let Some(val) = v.as_str() {
                    req = req.header(k, val);
                } else {
                    req = req.header(k, v.to_string());
                }
            }
        }
    }

    if let Some(b) = body {
        // assume JSON body
        let val: serde_json::Value = serde_json::from_str(&b).unwrap_or(serde_json::Value::Null);
        req = req.json(&val);
    }

    let resp = req.send().await?;
    let content_type = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");

    let text = if content_type.contains("application/json") {
        let v: serde_json::Value = resp.json().await?;
        // try common fields
        v.get("content").and_then(|c| c.as_str()).map(|s| s.to_string())
            .or_else(|| v.get("markdown").and_then(|c| c.as_str()).map(|s| s.to_string()))
            .or_else(|| v.get("data").and_then(|c| c.as_str()).map(|s| s.to_string()))
            .unwrap_or_else(|| v.to_string())
    } else {
        resp.text().await?
    };

    std::fs::write("./README.md", text)?;
    Ok(())
}
