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
just run                # builds the SPA and serves it on :7778
```

`just` on its own lists every recipe. The two-terminal dev loop is
`just dev-api` (server) alongside `just dev-web` (vite on :5174, proxying
`/api`). `just check` runs the CI gate: `cargo fmt`/`clippy`/`test`,
`svelte-check`, prettier, and an SPA build.

## Deployment

`just deploy` installs it as a systemd unit on the machine you run it from —
see [docs/deploy.md](docs/deploy.md).

## Development

This repo uses the [kprojects](https://github.com/kenhia/kprojects) minimal
harness: sprint records live under `sprints/`, design notes in
[docs/design.md](docs/design.md).

## License

MIT — see [LICENSE](LICENSE).
