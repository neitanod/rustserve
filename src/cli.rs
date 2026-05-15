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

    /// Enable HTTP file server (assumed in console mode if no service flag is given)
    #[arg(long)]
    pub web: bool,

    /// Enable monitoring panel (web UI)
    #[arg(long)]
    pub web_monitor: bool,

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

    /// Max upload size in MB (default: 2048 = 2GB)
    #[arg(long, default_value_t = 2048)]
    pub max_upload_size: usize,

    /// Enable WebDAV server (read-only)
    #[arg(long)]
    pub webdav: bool,

    /// Enable write operations on WebDAV (implies --webdav)
    #[arg(long)]
    pub webdav_rw: bool,

    /// WebDAV port (default: auto-find from 5001)
    #[arg(long)]
    pub port_dav: Option<u16>,
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

    fn base_cli() -> Cli {
        Cli {
            dir: ".".into(),
            port: None,
            port_ssl: None,
            cert: None,
            key: None,
            web: false,
            web_monitor: false,
            port_gui: None,
            user: None,
            pass: None,
            cors: false,
            upload: false,
            max_upload_size: 2048,
            webdav: false,
            webdav_rw: false,
            port_dav: None,
        }
    }

    #[test]
    fn test_validate_cert_without_key() {
        let mut cli = base_cli();
        cli.cert = Some("/tmp/cert.pem".into());
        assert!(cli.validate().is_err());
    }

    #[test]
    fn test_validate_user_without_pass() {
        let mut cli = base_cli();
        cli.user = Some("admin".into());
        assert!(cli.validate().is_err());
    }

    #[test]
    fn test_validate_ok_defaults() {
        assert!(base_cli().validate().is_ok());
    }

    #[test]
    fn test_parse_web_monitor_flag() {
        let cli = Cli::try_parse_from(["serve", "--web-monitor"]).unwrap();
        assert!(cli.web_monitor);
    }

    #[test]
    fn test_parse_web_ui_is_rejected() {
        let result = Cli::try_parse_from(["serve", "--web-ui"]);
        assert!(result.is_err(), "--web-ui must no longer be accepted");
    }

    #[test]
    fn test_parse_web_flag() {
        let cli = Cli::try_parse_from(["serve", "--web"]).unwrap();
        assert!(cli.web);
    }

    #[test]
    fn test_parse_no_web_flag_defaults_false() {
        let cli = Cli::try_parse_from(["serve"]).unwrap();
        assert!(!cli.web);
    }
}
