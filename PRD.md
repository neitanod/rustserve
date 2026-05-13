# RustServe — File Server con TUI y Web UI

## Resumen

Servidor HTTP/HTTPS de archivos estáticos escrito en Rust con Tokio. Sirve un directorio local con una interfaz de navegación web estilizada, una TUI en terminal con paneles y colores, y opcionalmente una Web UI de administración.

## Argumentos CLI

| Argumento | Default | Descripción |
|-----------|---------|-------------|
| `<DIR>` (posicional) | `.` (directorio actual) | DocumentRoot a servir |
| `--port=<N>` | `4701` | Puerto HTTP |
| `--port-ssl=<N>` | `4801` | Puerto HTTPS (solo si se pasa `--cert`) |
| `--cert=<FILE>` | — | Ruta al certificado TLS (PEM). Habilita HTTPS. |
| `--key=<FILE>` | — | Ruta a la clave privada TLS (PEM). Requerido si se usa `--cert`. |
| `--web-ui` | off | Activa la Web UI de administración |
| `--port-gui=<N>` | auto (busca libre) | Puerto para la Web UI |
| `--user=<USER>` | — | Usuario para Basic Auth (requiere `--pass`) |
| `--pass=<PASS>` | — | Contraseña para Basic Auth (requiere `--user`) |
| `--cors` | off | Habilita CORS headers (Access-Control-Allow-Origin: *) |
| `--upload` | off | Permite subir archivos vía la interfaz web |
| `--help` | — | Muestra ayuda y sale |

### Auto-búsqueda de puertos

Si `--port` y `--port-ssl` NO se pasan explícitamente, el servidor intenta los puertos por defecto (4701 y 4801). Si están ocupados, incrementa de a uno hasta encontrar puertos libres. Si se pasan explícitamente y están ocupados, falla con error.

El puerto de `--port-gui` siempre busca automáticamente (default 4901+), salvo que se indique explícitamente.

## Paleta de colores (tomada de ip1.cc)

| Nombre | Hex | Uso |
|--------|-----|-----|
| Background | `#0d1117` | Fondo principal |
| Card BG | `#161b22` | Fondo de bloques/cards |
| Text | `#c9d1d9` | Texto principal |
| Muted | `#8b949e` | Texto secundario |
| Dim | `#6e7681` | Texto terciario, bordes |
| Orange | `#f0883e` | Títulos, acentos |
| Blue | `#58a6ff` | Links, elementos interactivos |
| Green | `#7ee787` | Código, estados positivos |
| Border | `#30363d` | Bordes de paneles |
| Font | `JetBrains Mono`, `Fira Code`, `Monaco`, monospace | |

## Interfaz web de navegación de archivos (para clientes)

Cuando un usuario accede al servidor (HTTP/HTTPS), ve un listado de archivos y carpetas del DocumentRoot con:

- Diseño visual usando la paleta de ip1.cc (fondo oscuro, naranja, celeste, verde)
- Iconos por tipo de archivo (carpeta, imagen, documento, código, etc.)
- Columnas: nombre, tamaño, fecha de modificación, tipo MIME
- Carpetas primero, luego archivos ordenados alfabéticamente
- Breadcrumbs de navegación para la ruta actual
- **Si la carpeta contiene `index.html`, se sirve ese archivo en lugar del listado**

### Seguridad

- Path traversal bloqueado: cualquier intento de acceder fuera del DocumentRoot devuelve 403
- No se permite `..` en rutas resueltas
- Symlinks que apunten fuera del DocumentRoot: bloqueados

## TUI (Terminal UI)

Se muestra siempre al ejecutar el servidor. Paneles:

- **Panel superior**: URLs del servidor (todas las interfaces de red) como links clickeables
- **Panel central izquierdo**: Lista de clientes conectados (scrolleable con mouse)
- **Panel central derecho**: Archivos en descarga (quién descarga qué, progreso si es posible)
- **Panel inferior**: Uptime del servicio, estadísticas generales

Framework sugerido: `ratatui` + `crossterm`

## Web UI de administración (`--web-ui`)

Muestra los mismos datos que la TUI pero en un browser:

- Se abre automáticamente al iniciar (usando `open` / `xdg-open`)
- URLs del servidor como links clickeables
- Clientes conectados (actualización en tiempo real vía WebSocket)
- Archivos en descarga con progreso
- Uptime

## Tracking de clientes y descargas

- Cada conexión HTTP se registra (IP, user-agent, timestamp)
- Las descargas activas se trackean (archivo, cliente, bytes enviados / total)
- Soporte de Range Requests para resumir descargas grandes

## Dependencias principales

- `tokio` — async runtime
- `axum` o `hyper` — HTTP server
- `axum-server` o `rustls` — TLS
- `ratatui` + `crossterm` — TUI
- `clap` — CLI argument parsing
- `serde` + `serde_json` — serialización
- `tokio-tungstenite` o similar — WebSocket para Web UI

## Estructura del proyecto

```
rustserve/
├── Cargo.toml
├── PRD.md
├── src/
│   ├── main.rs           # Entry point, CLI parsing
│   ├── server/
│   │   ├── mod.rs
│   │   ├── handler.rs    # Request handlers
│   │   ├── listing.rs    # Directory listing HTML
│   │   ├── security.rs   # Path validation
│   │   └── tls.rs        # TLS setup
│   ├── tui/
│   │   ├── mod.rs
│   │   ├── app.rs        # TUI app state
│   │   └── ui.rs         # TUI rendering
│   ├── webui/
│   │   ├── mod.rs
│   │   └── assets.rs     # Embedded HTML/CSS/JS
│   └── state.rs          # Shared state (clients, downloads, uptime)
```
