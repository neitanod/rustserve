# `serve` — File Server Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a Rust file server (`serve`) with TUI dashboard, optional Web UI, TLS, auth, CORS, and upload support.

**Architecture:** Axum HTTP server with shared `Arc<AppState>` state, ratatui TUI running on the main thread, optional Web UI admin panel via WebSocket. The server spawns as tokio tasks; the TUI blocks the main thread and polls shared state for rendering. Graceful shutdown via tokio broadcast channel.

**Tech Stack:** Rust, Tokio, Axum, ratatui + crossterm, rustls, clap, serde

**PRD:** `/home/sebas/robotin/apps/rustserve/PRD.md`

---

## File Structure

```
rustserve/
├── Cargo.toml
├── PRD.md
├── src/
│   ├── main.rs              # Entry point: parse CLI, wire everything, run
│   ├── cli.rs               # Clap CLI definition
│   ├── state.rs             # Arc<AppState>: clients, downloads, uptime, config
│   ├── ports.rs             # Port finding (try+increment or fail)
│   ├── network.rs           # Discover local network interfaces & IPs
│   ├── server/
│   │   ├── mod.rs           # Build axum Router, launch server tasks
│   │   ├── handlers.rs      # File serving & directory listing dispatch
│   │   ├── listing.rs       # HTML generation for directory listing
│   │   ├── security.rs      # Path validation & traversal prevention
│   │   ├── auth.rs          # Basic Auth middleware (optional)
│   │   ├── cors.rs          # CORS middleware (optional)
│   │   ├── upload.rs        # Upload handler (optional)
│   │   ├── tracking.rs      # Download tracking middleware (bytes sent, active clients)
│   │   └── tls.rs           # TLS setup with rustls
│   ├── tui/
│   │   ├── mod.rs           # TUI entry: setup terminal, run event loop, restore
│   │   ├── app.rs           # TUI app state: scroll positions, selected tab
│   │   └── ui.rs            # TUI rendering: layout, panels, widgets
│   └── webui/
│       ├── mod.rs           # Web UI axum server + WebSocket handler
│       └── assets.rs        # Embedded HTML/CSS/JS (include_str!)
```

**Responsibilities by file:**

| File | Responsibility | Key types/functions |
|------|---------------|-------------------|
| `cli.rs` | CLI arg parsing | `Cli` struct (clap derive) |
| `state.rs` | Shared mutable state | `AppState`, `ClientInfo`, `DownloadInfo` |
| `ports.rs` | Find free ports | `find_port(default, explicit) -> Result<u16>` |
| `network.rs` | List local IPs | `local_interfaces() -> Vec<NetInterface>` |
| `server/mod.rs` | Router + server launch | `build_router()`, `run_server()` |
| `server/handlers.rs` | Serve files/dirs | `handle_path()` |
| `server/listing.rs` | HTML dir listing | `render_listing()` |
| `server/security.rs` | Path validation | `validate_path()` |
| `server/auth.rs` | Basic Auth layer | `auth_middleware()` |
| `server/cors.rs` | CORS layer | `cors_layer()` |
| `server/upload.rs` | File upload | `handle_upload()` |
| `server/tracking.rs` | Track downloads | `TrackingBody` wrapper |
| `server/tls.rs` | TLS config | `tls_config()` |
| `tui/mod.rs` | Terminal setup/teardown | `run_tui()` |
| `tui/app.rs` | TUI interaction state | `TuiApp` |
| `tui/ui.rs` | Render TUI frames | `draw_ui()` |
| `webui/mod.rs` | Admin WebSocket server | `run_webui()` |
| `webui/assets.rs` | Embedded web assets | `ADMIN_HTML`, `ADMIN_JS` |

---

## Chunk 1: Foundation (CLI, State, Ports, Network)

### Task 1: Initialize Cargo project

**Files:**
- Create: `Cargo.toml`
- Create: `src/main.rs`

- [ ] **Step 1: Create the project**

```bash
cd /home/sebas/robotin/apps/rustserve
cargo init --name serve
```

- [ ] **Step 2: Set up Cargo.toml with all dependencies**

Replace `Cargo.toml` with:

```toml
[package]
name = "serve"
version = "0.1.0"
edition = "2021"

[dependencies]
axum = { version = "0.8", features = ["multipart"] }
axum-server = { version = "0.7", features = ["tls-rustls"] }
clap = { version = "4", features = ["derive"] }
crossterm = { version = "0.28", features = ["event-stream"] }
chrono = "0.4"
futures = "0.3"
humansize = "2"
hyper = { version = "1", features = ["full"] }
mime_guess = "2"
network-interface = "2"
ratatui = "0.29"
rustls = "0.23"
rustls-pemfile = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }
tokio-tungstenite = "0.24"
tokio-util = { version = "0.7", features = ["io"] }
tower = { version = "0.5", features = ["util"] }
tower-http = { version = "0.6", features = ["cors", "set-header"] }
http-body = "1"
http-body-util = "0.1"
bytes = "1"
percent-encoding = "2"
open = "5"

[profile.release]
strip = true
lto = true
```

- [ ] **Step 3: Minimal main.rs to verify it compiles**

```rust
fn main() {
    println!("serve starting...");
}
```

- [ ] **Step 4: Verify compilation**

Run: `cargo build`
Expected: Compiles successfully (downloads deps on first run)

---

### Task 2: CLI argument parsing

**Files:**
- Create: `src/cli.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Write cli.rs**

```rust
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "serve", about = "File server with TUI and Web UI")]
pub struct Cli {
    /// Directory to serve
    #[arg(default_value = ".")]
    pub dir: PathBuf,

    /// HTTP port (default: 4701, auto-finds free port if not specified)
    #[arg(long)]
    pub port: Option<u16>,

    /// HTTPS port (default: 4801, requires --cert and --key)
    #[arg(long)]
    pub port_ssl: Option<u16>,

    /// TLS certificate file (PEM)
    #[arg(long)]
    pub cert: Option<PathBuf>,

    /// TLS private key file (PEM)
    #[arg(long)]
    pub key: Option<PathBuf>,

    /// Enable Web UI admin panel
    #[arg(long)]
    pub web_ui: bool,

    /// Web UI port (default: auto-find from 4901)
    #[arg(long)]
    pub port_gui: Option<u16>,

    /// Basic Auth username (requires --pass)
    #[arg(long)]
    pub user: Option<String>,

    /// Basic Auth password (requires --user)
    #[arg(long)]
    pub pass: Option<String>,

    /// Enable CORS headers
    #[arg(long)]
    pub cors: bool,

    /// Allow file uploads
    #[arg(long)]
    pub upload: bool,
}

impl Cli {
    pub fn validate(&self) -> Result<(), String> {
        if self.cert.is_some() != self.key.is_some() {
            return Err("--cert and --key must be used together".into());
        }
        if self.user.is_some() != self.pass.is_some() {
            return Err("--user and --pass must be used together".into());
        }
        if let Some(ref cert) = self.cert {
            if !cert.exists() {
                return Err(format!("cert file not found: {}", cert.display()));
            }
        }
        if let Some(ref key) = self.key {
            if !key.exists() {
                return Err(format!("key file not found: {}", key.display()));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_cert_without_key() {
        let cli = Cli {
            dir: ".".into(),
            port: None,
            port_ssl: None,
            cert: Some("/tmp/cert.pem".into()),
            key: None,
            web_ui: false,
            port_gui: None,
            user: None,
            pass: None,
            cors: false,
            upload: false,
        };
        assert!(cli.validate().is_err());
    }

    #[test]
    fn test_validate_user_without_pass() {
        let cli = Cli {
            dir: ".".into(),
            port: None,
            port_ssl: None,
            cert: None,
            key: None,
            web_ui: false,
            port_gui: None,
            user: Some("admin".into()),
            pass: None,
            cors: false,
            upload: false,
        };
        assert!(cli.validate().is_err());
    }
}
```

- [ ] **Step 2: Wire into main.rs**

```rust
mod cli;

use clap::Parser;
use cli::Cli;

fn main() {
    let cli = Cli::parse();
    if let Err(e) = cli.validate() {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
    println!("Serving directory: {}", cli.dir.display());
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test -- --lib cli`
Expected: 2 tests pass

- [ ] **Step 4: Verify --help works**

Run: `cargo run -- --help`
Expected: Shows usage with all arguments

---

### Task 3: Port finding

**Files:**
- Create: `src/ports.rs`

- [ ] **Step 1: Write port finding with tests**

```rust
use std::net::TcpListener;

pub fn find_port(default: u16, explicit: Option<u16>) -> Result<u16, String> {
    match explicit {
        Some(port) => {
            if is_port_available(port) {
                Ok(port)
            } else {
                Err(format!("port {port} is already in use"))
            }
        }
        None => find_free_port(default)
            .ok_or_else(|| format!("no free port found starting from {default}")),
    }
}

fn is_port_available(port: u16) -> bool {
    TcpListener::bind(("0.0.0.0", port)).is_ok()
}

fn find_free_port(start: u16) -> Option<u16> {
    (start..=start.saturating_add(100)).find(|&p| is_port_available(p))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    #[test]
    fn test_find_free_port_from_default() {
        let port = find_port(19876, None).unwrap();
        assert!(port >= 19876);
        assert!(is_port_available(port));
    }

    #[test]
    fn test_explicit_port_busy() {
        let listener = TcpListener::bind(("0.0.0.0", 19877)).unwrap();
        let result = find_port(19877, Some(19877));
        assert!(result.is_err());
        drop(listener);
    }

    #[test]
    fn test_auto_skips_busy_port() {
        let listener = TcpListener::bind(("0.0.0.0", 19878)).unwrap();
        let port = find_port(19878, None).unwrap();
        assert!(port > 19878);
        drop(listener);
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -- --lib ports`
Expected: 3 tests pass

---

### Task 4: Network interface discovery

**Files:**
- Create: `src/network.rs`

- [ ] **Step 1: Write network.rs**

```rust
use network_interface::{NetworkInterface, NetworkInterfaceConfig};
use std::net::IpAddr;

#[derive(Debug, Clone)]
pub struct NetIface {
    pub name: String,
    pub ip: IpAddr,
}

pub fn local_interfaces() -> Vec<NetIface> {
    let mut result = vec![NetIface {
        name: "loopback".into(),
        ip: IpAddr::from([127, 0, 0, 1]),
    }];

    if let Ok(ifaces) = NetworkInterface::show() {
        for iface in ifaces {
            for addr in &iface.addr {
                let ip = addr.ip();
                if ip.is_loopback() || ip.is_unspecified() {
                    continue;
                }
                if let IpAddr::V4(_) = ip {
                    result.push(NetIface {
                        name: iface.name.clone(),
                        ip,
                    });
                }
            }
        }
    }

    result
}

pub fn format_urls(interfaces: &[NetIface], port: u16, is_https: bool) -> Vec<String> {
    let scheme = if is_https { "https" } else { "http" };
    interfaces
        .iter()
        .map(|i| format!("{scheme}://{}:{port}", i.ip))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_local_interfaces_includes_loopback() {
        let ifaces = local_interfaces();
        assert!(ifaces.iter().any(|i| i.ip.is_loopback()));
    }

    #[test]
    fn test_format_urls() {
        let ifaces = vec![NetIface {
            name: "lo".into(),
            ip: IpAddr::from([127, 0, 0, 1]),
        }];
        let urls = format_urls(&ifaces, 4701, false);
        assert_eq!(urls, vec!["http://127.0.0.1:4701"]);
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -- --lib network`
Expected: 2 tests pass

---

### Task 5: Shared application state

**Files:**
- Create: `src/state.rs`

- [ ] **Step 1: Write state.rs**

```rust
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{broadcast, RwLock};

use crate::network::NetIface;

#[derive(Debug, Clone)]
pub struct ClientInfo {
    pub ip: IpAddr,
    pub user_agent: String,
    pub connected_at: Instant,
    pub last_seen: Instant,
}

#[derive(Debug, Clone)]
pub struct DownloadInfo {
    pub path: String,
    pub client_ip: IpAddr,
    pub bytes_sent: u64,
    pub total_bytes: u64,
    pub started_at: Instant,
}

pub struct AppState {
    pub root: std::path::PathBuf,
    pub start_time: Instant,
    pub http_port: u16,
    pub https_port: Option<u16>,
    pub webui_port: Option<u16>,
    pub interfaces: Vec<NetIface>,
    pub auth: Option<(String, String)>,
    pub cors: bool,
    pub upload: bool,
    pub clients: RwLock<HashMap<String, ClientInfo>>,
    pub downloads: RwLock<HashMap<String, DownloadInfo>>,
    pub shutdown: broadcast::Sender<()>,
}

impl AppState {
    pub fn new(
        root: std::path::PathBuf,
        http_port: u16,
        https_port: Option<u16>,
        webui_port: Option<u16>,
        interfaces: Vec<NetIface>,
        auth: Option<(String, String)>,
        cors: bool,
        upload: bool,
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
            cors,
            upload,
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

    pub async fn client_count(&self) -> usize {
        self.clients.read().await.len()
    }

    pub async fn download_count(&self) -> usize {
        self.downloads.read().await.len()
    }
}
```

- [ ] **Step 2: Run tests (compile check)**

Run: `cargo build`
Expected: Compiles with no errors

---

### Task 6: Wire foundation into main.rs

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Update main.rs to use all foundation modules**

```rust
mod cli;
mod network;
mod ports;
mod state;

use clap::Parser;
use cli::Cli;
use network::local_interfaces;
use ports::find_port;
use state::AppState;

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    if let Err(e) = cli.validate() {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }

    let root = std::fs::canonicalize(&cli.dir).unwrap_or_else(|e| {
        eprintln!("Error: cannot resolve directory '{}': {e}", cli.dir.display());
        std::process::exit(1);
    });

    if !root.is_dir() {
        eprintln!("Error: '{}' is not a directory", root.display());
        std::process::exit(1);
    }

    let http_port = find_port(4701, cli.port).unwrap_or_else(|e| {
        eprintln!("Error: {e}");
        std::process::exit(1);
    });

    let https_port = if cli.cert.is_some() {
        Some(find_port(4801, cli.port_ssl).unwrap_or_else(|e| {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }))
    } else {
        None
    };

    let webui_port = if cli.web_ui {
        Some(find_port(4901, cli.port_gui).unwrap_or_else(|e| {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }))
    } else {
        None
    };

    let interfaces = local_interfaces();

    let auth = match (cli.user, cli.pass) {
        (Some(u), Some(p)) => Some((u, p)),
        _ => None,
    };

    let _state = AppState::new(
        root,
        http_port,
        https_port,
        webui_port,
        interfaces,
        auth,
        cli.cors,
        cli.upload,
    );

    println!("HTTP on port {http_port}");
    if let Some(p) = https_port {
        println!("HTTPS on port {p}");
    }
    println!("Press Ctrl+C to stop");

    tokio::signal::ctrl_c().await.ok();
}
```

- [ ] **Step 2: Verify full compilation and tests**

Run: `cargo test && cargo run -- --help`
Expected: All tests pass, help shows correctly

- [ ] **Step 3: Commit**

```bash
git add -A && git commit -m "feat: foundation — CLI, state, ports, network"
```

---

## Chunk 2: Core HTTP Server

### Task 7: Path security validation

**Files:**
- Create: `src/server/mod.rs`
- Create: `src/server/security.rs`

- [ ] **Step 1: Create server/mod.rs**

```rust
pub mod security;
```

- [ ] **Step 2: Write security.rs with tests first**

```rust
use std::path::{Path, PathBuf};

pub fn validate_path(root: &Path, requested: &str) -> Result<PathBuf, SecurityError> {
    let decoded = percent_encoding::percent_decode_str(requested)
        .decode_utf8()
        .map_err(|_| SecurityError::InvalidEncoding)?;

    let cleaned = decoded.trim_start_matches('/');

    if cleaned.split('/').any(|seg| seg == ".." || seg == ".") {
        return Err(SecurityError::TraversalAttempt);
    }

    let full_path = root.join(cleaned);

    let canonical = if full_path.exists() {
        full_path
            .canonicalize()
            .map_err(|_| SecurityError::NotFound)?
    } else {
        return Err(SecurityError::NotFound);
    };

    if !canonical.starts_with(root) {
        return Err(SecurityError::TraversalAttempt);
    }

    Ok(canonical)
}

#[derive(Debug, PartialEq)]
pub enum SecurityError {
    TraversalAttempt,
    InvalidEncoding,
    NotFound,
}

impl std::fmt::Display for SecurityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TraversalAttempt => write!(f, "access denied"),
            Self::InvalidEncoding => write!(f, "invalid path encoding"),
            Self::NotFound => write!(f, "not found"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn setup_test_dir() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("file.txt"), "hello").unwrap();
        fs::create_dir(dir.path().join("subdir")).unwrap();
        fs::write(dir.path().join("subdir/inner.txt"), "inner").unwrap();
        dir
    }

    #[test]
    fn test_valid_file() {
        let dir = setup_test_dir();
        let result = validate_path(dir.path(), "/file.txt");
        assert!(result.is_ok());
        assert!(result.unwrap().ends_with("file.txt"));
    }

    #[test]
    fn test_valid_subdir() {
        let dir = setup_test_dir();
        let result = validate_path(dir.path(), "/subdir/inner.txt");
        assert!(result.is_ok());
    }

    #[test]
    fn test_traversal_dotdot() {
        let dir = setup_test_dir();
        let result = validate_path(dir.path(), "/../etc/passwd");
        assert_eq!(result.unwrap_err(), SecurityError::TraversalAttempt);
    }

    #[test]
    fn test_traversal_encoded() {
        let dir = setup_test_dir();
        let result = validate_path(dir.path(), "/%2e%2e/etc/passwd");
        assert_eq!(result.unwrap_err(), SecurityError::TraversalAttempt);
    }

    #[test]
    fn test_not_found() {
        let dir = setup_test_dir();
        let result = validate_path(dir.path(), "/nonexistent.txt");
        assert_eq!(result.unwrap_err(), SecurityError::NotFound);
    }

    #[test]
    fn test_root_path() {
        let dir = setup_test_dir();
        let result = validate_path(dir.path(), "/");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), dir.path().canonicalize().unwrap());
    }
}
```

- [ ] **Step 3: Add tempfile dev-dependency to Cargo.toml**

Add to `Cargo.toml`:

```toml
[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 4: Run tests**

Run: `cargo test -- --lib security`
Expected: 6 tests pass

---

### Task 8: Directory listing HTML

**Files:**
- Create: `src/server/listing.rs`
- Modify: `src/server/mod.rs`

- [ ] **Step 1: Add to mod.rs**

```rust
pub mod security;
pub mod listing;
```

- [ ] **Step 2: Write listing.rs**

This generates the HTML page with the ip1.cc color scheme. Full implementation:

```rust
use std::path::Path;
use std::time::SystemTime;
use chrono::{DateTime, Local};
use humansize::{format_size, BINARY};

pub struct FileEntry {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified: Option<SystemTime>,
    pub mime_type: String,
}

pub fn scan_directory(path: &Path) -> std::io::Result<Vec<FileEntry>> {
    let mut entries = Vec::new();
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let meta = entry.metadata()?;
        let name = entry.file_name().to_string_lossy().to_string();
        let is_dir = meta.is_dir();
        let mime_type = if is_dir {
            "directory".into()
        } else {
            mime_guess::from_path(&name)
                .first_or_octet_stream()
                .to_string()
        };
        entries.push(FileEntry {
            name,
            is_dir,
            size: if is_dir { 0 } else { meta.len() },
            modified: meta.modified().ok(),
            mime_type,
        });
    }
    entries.sort_by(|a, b| {
        b.is_dir.cmp(&a.is_dir).then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(entries)
}

pub fn render_listing(request_path: &str, entries: &[FileEntry], upload_enabled: bool) -> String {
    let breadcrumbs = build_breadcrumbs(request_path);
    let rows: String = entries
        .iter()
        .map(|e| {
            let icon = file_icon(e);
            let href = if request_path == "/" || request_path.is_empty() {
                format!("/{}", percent_encoding::utf8_percent_encode(&e.name, percent_encoding::NON_ALPHANUMERIC))
            } else {
                let trimmed = request_path.trim_end_matches('/');
                format!("{}/{}", trimmed, percent_encoding::utf8_percent_encode(&e.name, percent_encoding::NON_ALPHANUMERIC))
            };
            let name_display = if e.is_dir {
                format!("{}/", e.name)
            } else {
                e.name.clone()
            };
            let size_display = if e.is_dir {
                "—".to_string()
            } else {
                format_size(e.size, BINARY)
            };
            let date_display = e
                .modified
                .map(|t| {
                    let dt: DateTime<Local> = t.into();
                    dt.format("%Y-%m-%d %H:%M").to_string()
                })
                .unwrap_or_else(|| "—".into());

            format!(
                r#"<tr onclick="window.location='{href}'">
                    <td class="icon">{icon}</td>
                    <td class="name"><a href="{href}">{name_display}</a></td>
                    <td class="size">{size_display}</td>
                    <td class="date">{date_display}</td>
                    <td class="mime">{mime}</td>
                </tr>"#,
                mime = e.mime_type
            )
        })
        .collect();

    let upload_form = if upload_enabled {
        r#"<div class="upload-area" id="upload-area">
            <form method="POST" enctype="multipart/form-data" id="upload-form">
                <input type="file" name="file" id="file-input" multiple>
                <button type="submit">Upload</button>
            </form>
            <div id="upload-status"></div>
        </div>"#
    } else {
        ""
    };

    let parent_row = if request_path != "/" && !request_path.is_empty() {
        let parent = request_path.trim_end_matches('/');
        let parent = parent.rsplit_once('/').map(|(p, _)| p).unwrap_or("/");
        let parent = if parent.is_empty() { "/" } else { parent };
        format!(
            r#"<tr onclick="window.location='{parent}'">
                <td class="icon">&#x1F519;</td>
                <td class="name"><a href="{parent}">..</a></td>
                <td class="size">—</td>
                <td class="date">—</td>
                <td class="mime">—</td>
            </tr>"#
        )
    } else {
        String::new()
    };

    format!(
        r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>serve — {path}</title>
<style>
* {{ margin: 0; padding: 0; box-sizing: border-box; }}
body {{
    background: #0d1117;
    color: #c9d1d9;
    font-family: 'JetBrains Mono', 'Fira Code', 'Monaco', monospace;
    min-height: 100vh;
    padding: 1.5rem;
    line-height: 1.6;
}}
.container {{ max-width: 960px; margin: 0 auto; }}
.header {{
    margin-bottom: 1.5rem;
    padding-bottom: 1rem;
    border-bottom: 1px solid #30363d;
}}
.logo {{ font-size: 1.3rem; font-weight: 700; color: #58a6ff; }}
.logo .accent {{ color: #f0883e; }}
.breadcrumbs {{ margin-top: 0.5rem; font-size: 0.85rem; color: #8b949e; }}
.breadcrumbs a {{ color: #58a6ff; text-decoration: none; }}
.breadcrumbs a:hover {{ text-decoration: underline; }}
.breadcrumbs .sep {{ color: #6e7681; margin: 0 0.3rem; }}
table {{
    width: 100%;
    border-collapse: collapse;
    margin-top: 1rem;
    font-size: 0.85rem;
}}
th {{
    text-align: left;
    padding: 0.6rem 0.8rem;
    border-bottom: 2px solid #30363d;
    color: #f0883e;
    font-weight: 400;
    font-size: 0.75rem;
    text-transform: uppercase;
    letter-spacing: 0.05em;
}}
td {{
    padding: 0.45rem 0.8rem;
    border-bottom: 1px solid #21262d;
}}
tr:hover {{ background: #161b22; cursor: pointer; }}
.icon {{ width: 2rem; text-align: center; }}
.name a {{ color: #58a6ff; text-decoration: none; }}
.name a:hover {{ text-decoration: underline; }}
tr:has(td .dir-link) .name a {{ color: #7ee787; }}
.size {{ color: #8b949e; text-align: right; width: 7rem; }}
.date {{ color: #8b949e; width: 10rem; }}
.mime {{ color: #6e7681; font-size: 0.75rem; }}
.upload-area {{
    margin-top: 1.5rem;
    padding: 1rem;
    border: 1px dashed #30363d;
    border-radius: 6px;
    background: #161b22;
}}
.upload-area button {{
    background: #238636;
    color: #fff;
    border: none;
    padding: 0.4rem 1rem;
    border-radius: 4px;
    cursor: pointer;
    font-family: inherit;
    margin-left: 0.5rem;
}}
.upload-area button:hover {{ background: #2ea043; }}
.upload-area input[type=file] {{ color: #c9d1d9; }}
.footer {{
    margin-top: 2rem;
    padding-top: 0.8rem;
    border-top: 1px solid #30363d;
    color: #6e7681;
    font-size: 0.7rem;
    text-align: center;
}}
@media (max-width: 700px) {{
    .date, .mime {{ display: none; }}
    body {{ padding: 0.8rem; }}
}}
</style>
</head>
<body>
<div class="container">
    <div class="header">
        <div class="logo"><span class="accent">serve</span> file server</div>
        <div class="breadcrumbs">{breadcrumbs}</div>
    </div>
    <table>
        <thead>
            <tr><th></th><th>Name</th><th class="size">Size</th><th>Modified</th><th>Type</th></tr>
        </thead>
        <tbody>
            {parent_row}
            {rows}
        </tbody>
    </table>
    {upload_form}
    <div class="footer">serve &mdash; rust file server</div>
</div>
</body>
</html>"##,
        path = request_path,
        breadcrumbs = breadcrumbs,
        parent_row = parent_row,
        rows = rows,
        upload_form = upload_form,
    )
}

fn build_breadcrumbs(path: &str) -> String {
    let mut crumbs = vec![r#"<a href="/">/</a>"#.to_string()];
    let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    let mut accumulated = String::new();
    for part in &parts {
        accumulated.push('/');
        accumulated.push_str(part);
        crumbs.push(format!(
            r#"<span class="sep">/</span><a href="{accumulated}">{part}</a>"#
        ));
    }
    crumbs.join("")
}

fn file_icon(entry: &FileEntry) -> &'static str {
    if entry.is_dir {
        "\u{1F4C1}" // folder
    } else if entry.mime_type.starts_with("image/") {
        "\u{1F5BC}" // image
    } else if entry.mime_type.starts_with("video/") {
        "\u{1F3AC}" // video
    } else if entry.mime_type.starts_with("audio/") {
        "\u{1F3B5}" // music
    } else if entry.mime_type.starts_with("text/") || entry.mime_type.contains("json") || entry.mime_type.contains("xml") || entry.mime_type.contains("javascript") {
        "\u{1F4C4}" // text/code
    } else if entry.mime_type.contains("zip") || entry.mime_type.contains("tar") || entry.mime_type.contains("gzip") || entry.mime_type.contains("rar") {
        "\u{1F4E6}" // archive
    } else if entry.mime_type.contains("pdf") {
        "\u{1F4D5}" // pdf
    } else {
        "\u{1F4CE}" // generic file
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_scan_directory_sorted() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("b.txt"), "b").unwrap();
        fs::write(dir.path().join("a.txt"), "a").unwrap();
        fs::create_dir(dir.path().join("zdir")).unwrap();

        let entries = scan_directory(dir.path()).unwrap();
        assert!(entries[0].is_dir); // dirs first
        assert_eq!(entries[0].name, "zdir");
        assert_eq!(entries[1].name, "a.txt"); // then alpha
        assert_eq!(entries[2].name, "b.txt");
    }

    #[test]
    fn test_render_listing_contains_filenames() {
        let entries = vec![FileEntry {
            name: "test.txt".into(),
            is_dir: false,
            size: 1234,
            modified: None,
            mime_type: "text/plain".into(),
        }];
        let html = render_listing("/", &entries, false);
        assert!(html.contains("test.txt"));
        assert!(html.contains("#0d1117")); // ip1.cc bg color
    }

    #[test]
    fn test_breadcrumbs() {
        let bc = build_breadcrumbs("/foo/bar");
        assert!(bc.contains(r#"href="/foo""#));
        assert!(bc.contains(r#"href="/foo/bar""#));
    }
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test -- --lib listing`
Expected: 3 tests pass

---

### Task 9: Request handlers (file serving + directory dispatch)

**Files:**
- Create: `src/server/handlers.rs`
- Modify: `src/server/mod.rs`

- [ ] **Step 1: Update server/mod.rs**

```rust
pub mod security;
pub mod listing;
pub mod handlers;
```

- [ ] **Step 2: Write handlers.rs**

```rust
use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{header, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use std::sync::Arc;
use tokio::fs::File;
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
                    Err(e) => {
                        (StatusCode::INTERNAL_SERVER_ERROR, format!("Error reading directory: {e}"))
                            .into_response()
                    }
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
            let file = match File::open(path).await {
                Ok(f) => f,
                Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Read error").into_response(),
            };
            use tokio::io::AsyncSeekExt;
            let mut file = file;
            if file
                .seek(std::io::SeekFrom::Start(start))
                .await
                .is_err()
            {
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
    let parts: Vec<&str> = range.splitn(2, '-').collect();
    if parts.len() != 2 {
        return None;
    }
    let start: u64 = if parts[0].is_empty() {
        let suffix: u64 = parts[1].parse().ok()?;
        total.saturating_sub(suffix)
    } else {
        parts[0].parse().ok()?
    };
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
```

- [ ] **Step 3: Add `use tokio::io::AsyncReadExt;` import and run tests**

Run: `cargo test -- --lib handlers`
Expected: 4 tests pass

---

### Task 10: Build router and launch server

**Files:**
- Modify: `src/server/mod.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Update server/mod.rs with router builder**

```rust
pub mod handlers;
pub mod listing;
pub mod security;

use axum::Router;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;

use crate::state::AppState;
use handlers::handle_request;

pub fn build_router(state: Arc<AppState>) -> Router {
    Router::new()
        .fallback(handle_request)
        .with_state(state)
}

pub async fn run_http(router: Router, port: u16) -> std::io::Result<()> {
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = TcpListener::bind(addr).await?;
    axum::serve(listener, router)
        .await
}
```

- [ ] **Step 2: Update main.rs to start the server**

```rust
mod cli;
mod network;
mod ports;
mod server;
mod state;

use clap::Parser;
use cli::Cli;
use network::local_interfaces;
use ports::find_port;
use state::AppState;

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    if let Err(e) = cli.validate() {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }

    let root = std::fs::canonicalize(&cli.dir).unwrap_or_else(|e| {
        eprintln!("Error: cannot resolve directory '{}': {e}", cli.dir.display());
        std::process::exit(1);
    });

    if !root.is_dir() {
        eprintln!("Error: '{}' is not a directory", root.display());
        std::process::exit(1);
    }

    let http_port = find_port(4701, cli.port).unwrap_or_else(|e| {
        eprintln!("Error: {e}");
        std::process::exit(1);
    });

    let https_port = if cli.cert.is_some() {
        Some(find_port(4801, cli.port_ssl).unwrap_or_else(|e| {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }))
    } else {
        None
    };

    let webui_port = if cli.web_ui {
        Some(find_port(4901, cli.port_gui).unwrap_or_else(|e| {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }))
    } else {
        None
    };

    let interfaces = local_interfaces();
    let auth = match (cli.user, cli.pass) {
        (Some(u), Some(p)) => Some((u, p)),
        _ => None,
    };

    let state = AppState::new(
        root.clone(),
        http_port,
        https_port,
        webui_port,
        interfaces.clone(),
        auth,
        cli.cors,
        cli.upload,
    );

    let router = server::build_router(state.clone());

    // Spawn HTTP server
    let http_handle = tokio::spawn(server::run_http(router, http_port));

    // Print server URLs
    let urls = network::format_urls(&interfaces, http_port, false);
    println!("\n  serve — file server\n");
    println!("  Serving: {}\n", root.display());
    for url in &urls {
        println!("  \x1b[34m{url}\x1b[0m");
    }
    println!("\n  Press Ctrl+C to stop\n");

    // Wait for Ctrl+C or server error
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            println!("\nShutting down...");
        }
        result = http_handle => {
            if let Err(e) = result {
                eprintln!("Server error: {e}");
            }
        }
    }
}
```

- [ ] **Step 3: Build and test manually**

Run: `cargo build && cargo run -- /tmp`
Then in another terminal: `curl http://127.0.0.1:4701/`
Expected: Returns HTML directory listing with ip1.cc styling

- [ ] **Step 4: Commit**

```bash
git add -A && git commit -m "feat: core HTTP server with file serving and directory listing"
```

---

## Chunk 3: Middleware (Auth, CORS, Upload, TLS, Tracking)

### Task 11: Basic Auth middleware

**Files:**
- Create: `src/server/auth.rs`
- Modify: `src/server/mod.rs`

- [ ] **Step 1: Write auth.rs**

```rust
use axum::extract::Request;
use axum::http::{header, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use base64::Engine;

pub async fn auth_middleware(
    req: Request,
    next: Next,
    expected_user: String,
    expected_pass: String,
) -> Response {
    if let Some(auth_header) = req.headers().get(header::AUTHORIZATION) {
        if let Ok(auth_str) = auth_header.to_str() {
            if let Some(credentials) = auth_str.strip_prefix("Basic ") {
                if let Ok(decoded) = base64::engine::general_purpose::STANDARD.decode(credentials) {
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
```

Add `base64 = "0.22"` to `[dependencies]` in `Cargo.toml`.

- [ ] **Step 2: Wire into router in server/mod.rs**

```rust
pub mod auth;
pub mod handlers;
pub mod listing;
pub mod security;

use axum::middleware;
use axum::Router;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;

use crate::state::AppState;
use handlers::handle_request;

pub fn build_router(state: Arc<AppState>) -> Router {
    let mut router = Router::new().fallback(handle_request);

    if let Some((ref user, ref pass)) = state.auth {
        let u = user.clone();
        let p = pass.clone();
        router = router.layer(middleware::from_fn(move |req, next| {
            let u = u.clone();
            let p = p.clone();
            async move { auth::auth_middleware(req, next, u, p).await }
        }));
    }

    if state.cors {
        use tower_http::cors::{Any, CorsLayer};
        router = router.layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        );
    }

    router.with_state(state)
}

pub async fn run_http(router: Router, port: u16) -> std::io::Result<()> {
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = TcpListener::bind(addr).await?;
    axum::serve(listener, router).await
}
```

- [ ] **Step 3: Build and test auth**

Run: `cargo build`
Test: `cargo run -- /tmp --user admin --pass secret` then:
- `curl http://127.0.0.1:4701/` → 401 Unauthorized
- `curl -u admin:secret http://127.0.0.1:4701/` → 200 OK

---

### Task 12: File upload handler

**Files:**
- Create: `src/server/upload.rs`
- Modify: `src/server/handlers.rs`
- Modify: `src/server/mod.rs`

- [ ] **Step 1: Write upload.rs**

```rust
use axum::extract::{Multipart, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use std::sync::Arc;
use tokio::io::AsyncWriteExt;

use crate::state::AppState;

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

    while let Ok(Some(field)) = multipart.next_field().await {
        let file_name = match field.file_name() {
            Some(name) => sanitize_filename(name),
            None => continue,
        };

        if file_name.is_empty() || file_name.starts_with('.') {
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

        let data = match field.bytes().await {
            Ok(d) => d,
            Err(_) => return (StatusCode::BAD_REQUEST, "Read error").into_response(),
        };

        let mut file = match tokio::fs::File::create(&dest).await {
            Ok(f) => f,
            Err(e) => {
                return (StatusCode::INTERNAL_SERVER_ERROR, format!("Write error: {e}"))
                    .into_response()
            }
        };

        if file.write_all(&data).await.is_err() {
            return (StatusCode::INTERNAL_SERVER_ERROR, "Write error").into_response();
        }
    }

    Redirect::to(&redirect_path).into_response()
}

fn sanitize_filename(name: &str) -> String {
    let name = name.replace(['/', '\\', '\0'], "");
    let name = name.trim_start_matches('.');
    name.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_filename() {
        assert_eq!(sanitize_filename("normal.txt"), "normal.txt");
        assert_eq!(sanitize_filename("../etc/passwd"), "etc/passwd");
        assert_eq!(sanitize_filename(".hidden"), "hidden");
        assert_eq!(sanitize_filename("a/b\\c"), "abc");
    }
}
```

Add `url = "2"` to `[dependencies]` in `Cargo.toml`.

Note: `sanitize_filename` still allows `/` to get through since the replace catches it. Let me fix the test — actually the replace removes `/` and `\`, so `"../etc/passwd"` → `"..etcpasswd"` then trim `.` → `"etcpasswd"`. That's safe. Update the test expectation:

```rust
assert_eq!(sanitize_filename("../etc/passwd"), "etcpasswd");
```

- [ ] **Step 2: Add POST route to server/mod.rs**

Add to `build_router()`, before the auth layer:

```rust
use axum::routing::post;
use super::upload::handle_upload;

// In build_router:
let mut router = Router::new()
    .route("/*path", post(handle_upload))
    .route("/", post(handle_upload))
    .fallback(handle_request);
```

Also add to mod.rs: `pub mod upload;`

- [ ] **Step 3: Build and test**

Run: `cargo build`
Test: `cargo run -- /tmp --upload` then:
`curl -F "file=@/etc/hostname" http://127.0.0.1:4701/`
Check: `ls /tmp/hostname` should exist

---

### Task 13: TLS support

**Files:**
- Create: `src/server/tls.rs`
- Modify: `src/server/mod.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Write tls.rs**

```rust
use axum_server::tls_rustls::RustlsConfig;
use std::path::Path;

pub async fn tls_config(cert_path: &Path, key_path: &Path) -> Result<RustlsConfig, String> {
    RustlsConfig::from_pem_file(cert_path, key_path)
        .await
        .map_err(|e| format!("TLS setup failed: {e}"))
}
```

- [ ] **Step 2: Add run_https to server/mod.rs**

```rust
pub mod tls;

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
```

- [ ] **Step 3: Wire HTTPS into main.rs**

In main.rs, after spawning HTTP, add:

```rust
if let (Some(ssl_port), Some(ref cert), Some(ref key)) =
    (https_port, &cli.cert, &cli.key)
{
    let router_ssl = server::build_router(state.clone());
    let cert = cert.clone();
    let key = key.clone();
    tokio::spawn(async move {
        if let Err(e) = server::run_https(router_ssl, ssl_port, &cert, &key).await {
            eprintln!("HTTPS error: {e}");
        }
    });

    let ssl_urls = network::format_urls(&interfaces, ssl_port, true);
    for url in &ssl_urls {
        println!("  \x1b[32m{url}\x1b[0m");
    }
}
```

Note: `cli.cert` and `cli.key` need to remain accessible after `cli.user`/`cli.pass` are consumed. Restructure main.rs to clone `cert` and `key` before consuming `user`/`pass`.

- [ ] **Step 4: Build**

Run: `cargo build`
Expected: Compiles. (TLS test requires actual certs, skip for now)

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat: auth, CORS, upload, and TLS support"
```

---

## Chunk 4: TUI Dashboard

### Task 14: TUI app state and event handling

**Files:**
- Create: `src/tui/mod.rs`
- Create: `src/tui/app.rs`

- [ ] **Step 1: Create tui/mod.rs**

```rust
pub mod app;
pub mod ui;

use std::sync::Arc;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;
use ratatui::Terminal;

use crate::state::AppState;
use app::TuiApp;
use ui::draw_ui;

pub async fn run_tui(state: Arc<AppState>) -> std::io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut tui_app = TuiApp::new();

    loop {
        let clients: Vec<_> = state.clients.read().await.values().cloned().collect();
        let downloads: Vec<_> = state.downloads.read().await.values().cloned().collect();

        terminal.draw(|frame| {
            draw_ui(frame, &state, &tui_app, &clients, &downloads);
        })?;

        if event::poll(std::time::Duration::from_millis(200))? {
            match event::read()? {
                Event::Key(key) => match key.code {
                    KeyCode::Char('q') => break,
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => break,
                    KeyCode::Up => tui_app.scroll_up(),
                    KeyCode::Down => tui_app.scroll_down(),
                    KeyCode::Tab => tui_app.next_panel(),
                    _ => {}
                },
                Event::Mouse(mouse) => {
                    use crossterm::event::MouseEventKind;
                    match mouse.kind {
                        MouseEventKind::ScrollUp => tui_app.scroll_up(),
                        MouseEventKind::ScrollDown => tui_app.scroll_down(),
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    }

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}
```

- [ ] **Step 2: Write tui/app.rs**

```rust
#[derive(Debug)]
pub struct TuiApp {
    pub client_scroll: usize,
    pub download_scroll: usize,
    pub active_panel: Panel,
}

#[derive(Debug, PartialEq)]
pub enum Panel {
    Clients,
    Downloads,
}

impl TuiApp {
    pub fn new() -> Self {
        Self {
            client_scroll: 0,
            download_scroll: 0,
            active_panel: Panel::Clients,
        }
    }

    pub fn scroll_up(&mut self) {
        match self.active_panel {
            Panel::Clients => self.client_scroll = self.client_scroll.saturating_sub(1),
            Panel::Downloads => self.download_scroll = self.download_scroll.saturating_sub(1),
        }
    }

    pub fn scroll_down(&mut self) {
        match self.active_panel {
            Panel::Clients => self.client_scroll += 1,
            Panel::Downloads => self.download_scroll += 1,
        }
    }

    pub fn next_panel(&mut self) {
        self.active_panel = match self.active_panel {
            Panel::Clients => Panel::Downloads,
            Panel::Downloads => Panel::Clients,
        };
    }
}
```

---

### Task 15: TUI rendering

**Files:**
- Create: `src/tui/ui.rs`

- [ ] **Step 1: Write ui.rs with full layout**

```rust
use ratatui::prelude::*;
use ratatui::widgets::*;
use std::sync::Arc;

use crate::state::{AppState, ClientInfo, DownloadInfo};
use super::app::{Panel, TuiApp};

pub fn draw_ui(
    frame: &mut Frame,
    state: &Arc<AppState>,
    tui_app: &TuiApp,
    clients: &[ClientInfo],
    downloads: &[DownloadInfo],
) {
    let area = frame.area();

    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3 + state.interfaces.len() as u16 * 1 + 2),
            Constraint::Min(10),
            Constraint::Length(3),
        ])
        .split(area);

    draw_header(frame, state, main_layout[0]);
    draw_panels(frame, tui_app, clients, downloads, main_layout[1]);
    draw_footer(frame, state, main_layout[2]);
}

fn draw_header(frame: &mut Frame, state: &Arc<AppState>, area: Rect) {
    let mut lines = vec![
        Line::from(vec![
            Span::styled("serve ", Style::default().fg(Color::Rgb(240, 136, 62)).bold()),
            Span::styled("file server", Style::default().fg(Color::Rgb(201, 209, 217))),
        ]),
        Line::from(vec![
            Span::styled("root: ", Style::default().fg(Color::Rgb(139, 148, 158))),
            Span::styled(
                state.root.display().to_string(),
                Style::default().fg(Color::Rgb(126, 231, 135)),
            ),
        ]),
        Line::from(""),
    ];

    for iface in &state.interfaces {
        let http_url = format!("http://{}:{}", iface.ip, state.http_port);
        let mut spans = vec![
            Span::styled(
                format!("  {} ", iface.name),
                Style::default().fg(Color::Rgb(139, 148, 158)),
            ),
            Span::styled(
                http_url,
                Style::default()
                    .fg(Color::Rgb(88, 166, 255))
                    .add_modifier(Modifier::UNDERLINED),
            ),
        ];

        if let Some(ssl_port) = state.https_port {
            let https_url = format!("  https://{}:{}", iface.ip, ssl_port);
            spans.push(Span::styled(
                https_url,
                Style::default()
                    .fg(Color::Rgb(126, 231, 135))
                    .add_modifier(Modifier::UNDERLINED),
            ));
        }

        lines.push(Line::from(spans));
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Rgb(48, 54, 61)))
        .title(Span::styled(
            " URLs ",
            Style::default().fg(Color::Rgb(240, 136, 62)),
        ))
        .style(Style::default().bg(Color::Rgb(13, 17, 23)));

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}

fn draw_panels(
    frame: &mut Frame,
    tui_app: &TuiApp,
    clients: &[ClientInfo],
    downloads: &[DownloadInfo],
    area: Rect,
) {
    let panels = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    draw_clients_panel(frame, tui_app, clients, panels[0]);
    draw_downloads_panel(frame, tui_app, downloads, panels[1]);
}

fn draw_clients_panel(
    frame: &mut Frame,
    tui_app: &TuiApp,
    clients: &[ClientInfo],
    area: Rect,
) {
    let is_active = tui_app.active_panel == Panel::Clients;
    let border_color = if is_active {
        Color::Rgb(88, 166, 255)
    } else {
        Color::Rgb(48, 54, 61)
    };

    let title = format!(" Clients ({}) ", clients.len());
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(Span::styled(
            title,
            Style::default().fg(Color::Rgb(240, 136, 62)),
        ))
        .style(Style::default().bg(Color::Rgb(13, 17, 23)));

    let rows: Vec<Row> = clients
        .iter()
        .map(|c| {
            let elapsed = c.last_seen.elapsed().as_secs();
            Row::new(vec![
                Cell::from(c.ip.to_string()).style(Style::default().fg(Color::Rgb(88, 166, 255))),
                Cell::from(c.user_agent.chars().take(30).collect::<String>())
                    .style(Style::default().fg(Color::Rgb(139, 148, 158))),
                Cell::from(format!("{elapsed}s ago"))
                    .style(Style::default().fg(Color::Rgb(110, 118, 129))),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(16),
            Constraint::Min(20),
            Constraint::Length(10),
        ],
    )
    .header(
        Row::new(vec!["IP", "User-Agent", "Last Seen"])
            .style(Style::default().fg(Color::Rgb(240, 136, 62)))
            .bottom_margin(1),
    )
    .block(block)
    .row_highlight_style(Style::default().bg(Color::Rgb(22, 27, 34)));

    let mut table_state = TableState::default();
    if !clients.is_empty() {
        table_state.select(Some(tui_app.client_scroll.min(clients.len() - 1)));
    }

    frame.render_stateful_widget(table, area, &mut table_state);
}

fn draw_downloads_panel(
    frame: &mut Frame,
    tui_app: &TuiApp,
    downloads: &[DownloadInfo],
    area: Rect,
) {
    let is_active = tui_app.active_panel == Panel::Downloads;
    let border_color = if is_active {
        Color::Rgb(88, 166, 255)
    } else {
        Color::Rgb(48, 54, 61)
    };

    let title = format!(" Downloads ({}) ", downloads.len());
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(Span::styled(
            title,
            Style::default().fg(Color::Rgb(240, 136, 62)),
        ))
        .style(Style::default().bg(Color::Rgb(13, 17, 23)));

    let rows: Vec<Row> = downloads
        .iter()
        .map(|d| {
            let progress = if d.total_bytes > 0 {
                format!("{}%", d.bytes_sent * 100 / d.total_bytes)
            } else {
                "?".into()
            };
            let filename = d.path.rsplit('/').next().unwrap_or(&d.path);
            Row::new(vec![
                Cell::from(filename.to_string())
                    .style(Style::default().fg(Color::Rgb(126, 231, 135))),
                Cell::from(d.client_ip.to_string())
                    .style(Style::default().fg(Color::Rgb(88, 166, 255))),
                Cell::from(progress).style(Style::default().fg(Color::Rgb(240, 136, 62))),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Min(20),
            Constraint::Length(16),
            Constraint::Length(8),
        ],
    )
    .header(
        Row::new(vec!["File", "Client", "Progress"])
            .style(Style::default().fg(Color::Rgb(240, 136, 62)))
            .bottom_margin(1),
    )
    .block(block)
    .row_highlight_style(Style::default().bg(Color::Rgb(22, 27, 34)));

    let mut table_state = TableState::default();
    if !downloads.is_empty() {
        table_state.select(Some(tui_app.download_scroll.min(downloads.len() - 1)));
    }

    frame.render_stateful_widget(table, area, &mut table_state);
}

fn draw_footer(frame: &mut Frame, state: &Arc<AppState>, area: Rect) {
    let uptime = state.format_uptime();
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Rgb(48, 54, 61)))
        .style(Style::default().bg(Color::Rgb(13, 17, 23)));

    let line = Line::from(vec![
        Span::styled(
            " uptime: ",
            Style::default().fg(Color::Rgb(110, 118, 129)),
        ),
        Span::styled(
            uptime,
            Style::default().fg(Color::Rgb(126, 231, 135)),
        ),
        Span::styled(
            "   q: quit  tab: switch panel  ↑↓: scroll",
            Style::default().fg(Color::Rgb(110, 118, 129)),
        ),
    ]);

    frame.render_widget(Paragraph::new(line).block(block), area);
}
```

---

### Task 16: Wire TUI into main.rs

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Add tui module and run it instead of Ctrl+C wait**

Replace the end of main.rs (from `// Print server URLs` onward) with:

```rust
mod tui;

// ... (in main, after spawning servers)

// Print startup info (shown briefly before TUI takes over)
let urls = network::format_urls(&interfaces, http_port, false);
eprintln!("\n  serve — file server");
eprintln!("  Serving: {}", root.display());
for url in &urls {
    eprintln!("  {url}");
}

// Run TUI (blocks main thread)
if let Err(e) = tui::run_tui(state.clone()).await {
    eprintln!("TUI error: {e}");
}

// Shutdown
let _ = state.shutdown.send(());
```

- [ ] **Step 2: Build and test TUI**

Run: `cargo build && cargo run -- /tmp`
Expected: TUI appears with panels showing URLs, empty client/download lists, and uptime counting

- [ ] **Step 3: Commit**

```bash
git add -A && git commit -m "feat: TUI dashboard with ratatui"
```

---

## Chunk 5: Web UI Admin Panel

### Task 17: Web UI server with WebSocket

**Files:**
- Create: `src/webui/mod.rs`
- Create: `src/webui/assets.rs`

- [ ] **Step 1: Write webui/assets.rs with embedded HTML/JS**

```rust
pub const ADMIN_HTML: &str = include_str!("admin.html");
```

- [ ] **Step 2: Create src/webui/admin.html**

Full HTML file with embedded CSS/JS for the admin dashboard. Uses the ip1.cc color palette. Connects via WebSocket for real-time updates.

```html
<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>serve — admin</title>
<style>
* { margin: 0; padding: 0; box-sizing: border-box; }
body {
    background: #0d1117;
    color: #c9d1d9;
    font-family: 'JetBrains Mono', 'Fira Code', 'Monaco', monospace;
    padding: 1.5rem;
    min-height: 100vh;
}
.container { max-width: 960px; margin: 0 auto; }
h1 { color: #f0883e; font-size: 1.3rem; margin-bottom: 0.5rem; }
h2 { color: #f0883e; font-size: 0.9rem; margin: 1.5rem 0 0.5rem; font-weight: 400; }
h2::before { content: "## "; color: #6e7681; }
.urls { margin: 1rem 0; }
.urls a {
    display: block;
    color: #58a6ff;
    text-decoration: none;
    padding: 0.2rem 0;
    font-size: 0.9rem;
}
.urls a:hover { text-decoration: underline; }
.panel {
    background: #161b22;
    border: 1px solid #30363d;
    border-radius: 6px;
    padding: 0.8rem 1rem;
    margin: 0.5rem 0;
}
.panel .empty { color: #6e7681; font-size: 0.8rem; }
table { width: 100%; border-collapse: collapse; font-size: 0.8rem; }
th {
    text-align: left;
    padding: 0.4rem 0.6rem;
    color: #f0883e;
    border-bottom: 1px solid #30363d;
    font-weight: 400;
    text-transform: uppercase;
    font-size: 0.7rem;
}
td { padding: 0.3rem 0.6rem; border-bottom: 1px solid #21262d; }
.ip { color: #58a6ff; }
.file { color: #7ee787; }
.muted { color: #8b949e; }
.dim { color: #6e7681; }
.uptime {
    margin-top: 1.5rem;
    padding-top: 0.8rem;
    border-top: 1px solid #30363d;
    color: #6e7681;
    font-size: 0.75rem;
}
.uptime .value { color: #7ee787; }
.status-dot {
    display: inline-block;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: #7ee787;
    margin-right: 0.5rem;
}
</style>
</head>
<body>
<div class="container">
    <h1><span style="color:#f0883e">serve</span> admin</h1>
    <p class="dim"><span class="status-dot"></span>connected</p>

    <h2>Server URLs</h2>
    <div class="urls" id="urls"></div>

    <h2>Clients</h2>
    <div class="panel" id="clients-panel">
        <p class="empty">No clients connected</p>
    </div>

    <h2>Downloads</h2>
    <div class="panel" id="downloads-panel">
        <p class="empty">No active downloads</p>
    </div>

    <div class="uptime">
        uptime: <span class="value" id="uptime">00:00:00</span>
    </div>
</div>
<script>
const ws = new WebSocket(`ws://${location.host}/ws`);
ws.onmessage = (e) => {
    const data = JSON.parse(e.data);
    updateUrls(data.urls);
    updateClients(data.clients);
    updateDownloads(data.downloads);
    document.getElementById('uptime').textContent = data.uptime;
};
ws.onclose = () => {
    document.querySelector('.status-dot').style.background = '#f85149';
    document.querySelector('.dim').innerHTML = '<span class="status-dot" style="background:#f85149"></span>disconnected';
};

function updateUrls(urls) {
    const el = document.getElementById('urls');
    el.innerHTML = urls.map(u => `<a href="${u}" target="_blank">${u}</a>`).join('');
}

function updateClients(clients) {
    const el = document.getElementById('clients-panel');
    if (!clients.length) {
        el.innerHTML = '<p class="empty">No clients connected</p>';
        return;
    }
    let html = '<table><tr><th>IP</th><th>User-Agent</th><th>Last Seen</th></tr>';
    for (const c of clients) {
        html += `<tr><td class="ip">${c.ip}</td><td class="muted">${c.user_agent}</td><td class="dim">${c.last_seen}</td></tr>`;
    }
    el.innerHTML = html + '</table>';
}

function updateDownloads(downloads) {
    const el = document.getElementById('downloads-panel');
    if (!downloads.length) {
        el.innerHTML = '<p class="empty">No active downloads</p>';
        return;
    }
    let html = '<table><tr><th>File</th><th>Client</th><th>Progress</th></tr>';
    for (const d of downloads) {
        const pct = d.total > 0 ? Math.round(d.sent * 100 / d.total) + '%' : '?';
        html += `<tr><td class="file">${d.file}</td><td class="ip">${d.client}</td><td style="color:#f0883e">${pct}</td></tr>`;
    }
    el.innerHTML = html + '</table>';
}
</script>
</body>
</html>
```

- [ ] **Step 3: Write webui/mod.rs**

```rust
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
```

- [ ] **Step 4: Wire Web UI into main.rs**

After spawning HTTP/HTTPS servers:

```rust
if let Some(gui_port) = webui_port {
    let webui_state = state.clone();
    tokio::spawn(async move {
        if let Err(e) = webui::run_webui(webui_state, gui_port).await {
            eprintln!("Web UI error: {e}");
        }
    });

    // Auto-open browser
    let gui_url = format!("http://127.0.0.1:{gui_port}");
    let _ = open::that(&gui_url);
}
```

Add `mod webui;` at the top of main.rs.

- [ ] **Step 5: Build and test**

Run: `cargo build && cargo run -- /tmp --web-ui`
Expected: Browser opens showing admin panel with URLs, empty client/download lists, and ticking uptime

- [ ] **Step 6: Commit**

```bash
git add -A && git commit -m "feat: Web UI admin panel with WebSocket"
```

---

### Task 18: Download tracking middleware

**Files:**
- Create: `src/server/tracking.rs`
- Modify: `src/server/handlers.rs`

- [ ] **Step 1: Write tracking.rs**

This wraps the response body to track bytes sent and registers/deregisters downloads in AppState.

```rust
use axum::body::Body;
use axum::extract::Request;
use axum::http::header;
use axum::middleware::Next;
use axum::response::Response;
use bytes::Bytes;
use futures::Stream;
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

    state.add_client(client_id.clone(), client_ip, user_agent).await;

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
```

Add `uuid = { version = "1", features = ["v4"] }` to `[dependencies]` in `Cargo.toml`.

- [ ] **Step 2: Wire tracking into server/mod.rs**

Add tracking middleware to the router (before auth, so all requests are tracked):

```rust
pub mod tracking;

// In build_router, add as the outermost layer:
use axum::middleware as mw;
let state_for_tracking = state.clone();
router = router.layer(mw::from_fn(move |req, next| {
    let s = state_for_tracking.clone();
    async move { tracking::tracking_middleware(s, req, next).await }
}));
```

Note: For `ConnectInfo` to work, the server needs `into_make_service_with_connect_info`. Update `run_http`:

```rust
pub async fn run_http(router: Router, port: u16) -> std::io::Result<()> {
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = TcpListener::bind(addr).await?;
    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
}
```

- [ ] **Step 3: Build and test**

Run: `cargo build && cargo run -- /tmp`
Then download a file: `curl -o /dev/null http://127.0.0.1:4701/some-file`
Expected: TUI shows the client and download in progress

- [ ] **Step 4: Commit**

```bash
git add -A && git commit -m "feat: download tracking with byte progress"
```

---

### Task 19: Install binary

- [ ] **Step 1: Build release and install**

```bash
cd /home/sebas/robotin/apps/rustserve
cargo build --release
cp target/release/serve ~/bin/serve
```

- [ ] **Step 2: Verify**

Run: `serve --help`
Expected: Shows help with all options

Run: `serve /tmp`
Expected: TUI launches, files accessible at http://127.0.0.1:4701

- [ ] **Step 3: Final commit**

```bash
git add -A && git commit -m "feat: serve v0.1.0 — file server with TUI, WebUI, TLS, auth, upload"
```

---

## Summary of dependencies (final Cargo.toml)

```toml
[dependencies]
axum = { version = "0.8", features = ["multipart"] }
axum-server = { version = "0.7", features = ["tls-rustls"] }
base64 = "0.22"
bytes = "1"
chrono = "0.4"
clap = { version = "4", features = ["derive"] }
crossterm = { version = "0.28", features = ["event-stream"] }
futures = "0.3"
http-body = "1"
http-body-util = "0.1"
humansize = "2"
hyper = { version = "1", features = ["full"] }
mime_guess = "2"
network-interface = "2"
open = "5"
percent-encoding = "2"
ratatui = "0.29"
rustls = "0.23"
rustls-pemfile = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }
tokio-tungstenite = "0.24"
tokio-util = { version = "0.7", features = ["io"] }
tower = { version = "0.5", features = ["util"] }
tower-http = { version = "0.6", features = ["cors", "set-header"] }
url = "2"
uuid = { version = "1", features = ["v4"] }

[dev-dependencies]
tempfile = "3"
```
