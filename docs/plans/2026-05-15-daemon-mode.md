# Daemon Mode Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Agregar un modo `--daemon` a `serve` que corra sin TUI, sin auto-find de puertos y sin asumir servicios, permitiendo desplegarlo como systemd unit y montarlo remotamente como WebDAV con basic auth obligatorio.

**Architecture:** Cada servicio (file server HTTP, HTTPS, panel de monitoreo, WebDAV) se vuelve opt-in mediante un flag explícito (`--web`, `--web-ssl`, `--web-monitor`, `--webdav`/`--webdav-rw`). En modo consola, `--web` se asume por compatibilidad. En modo daemon, nada se asume, los puertos son fijos (strict), y se omite TUI y apertura de browser. WebDAV recibe credenciales propias (`--dav-user`/`--dav-pass`) separadas de las del file server (`--user`/`--pass`).

**Tech Stack:** Rust 2021, `clap` 4 (CLI), `axum` 0.8 (HTTP + middleware), `tokio` 1 (async runtime), `reqwest` 0.12 blocking (integration tests), `tempfile` 3, `assert_cmd` (a agregar para e2e contra el binario).

**Referencia de requerimientos:** `docs/prd-daemon-mode.md`

---

## File Structure

Archivos creados o modificados por este plan:

- **Modify:** `src/cli.rs` — flags nuevos, renombre `web_ui→web_monitor`, validaciones nuevas, tests.
- **Modify:** `src/ports.rs` — agregar parámetro `strict` a `find_port`.
- **Modify:** `src/state.rs` — agregar `dav_auth: Option<(String, String)>` al struct y al constructor.
- **Modify:** `src/server/webdav.rs` — usar `state.dav_auth` en vez de `state.auth` para el middleware de auth.
- **Create:** `src/logging.rs` — helper journald-friendly (sin deps nuevas), exportar desde `lib.rs`.
- **Modify:** `src/lib.rs` — `pub mod logging;`.
- **Modify:** `src/main.rs` — refactor de arranque con rama daemon vs consola.
- **Modify:** `src/webui/mod.rs` — sólo si el rename `web_ui→web_monitor` toca aquí (no parece, pero verificar).
- **Modify:** `tests/integration_test.rs` — actualizar `start_dav_server` para pasar `dav_auth` al nuevo constructor, agregar tests de auth WebDAV.
- **Create:** `tests/cli_e2e.rs` — tests end-to-end del binario (modo daemon, fallos de puerto, etc.).
- **Modify:** `Cargo.toml` — agregar `assert_cmd` como dev-dependency.
- **Modify:** `README.md` — documentar modo daemon y flags nuevos.

---

## Chunk 1: Rename `--web-ui` → `--web-monitor`

**Files:**
- Modify: `src/cli.rs` (campos `web_ui` → `web_monitor`, `port_gui` se mantiene)
- Modify: `src/main.rs` (uso `cli.web_ui` → `cli.web_monitor`)
- Test: `src/cli.rs` (módulo `tests` ya existente)

Cambio mecánico. El campo en clap se llama por la sintaxis kebab del nombre del field: `web_monitor` ↔ `--web-monitor`. Mantenemos `port_gui` (controla el puerto del monitor) — renombrarlo aumentaría el blast radius sin beneficio.

- [ ] **Step 1: Escribir el test que falla**

Agregar al módulo `#[cfg(test)] mod tests` de `src/cli.rs`:

```rust
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
```

Y en el helper `base_cli()` del mismo módulo, cambiar `web_ui: false` por `web_monitor: false`.

- [ ] **Step 2: Correr los tests para verificar que fallan**

```
cargo test --lib cli::tests::test_parse_web_monitor
```

Esperado: compile error o test fail por ausencia del field.

- [ ] **Step 3: Implementar el cambio mínimo**

En `src/cli.rs`, renombrar:

```rust
/// Enable Web UI admin panel
#[arg(long)]
pub web_ui: bool,
```

por:

```rust
/// Enable monitoring panel (web UI)
#[arg(long)]
pub web_monitor: bool,
```

En `src/main.rs` cambiar:

```rust
let webui_port = if cli.web_ui {
```

por:

```rust
let webui_port = if cli.web_monitor {
```

Buscar otras referencias a `web_ui` (no debería haber más en `src/` según el grep previo) y actualizarlas.

- [ ] **Step 4: Correr los tests para verificar que pasan**

```
cargo test --lib cli::tests
cargo build --release
cargo test
```

Esperado: todos verdes, build limpio.

- [ ] **Step 5: Commit**

```
git add -A
git commit -m "chunk 1: rename --web-ui to --web-monitor"
```

---

## Chunk 2: Agregar flag `--web`

**Files:**
- Modify: `src/cli.rs` (nuevo field `web`, base_cli, tests)

- [ ] **Step 1: Escribir el test que falla**

Agregar a `src/cli.rs`:

```rust
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
```

Actualizar `base_cli()` para incluir `web: false,`.

- [ ] **Step 2: Correr los tests para verificar que fallan**

```
cargo test --lib cli::tests::test_parse_web_flag
```

Esperado: compile error por field inexistente.

- [ ] **Step 3: Implementar**

En `src/cli.rs`, agregar al struct `Cli` (cerca de `port`, antes de `port_ssl`):

```rust
/// Enable HTTP file server (assumed in console mode if no service flag is given)
#[arg(long)]
pub web: bool,
```

Solo el field. Sin validaciones todavía. Sin cambios en `main.rs` (el wiring va en chunk 9).

- [ ] **Step 4: Correr los tests**

```
cargo test --lib cli::tests
cargo build --release
cargo test
```

Esperado: verde.

- [ ] **Step 5: Commit**

```
git add src/cli.rs
git commit -m "chunk 2: add --web flag"
```

---

## Chunk 3: Agregar flag `--web-ssl` con validaciones

**Files:**
- Modify: `src/cli.rs` (field `web_ssl`, 2 validaciones nuevas, tests)

- [ ] **Step 1: Escribir los tests que fallan**

Agregar a `src/cli.rs`:

```rust
#[test]
fn test_web_ssl_requires_web() {
    let mut cli = base_cli();
    cli.web_ssl = true;
    // cert/key presentes para aislar el error a la falta de --web
    cli.cert = Some("/tmp/cert.pem".into());
    cli.key = Some("/tmp/key.pem".into());
    assert!(cli.validate().is_err(), "--web-ssl without --web must fail");
}

#[test]
fn test_web_ssl_requires_cert_and_key() {
    let mut cli = base_cli();
    cli.web = true;
    cli.web_ssl = true;
    assert!(cli.validate().is_err(), "--web-ssl without --cert/--key must fail");
}

#[test]
fn test_web_ssl_with_web_and_cert_key_ok() {
    // Crear archivos temporales para que el chequeo .exists() pase
    let tmp = tempfile::tempdir().unwrap();
    let cert = tmp.path().join("cert.pem");
    let key = tmp.path().join("key.pem");
    std::fs::write(&cert, "").unwrap();
    std::fs::write(&key, "").unwrap();

    let mut cli = base_cli();
    cli.web = true;
    cli.web_ssl = true;
    cli.cert = Some(cert);
    cli.key = Some(key);
    assert!(cli.validate().is_ok());
}
```

Actualizar `base_cli()` para incluir `web_ssl: false,`.

Si `tempfile` no está importado en el módulo de tests, agregar en el `mod tests` el `use tempfile;` o bien `use ::tempfile;` (ya es dev-dep).

- [ ] **Step 2: Correr los tests para verificar que fallan**

```
cargo test --lib cli::tests::test_web_ssl
```

Esperado: compile error por field inexistente.

- [ ] **Step 3: Implementar**

En `src/cli.rs`, agregar al struct `Cli`:

```rust
/// Enable HTTPS file server (requires --web, --cert and --key)
#[arg(long)]
pub web_ssl: bool,
```

En `Cli::validate()`, agregar antes del `Ok(())`:

```rust
if self.web_ssl && !self.web {
    return Err("--web-ssl requires --web".into());
}
if self.web_ssl && (self.cert.is_none() || self.key.is_none()) {
    return Err("--web-ssl requires --cert and --key".into());
}
```

- [ ] **Step 4: Correr los tests**

```
cargo test --lib cli::tests
cargo build --release
cargo test
```

Esperado: verde.

- [ ] **Step 5: Commit**

```
git add src/cli.rs
git commit -m "chunk 3: add --web-ssl flag with validations"
```

---

## Chunk 4: Agregar `--daemon` y validación "al menos un servicio"

**Files:**
- Modify: `src/cli.rs` (field `daemon`, validación, tests)

- [ ] **Step 1: Escribir los tests que fallan**

```rust
#[test]
fn test_daemon_without_services_fails() {
    let mut cli = base_cli();
    cli.daemon = true;
    assert!(cli.validate().is_err());
}

#[test]
fn test_daemon_with_web_ok() {
    let mut cli = base_cli();
    cli.daemon = true;
    cli.web = true;
    assert!(cli.validate().is_ok());
}

#[test]
fn test_daemon_with_web_monitor_ok() {
    let mut cli = base_cli();
    cli.daemon = true;
    cli.web_monitor = true;
    assert!(cli.validate().is_ok());
}

// El test de --daemon + --webdav (que requiere dav_user/dav_pass) se agrega
// en el Chunk 5, una vez que los fields existen. No lo incluimos acá.

#[test]
fn test_console_no_flags_ok() {
    // Sin daemon y sin nada: validate sigue siendo OK.
    // Lo de "asumir --web" lo aplica main.rs, no validate.
    let cli = base_cli();
    assert!(cli.validate().is_ok());
}
```

Actualizar `base_cli()` con `daemon: false,`. **No agregar todavía** `dav_user`/`dav_pass` al helper — esos fields los introduce el Chunk 5; agregarlos acá causaría un compile error.

- [ ] **Step 2: Correr los tests para verificar que fallan**

```
cargo test --lib cli::tests::test_daemon
```

Esperado: compile error por field `daemon` inexistente.

- [ ] **Step 3: Implementar**

En `src/cli.rs`, agregar al struct:

```rust
/// Run as background service: no TUI, no port auto-find, no implicit services
#[arg(long)]
pub daemon: bool,
```

En `Cli::validate()`:

```rust
if self.daemon
    && !self.web
    && !self.web_monitor
    && !self.webdav
    && !self.webdav_rw
{
    return Err(
        "--daemon requires at least one service: --web, --web-monitor, --webdav or --webdav-rw".into(),
    );
}
```

- [ ] **Step 4: Correr los tests**

```
cargo test --lib cli::tests
cargo build --release
cargo test
```

Esperado: verde.

- [ ] **Step 5: Commit**

```
git add src/cli.rs
git commit -m "chunk 4: add --daemon flag and at-least-one-service validation"
```

---

## Chunk 5: Agregar `--dav-user` / `--dav-pass` con validaciones

**Files:**
- Modify: `src/cli.rs` (fields y validaciones)

- [ ] **Step 1: Escribir los tests que fallan**

```rust
#[test]
fn test_dav_user_without_pass_fails() {
    let mut cli = base_cli();
    cli.dav_user = Some("u".into());
    assert!(cli.validate().is_err());
}

#[test]
fn test_dav_pass_without_user_fails() {
    let mut cli = base_cli();
    cli.dav_pass = Some("p".into());
    assert!(cli.validate().is_err());
}

#[test]
fn test_webdav_without_dav_creds_fails() {
    let mut cli = base_cli();
    cli.webdav = true;
    assert!(cli.validate().is_err());
}

#[test]
fn test_webdav_rw_without_dav_creds_fails() {
    let mut cli = base_cli();
    cli.webdav_rw = true;
    assert!(cli.validate().is_err());
}

#[test]
fn test_webdav_with_dav_creds_ok() {
    let mut cli = base_cli();
    cli.webdav = true;
    cli.dav_user = Some("u".into());
    cli.dav_pass = Some("p".into());
    assert!(cli.validate().is_ok());
}
```

- [ ] **Step 2: Correr los tests para verificar que fallan**

```
cargo test --lib cli::tests::test_dav
cargo test --lib cli::tests::test_webdav
```

Esperado: compile error.

- [ ] **Step 3: Implementar**

En `src/cli.rs`, agregar:

```rust
/// WebDAV basic-auth user (required when --webdav or --webdav-rw)
#[arg(long)]
pub dav_user: Option<String>,

/// WebDAV basic-auth password (required when --webdav or --webdav-rw)
#[arg(long)]
pub dav_pass: Option<String>,
```

Actualizar `base_cli()` con `dav_user: None, dav_pass: None,` (si no se hizo en el chunk 4).

En `Cli::validate()`:

```rust
if self.dav_user.is_some() != self.dav_pass.is_some() {
    return Err("--dav-user and --dav-pass must be used together".into());
}
if (self.webdav || self.webdav_rw)
    && (self.dav_user.is_none() || self.dav_pass.is_none())
{
    return Err("--webdav requires --dav-user and --dav-pass".into());
}
```

- [ ] **Step 4: Correr los tests**

```
cargo test --lib cli::tests
cargo build --release
cargo test
```

Esperado: verde.

- [ ] **Step 5: Commit**

```
git add src/cli.rs
git commit -m "chunk 5: require --dav-user/--dav-pass when webdav is enabled"
```

---

## Chunk 6: `find_port` con modo strict

**Files:**
- Modify: `src/ports.rs` (firma de `find_port`, tests)
- Modify: `src/main.rs` (call sites: pasar `false` por ahora)

- [ ] **Step 1: Escribir el test que falla**

Agregar al módulo `tests` de `src/ports.rs`:

```rust
#[test]
fn test_strict_default_busy_fails() {
    let _listener = TcpListener::bind(("0.0.0.0", 19879)).unwrap();
    let result = find_port(19879, None, true);
    assert!(result.is_err(), "strict mode must not look for the next free port");
}

#[test]
fn test_strict_default_free_returns_default() {
    let port = find_port(19880, None, true).unwrap();
    assert_eq!(port, 19880);
}

#[test]
fn test_non_strict_default_busy_finds_next() {
    let _listener = TcpListener::bind(("0.0.0.0", 19881)).unwrap();
    let port = find_port(19881, None, false).unwrap();
    assert!(port > 19881);
}
```

Y actualizar los tests existentes para pasar `false` (no-strict) en sus llamadas:

```rust
let port = find_port(19876, None, false).unwrap();
// ...
let result = find_port(19877, Some(19877), false);
// ...
let port = find_port(19878, None, false).unwrap();
```

- [ ] **Step 2: Correr los tests para verificar que fallan**

```
cargo test --lib ports::tests::test_strict
```

Esperado: compile error por firma cambiada (los tests existentes que llaman `find_port` con 2 args también fallan en compilación — eso es esperado).

- [ ] **Step 3: Implementar**

Reemplazar el cuerpo de `src/ports.rs` (mantener tests existentes con la nueva firma). El rango de búsqueda en `find_free_port` queda **idéntico al actual** (`start..=start.saturating_add(100)`) — no cambiar el comportamiento del modo no-strict en este chunk.

```rust
use std::net::TcpListener;

pub fn find_port(default: u16, explicit: Option<u16>, strict: bool) -> Result<u16, String> {
    match explicit {
        Some(port) => {
            if is_port_available(port) {
                Ok(port)
            } else {
                Err(format!("port {port} is already in use"))
            }
        }
        None if strict => {
            if is_port_available(default) {
                Ok(default)
            } else {
                Err(format!("port {default} is already in use (strict mode)"))
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
```

En `src/main.rs`, actualizar **todas** las llamadas a `find_port(...)` para pasar un tercer argumento `false`:

```rust
let http_port = find_port(4701, cli.port, false).unwrap_or_else(|e| { ... });
// idem para 4801, 4901, 5001
```

- [ ] **Step 4: Correr los tests**

```
cargo test --lib ports::tests
cargo build --release
cargo test
```

Esperado: verde.

- [ ] **Step 5: Commit**

```
git add src/ports.rs src/main.rs
git commit -m "chunk 6: add strict mode to find_port"
```

---

## Chunk 7: `dav_auth` en `AppState`

**Files:**
- Modify: `src/state.rs` (field nuevo + parámetro nuevo en `new()`)
- Modify: `src/main.rs` (pasar `None` por ahora)
- Modify: `tests/integration_test.rs` (helper `start_dav_server` actualizado)

El constructor `AppState::new` se llama desde `main.rs` y desde `tests/integration_test.rs`. Hay que actualizar ambos call sites cuando se agrega el parámetro.

- [ ] **Step 1: Escribir el test que falla**

Agregar al módulo `#[cfg(test)] mod tests` que correspondiera (o crear uno en `state.rs` si no existe):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::NetIface;

    #[test]
    fn test_appstate_carries_dav_auth() {
        let state = AppState::new(
            std::env::temp_dir(),
            4701,
            None,
            None,
            vec![NetIface { name: "lo".into(), ip: std::net::IpAddr::from([127, 0, 0, 1]) }],
            None,
            false,
            false,
            2048 * 1024 * 1024,
            Some(5001),
            false,
            Some(("u".into(), "p".into())), // dav_auth nuevo
        );
        assert_eq!(state.dav_auth.as_ref().map(|(u, _)| u.as_str()), Some("u"));
    }
}
```

- [ ] **Step 2: Correr el test para verificar que falla**

```
cargo test --lib state::tests::test_appstate_carries_dav_auth
```

Esperado: compile error por parámetro y field inexistentes.

- [ ] **Step 3: Implementar**

Firma actual de `AppState::new` (para referencia, según el código y los call sites):

```rust
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
) -> Arc<Self>
```

(Verificar el orden exacto leyendo `src/state.rs` antes de tocar. Si difiere, ajustar.)

En `src/state.rs`:

- Agregar al struct `AppState` (cerca de `auth`):

```rust
pub dav_auth: Option<(String, String)>,
```

- Agregar parámetro `dav_auth` al final de `AppState::new`:

```rust
pub fn new(
    // ...existing params, en el mismo orden de hoy...
    dav_port: Option<u16>,
    dav_rw: bool,
    dav_auth: Option<(String, String)>,
) -> Arc<Self> {
    Arc::new(Self {
        // ...existing fields...
        dav_auth,
    })
}
```

Nota: la firma de `AppState::new` es ya larga (11+ params). Considerar un builder en
un follow-up; fuera de alcance acá.

En `src/main.rs`, en la llamada a `AppState::new(...)`, pasar al final:

```rust
let dav_auth = match (cli.dav_user.clone(), cli.dav_pass.clone()) {
    (Some(u), Some(p)) => Some((u, p)),
    _ => None,
};
let state = AppState::new(
    // ...
    cli.webdav_rw,
    dav_auth,
);
```

En `tests/integration_test.rs`, en `start_dav_server`, pasar `None` como último argumento de `AppState::new(...)`.

- [ ] **Step 4: Correr los tests**

```
cargo test
cargo build --release
```

Esperado: todo verde.

- [ ] **Step 5: Commit**

```
git add src/state.rs src/main.rs tests/integration_test.rs
git commit -m "chunk 7: add dav_auth to AppState"
```

---

## Chunk 8: Auth middleware en WebDAV usando `dav_auth`

**Files:**
- Modify: `src/server/webdav.rs` (cambiar `state.auth` por `state.dav_auth` en `build_webdav_router`)
- Modify: `tests/integration_test.rs` (nuevos tests de auth)

Observación clave: hoy `webdav.rs` usa `state.auth`. Cambiar a `state.dav_auth` es un swap directo. La validación de CLI ya garantiza que `dav_auth` esté presente cuando WebDAV está activo (después del wiring del chunk 9), pero el middleware sigue siendo robusto al caso `None`: si es `None`, no se aplica auth.

- [ ] **Step 1: Escribir los tests que fallan**

**Cliente HTTP:** usar `reqwest::blocking::Client`. La dev-dep ya tiene `features = ["blocking", "multipart"]` — el async client NO está habilitado y no lo vamos a habilitar acá (mantiene tiempos de compilación bajos). El servidor sigue siendo async; lo levantamos en un task de tokio y hacemos las llamadas blocking desde `spawn_blocking`.

Agregar a `tests/integration_test.rs`:

```rust
async fn start_dav_server_with_auth(
    dir: &std::path::Path,
    rw: bool,
    user: &str,
    pass: &str,
) -> (u16, Arc<AppState>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    let interfaces = vec![NetIface {
        name: "lo".into(),
        ip: std::net::IpAddr::from([127, 0, 0, 1]),
    }];

    let state = AppState::new(
        dir.canonicalize().unwrap(),
        0,
        None,
        None,
        interfaces,
        None,
        false,
        false,
        2048 * 1024 * 1024,
        Some(port),
        rw,
        Some((user.into(), pass.into())),
    );

    let router = serve::server::webdav::build_webdav_router(state.clone());

    tokio::spawn(async move {
        let _ = axum::serve(
            listener,
            router.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await;
    });

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    (port, state)
}

#[tokio::test]
async fn webdav_without_auth_returns_401() {
    let tmp = TempDir::new().unwrap();
    let (port, _state) = start_dav_server_with_auth(tmp.path(), false, "u", "p").await;

    let status = tokio::task::spawn_blocking(move || {
        let client = reqwest::blocking::Client::new();
        client
            .get(format!("http://127.0.0.1:{port}/"))
            .send()
            .unwrap()
            .status()
            .as_u16()
    })
    .await
    .unwrap();
    assert_eq!(status, 401);
}

#[tokio::test]
async fn webdav_with_correct_auth_returns_200() {
    let tmp = TempDir::new().unwrap();
    std::fs::write(tmp.path().join("index.html"), "hi").unwrap();
    let (port, _state) = start_dav_server_with_auth(tmp.path(), false, "u", "p").await;

    let status = tokio::task::spawn_blocking(move || {
        let client = reqwest::blocking::Client::new();
        client
            .get(format!("http://127.0.0.1:{port}/"))
            .basic_auth("u", Some("p"))
            .send()
            .unwrap()
            .status()
            .as_u16()
    })
    .await
    .unwrap();
    assert_eq!(status, 200);
}

#[tokio::test]
async fn webdav_with_wrong_auth_returns_401() {
    let tmp = TempDir::new().unwrap();
    let (port, _state) = start_dav_server_with_auth(tmp.path(), false, "u", "p").await;

    let status = tokio::task::spawn_blocking(move || {
        let client = reqwest::blocking::Client::new();
        client
            .get(format!("http://127.0.0.1:{port}/"))
            .basic_auth("u", Some("WRONG"))
            .send()
            .unwrap()
            .status()
            .as_u16()
    })
    .await
    .unwrap();
    assert_eq!(status, 401);
}
```

- [ ] **Step 2: Correr los tests para verificar que fallan**

```
cargo test --test integration_test webdav_without_auth
```

Esperado: 200 (porque `webdav.rs` todavía usa `state.auth`, que es `None`).

- [ ] **Step 3: Implementar**

En `src/server/webdav.rs`, en `build_webdav_router`, cambiar:

```rust
if let Some((ref user, ref pass)) = state.auth {
```

por:

```rust
if let Some((ref user, ref pass)) = state.dav_auth {
```

- [ ] **Step 4: Correr los tests**

```
cargo test --test integration_test
cargo build --release
cargo test
```

Esperado: verde. Confirmar que los tests viejos de webdav (los que pasaban `None` como auth) siguen pasando.

- [ ] **Step 5: Commit**

```
git add src/server/webdav.rs tests/integration_test.rs
git commit -m "chunk 8: enforce basic auth on WebDAV router via dav_auth"
```

---

## Chunk 9: Cablear daemon + flags de servicio en `main.rs`

**Files:**
- Modify: `Cargo.toml` (agregar `assert_cmd` como dev-dep)
- Modify: `src/main.rs` (refactor de arranque)
- Create: `tests/cli_e2e.rs` (tests end-to-end del binario)

Comportamiento esperado, repetido:

- `--daemon` ⇒ no TUI, no abre browser, `strict=true` para `find_port`.
- En consola, si no se pasa ningún flag de servicio, asumimos `--web=true` (compat con el uso de hoy).
- `--web-ssl` activa HTTPS (hoy se activaba con solo pasar `--cert`/`--key`).
- `--web-monitor` levanta el panel; en daemon no abre browser.

- [ ] **Step 1: Agregar `assert_cmd` como dev-dep**

En `Cargo.toml`, sección `[dev-dependencies]`:

```toml
assert_cmd = "2"
```

- [ ] **Step 2: Escribir los tests e2e que fallan**

Crear `tests/cli_e2e.rs`:

```rust
use assert_cmd::Command;
use std::net::TcpListener;
use tempfile::TempDir;

/// Encuentra un puerto libre para usar en el test, lo cierra y devuelve el número.
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
```

- [ ] **Step 3: Correr los tests para verificar que fallan**

```
cargo test --test cli_e2e
```

Esperado: probablemente fallen por la lógica de fallback (sin `--daemon`, el binario asume `--web` por TTY-detect; pero el wiring del daemon aún no está).

- [ ] **Step 4: Implementar el refactor en `main.rs`**

Reemplazar el cuerpo de `main()` en `src/main.rs`. Esquema:

```rust
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
        Some(find_port(4701, cli.port, strict).unwrap_or_else(|e| { eprintln!("Error: {e}"); std::process::exit(1); }))
    } else {
        None
    };

    let https_port = if web_enabled && cli.web_ssl {
        Some(find_port(4801, cli.port_ssl, strict).unwrap_or_else(|e| { eprintln!("Error: {e}"); std::process::exit(1); }))
    } else {
        None
    };

    let webui_port = if cli.web_monitor {
        Some(find_port(4901, cli.port_gui, strict).unwrap_or_else(|e| { eprintln!("Error: {e}"); std::process::exit(1); }))
    } else {
        None
    };

    let webdav = cli.webdav || cli.webdav_rw;
    let dav_port = if webdav {
        Some(find_port(5001, cli.port_dav, strict).unwrap_or_else(|e| { eprintln!("Error: {e}"); std::process::exit(1); }))
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
    }

    if let (Some(ssl_port), Some(ref cert_path), Some(ref key_path)) = (https_port, &cli.cert, &cli.key) {
        let router_ssl = server::build_router(state.clone());
        let c = cert_path.clone();
        let k = key_path.clone();
        tokio::spawn(async move {
            if let Err(e) = server::run_https(router_ssl, ssl_port, &c, &k).await {
                eprintln!("HTTPS error: {e}");
            }
        });
    }

    if let Some(dp) = dav_port {
        let dav_router = server::webdav::build_webdav_router(state.clone());
        tokio::spawn(server::webdav::run_webdav(dav_router, dp));
    }

    if let Some(gui_port) = webui_port {
        let webui_state = state.clone();
        tokio::spawn(async move {
            if let Err(e) = webui::run_webui(webui_state, gui_port).await {
                eprintln!("Web UI error: {e}");
            }
        });
        if !cli.daemon {
            let gui_url = format!("http://127.0.0.1:{gui_port}");
            let _ = open::that(&gui_url);
        }
    }

    if cli.daemon {
        eprintln!("serve daemon running (press Ctrl+C to stop)");
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
```

**Nota sobre `http_port: u16` en AppState:** hoy el field espera un `u16` no opcional. Si `web_enabled == false`, no tenemos puerto HTTP. Opciones:

A. Cambiar `AppState.http_port` a `Option<u16>` (más correcto, cambio de superficie).
B. Pasar `0` cuando no hay HTTP (lo que muestra el snippet). Los consumidores son `webui/mod.rs::build_update` (URLs del HTTP) y la TUI. Verificar que no rompe nada cuando es `0`.

**Decisión recomendada:** B por simplicidad (cambio breaking más chico). Si el panel de monitoreo está activo sin HTTP, las URLs HTTP se mostrarían apuntando a `:0`. Lo aceptamos por ahora; si molesta, en un chunk posterior se hace opcional.

- [ ] **Step 5: Correr los tests**

```
cargo test
cargo build --release
```

Esperado: verde.

- [ ] **Step 6: Smoke manual rápido**

```
# Modo consola actual (compat): debe levantar HTTP en 4701 y la TUI.
./target/release/serve ./docs &
sleep 1
curl -sf http://127.0.0.1:4701/ | head -c 80
kill %1

# Modo daemon: HTTP + WebDAV con auth.
./target/release/serve --daemon --web --webdav --dav-user u --dav-pass p ./docs &
sleep 1
curl -sf http://127.0.0.1:4701/ | head -c 80
curl -sf -u u:p http://127.0.0.1:5001/
kill %1
```

- [ ] **Step 7: Commit**

```
git add Cargo.toml src/main.rs tests/cli_e2e.rs
git commit -m "chunk 9: wire --daemon, --web, --web-ssl, --web-monitor in main"
```

---

## Chunk 10: Helper de logging journald-friendly

**Files:**
- Create: `src/logging.rs`
- Modify: `src/lib.rs` (`pub mod logging;`)

Sin agregar dependencias. Formato pensado para journald: una línea por evento, sin colores, prefijo de nivel + componente. journald agrega timestamp y unidad automáticamente.

Formato: `<LEVEL> <component>: <msg>\n`. Ej: `INFO http: listening on 0.0.0.0:4701`.

- [ ] **Step 1: Escribir el test que falla**

Crear `src/logging.rs` con sólo los tests (sin la impl):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_info_line() {
        assert_eq!(
            format_event("INFO", "http", "listening on 0.0.0.0:4701"),
            "INFO http: listening on 0.0.0.0:4701"
        );
    }

    #[test]
    fn formats_error_line() {
        assert_eq!(
            format_event("ERROR", "webdav", "bind failed"),
            "ERROR webdav: bind failed"
        );
    }
}
```

Agregar al `src/lib.rs`:

```rust
pub mod logging;
```

- [ ] **Step 2: Correr el test para verificar que falla**

```
cargo test --lib logging::tests
```

Esperado: compile error (`format_event` no existe).

- [ ] **Step 3: Implementar**

En `src/logging.rs`, encima del bloque de tests:

```rust
use std::io::Write;

pub fn format_event(level: &str, component: &str, msg: &str) -> String {
    format!("{level} {component}: {msg}")
}

pub fn log_info(component: &str, msg: &str) {
    let line = format_event("INFO", component, msg);
    let _ = writeln!(std::io::stderr(), "{line}");
}

pub fn log_error(component: &str, msg: &str) {
    let line = format_event("ERROR", component, msg);
    let _ = writeln!(std::io::stderr(), "{line}");
}
```

- [ ] **Step 4: Correr los tests**

```
cargo test --lib logging
cargo build --release
cargo test
```

Esperado: verde.

- [ ] **Step 5: Commit**

```
git add src/logging.rs src/lib.rs
git commit -m "chunk 10: add journald-friendly logging helpers"
```

---

## Chunk 11: Imprimir URLs al arrancar

**Files:**
- Modify: `src/main.rs` (usar `logging::log_info` después de bindear cada servicio)
- Modify: `tests/cli_e2e.rs` (verificar que stderr contiene las líneas esperadas)

- [ ] **Step 1: Escribir el test que falla**

Agregar a `tests/cli_e2e.rs`:

```rust
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
```

Nota: `assert_cmd` con `.timeout()` mata el proceso al timeout; los logs ya impresos quedan en `output.stderr`. Usamos 3 s para que CI con carga no flakee. **El test consume ~3 s de wall-time intencionalmente** — el daemon no termina solo y necesitamos el timeout para forzar el flush + recolectar stderr. No "optimizar" reduciendo el timeout. Si en algún momento molesta, sustituir por una readiness probe (poll a `/` hasta 200) seguida de un `kill` explícito.

- [ ] **Step 2: Correr el test para verificar que falla**

```
cargo test --test cli_e2e daemon_logs_listening_lines
```

Esperado: fail (no se loguea nada todavía).

- [ ] **Step 3: Implementar**

En `src/main.rs`, agregar `use crate::logging::log_info;` arriba.

Después de cada `tokio::spawn(...)` de un servicio, agregar el log correspondiente. Ej:

```rust
if let Some(p) = http_port {
    let router = server::build_router(state.clone());
    tokio::spawn(server::run_http(router, p));
    log_info("http", &format!("listening on 0.0.0.0:{p}"));
}

if let (Some(ssl_port), Some(ref cert_path), Some(ref key_path)) = (https_port, &cli.cert, &cli.key) {
    // ...
    log_info("https", &format!("listening on 0.0.0.0:{ssl_port}"));
}

if let Some(dp) = dav_port {
    // ...
    log_info("webdav", &format!("listening on 0.0.0.0:{dp}"));
}

if let Some(gui_port) = webui_port {
    // ...
    log_info("monitor", &format!("listening on 0.0.0.0:{gui_port}"));
}
```

Y para el shutdown:

```rust
log_info("serve", "daemon running, press Ctrl+C to stop");
```

Reemplazar el `eprintln!("serve daemon running...")` por la línea de arriba.

**Caveat:** los `spawn(...)` arrancan los servidores async; el log se imprime aunque el bind falle dentro del task. Aceptable por ahora (los errores se imprimen aparte vía `eprintln!` dentro del task — opcionalmente cambiarlos a `log_error` también).

- [ ] **Step 4: Correr los tests**

```
cargo test --test cli_e2e
cargo test
cargo build --release
```

Esperado: verde.

- [ ] **Step 5: Commit**

```
git add src/main.rs tests/cli_e2e.rs
git commit -m "chunk 11: log service URLs at startup"
```

---

## Chunk 12: Actualizar README

**Files:**
- Modify: `README.md` (sección modo daemon, tabla de flags, ejemplos)

- [ ] **Step 1: Editar README**

Agregar una sección "Daemon mode" con:

- Tabla de flags por servicio (copiar de `docs/prd-daemon-mode.md`).
- Ejemplo de invocación:

```
serve --daemon --web --webdav-rw --dav-user alice --dav-pass s3cret /srv/files
```

- Ejemplo de systemd unit:

```ini
[Unit]
Description=serve file server
After=network.target

[Service]
Type=simple
ExecStart=/usr/local/bin/serve --daemon --web --webdav-rw \
  --port 8080 --port-dav 8081 \
  --dav-user alice --dav-pass CHANGEME \
  /srv/files
Restart=on-failure
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
```

- Mencionar los 3 cambios breaking:
  - `--web-ui` → `--web-monitor`
  - HTTPS pasa a requerir `--web-ssl` explícito
  - WebDAV requiere `--dav-user`/`--dav-pass`

- [ ] **Step 2: Verificar**

```
grep -n "daemon\|--web\|--dav" README.md
```

- [ ] **Step 3: Commit**

```
git add README.md
git commit -m "docs: document --daemon mode and service flags"
```

---

## Verificaciones finales (gates pre-merge)

- [ ] `cargo fmt --check` limpio.
- [ ] `cargo test` todo verde.
- [ ] `cargo clippy --all-targets -- -D warnings` limpio (incluye tests).
- [ ] `cargo build --release` ok.
- [ ] Smoke manual: `serve ./docs` levanta TUI como hoy.
- [ ] Smoke manual: `serve --daemon --web ./docs` levanta HTTP, sin TUI, sin abrir browser.
- [ ] Smoke manual: `serve --daemon --webdav --dav-user u --dav-pass p ./docs`; montar con `cadaver http://localhost:5001` y verificar que pide auth y autentica.
- [ ] Smoke manual: `serve --daemon --web --port 4701` con 4701 ocupado → falla con error claro y exit code ≠ 0.
- [ ] Logs en journald-friendly: `journalctl -u serve.service` muestra líneas legibles (probar en host.framework.cc con un systemd unit de prueba).

## Notas y caveats

- **Orden de chunks 4/5:** el chunk 4 introduce la validación "al menos un servicio" pero los fields `dav_user`/`dav_pass` recién existen en el chunk 5. Por eso el test `test_daemon_with_webdav_ok` se difiere al chunk 5. Confirmado en los pasos.

- **`AppState.http_port: u16` cuando no hay HTTP:** chunk 9 pasa `0`. Si en pruebas manuales el panel de monitoreo muestra URLs feas, en un follow-up volver `http_port` a `Option<u16>`. Fuera de alcance acá.

- **Tests async con reqwest:** verificar features de `reqwest` en `Cargo.toml`. Si los `#[tokio::test]` con `reqwest::Client` no compilan, usar `reqwest::blocking::Client` (la feature `blocking` ya está) envolviéndolo con `tokio::task::spawn_blocking` o creando un `tokio::runtime::Runtime` manual.

- **No alcanzado por este plan:** auth opcional para `--web-monitor`, daemonización Unix real (fork/PID file), reload por SIGHUP. Ver "No incluido" en `docs/prd-daemon-mode.md`.
