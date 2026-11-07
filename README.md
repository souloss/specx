# specX

specX is a service directory and specification workspace. It organizes services, nested groups, tags, and Markdown specifications behind a small Rust API and a shared Vue 3 frontend. The same `apps/desktop` frontend runs in the browser through the Web host and in the native Tauri shell.

## Run locally

Requirements: Node.js 22+, pnpm 10.34.7, and Rust 1.90+.

```sh
pnpm install
pnpm dev
```

The frontend opens on the Vite URL shown in the terminal and proxies `/api` requests to the Rust host. Run the persistent API host in another terminal:

```sh
SPECX_PASSWORD='choose-a-password-at-least-12-chars' pnpm dev:backend
```

启用私有仓库凭据和仓库 Webhook 时，另外设置 `SPECX_SECRET_KEY`（用于 AES-GCM
密钥派生）和 `SPECX_WEBHOOK_SECRET`（用于 `sha256=` HMAC 签名校验）。Web Host
会在后台周期性运行 Repository Controller；可用 `SPECX_CONTROLLER_INTERVAL_SECONDS`
调整执行间隔，`SPECX_REPOSITORY_POLL_INTERVAL_SECONDS` 调整分支轮询间隔。

The host stores local data in `SPECX_DATA_DIR/specx.sqlite` by default. Set `SPECX_DATABASE_URL` to select a backend explicitly: use `sqlite:///absolute/path/specx.sqlite` for SQLite or `postgres://user:password@host/specx` for PostgreSQL. Both backends expose the same API and domain operations. The browser uses a server-side password session and reads or writes catalog data through `/api/specx`; only temporary panel state stays in local storage.

To run the native desktop shell, use `pnpm dev:desktop`; `pnpm build:desktop` creates the platform installer. Desktop data defaults to the operating system's application data directory. It accepts the same `SPECX_DATABASE_URL` schemes as the Web host.

## Docker deployment

The supported deployment is Docker Compose. It builds the frontend and Rust host into one image and persists application data and backups in named volumes.

```sh
export SPECX_PASSWORD='choose-a-password-at-least-12-chars'
export SPECX_SESSION_SECRET="$(openssl rand -hex 32)"
docker compose -f deploy/docker-compose.yml up --build -d
```

Open `http://localhost:4224`. The complete image and release compose file are in [`deploy/`](deploy/).

## Repository layout

- `apps/desktop/`: Vue 3 + Pinia frontend shared by Web and Tauri; `lib/backend` selects HTTP or Tauri IPC.
- `src-tauri/`: standalone native shell. It dispatches IPC requests into the shared `specx-api` router and is intentionally outside `crates/`.
- `core/`: specX domain rules and SQLite/PostgreSQL persistence adapters.
- `crates/specx-types/`: transport-neutral shared domain data types.
- `crates/specx-formats/`: pure content normalization and format helpers.
- `crates/specx-platform/`: process-level path and environment primitives.
- `crates/specx-api/`: authenticated service directory API.
- `crates/specx-web/`: Rust HTTP host and static asset server.
- `deploy/`: Dockerfile and Docker Compose deployment files.
- `docs/`: architecture and API notes.

## Checks

```sh
pnpm build:checked
pnpm test:specx
```

`refs/` contains local reference repositories and is intentionally ignored by Git.

Desktop versions, signed updates, bilingual release notes, and Tag-based publishing
follow the [desktop release conventions](docs/guides/desktop-releases.md).

The [application foundation baseline](docs/architecture/foundation-baseline.md)
records the shared frontend/backend boundaries, current capabilities, validation,
and the entry point for the next business feature. Coding agents should read the
root [AGENTS.md](AGENTS.md); API and i18n conventions are recorded in
[`docs/architecture/`](docs/architecture/).
