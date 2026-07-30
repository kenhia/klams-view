# klams-view

A web-based viewer and dashboard for [klams](https://github.com/kenhia/klams),
the local agent memory system. Where the original `viewport` desktop app shows
raw records, klams-view aims to make the store *legible*: live activity, metrics
and time-series of memory growth, per-author contribution profiles, a search
workbench that exposes ranking, and curation surfaces (dissents) — all in a
dark-themed web UI reachable from any browser on the tailnet.

It follows the korg deployment shape: one Rust (axum) binary that serves the
built SvelteKit SPA and talks to the klams HTTP API server-side, holding the
bearer token so the browser never sees it. The server also computes the
aggregations the klams API doesn't expose directly.

- `src/` — the axum server (static bundle + `/api/*` aggregation layer)
- `web/` — SvelteKit (Svelte 5 + Tailwind 4) static SPA

## Development

This repo uses the [kprojects](https://github.com/kenhia/kprojects) minimal
harness: `just` lists recipes, `just check` runs the CI gates, sprint records
live under `sprints/`.

## License

MIT — see [LICENSE](LICENSE).
