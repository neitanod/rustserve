mod cli;
mod logging;
mod network;
mod ports;
mod server;
mod state;
mod tui;
mod webui;

use clap::Parser;
use cli::Cli;
use logging::log_info;
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

    // Política de --web:
    //   - En daemon: explícito (cli.web).
    //   - En consola sin ningún flag de servicio: asumimos --web (compat).
    //   - En consola con algún flag de servicio: respetar lo que pidió el usuario.
    let web_enabled = if cli.daemon {
        cli.web
    } else if !cli.web && !cli.web_monitor && !cli.webdav && !cli.webdav_rw {
        true
    } else {
        cli.web
    };

    let strict = cli.daemon;

    let http_port = if web_enabled {
        Some(find_port(4701, cli.port, strict).unwrap_or_else(|e| {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }))
    } else {
        None
    };

    let https_port = if web_enabled && cli.web_ssl {
        Some(find_port(4801, cli.port_ssl, strict).unwrap_or_else(|e| {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }))
    } else {
        None
    };

    let webui_port = if cli.web_monitor {
        Some(find_port(4901, cli.port_gui, strict).unwrap_or_else(|e| {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }))
    } else {
        None
    };

    let webdav = cli.webdav || cli.webdav_rw;
    let dav_port = if webdav {
        Some(find_port(5001, cli.port_dav, strict).unwrap_or_else(|e| {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }))
    } else {
        None
    };

    let interfaces = local_interfaces();
    let auth = match (cli.user.clone(), cli.pass.clone()) {
        (Some(u), Some(p)) => Some((u, p)),
        _ => None,
    };
    let dav_auth = match (cli.dav_user.clone(), cli.dav_pass.clone()) {
        (Some(u), Some(p)) => Some((u, p)),
        _ => None,
    };

    // TODO follow-up: cambiar AppState.http_port a Option<u16> para evitar
    // el placeholder 0 cuando --web no está activo. Mientras tanto, el panel
    // de monitoreo podría mostrar URLs ":0" si web no está activo — aceptable.
    let state = AppState::new(
        root.clone(),
        http_port.unwrap_or(0),
        https_port,
        webui_port,
        interfaces,
        auth,
        cli.cors,
        cli.upload,
        cli.max_upload_size * 1024 * 1024,
        dav_port,
        cli.webdav_rw,
        dav_auth,
    );

    if let Some(p) = http_port {
        let router = server::build_router(state.clone());
        tokio::spawn(server::run_http(router, p));
        log_info("http", &format!("listening on 0.0.0.0:{p}"));
    }

    if let (Some(ssl_port), Some(ref cert_path), Some(ref key_path)) =
        (https_port, &cli.cert, &cli.key)
    {
        let router_ssl = server::build_router(state.clone());
        let c = cert_path.clone();
        let k = key_path.clone();
        tokio::spawn(async move {
            if let Err(e) = server::run_https(router_ssl, ssl_port, &c, &k).await {
                eprintln!("HTTPS error: {e}");
            }
        });
        log_info("https", &format!("listening on 0.0.0.0:{ssl_port}"));
    }

    if let Some(dp) = dav_port {
        let dav_router = server::webdav::build_webdav_router(state.clone());
        tokio::spawn(server::webdav::run_webdav(dav_router, dp));
        log_info("webdav", &format!("listening on 0.0.0.0:{dp}"));
    }

    if let Some(gui_port) = webui_port {
        let webui_state = state.clone();
        tokio::spawn(async move {
            if let Err(e) = webui::run_webui(webui_state, gui_port).await {
                eprintln!("Web UI error: {e}");
            }
        });
        log_info("monitor", &format!("listening on 0.0.0.0:{gui_port}"));
        if !cli.daemon {
            let gui_url = format!("http://127.0.0.1:{gui_port}");
            let _ = open::that(&gui_url);
        }
    }

    if cli.daemon {
        log_info("serve", "daemon running, press Ctrl+C to stop");
        tokio::signal::ctrl_c().await.ok();
    } else if std::io::IsTerminal::is_terminal(&std::io::stdout()) {
        if let Err(e) = tui::run_tui(state.clone()).await {
            eprintln!("TUI error: {e}");
        }
    } else {
        eprintln!("serve running (no TTY, press Ctrl+C to stop)");
        tokio::signal::ctrl_c().await.ok();
    }

    let _ = state.shutdown.send(());
}
