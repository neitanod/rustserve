pub mod auth;
pub mod handlers;
pub mod listing;
pub mod security;
pub mod tls;
pub mod tracking;
pub mod upload;

use axum::middleware as mw;
use axum::routing::get;
use axum::Router;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tower_http::cors::{Any, CorsLayer};

use crate::state::AppState;
use handlers::handle_request;
use upload::handle_upload;

pub fn build_router(state: Arc<AppState>) -> Router {
    let mut router = Router::new()
        .route("/", get(handle_request).post(handle_upload))
        .route("/{*path}", get(handle_request).post(handle_upload))
        .layer(upload::upload_body_limit(state.max_upload_bytes))
        .with_state(state.clone());

    if let Some((ref user, ref pass)) = state.auth {
        let u = user.clone();
        let p = pass.clone();
        router = router.layer(mw::from_fn(move |req, next| {
            let u = u.clone();
            let p = p.clone();
            async move { auth::auth_middleware(req, next, u, p).await }
        }));
    }

    if state.cors {
        router = router.layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        );
    }

    let state_for_tracking = state.clone();
    router = router.layer(mw::from_fn(move |req, next| {
        let s = state_for_tracking.clone();
        async move { tracking::tracking_middleware(s, req, next).await }
    }));

    router
}

pub async fn run_http(router: Router, port: u16) -> std::io::Result<()> {
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = TcpListener::bind(addr).await?;
    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
}

pub async fn run_https(
    router: Router,
    port: u16,
    cert_path: &std::path::Path,
    key_path: &std::path::Path,
) -> Result<(), String> {
    let config = tls::tls_config(cert_path, key_path).await?;
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    axum_server::bind_rustls(addr, config)
        .serve(router.into_make_service())
        .await
        .map_err(|e| format!("HTTPS server error: {e}"))
}
