use axum_server::tls_rustls::RustlsConfig;
use std::path::Path;

pub async fn tls_config(cert_path: &Path, key_path: &Path) -> Result<RustlsConfig, String> {
    RustlsConfig::from_pem_file(cert_path, key_path)
        .await
        .map_err(|e| format!("TLS setup failed: {e}"))
}
