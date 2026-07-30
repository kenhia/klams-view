<!-- kproject:begin — managed by kprojects/install.sh; do not edit inside this block -->
## kproject conventions

This project uses the kproject minimal harness
(`~/src/ai-agents/kprojects`). Keep context small; prefer doing over
ceremony.

### Layout

- `sprints/` — the project's evolution, one record per PR-sized unit of
  work (a "sprint")
  - `planning/` — planning docs; at minimum `roadmap.md` (the general plan)
  - `review/` — more formal reviews as the project matures
  - sprint records: `###-<short-name>.md` for small projects, or a
    `###-<short-name>/` directory of files for larger/more formal ones
  - a sprint record is one informal narrative: goal, decisions, what
    shipped, follow-ups — written during the sprint, not after
- `docs/` — project documentation, architecture, usage
- `.scratch/` — git-ignored scratch space for user or agent ephemera;
  use it instead of /tmp
- `justfile` — dev recipes; default recipe is `@just --list`; `just check`
  runs the CI gates; `just deploy` (or variants) if the project deploys
- `.env` — git-ignored; tokens and environment vars

### Workflow

- One sprint ≈ one PR. Sprint proposals and work items are managed in
  `korg`; durable cross-project knowledge goes in `klams`.
- If the korg or klams MCP tools are unavailable in your session, say so
  up front — don't silently work around missing infrastructure.
- TDD preferred: write the failing test first when practical.

### Tooling preferences

- Python managed by `uv`; lint/format with `ruff`; typecheck with `ty`
  (astral toolchain)
- License is MIT unless specifically directed otherwise
<!-- kproject:end -->

## Project

klams-view is a web viewer + dashboard for the klams memory service —
a korg-shaped single binary: axum serves the built SvelteKit SPA from
`web/build` and an `/api/*` aggregation layer that calls the klams
HTTP API server-side (bearer token stays in the server; the browser
never sees it). It replaces the parked Tauri `viewport` app with a
dark-themed, chart-forward, tailnet-reachable UI. Status: fresh
scaffold; design + first real pages in progress.

### Build / run / test

- `just check` — fmt, clippy `-D warnings`, cargo test, svelte-check,
  prettier check, SPA build. This is the CI gate.
- `just run` — build SPA + run server on `127.0.0.1:7778` (sources
  `.env`: `KLAMS_URL`, `KLAMS_TOKEN`, `KLAMS_VIEW_ADDR`,
  `KLAMS_VIEW_STATIC`).
- `just dev-api` + `just dev-web` — two-terminal dev loop; vite on
  :5174 proxies `/api` to :7778.

### Read first

- `src/main.rs`, `src/api.rs`, `src/config.rs` — the whole server.
- `web/src/routes/+layout.svelte` — nav + shell; `web/src/app.css` —
  the oklch design tokens (dark only, korg-lineage).
- `sprints/planning/roadmap.md` — where this is going.
- Cross-repo: a read-only klams clone lives at
  `/home/ken/tmp-clone/klams` (API truth:
  `crates/klams-api/src/router.rs`, types: `crates/klams-types`);
  korg (`/home/ken/tmp-clone/korg`) is the design/deploy pattern
  donor. The real klams working copy `~/src/ai/klams` has an active
  sprint — don't read it mid-session, use the tmp-clone.

### Gotchas

- klams-view talks to klams **only** through the public HTTP API —
  never Postgres directly; the klams schema is not a contract.
- The klams service listens on `:7777` (kubs0); klams-view claims
  `:7778`. Vite dev uses :5174 (korg's dev server owns 5173).
- SPA fallback must stay `ServeDir::fallback(ServeFile)` so deep
  links return 200, not 404 (korg WI #284 lesson).
- Dark theme only; tokens are oklch CSS vars in `@theme` — no
  hardcoded hex in components.
