# PRD — Modo `--daemon` y separación de servicios

## Contexto

Hoy `serve` levanta automáticamente el file server HTTP en el puerto 4701,
busca puertos libres si están ocupados, y arranca una TUI cuando detecta TTY.
Los flags `--web-ui`, `--webdav` y `--webdav-rw` agregan servicios extra sobre
eso.

Queremos poder correr `serve` como servicio (systemd) en una máquina remota
y montarla localmente como WebDAV, decidiendo explícitamente qué servicios
exponer.

## Objetivo

Introducir un modo `--daemon` que:

- Corra sin TUI.
- No abra navegador automáticamente.
- Use puertos fijos (sin auto-find): si un puerto está ocupado, falla y sale.
- No asuma ningún servicio: solo levanta los que se piden explícitamente.

Y, como cambio transversal: WebDAV pasa a requerir basic auth obligatorio
(con credenciales independientes de las del file server web).

Aceptamos cambios breaking — nadie está usando esto en producción aún.

## Servicios

`serve` expone hasta cuatro servicios independientes:

| Servicio | Flag activador | Puerto default | Flag de puerto |
|---|---|---|---|
| File server HTTP (web) | `--web` | 4701 | `--port` |
| File server HTTPS (web SSL) | `--web-ssl` (req. `--cert` + `--key`) | 4801 | `--port-ssl` |
| Panel de monitoreo | `--web-monitor` | 4901 | `--port-gui` |
| WebDAV (RO o RW) | `--webdav` / `--webdav-rw` | 5001 | `--port-dav` |

Notas:

- `--web-ssl` requiere `--cert` y `--key`. Hoy HTTPS se levanta automáticamente
  con solo pasar `--cert`/`--key`; eso cambia: ahora hay que pedirlo explícito.
- `--web-ssl` también requiere `--web` (HTTPS sirve el mismo file server que
  HTTP; no hay HTTPS sin file server). Esta validación aplica en cualquier
  modo, no solo daemon.
- `--webdav-rw` implica `--webdav`.
- Los flags son combinables entre sí en cualquier orden y en cualquier modo.

## Modos

### Modo consola (default, sin `--daemon`)

Es el comportamiento actual con un solo cambio de naming:

- Si no se pasan flags de servicio, se asume `--web` (compat con el uso
  interactivo de hoy: `serve ./mi-carpeta` levanta el file server).
- Los puertos se buscan dinámicamente: si el default está ocupado, se prueba
  el siguiente libre dentro de un rango (comportamiento de hoy).
- Si hay TTY, se levanta la TUI.
- Si se pasa `--web-monitor`, se abre el navegador apuntando al panel.

### Modo daemon (`--daemon`)

- TUI **deshabilitada**. No se levanta nunca; tampoco se puede pedir.
- **Nada se asume**: para que sirva algo, hay que pasar al menos uno de
  `--web`, `--web-monitor`, `--webdav` o `--webdav-rw`. Si no se pasa ninguno,
  error de validación.
- **Sin auto-find de puertos**: cada servicio intenta su puerto default (o el
  explícito) y, si está ocupado, falla con error claro y exit code != 0.
- **Sin abrir navegador**: aunque se pase `--web-monitor`, no se llama a
  `open::that(...)`.
- **Logging journald-friendly**: salida a stderr, una línea por evento, sin
  colores ANSI, prefijo de nivel/componente (ej.
  `INFO http: listening on 0.0.0.0:4701`). Los timestamps los agrega journald.

## Autenticación

### File server (HTTP/HTTPS)

- Flags existentes `--user` / `--pass`. Opcionales. Si están, protegen `--web`
  y `--web-ssl`.
- Deben ir juntos (validación existente se mantiene).

### Panel de monitoreo (`--web-monitor`)

- Sin auth (comportamiento de hoy). No se cambia en este PRD.

### WebDAV (`--webdav` / `--webdav-rw`)

- **Auth obligatoria, siempre**, tanto en modo consola como daemon.
- Flags nuevos: `--dav-user` y `--dav-pass`, independientes de `--user`/`--pass`.
- Validaciones:
  - `--dav-user` y `--dav-pass` deben ir juntos.
  - Si `--webdav` o `--webdav-rw` están activos, `--dav-user`/`--dav-pass`
    son obligatorios.

Razón: permitir publicar el file server público o con una credencial, y
WebDAV con otra credencial distinta (caso típico: file server abierto en LAN,
WebDAV expuesto a Internet con auth fuerte).

## Resumen de flags

```
serve [DIR]
  --daemon                 Sin TUI, sin auto-find de puertos, sin asumir servicios

  --web                    File server HTTP
  --port <N>               Puerto HTTP (default 4701)

  --web-ssl                File server HTTPS (requiere --web, --cert, --key)
  --port-ssl <N>           Puerto HTTPS (default 4801)
  --cert <PATH>            Certificado TLS (PEM)
  --key <PATH>             Llave privada TLS (PEM)

  --web-monitor            Panel de monitoreo
  --port-gui <N>           Puerto del panel (default 4901)

  --webdav                 WebDAV read-only
  --webdav-rw              WebDAV read-write (implica --webdav)
  --port-dav <N>           Puerto WebDAV (default 5001)
  --dav-user <USER>        Usuario WebDAV (obligatorio si webdav activo)
  --dav-pass <PASS>        Pass WebDAV (obligatorio si webdav activo)

  --user <USER>            Basic auth file server (opcional)
  --pass <PASS>            Basic auth file server (opcional)

  --cors                   CORS abierto
  --upload                 Permite uploads
  --max-upload-size <MB>   Tamaño máx de upload (default 2048)
```

## Validaciones nuevas

1. `--cert` ↔ `--key`: deben ir juntos (ya existe).
2. `--user` ↔ `--pass`: deben ir juntos (ya existe).
3. `--web-ssl` requiere `--web`.
4. `--web-ssl` requiere `--cert` y `--key`.
5. `--dav-user` ↔ `--dav-pass`: deben ir juntos.
6. Si `--webdav` o `--webdav-rw`: `--dav-user` y `--dav-pass` son obligatorios.
7. Si `--daemon`: al menos uno de `--web`, `--web-monitor`, `--webdav`,
   `--webdav-rw` debe estar presente.

## Cambios de comportamiento (breaking)

1. WebDAV ahora exige `--dav-user`/`--dav-pass`. Antes no.
2. HTTPS ya no se levanta solo con `--cert`/`--key`: hay que pasar `--web-ssl`.
3. El flag `--web-ui` se renombra a `--web-monitor`.

## Impacto técnico

- `src/cli.rs`: nuevos flags, renombre de `web_ui` → `web_monitor`,
  nuevas validaciones, tests.
- `src/ports.rs`: `find_port(default, explicit, strict)` (o equivalente) — en
  modo strict no busca el siguiente libre.
- `src/state.rs`: agregar `dav_auth: Option<(String, String)>` separado de
  `auth`.
- `src/server/webdav.rs`: aplicar middleware basic auth con `dav_auth` siempre
  que WebDAV esté activo.
- `src/main.rs`:
  - Rama `--daemon` vs consola para puertos, TUI, browser, logging.
  - Helper de logging journald-friendly (sin dependencias nuevas; reemplazar
    `eprintln!` por un mini-helper con formato consistente).
  - Imprimir al arrancar las URLs/puertos de cada servicio para auditoría.
- `src/tui/`: sin cambios funcionales (los WIP de URL overlays son
  ortogonales y quedan como están).

## No incluido en este PRD

- Auth opcional para el panel `--web-monitor` — se puede agregar después si
  hace falta.
- Unificar WebDAV y file server bajo un mismo puerto / paths — se mantienen
  separados, cada uno en su puerto.
- Daemonización Unix real (`fork`, PID file, etc.) — `--daemon` solo cambia
  comportamiento interno; el proceso sigue corriendo en foreground, pensado
  para que systemd lo administre.
- Reload de config por SIGHUP — fuera de alcance.
