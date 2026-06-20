use std::path::{Path, PathBuf};

use dioxus::desktop::use_asset_handler;
use dioxus::desktop::wry::RequestAsyncResponder;
use http::Response;

use crate::shared::{WORKSPACE_ASSET_SCHEME, WORKSPACE_ASSET_URL_PREFIX};

/// `<img src="/workspace-asset/{path}">` のような URL を `{workspace_dir}/{path}` に解決する。
///
/// 例: `src="/workspace-asset/data/characters/MooR_01/sprite-groups/thumbnail/sprites/portrait_001.png"`
/// → `{workspace}/data/characters/MooR_01/sprite-groups/thumbnail/sprites/portrait_001.png`
///
/// パストラバーサル対策として canonicalize 後に workspace_dir 配下にあるかを検証する。
pub fn use_workspace_asset_handler(workspace_dir: PathBuf) {
    let canonical_root = workspace_dir.canonicalize().unwrap_or(workspace_dir);

    use_asset_handler(WORKSPACE_ASSET_SCHEME, move |req, responder| {
        let path = req.uri().path().to_string();
        let rel = path
            .strip_prefix(WORKSPACE_ASSET_URL_PREFIX)
            .unwrap_or(path.as_str());
        let full = canonical_root.join(rel);

        let canonical = match full.canonicalize() {
            Ok(p) if p.starts_with(&canonical_root) => p,
            _ => {
                respond_status(responder, 404);
                return;
            }
        };

        match std::fs::read(&canonical) {
            Ok(bytes) => {
                let mime = mime_for(&canonical);
                let resp = Response::builder()
                    .header("Content-Type", mime)
                    .header("Cache-Control", "max-age=3600")
                    .body(bytes)
                    .expect("response should build");
                responder.respond(resp);
            }
            Err(_) => respond_status(responder, 404),
        }
    });
}

fn respond_status(responder: RequestAsyncResponder, code: u16) {
    let resp = Response::builder()
        .status(code)
        .body(Vec::new())
        .expect("static response should build");
    responder.respond(resp);
}

fn mime_for(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("svg") => "image/svg+xml",
        Some("wav") => "audio/wav",
        Some("mp3") => "audio/mpeg",
        Some("ogg") => "audio/ogg",
        _ => "application/octet-stream",
    }
}
