use axum::extract::{DefaultBodyLimit, Multipart, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use futures::TryStreamExt;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;

use crate::state::AppState;

pub fn upload_body_limit(max_bytes: usize) -> DefaultBodyLimit {
    DefaultBodyLimit::max(max_bytes)
}

pub async fn handle_upload(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    mut multipart: Multipart,
) -> Response {
    if !state.upload {
        return (StatusCode::FORBIDDEN, "Uploads disabled").into_response();
    }

    let referer = headers
        .get("referer")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("/");

    let redirect_path = url::Url::parse(referer)
        .map(|u| u.path().to_string())
        .unwrap_or_else(|_| "/".to_string());

    while let Ok(Some(mut field)) = multipart.next_field().await {
        let file_name = match field.file_name() {
            Some(name) => sanitize_filename(name),
            None => continue,
        };

        if file_name.is_empty() {
            continue;
        }

        let sub_path = redirect_path.trim_start_matches('/');
        let target_dir = if sub_path.is_empty() {
            state.root.clone()
        } else {
            state.root.join(sub_path)
        };

        if !target_dir.starts_with(&state.root) {
            return (StatusCode::FORBIDDEN, "Invalid path").into_response();
        }

        let dest = target_dir.join(&file_name);
        if !dest.starts_with(&state.root) {
            return (StatusCode::FORBIDDEN, "Invalid filename").into_response();
        }

        let mut file = match tokio::fs::File::create(&dest).await {
            Ok(f) => f,
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Write error: {e}"),
                )
                    .into_response()
            }
        };

        while let Ok(Some(chunk)) = field.try_next().await {
            if file.write_all(&chunk).await.is_err() {
                return (StatusCode::INTERNAL_SERVER_ERROR, "Write error").into_response();
            }
        }
    }

    Redirect::to(&redirect_path).into_response()
}

fn sanitize_filename(name: &str) -> String {
    name.replace(['/', '\\', '\0'], "")
        .trim_start_matches('.')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_filename() {
        assert_eq!(sanitize_filename("normal.txt"), "normal.txt");
        assert_eq!(sanitize_filename("..etcpasswd"), "etcpasswd");
        assert_eq!(sanitize_filename(".hidden"), "hidden");
        assert_eq!(sanitize_filename("a/b\\c"), "abc");
    }
}
