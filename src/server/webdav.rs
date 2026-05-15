use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{HeaderMap, Method, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::routing::any;
use axum::Router;
use futures::TryStreamExt;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;

use super::webdav_xml::{self, DavEntry};
use crate::state::AppState;

pub fn build_webdav_router(state: Arc<AppState>) -> Router {
    let mut router = Router::new()
        .route("/", any(webdav_dispatch))
        .route("/{*path}", any(webdav_dispatch))
        .with_state(state.clone());

    if state.dav_rw {
        router = router.layer(super::upload::upload_body_limit(state.max_upload_bytes));
    }

    if let Some((ref user, ref pass)) = state.dav_auth {
        let u = user.clone();
        let p = pass.clone();
        router = router.layer(axum::middleware::from_fn(move |req, next| {
            let u = u.clone();
            let p = p.clone();
            async move { super::auth::auth_middleware(req, next, u, p).await }
        }));
    }

    let state_for_tracking = state.clone();
    router = router.layer(axum::middleware::from_fn(move |req, next| {
        let s = state_for_tracking.clone();
        async move { super::tracking::tracking_middleware(s, req, next).await }
    }));

    router
}

pub async fn run_webdav(router: Router, port: u16) -> std::io::Result<()> {
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = TcpListener::bind(addr).await?;
    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
}

async fn webdav_dispatch(State(state): State<Arc<AppState>>, req: Request) -> Response {
    let method = req.method().clone();

    if method == Method::GET || method == Method::HEAD {
        return super::handlers::handle_request(State(state), req).await;
    }

    let (parts, body) = req.into_parts();
    let uri = parts.uri;
    let headers = parts.headers;

    match method.as_str() {
        "OPTIONS" => dav_options(&state),
        "PROPFIND" => dav_propfind(&state, &uri, &headers).await,
        "LOCK" => dav_lock(),
        "UNLOCK" => StatusCode::NO_CONTENT.into_response(),
        "PUT" => dav_put(&state, &uri, body).await,
        "DELETE" => dav_delete(&state, &uri).await,
        "MKCOL" => dav_mkcol(&state, &uri).await,
        "COPY" => dav_copy(&state, &uri, &headers).await,
        "MOVE" => dav_move(&state, &uri, &headers).await,
        _ => StatusCode::METHOD_NOT_ALLOWED.into_response(),
    }
}

fn dav_options(state: &AppState) -> Response {
    let methods = if state.dav_rw {
        "OPTIONS, PROPFIND, GET, HEAD, PUT, DELETE, MKCOL, COPY, MOVE, LOCK, UNLOCK"
    } else {
        "OPTIONS, PROPFIND, GET, HEAD, LOCK, UNLOCK"
    };
    Response::builder()
        .status(StatusCode::OK)
        .header("DAV", "1,2")
        .header("Allow", methods)
        .header("Ms-Author-Via", "DAV")
        .header("Content-Length", "0")
        .body(Body::empty())
        .unwrap()
}

async fn dav_propfind(state: &AppState, uri: &Uri, headers: &HeaderMap) -> Response {
    let depth = headers
        .get("depth")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("1");

    let request_path = uri.path();
    let decoded = percent_encoding::percent_decode_str(request_path).decode_utf8_lossy();
    let clean = decoded.trim_start_matches('/');

    let fs_path = if clean.is_empty() {
        state.root.clone()
    } else {
        state.root.join(clean)
    };

    let fs_path = match fs_path.canonicalize() {
        Ok(p) if p.starts_with(&state.root) => p,
        _ => return StatusCode::NOT_FOUND.into_response(),
    };

    let meta = match std::fs::metadata(&fs_path) {
        Ok(m) => m,
        Err(_) => return StatusCode::NOT_FOUND.into_response(),
    };

    let mut entries = Vec::new();

    let display_name = if clean.is_empty() {
        "/".to_string()
    } else {
        clean
            .trim_end_matches('/')
            .rsplit('/')
            .next()
            .unwrap_or(clean)
            .to_string()
    };

    entries.push(DavEntry {
        href: ensure_trailing_slash(request_path, meta.is_dir()),
        display_name,
        is_dir: meta.is_dir(),
        size: if meta.is_dir() { 0 } else { meta.len() },
        modified: meta.modified().ok(),
        content_type: if meta.is_dir() {
            "httpd/unix-directory".into()
        } else {
            mime_guess::from_path(&fs_path)
                .first_or_octet_stream()
                .to_string()
        },
    });

    if depth != "0" && meta.is_dir() {
        if let Ok(children) = super::listing::scan_directory(&fs_path) {
            let base = ensure_trailing_slash(request_path, true);
            for child in &children {
                let child_href = format!(
                    "{}{}",
                    base,
                    percent_encoding::utf8_percent_encode(
                        &child.name,
                        percent_encoding::NON_ALPHANUMERIC
                    )
                );
                entries.push(DavEntry {
                    href: ensure_trailing_slash(&child_href, child.is_dir),
                    display_name: child.name.clone(),
                    is_dir: child.is_dir,
                    size: child.size,
                    modified: child.modified,
                    content_type: if child.is_dir {
                        "httpd/unix-directory".into()
                    } else {
                        child.mime_type.clone()
                    },
                });
            }
        }
    }

    let xml = webdav_xml::multistatus(&entries);
    Response::builder()
        .status(StatusCode::MULTI_STATUS)
        .header("content-type", "application/xml; charset=utf-8")
        .body(Body::from(xml))
        .unwrap()
}

fn dav_lock() -> Response {
    let token = uuid::Uuid::new_v4().to_string();
    let xml = webdav_xml::lock_discovery(&token);
    let lock_header = format!("<urn:uuid:{token}>");
    Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "application/xml; charset=utf-8")
        .header("lock-token", lock_header)
        .body(Body::from(xml))
        .unwrap()
}

async fn dav_put(state: &AppState, uri: &Uri, body: Body) -> Response {
    if !state.dav_rw {
        return (StatusCode::FORBIDDEN, "WebDAV read-only").into_response();
    }

    let path = match resolve_new_path(state, uri) {
        Ok(p) => p,
        Err(s) => return s.into_response(),
    };

    let existed = path.exists();

    let mut file = match tokio::fs::File::create(&path).await {
        Ok(f) => f,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    let mut stream = body.into_data_stream();
    while let Ok(Some(chunk)) = stream.try_next().await {
        if file.write_all(&chunk).await.is_err() {
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    }

    if existed {
        StatusCode::NO_CONTENT.into_response()
    } else {
        StatusCode::CREATED.into_response()
    }
}

async fn dav_delete(state: &AppState, uri: &Uri) -> Response {
    if !state.dav_rw {
        return (StatusCode::FORBIDDEN, "WebDAV read-only").into_response();
    }

    let path = match resolve_existing_path(state, uri) {
        Ok(p) => p,
        Err(s) => return s.into_response(),
    };

    if path == state.root {
        return StatusCode::FORBIDDEN.into_response();
    }

    let result = if path.is_dir() {
        tokio::fs::remove_dir_all(&path).await
    } else {
        tokio::fs::remove_file(&path).await
    };

    match result {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn dav_mkcol(state: &AppState, uri: &Uri) -> Response {
    if !state.dav_rw {
        return (StatusCode::FORBIDDEN, "WebDAV read-only").into_response();
    }

    let path = match resolve_new_path(state, uri) {
        Ok(p) => p,
        Err(s) => return s.into_response(),
    };

    match tokio::fs::create_dir(&path).await {
        Ok(()) => StatusCode::CREATED.into_response(),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            StatusCode::METHOD_NOT_ALLOWED.into_response()
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn dav_copy(state: &AppState, uri: &Uri, headers: &HeaderMap) -> Response {
    if !state.dav_rw {
        return (StatusCode::FORBIDDEN, "WebDAV read-only").into_response();
    }

    let src = match resolve_existing_path(state, uri) {
        Ok(p) => p,
        Err(s) => return s.into_response(),
    };

    let dst = match resolve_destination(state, headers) {
        Ok(p) => p,
        Err(s) => return s.into_response(),
    };

    let overwrite = headers
        .get("overwrite")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("T")
        == "T";

    if dst.exists() && !overwrite {
        return StatusCode::PRECONDITION_FAILED.into_response();
    }

    let existed = dst.exists();
    let src_clone = src.clone();
    let dst_clone = dst.clone();

    let result = tokio::task::spawn_blocking(move || copy_recursive(&src_clone, &dst_clone)).await;

    match result {
        Ok(Ok(())) => {
            if existed {
                StatusCode::NO_CONTENT.into_response()
            } else {
                StatusCode::CREATED.into_response()
            }
        }
        _ => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn dav_move(state: &AppState, uri: &Uri, headers: &HeaderMap) -> Response {
    if !state.dav_rw {
        return (StatusCode::FORBIDDEN, "WebDAV read-only").into_response();
    }

    let src = match resolve_existing_path(state, uri) {
        Ok(p) => p,
        Err(s) => return s.into_response(),
    };

    if src == state.root {
        return StatusCode::FORBIDDEN.into_response();
    }

    let dst = match resolve_destination(state, headers) {
        Ok(p) => p,
        Err(s) => return s.into_response(),
    };

    let overwrite = headers
        .get("overwrite")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("T")
        == "T";

    if dst.exists() && !overwrite {
        return StatusCode::PRECONDITION_FAILED.into_response();
    }

    let existed = dst.exists();

    if existed {
        let _ = if dst.is_dir() {
            tokio::fs::remove_dir_all(&dst).await
        } else {
            tokio::fs::remove_file(&dst).await
        };
    }

    match tokio::fs::rename(&src, &dst).await {
        Ok(()) => {
            if existed {
                StatusCode::NO_CONTENT.into_response()
            } else {
                StatusCode::CREATED.into_response()
            }
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

fn resolve_existing_path(state: &AppState, uri: &Uri) -> Result<PathBuf, StatusCode> {
    let decoded = percent_encoding::percent_decode_str(uri.path()).decode_utf8_lossy();
    let clean = decoded.trim_start_matches('/');
    let fs_path = if clean.is_empty() {
        state.root.clone()
    } else {
        state.root.join(clean)
    };
    fs_path
        .canonicalize()
        .ok()
        .filter(|p| p.starts_with(&state.root))
        .ok_or(StatusCode::NOT_FOUND)
}

fn resolve_new_path(state: &AppState, uri: &Uri) -> Result<PathBuf, StatusCode> {
    let decoded = percent_encoding::percent_decode_str(uri.path()).decode_utf8_lossy();
    let clean = decoded.trim_start_matches('/');
    if clean.is_empty() {
        return Err(StatusCode::FORBIDDEN);
    }
    let fs_path = state.root.join(clean);
    let parent = fs_path.parent().ok_or(StatusCode::FORBIDDEN)?;
    let canonical_parent = parent.canonicalize().map_err(|_| StatusCode::CONFLICT)?;
    if !canonical_parent.starts_with(&state.root) {
        return Err(StatusCode::FORBIDDEN);
    }
    let filename = fs_path.file_name().ok_or(StatusCode::FORBIDDEN)?;
    Ok(canonical_parent.join(filename))
}

fn resolve_destination(state: &AppState, headers: &HeaderMap) -> Result<PathBuf, StatusCode> {
    let dest_str = headers
        .get("destination")
        .and_then(|v| v.to_str().ok())
        .ok_or(StatusCode::BAD_REQUEST)?;

    let path = url::Url::parse(dest_str)
        .map(|u| u.path().to_string())
        .unwrap_or_else(|_| dest_str.to_string());

    let decoded = percent_encoding::percent_decode_str(&path).decode_utf8_lossy();
    let clean = decoded.trim_start_matches('/');
    if clean.is_empty() {
        return Err(StatusCode::FORBIDDEN);
    }
    let fs_path = state.root.join(clean);
    let parent = fs_path.parent().ok_or(StatusCode::FORBIDDEN)?;
    let canonical_parent = parent.canonicalize().map_err(|_| StatusCode::CONFLICT)?;
    if !canonical_parent.starts_with(&state.root) {
        return Err(StatusCode::FORBIDDEN);
    }
    let filename = fs_path.file_name().ok_or(StatusCode::FORBIDDEN)?;
    Ok(canonical_parent.join(filename))
}

fn ensure_trailing_slash(path: &str, is_dir: bool) -> String {
    if is_dir && !path.ends_with('/') {
        format!("{path}/")
    } else {
        path.to_string()
    }
}

fn copy_recursive(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    if src.is_dir() {
        std::fs::create_dir_all(dst)?;
        for entry in std::fs::read_dir(src)? {
            let entry = entry?;
            copy_recursive(&entry.path(), &dst.join(entry.file_name()))?;
        }
    } else {
        std::fs::copy(src, dst)?;
    }
    Ok(())
}
