# rustserve

Fast, single-binary file server with TUI dashboard, WebDAV, uploads, HTTPS, and Basic Auth.

**[Español](#español)** | **[English](#english)**

---

## English

### Features

- **Directory listing** with dark theme, file icons, breadcrumbs, mime types, sizes and dates
- **Serves `index.html`** automatically if present in a directory
- **Range requests** for resumable downloads
- **TUI dashboard** showing server URLs, connected clients, active downloads with progress, and uptime
- **Web UI** admin panel (`--web-ui`) with real-time WebSocket updates
- **WebDAV** server (`--webdav`) for mounting as a network drive from macOS, Windows, or Linux
- **File uploads** (`--upload`) with browser progress bar
- **HTTPS** with TLS certificates (`--cert` / `--key`)
- **Basic Auth** (`--user` / `--pass`)
- **CORS** headers (`--cors`)
- **Auto port finding** — if the default port is busy, it tries the next one
- **Path traversal protection** — cannot escape the served directory
- **Single binary**, no external files needed

### Install

```bash
cargo install --path .
```

Or build manually:

```bash
cargo build --release
cp target/release/serve ~/bin/
```

### Usage

```bash
# Serve current directory
serve

# Serve a specific directory on a specific port
serve /path/to/files --port 8080

# Enable uploads (max 2GB by default)
serve --upload --max-upload-size 4096

# HTTPS with certificates
serve --cert cert.pem --key key.pem

# Basic Auth
serve --user admin --pass secret

# WebDAV (read-only, separate port)
serve --webdav

# WebDAV with write access
serve --webdav-rw

# Web UI admin panel
serve --web-ui

# All together
serve /data --upload --webdav-rw --user admin --pass secret --cors --web-ui
```

### CLI Flags

| Flag | Description | Default |
|------|-------------|---------|
| `[DIR]` | Directory to serve | `.` |
| `--port` | HTTP port | auto-find from 4701 |
| `--port-ssl` | HTTPS port | auto-find from 4801 |
| `--cert` | TLS certificate (PEM) | — |
| `--key` | TLS private key (PEM) | — |
| `--user` | Basic Auth username | — |
| `--pass` | Basic Auth password | — |
| `--cors` | Enable CORS headers | off |
| `--upload` | Allow file uploads | off |
| `--max-upload-size` | Max upload size in MB | 2048 |
| `--webdav` | Enable WebDAV (read-only) | off |
| `--webdav-rw` | Enable WebDAV with write access | off |
| `--port-dav` | WebDAV port | auto-find from 5001 |
| `--web-ui` | Enable Web UI admin panel | off |
| `--port-gui` | Web UI port | auto-find from 4901 |

### Mounting via WebDAV

**macOS:** Finder > Go > Connect to Server > `http://host:5001/`

**Windows:** File Explorer > Map network drive > `http://host:5001/`

**Linux:**
```bash
# With davfs2
sudo mount -t davfs http://host:5001/ /mnt/dav

# Or via file manager (Nautilus, Thunar, Dolphin)
# Open: dav://host:5001/
```

### Running Tests

```bash
cargo test
```

---

## Español

### Características

- **Listado de directorios** con tema oscuro, íconos, breadcrumbs, tipos MIME, tamaños y fechas
- **Sirve `index.html`** automáticamente si existe en un directorio
- **Range requests** para descargas resumibles
- **Dashboard TUI** mostrando URLs del servidor, clientes conectados, descargas activas con progreso, y uptime
- **Web UI** panel de administración (`--web-ui`) con actualizaciones en tiempo real vía WebSocket
- **WebDAV** (`--webdav`) para montar como unidad de red desde macOS, Windows o Linux
- **Upload de archivos** (`--upload`) con barra de progreso en el browser
- **HTTPS** con certificados TLS (`--cert` / `--key`)
- **Autenticación Basic** (`--user` / `--pass`)
- **Headers CORS** (`--cors`)
- **Búsqueda automática de puerto** — si el puerto por defecto está ocupado, prueba el siguiente
- **Protección contra path traversal** — no se puede salir del directorio servido
- **Binario único**, no requiere archivos externos

### Instalación

```bash
cargo install --path .
```

O compilar manualmente:

```bash
cargo build --release
cp target/release/serve ~/bin/
```

### Uso

```bash
# Servir el directorio actual
serve

# Servir un directorio específico en un puerto específico
serve /ruta/a/archivos --port 8080

# Habilitar uploads (máximo 2GB por defecto)
serve --upload --max-upload-size 4096

# HTTPS con certificados
serve --cert cert.pem --key key.pem

# Autenticación
serve --user admin --pass secreto

# WebDAV (solo lectura, puerto separado)
serve --webdav

# WebDAV con escritura
serve --webdav-rw

# Panel de administración web
serve --web-ui

# Todo junto
serve /datos --upload --webdav-rw --user admin --pass secreto --cors --web-ui
```

### Montar vía WebDAV

**macOS:** Finder > Ir > Conectarse al servidor > `http://host:5001/`

**Windows:** Explorador > Conectar a unidad de red > `http://host:5001/`

**Linux:**
```bash
# Con davfs2
sudo mount -t davfs http://host:5001/ /mnt/dav

# O vía file manager (Nautilus, Thunar, Dolphin)
# Abrir: dav://host:5001/
```

### Tests

```bash
cargo test
```

---

## License

MIT
