use std::net::SocketAddr;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::net::TcpListener;

use serve::network::NetIface;
use serve::server;
use serve::state::AppState;

async fn start_test_server(dir: &std::path::Path, upload: bool) -> (u16, Arc<AppState>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    let interfaces = vec![NetIface {
        name: "lo".into(),
        ip: std::net::IpAddr::from([127, 0, 0, 1]),
    }];

    let state = AppState::new(
        dir.canonicalize().unwrap(),
        port,
        None,
        None,
        interfaces,
        None,
        false,
        upload,
        2048 * 1024 * 1024,
    );

    let router = server::build_router(state.clone());

    tokio::spawn(async move {
        axum::serve(
            listener,
            router.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .unwrap();
    });

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    (port, state)
}

async fn start_auth_server(dir: &std::path::Path, user: &str, pass: &str) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    let interfaces = vec![NetIface {
        name: "lo".into(),
        ip: std::net::IpAddr::from([127, 0, 0, 1]),
    }];

    let state = AppState::new(
        dir.canonicalize().unwrap(),
        port,
        None,
        None,
        interfaces,
        Some((user.to_string(), pass.to_string())),
        false,
        false,
        2048 * 1024 * 1024,
    );

    let router = server::build_router(state);

    tokio::spawn(async move {
        axum::serve(
            listener,
            router.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .unwrap();
    });

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    port
}

fn setup_test_dir() -> TempDir {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("hello.txt"), "hello world\n").unwrap();
    std::fs::write(dir.path().join("binary.bin"), vec![0u8; 1024]).unwrap();
    std::fs::create_dir(dir.path().join("subdir")).unwrap();
    std::fs::write(dir.path().join("subdir/nested.txt"), "nested content").unwrap();
    std::fs::create_dir(dir.path().join("indexed")).unwrap();
    std::fs::write(
        dir.path().join("indexed/index.html"),
        "<h1>Custom Index</h1>",
    )
    .unwrap();
    dir
}

#[tokio::test]
async fn test_directory_listing() {
    let dir = setup_test_dir();
    let (port, _state) = start_test_server(dir.path(), false).await;

    let resp = reqwest::get(format!("http://127.0.0.1:{port}/"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let body = resp.text().await.unwrap();
    assert!(body.contains("hello.txt"), "listing should contain hello.txt");
    assert!(body.contains("subdir"), "listing should contain subdir/");
    assert!(body.contains("#0d1117"), "listing should have ip1.cc bg color");
    assert!(
        body.contains("serve"),
        "listing should contain the serve branding"
    );
}

#[tokio::test]
async fn test_file_download() {
    let dir = setup_test_dir();
    let (port, _state) = start_test_server(dir.path(), false).await;

    let resp = reqwest::get(format!("http://127.0.0.1:{port}/hello.txt"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let ct = resp
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    assert!(ct.contains("text/plain"), "content-type should be text/plain");

    let body = resp.text().await.unwrap();
    assert_eq!(body, "hello world\n");
}

#[tokio::test]
async fn test_nested_file() {
    let dir = setup_test_dir();
    let (port, _state) = start_test_server(dir.path(), false).await;

    let resp = reqwest::get(format!("http://127.0.0.1:{port}/subdir/nested.txt"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.text().await.unwrap(), "nested content");
}

#[tokio::test]
async fn test_index_html_served() {
    let dir = setup_test_dir();
    let (port, _state) = start_test_server(dir.path(), false).await;

    let resp = reqwest::get(format!("http://127.0.0.1:{port}/indexed/"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let body = resp.text().await.unwrap();
    assert!(
        body.contains("Custom Index"),
        "should serve index.html instead of listing"
    );
    assert!(
        !body.contains("#0d1117"),
        "should NOT contain the listing CSS"
    );
}

#[tokio::test]
async fn test_404_not_found() {
    let dir = setup_test_dir();
    let (port, _state) = start_test_server(dir.path(), false).await;

    let resp = reqwest::get(format!("http://127.0.0.1:{port}/nonexistent.txt"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn test_path_traversal_blocked() {
    let dir = setup_test_dir();
    let (port, _state) = start_test_server(dir.path(), false).await;

    let client = reqwest::Client::new();

    let resp = client
        .get(format!("http://127.0.0.1:{port}/%2e%2e/etc/passwd"))
        .send()
        .await
        .unwrap();
    let status = resp.status().as_u16();
    assert!(
        status == 403 || status == 400 || status == 404,
        "traversal should be blocked, got {status}"
    );
}

#[tokio::test]
async fn test_range_request() {
    let dir = setup_test_dir();
    let (port, _state) = start_test_server(dir.path(), false).await;

    let client = reqwest::Client::new();
    let resp = client
        .get(format!("http://127.0.0.1:{port}/hello.txt"))
        .header("Range", "bytes=0-4")
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 206);
    let body = resp.text().await.unwrap();
    assert_eq!(body, "hello", "range 0-4 should return 'hello'");
}

#[tokio::test]
async fn test_range_suffix() {
    let dir = setup_test_dir();
    let (port, _state) = start_test_server(dir.path(), false).await;

    let client = reqwest::Client::new();
    let resp = client
        .get(format!("http://127.0.0.1:{port}/hello.txt"))
        .header("Range", "bytes=-6")
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 206);
    let body = resp.text().await.unwrap();
    assert_eq!(body, "world\n", "suffix range should return last 6 bytes");
}

#[tokio::test]
async fn test_accept_ranges_header() {
    let dir = setup_test_dir();
    let (port, _state) = start_test_server(dir.path(), false).await;

    let resp = reqwest::get(format!("http://127.0.0.1:{port}/hello.txt"))
        .await
        .unwrap();
    assert_eq!(
        resp.headers().get("accept-ranges").unwrap().to_str().unwrap(),
        "bytes"
    );
}

#[tokio::test]
async fn test_basic_auth_rejects_unauthenticated() {
    let dir = setup_test_dir();
    let port = start_auth_server(dir.path(), "admin", "secret").await;

    let resp = reqwest::get(format!("http://127.0.0.1:{port}/"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 401);
    assert!(resp.headers().contains_key("www-authenticate"));
}

#[tokio::test]
async fn test_basic_auth_accepts_valid() {
    let dir = setup_test_dir();
    let port = start_auth_server(dir.path(), "admin", "secret").await;

    let client = reqwest::Client::new();
    let resp = client
        .get(format!("http://127.0.0.1:{port}/hello.txt"))
        .basic_auth("admin", Some("secret"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.text().await.unwrap(), "hello world\n");
}

#[tokio::test]
async fn test_basic_auth_rejects_wrong_password() {
    let dir = setup_test_dir();
    let port = start_auth_server(dir.path(), "admin", "secret").await;

    let client = reqwest::Client::new();
    let resp = client
        .get(format!("http://127.0.0.1:{port}/"))
        .basic_auth("admin", Some("wrong"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 401);
}

#[tokio::test]
async fn test_upload_disabled_by_default() {
    let dir = setup_test_dir();
    let (port, _state) = start_test_server(dir.path(), false).await;

    let client = reqwest::Client::new();
    let form = reqwest::multipart::Form::new()
        .text("file", "test content");
    let resp = client
        .post(format!("http://127.0.0.1:{port}/"))
        .multipart(form)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 403);
}

#[tokio::test]
async fn test_upload_enabled() {
    let dir = setup_test_dir();
    let (port, _state) = start_test_server(dir.path(), true).await;

    let file_part = reqwest::multipart::Part::bytes(b"uploaded content".to_vec())
        .file_name("uploaded.txt");
    let form = reqwest::multipart::Form::new().part("file", file_part);

    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();

    let resp = client
        .post(format!("http://127.0.0.1:{port}/"))
        .multipart(form)
        .send()
        .await
        .unwrap();

    assert!(
        resp.status() == 303 || resp.status() == 302 || resp.status() == 200,
        "upload should redirect or succeed, got {}",
        resp.status()
    );

    let uploaded = std::fs::read_to_string(dir.path().join("uploaded.txt")).unwrap();
    assert_eq!(uploaded, "uploaded content");
}

#[tokio::test]
async fn test_listing_shows_breadcrumbs() {
    let dir = setup_test_dir();
    let (port, _state) = start_test_server(dir.path(), false).await;

    let resp = reqwest::get(format!("http://127.0.0.1:{port}/subdir/"))
        .await
        .unwrap();
    let body = resp.text().await.unwrap();
    assert!(body.contains("breadcrumbs"), "should have breadcrumbs");
    assert!(
        body.contains(r#"href="/subdir""#),
        "breadcrumbs should link to subdir"
    );
}

#[tokio::test]
async fn test_listing_shows_mime_types() {
    let dir = setup_test_dir();
    let (port, _state) = start_test_server(dir.path(), false).await;

    let resp = reqwest::get(format!("http://127.0.0.1:{port}/"))
        .await
        .unwrap();
    let body = resp.text().await.unwrap();
    assert!(
        body.contains("text/plain"),
        "listing should show mime type for .txt files"
    );
}

#[tokio::test]
async fn test_binary_file_content_length() {
    let dir = setup_test_dir();
    let (port, _state) = start_test_server(dir.path(), false).await;

    let resp = reqwest::get(format!("http://127.0.0.1:{port}/binary.bin"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(
        resp.content_length().unwrap(),
        1024,
        "content-length should match file size"
    );
}

#[tokio::test]
async fn test_subdir_listing() {
    let dir = setup_test_dir();
    let (port, _state) = start_test_server(dir.path(), false).await;

    let resp = reqwest::get(format!("http://127.0.0.1:{port}/subdir/"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let body = resp.text().await.unwrap();
    assert!(
        body.contains("nested.txt"),
        "subdir listing should contain nested.txt"
    );
    assert!(body.contains(".."), "subdir listing should have parent link");
}
