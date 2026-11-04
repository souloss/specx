# syntax=docker/dockerfile:1

FROM node:22-slim AS frontend
WORKDIR /app
RUN npm install --global pnpm
COPY package.json pnpm-lock.yaml pnpm-workspace.yaml ./
COPY apps/desktop/ apps/desktop/
COPY crates/specx-api/tests/fixtures/error-catalog.json crates/specx-api/tests/fixtures/error-catalog.json
COPY release.config.json ./
COPY releases/ releases/
RUN --mount=type=cache,target=/root/.local/share/pnpm/store pnpm install --frozen-lockfile
RUN pnpm --dir apps/desktop build

FROM rust:1-bookworm AS backend
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY core/Cargo.toml core/Cargo.toml
COPY crates/specx-types/Cargo.toml crates/specx-types/Cargo.toml
COPY crates/specx-formats/Cargo.toml crates/specx-formats/Cargo.toml
COPY crates/specx-platform/Cargo.toml crates/specx-platform/Cargo.toml
COPY crates/specx-api/Cargo.toml crates/specx-api/Cargo.toml
COPY crates/specx-web/Cargo.toml crates/specx-web/Cargo.toml
RUN mkdir -p core/src crates/specx-types/src crates/specx-formats/src crates/specx-platform/src \
  crates/specx-api/src crates/specx-web/src \
  && touch core/src/lib.rs crates/specx-types/src/lib.rs crates/specx-formats/src/lib.rs \
  crates/specx-platform/src/lib.rs crates/specx-api/src/lib.rs crates/specx-web/src/main.rs \
  && cargo build --release -p specx-api -p specx-platform
COPY core/ core/
COPY crates/specx-types/ crates/specx-types/
COPY crates/specx-formats/ crates/specx-formats/
COPY crates/specx-platform/ crates/specx-platform/
COPY crates/specx-api/ crates/specx-api/
COPY crates/specx-web/ crates/specx-web/
RUN find core crates/specx-types crates/specx-formats crates/specx-platform crates/specx-api crates/specx-web \
  -type f -exec touch {} +
RUN cargo build --release -p specx-web

FROM debian:bookworm-slim
RUN apt-get update \
  && apt-get install --yes --no-install-recommends ca-certificates libssl3 postgresql-client \
  && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=backend /app/target/release/specx-web /usr/local/bin/specx-web
COPY --from=frontend /app/apps/desktop/dist /app/static
COPY LICENSES/ /app/licenses/
ENV SPECX_STATIC_DIR=/app/static
ENV SPECX_DATA_DIR=/app/data
ENV SPECX_BACKUP_ROOT=/app/backups
ENV SPECX_BIND_ADDR=0.0.0.0
ENV SPECX_PORT=4224
EXPOSE 4224
VOLUME ["/app/data", "/app/backups"]
ENTRYPOINT ["specx-web"]
