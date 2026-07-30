# Sprint 001 — scaffold and first light

**Goal**: stand up klams-view from an empty directory to a working,
dark-themed web dashboard running against the live klams on kubs0.

## Decisions

- **Stack**: korg-shaped single axum binary (`:7778`) serving a
  SvelteKit (Svelte 5 + Tailwind 4) static SPA plus an `/api/*`
  aggregation layer. The server holds the `read`-scope klams token;
  the browser only ever talks to klams-view. Rationale: matches the
  household deploy pattern, tailnet-reachable from any machine, and a
  server-side layer is the right home for the aggregations klams
  doesn't expose (corpus totals, time bucketing, prometheus parsing).
- **Contract research first**: two survey passes over the read-only
  clone produced docs/design.md's gotcha list — notably that the
  facts/events/dissents cursors are broken upstream (echoed
  first-page loop), `/v1/memories` + `/v1/authors` cursors work, and
  `/metrics` needs no token and carries per-agent MCP usage.
- **Design**: dataviz-skill method. Kind identity fixed app-wide
  (knowledge/fact/event = series 1/2/3, the all-pairs-safe trio);
  the 8-slot reference dark palette re-validated against this app's
  surface (`#17181f`) — all checks pass. Charts are hand-rolled SVG.
- **Dissents deliberately absent** (near-dead feature per the klams
  deep review); supersede lineage got a viewer instead (the preferred
  correction path had none anywhere).

## Shipped

- Rust server: config from env/.env, static hosting with SPA
  fallback, passthrough routes, `/api/overview`, `/api/activity`
  (server-side page walk + linear-time bucket fill + honest
  `covered_since` when the 100-page cap truncates), prometheus text
  parser (histograms render as summaries upstream — quantile labels),
  60s metrics sampler ring buffer (24h).
- SPA: Pulse (health strip, stat tiles, writes-over-time stacked
  columns, √-scale agent bars, latest-writes feed), Explore (unified
  search workbench with score bars + full knowledge drawer with
  supersede navigation), Activity (windowed browse, working cursor
  paging, auto-refresh), Authors (+ detail), Health (queue/latency
  tiles, sampler line charts, per-agent MCP usage).
- Verified live against klams 0.1.36 on kubs0 (180k+ knowledge
  chunks) with headless-chromium screenshots; fixed what the render
  pass caught (sparse-bucket axis compression, duplicate
  `agent_name` keys blanking Authors).

## Follow-ups

- Deploy story (kubs0 unit vs kubsdb container) — Ken's call.
- Dedicated `[[auth.tokens]]` read-only grant for klams-view (dev
  currently borrows the claude MCP token via `.env`).
- Later ideas live in the roadmap: MCP-backed ranking workbench,
  context-preview, supersede lineage graph, decay surfacing.
