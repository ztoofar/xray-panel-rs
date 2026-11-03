use axum::{
    extract::{Request, State},
    http::{StatusCode, Uri},
    middleware::Next,
    response::{IntoResponse, Response},
};

use crate::state::AppState;

const MAX_BASE_PATH_LEN: usize = 128;

/// Normalize an admin-supplied panel path to `/`, or `/<segments>/`.
/// Rejects anything that could escape the prefix (dots, spaces, query chars).
pub fn normalize(raw: &str) -> Option<String> {
    let raw = raw.trim();
    if raw.is_empty() || raw == "/" {
        return Some("/".to_string());
    }
    let prefixed;
    let raw = if raw.starts_with('/') {
        raw
    } else {
        prefixed = format!("/{raw}");
        prefixed.as_str()
    };
    if raw.len() > MAX_BASE_PATH_LEN {
        return None;
    }
    let mut segments = Vec::new();
    for segment in raw.split('/') {
        if segment.is_empty() {
            continue;
        }
        if segment == "." || segment == ".." {
            return None;
        }
        let safe = segment
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~'));
        if !safe {
            return None;
        }
        segments.push(segment);
    }
    if segments.is_empty() {
        return Some("/".to_string());
    }
    Some(format!("/{}/", segments.join("/")))
}

/// Map a request path to the path the router knows. `None` means the request
/// is outside the configured prefix and must not reach the panel.
pub fn strip(path: &str, base: &str) -> Option<String> {
    let base = base.trim_end_matches('/');
    if base.is_empty() {
        return Some(path.to_string());
    }
    if path == base {
        return Some("/".to_string());
    }
    let rest = path.strip_prefix(base)?;
    rest.starts_with('/').then(|| rest.to_string())
}

/// Rewrite the request URI before routing so a single static router serves
/// both `/` and any configured prefix without restarting the process.
pub async fn guard(State(state): State<AppState>, mut req: Request, next: Next) -> Response {
    let base = state.base_path();
    if base == "/" {
        return next.run(req).await;
    }
    let path = req.uri().path().to_string();
    // Probes should not need to know the secret panel path.
    if path == "/api/health" {
        return next.run(req).await;
    }
    let Some(rewritten) = strip(&path, &base) else {
        return (StatusCode::NOT_FOUND, "not found").into_response();
    };
    if rewritten != path {
        let Some(uri) = rewrite_uri(req.uri(), &rewritten) else {
            return (StatusCode::BAD_REQUEST, "bad request").into_response();
        };
        *req.uri_mut() = uri;
    }
    next.run(req).await
}

fn rewrite_uri(uri: &Uri, path: &str) -> Option<Uri> {
    let mut parts = uri.clone().into_parts();
    let path_and_query = match uri.query() {
        Some(query) => format!("{path}?{query}"),
        None => path.to_string(),
    };
    parts.path_and_query = Some(path_and_query.parse().ok()?);
    Uri::from_parts(parts).ok()
}

#[cfg(test)]
mod tests {
    use super::{normalize, rewrite_uri, strip};
    use axum::http::Uri;

    #[test]
    fn normalize_accepts_plain_segments() {
        assert_eq!(normalize("").as_deref(), Some("/"));
        assert_eq!(normalize("/").as_deref(), Some("/"));
        assert_eq!(normalize("/panel").as_deref(), Some("/panel/"));
        assert_eq!(normalize("panel").as_deref(), Some("/panel/"));
        assert_eq!(normalize("//a//b//").as_deref(), Some("/a/b/"));
        assert_eq!(normalize("/my-panel_2/").as_deref(), Some("/my-panel_2/"));
    }

    #[test]
    fn normalize_rejects_escapes_and_junk() {
        assert_eq!(normalize("/a/../b"), None);
        assert_eq!(normalize("/a/./b"), None);
        assert_eq!(normalize("/a b"), None);
        assert_eq!(normalize("/a?b"), None);
        assert_eq!(normalize(&format!("/{}", "a".repeat(200))), None);
    }

    #[test]
    fn strip_maps_prefixed_paths_to_router_paths() {
        assert_eq!(strip("/xui", "/xui/").as_deref(), Some("/"));
        assert_eq!(strip("/xui/", "/xui/").as_deref(), Some("/"));
        assert_eq!(
            strip("/xui/panel/inbounds", "/xui/").as_deref(),
            Some("/panel/inbounds")
        );
        assert_eq!(strip("/xuix/panel", "/xui/"), None);
        assert_eq!(strip("/", "/xui/"), None);
        assert_eq!(
            strip("/panel/inbounds", "/").as_deref(),
            Some("/panel/inbounds")
        );
    }

    #[test]
    fn rewrite_uri_keeps_query() {
        let uri: Uri = "/xui/api/clients?page=1&q=a".parse().unwrap();
        let rewritten = rewrite_uri(&uri, "/api/clients").unwrap();
        assert_eq!(rewritten.path(), "/api/clients");
        assert_eq!(rewritten.query(), Some("page=1&q=a"));
    }
}
