# List available recipes
default:
    @just --list

# Run CI gates: rust fmt/clippy/test + web check/format/build
check:
    cargo fmt --check
    cargo clippy --all-targets -- -D warnings
    cargo test
    cd web && pnpm check
    cd web && pnpm format:check
    cd web && pnpm build

# Build everything for release (SPA bundle + release binary)
build:
    cd web && pnpm build
    cargo build --release

# Run the server against the SPA bundle (sources .env if present)
run:
    cd web && pnpm build
    bash -c 'set -a; [ -f .env ] && . ./.env; set +a; cargo run'

# Frontend dev server (proxies /api to the rust server on :7778)
dev-web:
    cd web && pnpm dev

# Install/upgrade the systemd unit on THIS host (see docs/deploy.md)
deploy: build
    sudo deploy/install-systemd.sh

# Show what `just deploy` would do, without touching the host
deploy-dry-run: build
    deploy/install-systemd.sh --dry-run

# Tail the deployed service's log
deploy-logs:
    journalctl -u klams-view.service -f

# Rust server only, API mode (sources .env if present)
dev-api:
    bash -c 'set -a; [ -f .env ] && . ./.env; set +a; cargo run'
