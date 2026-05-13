pub mod assets;

use axum::extract::ws::{Message, WebSocket};
use axum::extract::{State, WebSocketUpgrade};
use axum::response::{Html, IntoResponse};
use axum::routing::get;
use axum::Router;
use serde::Serialize;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;

use crate::network::format_urls;
use crate::state::AppState;

#[derive(Serialize)]
struct WsUpdate {
    urls: Vec<String>,
    clients: Vec<WsClient>,
    downloads: Vec<WsDownload>,
    uptime: String,
}

#[derive(Serialize)]
struct WsClient {
    ip: String,
    user_agent: String,
    last_seen: String,
}

#[derive(Serialize)]
struct WsDownload {
    file: String,
    client: String,
    sent: u64,
    total: u64,
}

pub async fn run_webui(state: Arc<AppState>, port: u16) -> std::io::Result<()> {
    let router = Router::new()
        .route("/", get(admin_page))
        .route("/ws", get(ws_handler))
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = TcpListener::bind(addr).await?;
    axum::serve(listener, router).await
}

async fn admin_page() -> impl IntoResponse {
    Html(assets::ADMIN_HTML)
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| ws_connection(socket, state))
}

async fn ws_connection(mut socket: WebSocket, state: Arc<AppState>) {
    loop {
        let update = build_update(&state).await;
        let json = serde_json::to_string(&update).unwrap_or_default();

        if socket.send(Message::Text(json.into())).await.is_err() {
            break;
        }

        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
}

async fn build_update(state: &Arc<AppState>) -> WsUpdate {
    let mut urls = format_urls(&state.interfaces, state.http_port, false);
    if let Some(ssl_port) = state.https_port {
        urls.extend(format_urls(&state.interfaces, ssl_port, true));
    }

    let clients_map = state.clients.read().await;
    let clients: Vec<WsClient> = clients_map
        .values()
        .map(|c| WsClient {
            ip: c.ip.to_string(),
            user_agent: c.user_agent.chars().take(50).collect(),
            last_seen: format!("{}s ago", c.last_seen.elapsed().as_secs()),
        })
        .collect();

    let downloads_map = state.downloads.read().await;
    let downloads: Vec<WsDownload> = downloads_map
        .values()
        .map(|d| WsDownload {
            file: d.path.rsplit('/').next().unwrap_or(&d.path).to_string(),
            client: d.client_ip.to_string(),
            sent: d.bytes_sent,
            total: d.total_bytes,
        })
        .collect();

    WsUpdate {
        urls,
        clients,
        downloads,
        uptime: state.format_uptime(),
    }
}
