use std::path::Path;

pub fn path_to_route(base: &str, path: &Path) -> Option<String> {
    let rel = path.strip_prefix(base).ok()?;
    let mut s = rel.to_string_lossy().replace('\\', "/");
    if let Some(idx) = s.rfind('.') { s.truncate(idx); }
    if s.starts_with("api/") {
        Some(format!("/{}", s))
    } else {
        Some(format!("/api/{}", s))
    }
}
