mod cli;
mod network;
mod ports;
mod server;
mod state;
mod tui;
mod webui;

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
        eprintln!(
            "Error: cannot resolve directory '{}': {e}",
            cli.dir.display()
        );
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

    let cert = cli.cert.clone();
    let key = cli.key.clone();

    let state = AppState::new(
        root.clone(),
        http_port,
        https_port,
        webui_port,
        interfaces,
        auth,
        cli.cors,
        cli.upload,
        cli.max_upload_size * 1024 * 1024,
    );

    let router = server::build_router(state.clone());
    tokio::spawn(server::run_http(router, http_port));

    if let (Some(ssl_port), Some(ref cert_path), Some(ref key_path)) = (https_port, &cert, &key) {
        let router_ssl = server::build_router(state.clone());
        let c = cert_path.clone();
        let k = key_path.clone();
        tokio::spawn(async move {
            if let Err(e) = server::run_https(router_ssl, ssl_port, &c, &k).await {
                eprintln!("HTTPS error: {e}");
            }
        });
    }

    if let Some(gui_port) = webui_port {
        let webui_state = state.clone();
        tokio::spawn(async move {
            if let Err(e) = webui::run_webui(webui_state, gui_port).await {
                eprintln!("Web UI error: {e}");
            }
        });
        let gui_url = format!("http://127.0.0.1:{gui_port}");
        let _ = open::that(&gui_url);
    }

    if std::io::IsTerminal::is_terminal(&std::io::stdout()) {
        if let Err(e) = tui::run_tui(state.clone()).await {
            eprintln!("TUI error: {e}");
        }
    } else {
        eprintln!("serve running (no TTY, press Ctrl+C to stop)");
        tokio::signal::ctrl_c().await.ok();
    }

    let _ = state.shutdown.send(());
}
