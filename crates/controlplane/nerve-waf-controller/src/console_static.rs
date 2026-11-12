//! Built-in WAF Console static assets (`/opt/nerve-waf/console` in Docker images).

use axum::Router;
use std::path::PathBuf;
use tower_http::services::{ServeDir, ServeFile};

const DEFAULT_CONSOLE_DIR: &str = "/opt/nerve-waf/console";

pub fn resolve_console_dir() -> Option<PathBuf> {
    let dir = match std::env::var("WAF_CONSOLE_DIR") {
        Ok(raw) if raw.trim().is_empty() => return None,
        Ok(raw) => PathBuf::from(raw.trim()),
        Err(_) => PathBuf::from(DEFAULT_CONSOLE_DIR),
    };
    if dir.join("index.html").is_file() {
        Some(dir)
    } else {
        None
    }
}

pub fn attach_console_fallback<S>(router: Router<S>) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    let Some(dir) = resolve_console_dir() else {
        tracing::info!(
            "WAF Console UI disabled (no index.html under WAF_CONSOLE_DIR or {DEFAULT_CONSOLE_DIR})"
        );
        return router;
    };
    tracing::info!(
        "WAF Console UI http://{{host}}:{{port}}/  ({})",
        dir.display()
    );
    let index = dir.join("index.html");
    let service = ServeDir::new(dir).not_found_service(ServeFile::new(index));
    router.fallback_service(service)
}
