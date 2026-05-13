use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{header, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use std::sync::Arc;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio_util::io::ReaderStream;

use crate::state::AppState;
use super::listing::{render_listing, scan_directory};
use super::security::{validate_path, SecurityError};

pub async fn handle_request(
    State(state): State<Arc<AppState>>,
    req: Request,
) -> Response {
    let request_path = req.uri().path().to_string();
    let range_header = req
        .headers()
        .get(header::RANGE)
        .and_then(|v| v.to_str().ok().map(|s| s.to_string()));

    match validate_path(&state.root, &request_path) {
        Ok(full_path) => {
            if full_path.is_dir() {
                let index_html = full_path.join("index.html");
                if index_html.is_file() {
                    return serve_file(&index_html, range_header.as_deref()).await;
                }
                match scan_directory(&full_path) {
                    Ok(entries) => {
                        let html = render_listing(&request_path, &entries, state.upload);
                        Html(html).into_response()
                    }
                    Err(e) => (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("Error reading directory: {e}"),
                    )
                        .into_response(),
                }
            } else {
                serve_file(&full_path, range_header.as_deref()).await
            }
        }
        Err(SecurityError::NotFound) => {
            (StatusCode::NOT_FOUND, "Not found").into_response()
        }
        Err(SecurityError::TraversalAttempt) => {
            (StatusCode::FORBIDDEN, "Forbidden").into_response()
        }
        Err(SecurityError::InvalidEncoding) => {
            (StatusCode::BAD_REQUEST, "Bad request").into_response()
        }
    }
}

async fn serve_file(path: &std::path::Path, range: Option<&str>) -> Response {
    let mime = mime_guess::from_path(path)
        .first_or_octet_stream()
        .to_string();

    let meta = match tokio::fs::metadata(path).await {
        Ok(m) => m,
        Err(_) => return (StatusCode::NOT_FOUND, "Not found").into_response(),
    };
    let total_size = meta.len();

    if let Some(range_str) = range {
        if let Some((start, end)) = parse_range(range_str, total_size) {
            let len = end - start + 1;
            let mut file = match File::open(path).await {
                Ok(f) => f,
                Err(_) => {
                    return (StatusCode::INTERNAL_SERVER_ERROR, "Read error").into_response()
                }
            };
            if file.seek(std::io::SeekFrom::Start(start)).await.is_err() {
                return (StatusCode::INTERNAL_SERVER_ERROR, "Seek error").into_response();
            }
            let stream = ReaderStream::new(file.take(len));
            let body = Body::from_stream(stream);
            return Response::builder()
                .status(StatusCode::PARTIAL_CONTENT)
                .header(header::CONTENT_TYPE, &mime)
                .header(header::CONTENT_LENGTH, len)
                .header(
                    header::CONTENT_RANGE,
                    format!("bytes {start}-{end}/{total_size}"),
                )
                .header(header::ACCEPT_RANGES, "bytes")
                .body(body)
                .unwrap();
        }
    }

    let file = match File::open(path).await {
        Ok(f) => f,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Read error").into_response(),
    };
    let stream = ReaderStream::new(file);
    let body = Body::from_stream(stream);
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, &mime)
        .header(header::CONTENT_LENGTH, total_size)
        .header(header::ACCEPT_RANGES, "bytes")
        .body(body)
        .unwrap()
}

fn parse_range(range: &str, total: u64) -> Option<(u64, u64)> {
    let range = range.strip_prefix("bytes=")?;
    if let Some(suffix_str) = range.strip_prefix('-') {
        let suffix: u64 = suffix_str.parse().ok()?;
        if suffix == 0 || suffix > total {
            return None;
        }
        let start = total - suffix;
        return Some((start, total - 1));
    }
    let parts: Vec<&str> = range.splitn(2, '-').collect();
    if parts.len() != 2 {
        return None;
    }
    let start: u64 = parts[0].parse().ok()?;
    let end: u64 = if parts[1].is_empty() {
        total - 1
    } else {
        parts[1].parse().ok()?
    };
    if start > end || start >= total {
        return None;
    }
    Some((start, end.min(total - 1)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_range_full() {
        assert_eq!(parse_range("bytes=0-499", 1000), Some((0, 499)));
    }

    #[test]
    fn test_parse_range_open_end() {
        assert_eq!(parse_range("bytes=500-", 1000), Some((500, 999)));
    }

    #[test]
    fn test_parse_range_suffix() {
        assert_eq!(parse_range("bytes=-200", 1000), Some((800, 999)));
    }

    #[test]
    fn test_parse_range_invalid() {
        assert_eq!(parse_range("bytes=999-100", 1000), None);
    }
}
