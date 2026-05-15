use std::collections::HashMap;
use std::net::IpAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{broadcast, RwLock};

use crate::network::NetIface;

#[derive(Debug, Clone)]
pub struct ClientInfo {
    pub ip: IpAddr,
    pub user_agent: String,
    #[allow(dead_code)]
    pub connected_at: Instant,
    pub last_seen: Instant,
}

#[derive(Debug, Clone)]
pub struct DownloadInfo {
    pub path: String,
    pub client_ip: IpAddr,
    pub bytes_sent: u64,
    pub total_bytes: u64,
    #[allow(dead_code)]
    pub started_at: Instant,
}

pub struct AppState {
    pub root: PathBuf,
    pub start_time: Instant,
    pub http_port: u16,
    pub https_port: Option<u16>,
    #[allow(dead_code)]
    pub webui_port: Option<u16>,
    pub interfaces: Vec<NetIface>,
    pub auth: Option<(String, String)>,
    pub dav_auth: Option<(String, String)>,
    pub cors: bool,
    pub upload: bool,
    pub max_upload_bytes: usize,
    #[allow(dead_code)]
    pub dav_port: Option<u16>,
    pub dav_rw: bool,
    pub clients: RwLock<HashMap<String, ClientInfo>>,
    pub downloads: RwLock<HashMap<String, DownloadInfo>>,
    pub shutdown: broadcast::Sender<()>,
}

impl AppState {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        root: PathBuf,
        http_port: u16,
        https_port: Option<u16>,
        webui_port: Option<u16>,
        interfaces: Vec<NetIface>,
        auth: Option<(String, String)>,
        cors: bool,
        upload: bool,
        max_upload_bytes: usize,
        dav_port: Option<u16>,
        dav_rw: bool,
        dav_auth: Option<(String, String)>,
    ) -> Arc<Self> {
        let (shutdown, _) = broadcast::channel(1);
        Arc::new(Self {
            root,
            start_time: Instant::now(),
            http_port,
            https_port,
            webui_port,
            interfaces,
            auth,
            dav_auth,
            cors,
            upload,
            max_upload_bytes,
            dav_port,
            dav_rw,
            clients: RwLock::new(HashMap::new()),
            downloads: RwLock::new(HashMap::new()),
            shutdown,
        })
    }

    pub fn uptime(&self) -> std::time::Duration {
        self.start_time.elapsed()
    }

    pub fn format_uptime(&self) -> String {
        let secs = self.uptime().as_secs();
        let h = secs / 3600;
        let m = (secs % 3600) / 60;
        let s = secs % 60;
        format!("{h:02}:{m:02}:{s:02}")
    }

    pub async fn add_client(&self, id: String, ip: IpAddr, user_agent: String) {
        let now = Instant::now();
        self.clients.write().await.insert(
            id,
            ClientInfo {
                ip,
                user_agent,
                connected_at: now,
                last_seen: now,
            },
        );
    }

    pub async fn remove_client(&self, id: &str) {
        self.clients.write().await.remove(id);
    }

    pub async fn add_download(&self, id: String, path: String, client_ip: IpAddr, total: u64) {
        self.downloads.write().await.insert(
            id,
            DownloadInfo {
                path,
                client_ip,
                bytes_sent: 0,
                total_bytes: total,
                started_at: Instant::now(),
            },
        );
    }

    pub async fn update_download(&self, id: &str, bytes_sent: u64) {
        if let Some(dl) = self.downloads.write().await.get_mut(id) {
            dl.bytes_sent = bytes_sent;
        }
    }

    pub async fn remove_download(&self, id: &str) {
        self.downloads.write().await.remove(id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_appstate_carries_dav_auth() {
        let state = AppState::new(
            std::env::temp_dir(),
            4701,
            None,
            None,
            vec![NetIface {
                name: "lo".into(),
                ip: std::net::IpAddr::from([127, 0, 0, 1]),
            }],
            None,
            false,
            false,
            2048 * 1024 * 1024,
            Some(5001),
            false,
            Some(("u".into(), "p".into())),
        );
        assert_eq!(state.dav_auth.as_ref().map(|(u, _)| u.as_str()), Some("u"));
    }
}
