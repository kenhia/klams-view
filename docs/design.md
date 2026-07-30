# klams-view design

The store has become the interesting artifact: a dozen agents write to
it, retrieval quality is actively curated, and the operator's question
has shifted from "what row is in the table" (the old viewport's job)
to "what is happening in my memory system, and is it healthy?"
klams-view is built around that question.

## Shape

One axum binary (`:7779`). It serves the SvelteKit SPA and an `/api/*`
layer that is the only thing the browser talks to. The server holds a
`read`-scope klams token and calls the klams REST surface + the public
`/healthz` and `/metrics`; it also owns every aggregation the klams
API doesn't provide (corpus totals, time-bucketing, prometheus
parsing, metrics history sampling). klams-view never touches Postgres
or Qdrant — the HTTP surface is the contract.

## Pages

| Route | Name | Job |
|---|---|---|
| `/` | **Pulse** | The dashboard. Stat tiles (memories by kind, authors, version, uptime, queue), activity-over-time stacked chart, per-agent usage bars from `/metrics`, subsystem health strip, recent-writes feed. |
| `/explore` | **Explore** | Unified search workbench over `POST /memory/search` — the endpoint the old viewport never wired up. Query + type toggles + `RetrievalFilters` (repo/tag/host/source/time), score-bar results, detail drawer (knowledge → full `KnowledgeItem` with supersede chain links). |
| `/activity` | **Activity** | Time-windowed browse of `/v1/memories`: histogram on top, table under it, kind/author/state filters, cursor pagination, auto-refresh toggle. |
| `/authors` | **Authors** | Contribution profiles: table + per-author kind-mix bars, last-seen. Detail page: profile fields + their memories (section-scoped cursor). |
| `/health` | **Health** | `healthz` snapshot, queue gauges, latency quantiles, write/search counters, backup freshness, maintenance state — sparklines from the server's metrics sampler. |

Deliberately absent: dissents (a nearly-dead feature per the klams
deep review — its one elaborate viewport screen guards ~one dissent
ever filed) and write/curation actions (v1 is read-only by token
design; curation comes later behind a `manage` token if wanted).

## Server `/api`

| Endpoint | Backing | Notes |
|---|---|---|
| `GET /api/status` | `/healthz` reachability | exists |
| `GET /api/overview` | authors + healthz + metrics + `/v1/memories` first page | one call renders Pulse |
| `GET /api/activity?since&until&kinds&authors&state&bucket` | pages `/v1/memories` server-side | returns time buckets by kind; page fetch capped, cap reported |
| `GET /api/memories?…` | `/v1/memories` passthrough | table + cursor |
| `GET /api/authors`, `/api/authors/{id}`, `/api/authors/{id}/memories` | passthrough | |
| `POST /api/search` | `/memory/search` passthrough | |
| `GET /api/knowledge/{id}` | passthrough | richest knowledge shape |
| `GET /api/health` | `/healthz` passthrough | full snapshot |
| `GET /api/metrics/summary` | parsed `/metrics` | queue, writes by type, MCP per-agent, backup age, quantiles |
| `GET /api/metrics/history` | in-memory sampler | ring buffer, 60s interval; resets on restart — good enough for sparklines |

Aggregation contracts live in `src/api.rs` types; the frontend mirrors
them in `web/src/lib/types.ts` by hand (two files, kept small on
purpose — no codegen at this scale).

### klams contract gotchas the server encodes (so the UI never sees them)

- Facts/events/dissents cursors are broken server-side (echoed
  first-page loop); paginate those via `created_before`/`created_after`
  walks. `/v1/*` cursors work; treat as opaque.
- `/v1/memories` window ≤ 30 days (`window_too_large`), limit ≤ 200.
- Enum spellings differ per endpoint (PascalCase `Source`/`FactType`,
  lowercase `kind`). `PublicMemory` content is flattened, optional
  fields are *absent* not null, and deleted fact rows repeat
  `deleted_at`.
- Prometheus histograms render as summaries (`quantile` label, no
  `_bucket`).
- Maintenance 503s use `{error, retry_after_seconds}` — a different
  envelope from `ApiError`.
- `counts.restores_received` is hardcoded 0 upstream — never chart it.

## Visual identity

Dark only. Chrome tokens in `web/src/app.css`: near-black blue-grey
bg (`oklch(0.17 0.012 280)`), violet accent (hue 300) — deliberately
adjacent to korg's blue (240) so the two apps read as siblings, not
clones. Data never wears chrome colors: charts use the 8-slot
`--series-*` palette (dataviz-skill reference dark set, re-validated
against this surface — all checks pass), status uses `--status-*`
only.

Kind identity is fixed app-wide: **knowledge = series-1 (blue), fact
= series-2 (orange), event = series-3 (green)** — slots 1-3 are the
all-pairs-safe trio, and these three appear together everywhere.
Author series take slots in author order, fold to "Other" past 8 (3
for scatter-class forms). Charts are hand-rolled SVG in Svelte — no
chart library; the dataviz skill's mark/interaction specs are the
spec (thin marks, 2px gaps, crosshair+tooltip, direct labels, legend
for ≥2 series, one axis, no dual-scale ever).

## Later (roadmap holds the list)

Search-ranking workbench over MCP (`ScoredMemory` raw vs fused
scores, `memory_related` — REST doesn't expose them), context-preview
with token-budget slider, supersede lineage graph, trust/decay
surfacing, curation behind `manage`.
