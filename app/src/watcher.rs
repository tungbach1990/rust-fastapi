use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher, Event};
use std::{process::Command, path::{Path, PathBuf}, time::Duration, fs, sync::Arc};
use parking_lot::RwLock;
use axum::Router;
use crate::router::build_router_from;
use crate::features_loader::load_features;
use tokio::{sync::mpsc, time::sleep};
use walkdir::WalkDir;
use serde_json::Value;
use crate::openapi::build_openapi_from_modules;
use tracing::{info, warn};

pub async fn build_and_load(base_path: &str) {
    let settings = admin::load_settings();
    let plugins: Vec<_> = WalkDir::new(base_path)
        .min_depth(1).max_depth(2)
        .into_iter().flatten()
        .filter(|e| e.path().join("Cargo.toml").exists())
        .filter(|e| {
            // Nếu đang build thư mục features -> bỏ qua các plugin bị disable
            if base_path.ends_with("features") || base_path.contains("/features") || base_path.contains("\\features") {
                if let Some(name) = e.path().file_name().map(|s| s.to_string_lossy().to_string()) {
                    return !settings.disabled_features.iter().any(|f| f == &name);
                }
            }
            true
        })
        .map(|e| e.path().to_path_buf())
        .collect();

    for p in plugins {
        info!("⚙️ Building {:?}", p);
        let _ = Command::new("cargo")
            .args(["build", "--release"])
            .current_dir(&p)
            .status();

        // ✅ Xác định tên package thực tế từ Cargo.toml (ưu tiên) hoặc fallback tên thư mục
        let pkg_name = read_package_name(&p).unwrap_or_else(|| p.file_name().unwrap().to_string_lossy().to_string());
        let dir_name = p.file_name().unwrap().to_string_lossy().to_string();
        // Ưu tiên target chung của workspace, fallback sang target riêng của plugin
        let ws_target_dir = Path::new("./target").join("release");
        let local_target_dir = p.join("target").join("release");
        
        // Kiểm tra các định dạng file có thể có ở cả 2 nơi
        let mut built_candidates: Vec<PathBuf> = vec![
            ws_target_dir.join(format!("lib{}.so", pkg_name)),
            ws_target_dir.join(format!("{}.dll", pkg_name)),
            ws_target_dir.join(format!("lib{}.dll", pkg_name)),
            ws_target_dir.join(format!("lib{}.so", dir_name)),
            ws_target_dir.join(format!("{}.dll", dir_name)),
            ws_target_dir.join(format!("lib{}.dll", dir_name)),
            local_target_dir.join(format!("lib{}.so", pkg_name)),
            local_target_dir.join(format!("{}.dll", pkg_name)),
            local_target_dir.join(format!("lib{}.dll", pkg_name)),
            local_target_dir.join(format!("lib{}.so", dir_name)),
            local_target_dir.join(format!("{}.dll", dir_name)),
            local_target_dir.join(format!("lib{}.dll", dir_name)),
        ];
        // Loại trùng và giữ lại theo thứ tự xuất hiện
        built_candidates.sort();
        built_candidates.dedup();
        let built = built_candidates.into_iter().find(|p| p.exists())
            .unwrap_or_else(|| ws_target_dir.join(format!("{}.dll", pkg_name)));

        // ✅ Copy sang ./build
        if built.exists() {
            fs::create_dir_all("./build").ok();
            let dest = Path::new("./build").join(built.file_name().unwrap());
            fs::copy(&built, &dest).ok();
            info!("📦 Copied {:?} -> {:?}", built, dest);
        } else {
            warn!("⚠️ Không tìm thấy file biên dịch cho package {:?} (folder {:?})", pkg_name, dir_name);
        }
    }
}

pub async fn watch_dev(base_path: &str) {
    let (tx, mut rx) = mpsc::channel::<Event>(128);
    let mut watcher = RecommendedWatcher::new(
        move |res| {
            if let Ok(ev) = res {
                let _ = tx.blocking_send(ev);
            }
        },
        Config::default(),
    ).unwrap();

    watcher.watch(Path::new(base_path), RecursiveMode::Recursive).unwrap();

    // 🔁 Lặp vĩnh viễn, rebuild khi có file thay đổi, có debounce và bỏ qua file tạm
    loop {
        if let Some(ev) = rx.recv().await {
            if should_ignore_event(&ev) { continue; }
            // Debounce 400ms: gom nhiều thay đổi liên tiếp
            sleep(Duration::from_millis(400)).await;
            // Drain các sự kiện đến trong thời gian debounce
            while let Ok(ev2) = rx.try_recv() {
                if should_ignore_event(&ev2) { continue; }
            }
            build_and_load(base_path).await;
        }
    }
}

// Watch thư mục build và reload router + cập nhật OpenAPI khi DLL thay đổi
pub async fn watch_prod_build(build_path: &str, live_router: Arc<RwLock<Router>>, live_spec: Option<Arc<RwLock<Value>>>) {
    let (tx, mut rx) = tokio::sync::mpsc::channel::<Event>(128);
    let mut watcher = RecommendedWatcher::new(
        move |res| {
            if let Ok(ev) = res { let _ = tx.blocking_send(ev); }
        },
        Config::default(),
    ).unwrap();

    watcher.watch(Path::new(build_path), RecursiveMode::Recursive).unwrap();

    loop {
        if let Some(ev) = rx.recv().await {
            if should_ignore_event(&ev) { continue; }
            // Debounce nhẹ
            tokio::time::sleep(Duration::from_millis(300)).await;
            while let Ok(ev2) = rx.try_recv() {
                if should_ignore_event(&ev2) { continue; }
            }
            *live_router.write() = build_router_from(build_path);

            // Cập nhật OpenAPI nếu có
            if let Some(spec_lock) = &live_spec {
                let spec = build_openapi_from_modules("./modules", build_path);
                *spec_lock.write() = spec.clone();
            }

            // Nạp lại các feature plugins sau mỗi lần build thay đổi
            let _ = load_features("./features", build_path);

            // Log gộp: hiển thị danh sách routes mới từ OpenAPI
            let spec_for_log = if let Some(spec_lock) = &live_spec { Some(spec_lock.read().clone()) } else { None };
            if let Some(spec) = spec_for_log {
                if let Some(paths) = spec.get("paths").and_then(|p| p.as_object()) {
                    let mut names: Vec<String> = paths.keys().cloned().collect();
                    names.sort();
                    info!("🔄 Router reloaded ({} routes): {}", names.len(), names.join(", "));
                } else {
                    info!("🔄 Router reloaded from {}", build_path);
                }
            } else {
                info!("🔄 Router reloaded from {}", build_path);
            }
        }
    }
}

// Đọc tên package từ Cargo.toml trong thư mục plugin
fn read_package_name(plugin_dir: &Path) -> Option<String> {
    let cargo_toml = plugin_dir.join("Cargo.toml");
    let content = fs::read_to_string(cargo_toml).ok()?;
    // Tìm trong block [package]
    let mut in_package = false;
    for line in content.lines() {
        let l = line.trim();
        if l.starts_with("[") {
            in_package = l == "[package]";
            continue;
        }
        if in_package && l.starts_with("name") {
            // name = "xxx"
            let parts: Vec<&str> = l.split('=').collect();
            if parts.len() >= 2 {
                let val = parts[1].trim().trim_matches('"');
                if !val.is_empty() { return Some(val.to_string()); }
            }
        }
    }
    None
}

fn should_ignore_event(ev: &Event) -> bool {
    fn ignore_path(p: &Path) -> bool {
        if let Some(name) = p.file_name().and_then(|s| s.to_str()) {
            let lower = name.to_ascii_lowercase();
            return lower.ends_with(".tmp") || lower.ends_with(".swp") || lower.ends_with("~") || lower.ends_with(".crdownload");
        }
        false
    }

    // Nếu tất cả path trong event là file tạm -> bỏ qua
    if ev.paths.iter().all(|p| ignore_path(p)) { return true; }
    false
}
