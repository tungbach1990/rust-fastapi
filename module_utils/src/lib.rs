// Exported macro: read_asset!
#[macro_export]
macro_rules! read_asset {
    ($rel_path:expr) => {{
        let full = concat!(env!("CARGO_MANIFEST_DIR"), "/", $rel_path);
        match std::fs::read_to_string(full) {
            Ok(s) => s,
            Err(_) => {
                let ext = $rel_path.rsplit('.').next().unwrap_or("");
                match ext {
                    "html" => "<html><body><h1>HTML asset not found</h1></body></html>".to_string(),
                    "css" => "/* CSS asset not found */".to_string(),
                    "js" => "console.error('JS asset not found');".to_string(),
                    "txt" => "Text asset not found".to_string(),
                    "xml" => "<error>XML asset not found</error>".to_string(),
                    "json" => "{\"error\":\"JSON asset not found\"}".to_string(),
                    _ => "".to_string(),
                }
            }
        }
    }};
}

// Backward-compatible function (not used by macros but kept for convenience)
pub fn read_asset(path: &str, fallback: &str) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|_| fallback.to_string())
}