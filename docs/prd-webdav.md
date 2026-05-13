# PRD: WebDAV Support for serve

## Objetivo

Agregar soporte WebDAV a `serve` para que el directorio servido pueda montarse como unidad de red desde macOS Finder, Windows Explorer y Linux (davfs2/GVFS/Nautilus).

## Decisiones de diseño

- **Activación:** Flag `--webdav` habilita el servidor WebDAV. Sin el flag, no se expone.
- **Puerto separado:** `--port-dav <PORT>` permite especificar puerto. Si no se indica, auto-find desde 5001 (misma lógica que los otros puertos). Separar del HTTP normal permite reglas de firewall independientes.
- **Permisos configurables:** WebDAV arranca read-only. Con `--webdav-rw` se habilitan operaciones de escritura (PUT, DELETE, MKCOL, COPY, MOVE).
- **Autenticación:** Respeta `--user`/`--pass` existente. Si auth está habilitado, WebDAV también lo requiere (Basic Auth sobre HTTP, mismo middleware).
- **LOCK fake:** Se responde OK a LOCK/UNLOCK sin lockeo real. Satisface clientes exigentes (Windows/Office) sin complejidad.
- **Misma raíz:** WebDAV sirve el mismo `root` que el servidor HTTP.

## Métodos HTTP a implementar

### Read-only (siempre activos con --webdav)

| Método | Descripción |
|--------|-------------|
| OPTIONS | Retorna métodos soportados + headers DAV: 1,2 |
| PROPFIND | Listar propiedades de archivos/directorios (XML). Soportar Depth: 0, 1, infinity |
| GET | Descargar archivo (ya implementado, se reutiliza) |
| HEAD | Metadata sin body (ya implementado) |

### Read-write (requiere --webdav-rw)

| Método | Descripción |
|--------|-------------|
| PUT | Crear/sobrescribir archivo |
| DELETE | Borrar archivo o directorio (recursivo) |
| MKCOL | Crear directorio |
| COPY | Copiar archivo/directorio. Header Destination indica destino |
| MOVE | Mover/renombrar. Header Destination indica destino |

### LOCK fake (siempre activos)

| Método | Descripción |
|--------|-------------|
| LOCK | Responder 200 con un lock token generado (UUID). No lockea realmente |
| UNLOCK | Responder 204 No Content. No hace nada |

## Respuestas XML

### PROPFIND response (multistatus)

```xml
<?xml version="1.0" encoding="UTF-8"?>
<D:multistatus xmlns:D="DAV:">
  <D:response>
    <D:href>/path/to/resource</D:href>
    <D:propstat>
      <D:prop>
        <D:displayname>filename.txt</D:displayname>
        <D:getcontentlength>12345</D:getcontentlength>
        <D:getcontenttype>text/plain</D:getcontenttype>
        <D:getlastmodified>Mon, 12 May 2025 10:00:00 GMT</D:getlastmodified>
        <D:resourcetype/>           <!-- vacío = archivo -->
        <D:resourcetype><D:collection/></D:resourcetype>  <!-- directorio -->
        <D:creationdate>2025-05-12T10:00:00Z</D:creationdate>
      </D:prop>
      <D:status>HTTP/1.1 200 OK</D:status>
    </D:propstat>
  </D:response>
</D:multistatus>
```

### LOCK response

```xml
<?xml version="1.0" encoding="UTF-8"?>
<D:prop xmlns:D="DAV:">
  <D:lockdiscovery>
    <D:activelock>
      <D:locktype><D:write/></D:locktype>
      <D:lockscope><D:exclusive/></D:lockscope>
      <D:depth>0</D:depth>
      <D:timeout>Second-3600</D:timeout>
      <D:locktoken><D:href>urn:uuid:GENERATED-UUID</D:href></D:locktoken>
    </D:activelock>
  </D:lockdiscovery>
</D:prop>
```

## Arquitectura

### Nuevos archivos

- `src/server/webdav.rs` — Router y handlers WebDAV
- `src/server/webdav_xml.rs` — Generación de XML responses (PROPFIND, LOCK)

### Modificaciones

- `src/cli.rs` — Agregar `--webdav`, `--webdav-rw`, `--port-dav`
- `src/main.rs` — Spawn del server WebDAV en puerto separado
- `src/server/mod.rs` — Exponer módulos webdav

### Reutilización

- `security::validate_path()` — Misma validación de path traversal
- `listing::scan_directory()` — Para obtener entries del directorio en PROPFIND
- `handlers::serve_file()` — Para GET de archivos
- `auth::auth_middleware()` — Mismo middleware de autenticación
- `tracking::tracking_middleware()` — Tracking de clientes y descargas

### Router WebDAV

El router WebDAV es un `axum::Router` separado que corre en su propio `TcpListener`. Comparte el mismo `Arc<AppState>` con el servidor HTTP principal.

```
build_webdav_router(state, rw_enabled) -> Router
  ├── OPTIONS  /*  → dav_options()
  ├── PROPFIND /*  → dav_propfind()
  ├── GET      /*  → handle_request()  (reutilizado)
  ├── HEAD     /*  → handle_request()  (reutilizado)
  ├── LOCK     /*  → dav_lock()
  ├── UNLOCK   /*  → dav_unlock()
  └── (si rw_enabled)
      ├── PUT    /*  → dav_put()
      ├── DELETE /*  → dav_delete()
      ├── MKCOL  /*  → dav_mkcol()
      ├── COPY   /*  → dav_copy()
      └── MOVE   /*  → dav_move()
```

## Estado en AppState

Se agrega:

```rust
pub dav_port: Option<u16>,
pub dav_rw: bool,
```

## Seguridad

- Path traversal: misma validación que HTTP (`validate_path`)
- Auth: mismo Basic Auth si `--user`/`--pass` están configurados
- Write operations: solo si `--webdav-rw` está activo, sino 403
- Destination header (COPY/MOVE): validar que no escape del root
- No permitir PUT/DELETE sobre el root directory

## Compatibilidad por cliente

### macOS Finder
- Requiere PROPFIND con Depth:0 en la raíz antes de hacer Depth:1
- Espera `DAV: 1,2` en OPTIONS
- Envía LOCK al abrir archivos (nuestro fake LOCK lo satisface)

### Windows Explorer
- Envía PROPFIND con body XML pidiendo propiedades específicas (allprop suele funcionar)
- Requiere `DAV: 1,2` y `Ms-Author-Via: DAV` en OPTIONS
- Puede enviar LOCK — fake LOCK lo cubre

### Linux (davfs2/GVFS)
- El más tolerante
- davfs2 funciona con DAV nivel 1 básico
- GVFS (Nautilus/Thunar) también es permisivo

## Headers importantes

```
OPTIONS response:
  DAV: 1,2
  Allow: OPTIONS, PROPFIND, GET, HEAD, LOCK, UNLOCK [, PUT, DELETE, MKCOL, COPY, MOVE]
  Ms-Author-Via: DAV

PROPFIND response:
  Content-Type: application/xml; charset=utf-8
  HTTP/1.1 207 Multi-Status

LOCK response:
  Lock-Token: <urn:uuid:UUID>
  Content-Type: application/xml; charset=utf-8
```

## Testing

### Unit tests
- XML generation para PROPFIND (directorio con archivos, directorio vacío, archivo individual)
- XML generation para LOCK response
- Parsing de Destination header para COPY/MOVE
- Validación de paths WebDAV

### Integration tests
- PROPFIND en raíz (Depth:0 y Depth:1)
- PROPFIND en subdirectorio
- GET de archivo vía WebDAV
- OPTIONS retorna headers correctos
- PUT crea archivo (con --webdav-rw)
- PUT rechazado sin --webdav-rw (403)
- DELETE borra archivo/directorio
- MKCOL crea directorio
- COPY/MOVE con Destination header
- LOCK retorna token UUID válido
- UNLOCK retorna 204
- Auth requerida si --user/--pass configurados

### Test manual
- Montar desde macOS Finder: `Cmd+K` → `http://host:port/`
- Montar desde Linux: `mount -t davfs http://host:port/ /mnt/dav`
- Montar desde Windows: Map network drive → `http://host:port/`

## Flags CLI finales

```
--webdav          Habilita servidor WebDAV (read-only)
--webdav-rw       Habilita operaciones de escritura en WebDAV (implica --webdav)
--port-dav PORT   Puerto WebDAV (default: auto-find desde 5001)
```

## Dependencias nuevas

Ninguna. El XML se genera con `format!()` — son templates fijos y simples, no justifican una lib XML.
