use axum::extract::Request;
use axum::http::{header, StatusCode};
use axum::middleware::Next;
use axum::response::Response;

pub async fn auth_middleware(
    req: Request,
    next: Next,
    expected_user: String,
    expected_pass: String,
) -> Response {
    if let Some(auth_header) = req.headers().get(header::AUTHORIZATION) {
        if let Ok(auth_str) = auth_header.to_str() {
            if let Some(credentials) = auth_str.strip_prefix("Basic ") {
                if let Ok(decoded) =
                    base64::Engine::decode(&base64::engine::general_purpose::STANDARD, credentials)
                {
                    if let Ok(decoded_str) = String::from_utf8(decoded) {
                        if let Some((user, pass)) = decoded_str.split_once(':') {
                            if user == expected_user && pass == expected_pass {
                                return next.run(req).await;
                            }
                        }
                    }
                }
            }
        }
    }

    Response::builder()
        .status(StatusCode::UNAUTHORIZED)
        .header(header::WWW_AUTHENTICATE, r#"Basic realm="serve""#)
        .body(axum::body::Body::from("Unauthorized"))
        .unwrap()
}
