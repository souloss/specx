# specX Docker deployment

The specX image contains the shared Vue web interface from `apps/desktop`, the Rust API, and the Web Host. It is deployed as one Docker service and keeps application data and backups in mounted volumes. Native desktop packaging is built separately from `src-tauri/` and is not part of the server image.

## Docker Compose

```sh
export SPECX_PASSWORD='choose-a-password-at-least-12-chars'
export SPECX_SESSION_SECRET="$(openssl rand -hex 32)"
docker compose -f deploy/docker-compose.yml up --build -d
```

Open `http://localhost:4224` and sign in with `SPECX_PASSWORD`.

For a pre-built image, set `SPECX_IMAGE` to its published image reference and use `deploy/docker-compose.release.yml` with the same two required secrets.

## Configuration

| Variable | Default | Description |
| --- | --- | --- |
| `SPECX_PASSWORD` | required | Password for the Web Host login. |
| `SPECX_SESSION_SECRET` | required | Secret used to derive session cookies. Keep it stable across restarts. |
| `SPECX_DATA_DIR` | `/app/data` | Local SQLite data directory. |
| `SPECX_DATABASE_URL` | unset | Optional backend URL. Use `sqlite:///app/data/specx.sqlite` or a `postgres://` / `postgresql://` URL. |
| `SPECX_DATABASE_MAX_POOL_SIZE` | `16` | Maximum PostgreSQL connection pool size. |
| `SPECX_SECRET_KEY` | unset | AES-GCM key material for encrypted private repository credentials. Required before creating a Secret. Keep it stable across restarts. |
| `SPECX_WEBHOOK_SECRET` | unset | HMAC secret for repository webhook requests. Set it to enable signed `sha256=` webhook delivery. |
| `SPECX_CONTROLLER_INTERVAL_SECONDS` | `5` | Background controller worker interval. |
| `SPECX_REPOSITORY_POLL_INTERVAL_SECONDS` | `300` | Minimum branch polling interval after a repository-backed specification is configured. |
| `SPECX_BACKUP_ROOT` | `/app/backups` | Local backup directory. |
| `SPECX_PUBLIC_ORIGIN` | `http://localhost:4224` | Public origin used by API write protections behind a reverse proxy. |
| `SPECX_BIND_ADDR` | `0.0.0.0` | Listener address inside the container. |
| `SPECX_PORT` | `4224` | Listener port inside the container. |

The complete deployment runs as one container and uses its persisted SQLite file by default; no external data service is required. Set `SPECX_DATABASE_URL` when the same image should use PostgreSQL. Persist both `/app/data` and `/app/backups`. Do not put secrets in the image or commit an environment file containing real values. The release compose file is the only supported pre-built deployment entry point.

## Reverse proxy

Forward the public HTTPS origin to port `4224` and set `SPECX_PUBLIC_ORIGIN` to the exact `https://host[:port]` origin without a path. Restrict direct access to the container listener so requests cannot bypass the proxy policy.

## Published OpenAPI documentation

Open an OpenAPI specification and choose **Share documentation**. An owner or
administrator can select a saved revision, review its content, set the title,
description and light/dark theme, and choose whether to display the definition
download action and a browser request workbench. Publishing creates an immutable, anonymously readable site;
later edits require another publication. Revoke a link from the same dialog.

For an application at `https://specx.example.com`, links use
`https://<random-publication-id>.specx.example.com/`. Configure wildcard DNS for
`*.specx.example.com`, a TLS certificate covering that wildcard, and forward
those hosts to the same Web Host with the original `Host` header preserved.
Proxy both the reader HTML/assets and `/api/specx/public/openapi/` requests.
Keep `SPECX_PUBLIC_ORIGIN=https://specx.example.com` for authenticated writes;
publication subdomains only need anonymous reads from SpecX. Online debugging, when enabled, sends requests directly from the reader’s browser to an explicit HTTP(S) target; the target must permit CORS. Workspace environments and credentials are never included.

On a local installation, links use `<random-publication-id>.localhost:<port>`
and work without an external DNS provider. These local links are accessible
only on the machine running SpecX. The application creates publication IDs;
it does not provision public DNS records or TLS certificates.
