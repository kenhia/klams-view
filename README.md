# klams-view

A web-based viewer and dashboard for [klams](https://github.com/kenhia/klams),
the local agent memory system. Where the original `viewport` desktop app shows
raw records, klams-view aims to make the store *legible*: live activity, metrics
and time-series of memory growth, per-author contribution profiles, a search
workbench that exposes ranking, and curation surfaces (dissents) — in a
dark-themed web UI you can open from any browser that can reach it.

It follows the korg deployment shape: one Rust (axum) binary that serves the
built SvelteKit SPA and talks to the klams HTTP API server-side, holding the
bearer token so the browser never sees it. The server also computes the
aggregations the klams API doesn't expose directly.

- `src/` — the axum server (static bundle + `/api/*` aggregation layer)
- `web/` — SvelteKit (Svelte 5 + Tailwind 4) static SPA

klams-view is **read-only**: it needs nothing beyond a read-scoped klams token.
It has no authentication of its own, so treat "who can reach the port" as "who
can read the memory store", and choose the bind address accordingly.

## Quick start

You need a reachable klams instance, a Rust toolchain, and
[pnpm](https://pnpm.io) plus [just](https://github.com/casey/just).

```sh
cp .env.example .env    # then set KLAMS_URL and KLAMS_TOKEN
just run                # builds the SPA and serves it on :7779
```

`just` on its own lists every recipe. The two-terminal dev loop is
`just dev-api` (server) alongside `just dev-web` (vite on :5174, proxying
`/api`). `just check` runs the CI gate: `cargo fmt`/`clippy`/`test`,
`svelte-check`, prettier, and an SPA build.

## Testing

Two layers, and the split is deliberate:

- **`just check`** — hermetic and tokenless, so CI can run it.
  `tests/api_contract.rs` boots a stub klams on a loopback port and
  drives the real `/api` router against it over real HTTP, covering
  every route, the aggregations, and the error envelopes. It catches
  regressions in klams-view.
- **`just smoke-live`** — the same routes against a **real** klams
  (`KLAMS_URL` / `KLAMS_TOKEN` from `.env`), plus a second instance with
  a deliberately wrong token to prove the doctor tells "unreachable"
  and "unauthorized" apart. It catches skew in the *upstream*, which the
  hermetic layer structurally cannot see. Read-only — it seeds nothing,
  so pointing it at the live klams is safe.

Run `just smoke-live` before `just publish`: a green run against a newer
klams is what licenses bumping `KLAMS_VERIFIED_VERSION` in
`src/doctor.rs`.

## Diagnosing a bad connection

`/health` opens with the connection doctor: `KLAMS_URL` parses →
`KLAMS_TOKEN` set → DNS → TCP → TLS → unauthenticated `/healthz` →
**authenticated read** → klams version vs the version klams-view was
verified against. Each link reports separately, and the one that failed
carries the fix. The authenticated step is the point — `/healthz` is
unauthenticated, so it stays green while a stale token fails every read.

## Deployment

Releases go through the homelab package store: `just publish` puts a
versioned binary + SPA bundle in the store, and `just deploy` fetches,
checksum-verifies and installs them on the host you run it from —
including a check that the binary reports the version it was published
as. `just install-systemd` owns the system user, config and unit (first
install and unit changes); `just rollback` swaps both assets back
together. See [docs/deploy.md](docs/deploy.md).

## Development

This repo uses the [kprojects](https://github.com/kenhia/kprojects) minimal
harness: sprint records live under `sprints/`, design notes in
[docs/design.md](docs/design.md).

## License

MIT — see [LICENSE](LICENSE).
