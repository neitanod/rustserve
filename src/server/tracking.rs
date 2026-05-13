use axum::body::Body;
use axum::extract::Request;
use axum::http::header;
use axum::middleware::Next;
use axum::response::Response;
use bytes::Bytes;
use http_body::Frame;
use std::net::IpAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use uuid::Uuid;

use crate::state::AppState;

pub async fn tracking_middleware(
    state: Arc<AppState>,
    req: Request,
    next: Next,
) -> Response {
    let client_ip = req
        .extensions()
        .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
        .map(|ci| ci.0.ip())
        .unwrap_or(IpAddr::from([0, 0, 0, 0]));

    let user_agent = req
        .headers()
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown")
        .to_string();

    let path = req.uri().path().to_string();
    let client_id = format!("{}:{}", client_ip, Uuid::new_v4());

    state
        .add_client(client_id.clone(), client_ip, user_agent)
        .await;

    let response = next.run(req).await;

    let total_bytes = response
        .headers()
        .get(header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0);

    let is_file_download = total_bytes > 0
        && !response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .contains("text/html");

    if is_file_download {
        let download_id = Uuid::new_v4().to_string();
        state
            .add_download(download_id.clone(), path, client_ip, total_bytes)
            .await;

        let (parts, body) = response.into_parts();
        let tracked_body = TrackedBody {
            inner: body,
            state: state.clone(),
            download_id,
            client_id,
            bytes_sent: 0,
        };
        Response::from_parts(parts, Body::new(tracked_body))
    } else {
        let state_clone = state.clone();
        let cid = client_id.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            state_clone.remove_client(&cid).await;
        });
        response
    }
}

struct TrackedBody {
    inner: Body,
    state: Arc<AppState>,
    download_id: String,
    client_id: String,
    bytes_sent: u64,
}

impl http_body::Body for TrackedBody {
    type Data = Bytes;
    type Error = axum::Error;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        let this = unsafe { self.get_unchecked_mut() };
        let inner = unsafe { Pin::new_unchecked(&mut this.inner) };

        match inner.poll_frame(cx) {
            Poll::Ready(Some(Ok(frame))) => {
                if let Some(data) = frame.data_ref() {
                    this.bytes_sent += data.len() as u64;
                    let state = this.state.clone();
                    let did = this.download_id.clone();
                    let sent = this.bytes_sent;
                    tokio::spawn(async move {
                        state.update_download(&did, sent).await;
                    });
                }
                Poll::Ready(Some(Ok(frame)))
            }
            Poll::Ready(None) => {
                let state = this.state.clone();
                let did = this.download_id.clone();
                let cid = this.client_id.clone();
                tokio::spawn(async move {
                    state.remove_download(&did).await;
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    state.remove_client(&cid).await;
                });
                Poll::Ready(None)
            }
            other => other,
        }
    }
}
