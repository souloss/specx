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
| `SPECX_BACKUP_ROOT` | `/app/backups` | Local backup directory. |
| `SPECX_PUBLIC_ORIGIN` | `http://localhost:4224` | Public origin used by API write protections behind a reverse proxy. |
| `SPECX_BIND_ADDR` | `0.0.0.0` | Listener address inside the container. |
| `SPECX_PORT` | `4224` | Listener port inside the container. |

The complete deployment runs as one container and uses its persisted SQLite file by default; no external data service is required. Set `SPECX_DATABASE_URL` when the same image should use PostgreSQL. Persist both `/app/data` and `/app/backups`. Do not put secrets in the image or commit an environment file containing real values. The release compose file is the only supported pre-built deployment entry point.

## Reverse proxy

Forward the public HTTPS origin to port `4224` and set `SPECX_PUBLIC_ORIGIN` to the exact `https://host[:port]` origin without a path. Restrict direct access to the container listener so requests cannot bypass the proxy policy.
