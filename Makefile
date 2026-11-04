.DEFAULT_GOAL := help

PNPM ?= pnpm

.PHONY: help install dev dev-web dev-backend build check test test-core clean docker-build docker-up docker-down

help:
	@printf '%s\n' 'specX development targets:'
	@printf '  %-18s %s\n' 'make install' 'Install frontend dependencies'
	@printf '  %-18s %s\n' 'make dev' 'Start the specX frontend'
	@printf '  %-18s %s\n' 'make dev-backend' 'Start the specX API host'
	@printf '  %-18s %s\n' 'make build' 'Build the frontend and backend'
	@printf '  %-18s %s\n' 'make check' 'Run frontend and Rust checks'
	@printf '  %-18s %s\n' 'make test' 'Run the full test suite'
	@printf '  %-18s %s\n' 'make clean' 'Remove Rust build output'
	@printf '  %-18s %s\n' 'make docker-build' 'Build the specX Docker image'
	@printf '  %-18s %s\n' 'make docker-up' 'Build and start the Docker deployment'
	@printf '  %-18s %s\n' 'make docker-down' 'Stop the Docker deployment'

install:
	$(PNPM) install --frozen-lockfile

dev: install
	$(PNPM) dev

dev-web: dev

dev-backend:
	cargo run -p specx-web

build:
	$(PNPM) build:checked

check:
	$(PNPM) build:checked

test:
	$(PNPM) test:specx

test-core:
	$(PNPM) test:core

clean:
	cargo clean

docker-build:
	$(PNPM) docker:build

docker-up:
	$(PNPM) docker:up

docker-down:
	$(PNPM) docker:down
