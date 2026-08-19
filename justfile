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

# Sprint 003 (#809) — live-backend smoke: drive every /api route against
# a REAL klams, then again with a deliberately wrong token to prove the
# doctor tells "unreachable" and "unauthorized" apart.
#
# Deliberately NOT part of `just check`: the gate has to stay hermetic
# and tokenless. The CI-runnable half of #809 is
# `tests/api_contract.rs`, which `cargo test` already covers — it
# catches klams-view regressions; this catches upstream skew.
#
# Read-only. It seeds nothing, so pointing it at the live klams is safe.
smoke-live *ARGS:
    bash -c 'set -a; [ -f .env ] && . ./.env; set +a; exec scripts/smoke-live.sh {{ARGS}}'

# Frontend dev server (proxies /api to the rust server on :7779)
dev-web:
    cd web && pnpm dev

# Install/upgrade the systemd unit on THIS host (see docs/deploy.md)
deploy: build
    sudo deploy/install-systemd.sh

# Show what `just deploy` would do, without touching the host
deploy-dry-run: build
    deploy/install-systemd.sh --dry-run

# Publish the loopback listener to the tailnet over HTTPS (idempotent).
# Run once per host; see docs/deploy.md for why the service itself must
# stay bound to 127.0.0.1.
deploy-serve port="7779":
    tailscale serve --bg --https={{port}} http://localhost:{{port}}
    tailscale serve status

# Tail the deployed service's log
deploy-logs:
    journalctl -u klams-view.service -f

# Rust server only, API mode (sources .env if present)
dev-api:
    bash -c 'set -a; [ -f .env ] && . ./.env; set +a; cargo run'
