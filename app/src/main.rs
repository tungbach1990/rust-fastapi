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
use tower_http::cors::CorsLayer;
use dotenvy::dotenv;
use admin::build_router as build_admin_router;

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

    let live_router = Arc::new(RwLock::new(router::build_router_from(module_dir)));
    let live_spec: Arc<RwLock<Value>> = Arc::new(RwLock::new(openapi::build_openapi_from_modules("./modules", "./build")));

    if mode == "dev" {
        watcher::build_and_load(module_dir).await;
        // Build features/* giống modules/* để copy DLL vào ./build
        watcher::build_and_load("./features").await;
        // Sau khi build lần đầu, khởi tạo router từ build để có sẵn các API
        *live_router.write() = router::build_router_from("./build");
        *live_spec.write() = openapi::build_openapi_from_modules("./modules", "./build");
        // Nạp động các feature plugins (waf, rate_limit, oauth2, ...) từ thư mục build
        let _loaded_features = features_loader::load_features("./features", "./build");
        tokio::spawn({
            let module_dir = module_dir.to_string();
            async move { watcher::watch_dev(&module_dir).await; }
        });
        // Không cần watcher cho thư mục features source nữa

        // Hiện đại: dùng watcher sự kiện thay vì reload theo interval
        if hot_reload {
            let live_router_clone = live_router.clone();
            let live_spec_clone = live_spec.clone();
            tokio::spawn(async move {
                watcher::watch_prod_build("./build", live_router_clone, Some(live_spec_clone)).await;
            });
        }
    }

    if mode == "prod" && prod_autoload {
        let live_router_clone = live_router.clone();
        let live_spec_clone = live_spec.clone();
        tokio::spawn(async move {
            watcher::watch_prod_build("./build", live_router_clone, Some(live_spec_clone)).await;
        });
    }

    let app = Router::new()
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
        // Global middleware: request tracing and permissive CORS (dev-friendly)
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive());

    info!("🌐 WebApp running at http://0.0.0.0:{}", port);

    let listener = TcpListener::bind(format!("0.0.0.0:{}", port)).await.unwrap();
    // Cung cấp ConnectInfo<SocketAddr> để middleware/admin có thể biết IP source
    let svc = app.into_make_service_with_connect_info::<std::net::SocketAddr>();
    axum::serve(listener, svc).await.unwrap(); // ✅ axum 0.7 style
}
