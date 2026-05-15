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
| `--web-ui` | Enable Web UI admin panel (deprecated, see Daemon Mode) | off |
| `--port-gui` | Web UI port | auto-find from 4901 |
| `--daemon` | Run in daemon mode (no TUI, runs in background) | off |
| `--web` | Enable HTTP file server | off |
| `--web-ssl` | Enable HTTPS file server (requires `--cert` and `--key`) | off |
| `--webdav` | Enable WebDAV (read-only) | off |
| `--webdav-rw` | Enable WebDAV with write access (requires `--dav-user` and `--dav-pass`) | off |
| `--dav-user` | WebDAV username | — |
| `--dav-pass` | WebDAV password | — |
| `--web-monitor` | Enable monitoring panel (replaces `--web-ui`) | off |

### Daemon Mode

In daemon mode, `serve` runs without the TUI dashboard and is suitable for running as a systemd service or in the background.

```bash
# Run file server + WebDAV in daemon mode
serve --daemon --web --webdav-rw --dav-user alice --dav-pass s3cret /srv/files

# File server on custom ports
serve --daemon --web --port 8080 /data

# File server with HTTPS
serve --daemon --web --web-ssl --cert cert.pem --key key.pem --port 4701 --port-ssl 4801 /data

# All services together
serve --daemon --web --web-ssl --webdav-rw --web-monitor \
  --cert cert.pem --key key.pem \
  --dav-user alice --dav-pass s3cret \
  --port 4701 --port-ssl 4801 --port-dav 5001 --port-gui 4901 \
  /srv/files
```

#### Available Services

| Service | Flag | Default Port | Port Flag |
|---------|------|--------------|-----------|
| HTTP file server | `--web` | 4701 | `--port` |
| HTTPS file server | `--web-ssl` | 4801 | `--port-ssl` |
| Monitoring panel | `--web-monitor` | 4901 | `--port-gui` |
| WebDAV | `--webdav` / `--webdav-rw` | 5001 | `--port-dav` |

#### systemd Service Example

Create `/etc/systemd/system/serve.service`:

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

Enable and start:
```bash
sudo systemctl daemon-reload
sudo systemctl enable serve
sudo systemctl start serve
```

### Breaking Changes

- **`--web-ui` renamed to `--web-monitor`**: The old flag is deprecated. Use `--web-monitor` for the new monitoring panel.
- **HTTPS now requires `--web-ssl` flag**: Previously, providing `--cert` and `--key` would automatically enable HTTPS. Now you must explicitly pass `--web-ssl` to enable the HTTPS server.
- **WebDAV requires credentials**: Both `--webdav` and `--webdav-rw` now require `--dav-user` and `--dav-pass` to be set.

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

### Flags CLI

| Flag | Descripción | Por defecto |
|------|-------------|------------|
| `[DIR]` | Directorio a servir | `.` |
| `--port` | Puerto HTTP | búsqueda automática desde 4701 |
| `--port-ssl` | Puerto HTTPS | búsqueda automática desde 4801 |
| `--cert` | Certificado TLS (PEM) | — |
| `--key` | Clave privada TLS (PEM) | — |
| `--user` | Usuario Basic Auth | — |
| `--pass` | Contraseña Basic Auth | — |
| `--cors` | Habilitar headers CORS | off |
| `--upload` | Permitir carga de archivos | off |
| `--max-upload-size` | Tamaño máximo de carga en MB | 2048 |
| `--webdav` | Habilitar WebDAV (solo lectura) | off |
| `--webdav-rw` | Habilitar WebDAV con escritura (requiere `--dav-user` y `--dav-pass`) | off |
| `--port-dav` | Puerto WebDAV | búsqueda automática desde 5001 |
| `--web-ui` | Habilitar panel Web UI (deprecado, ver Modo Daemon) | off |
| `--port-gui` | Puerto Web UI | búsqueda automática desde 4901 |
| `--daemon` | Ejecutar en modo daemon (sin TUI, en background) | off |
| `--web` | Habilitar servidor HTTP de archivos | off |
| `--web-ssl` | Habilitar servidor HTTPS (requiere `--cert` y `--key`) | off |
| `--dav-user` | Usuario WebDAV | — |
| `--dav-pass` | Contraseña WebDAV | — |
| `--web-monitor` | Habilitar panel de monitoreo (reemplaza `--web-ui`) | off |

### Modo Daemon

En modo daemon, `serve` se ejecuta sin el dashboard TUI y es adecuado para ejecutarse como servicio systemd o en background.

```bash
# Ejecutar servidor de archivos + WebDAV en modo daemon
serve --daemon --web --webdav-rw --dav-user alice --dav-pass s3cret /srv/archivos

# Servidor de archivos en puertos personalizados
serve --daemon --web --port 8080 /datos

# Servidor de archivos con HTTPS
serve --daemon --web --web-ssl --cert cert.pem --key key.pem --port 4701 --port-ssl 4801 /datos

# Todos los servicios juntos
serve --daemon --web --web-ssl --webdav-rw --web-monitor \
  --cert cert.pem --key key.pem \
  --dav-user alice --dav-pass s3cret \
  --port 4701 --port-ssl 4801 --port-dav 5001 --port-gui 4901 \
  /srv/archivos
```

#### Servicios Disponibles

| Servicio | Flag | Puerto por defecto | Flag de Puerto |
|----------|------|-------------------|-----------------|
| Servidor HTTP de archivos | `--web` | 4701 | `--port` |
| Servidor HTTPS de archivos | `--web-ssl` | 4801 | `--port-ssl` |
| Panel de monitoreo | `--web-monitor` | 4901 | `--port-gui` |
| WebDAV | `--webdav` / `--webdav-rw` | 5001 | `--port-dav` |

#### Ejemplo de Servicio systemd

Crear `/etc/systemd/system/serve.service`:

```ini
[Unit]
Description=serve servidor de archivos
After=network.target

[Service]
Type=simple
ExecStart=/usr/local/bin/serve --daemon --web --webdav-rw \
  --port 8080 --port-dav 8081 \
  --dav-user alice --dav-pass CAMBIAR \
  /srv/archivos
Restart=on-failure
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
```

Habilitar e iniciar:
```bash
sudo systemctl daemon-reload
sudo systemctl enable serve
sudo systemctl start serve
```

### Cambios Incompatibles

- **`--web-ui` renombrado a `--web-monitor`**: El flag antiguo está deprecado. Usa `--web-monitor` para el nuevo panel de monitoreo.
- **HTTPS ahora requiere flag `--web-ssl`**: Anteriormente, proporcionar `--cert` y `--key` habilitaría HTTPS automáticamente. Ahora debes pasar explícitamente `--web-ssl` para habilitar el servidor HTTPS.
- **WebDAV requiere credenciales**: Tanto `--webdav` como `--webdav-rw` ahora requieren que se establezcan `--dav-user` y `--dav-pass`.

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
