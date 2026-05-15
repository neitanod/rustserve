use assert_cmd::Command;
use std::net::TcpListener;
use tempfile::TempDir;

/// Find a free port for use in the test, then drop the listener and return the number.
fn free_port() -> u16 {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let p = l.local_addr().unwrap().port();
    drop(l);
    p
}

#[test]
fn daemon_without_services_fails() {
    let tmp = TempDir::new().unwrap();
    Command::cargo_bin("serve")
        .unwrap()
        .args([tmp.path().to_str().unwrap(), "--daemon"])
        .assert()
        .failure();
}

#[test]
fn daemon_with_port_busy_fails_strict() {
    let tmp = TempDir::new().unwrap();
    let port = free_port();
    let _hold = TcpListener::bind(("127.0.0.1", port)).unwrap();

    Command::cargo_bin("serve")
        .unwrap()
        .args([
            tmp.path().to_str().unwrap(),
            "--daemon",
            "--web",
            "--port",
            &port.to_string(),
        ])
        .timeout(std::time::Duration::from_secs(3))
        .assert()
        .failure();
}

#[test]
fn daemon_logs_listening_lines() {
    let tmp = TempDir::new().unwrap();
    let http_port = free_port();
    let dav_port = free_port();

    let output = Command::cargo_bin("serve")
        .unwrap()
        .args([
            tmp.path().to_str().unwrap(),
            "--daemon",
            "--web",
            "--port",
            &http_port.to_string(),
            "--webdav",
            "--port-dav",
            &dav_port.to_string(),
            "--dav-user",
            "u",
            "--dav-pass",
            "p",
        ])
        .timeout(std::time::Duration::from_secs(3))
        .assert()
        .get_output()
        .clone();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(&format!("INFO http: listening on 0.0.0.0:{http_port}")),
        "missing http listening line, stderr was: {stderr}"
    );
    assert!(
        stderr.contains(&format!("INFO webdav: listening on 0.0.0.0:{dav_port}")),
        "missing webdav listening line, stderr was: {stderr}"
    );
}
