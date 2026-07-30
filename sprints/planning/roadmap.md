# Roadmap

> The general plan for this project. Keep it current; detail lives in the
> sprint records.

## Now

- Sprint 001 — scaffold + first light: repo harness, axum server
  (static hosting + `/api/status`), SvelteKit dark shell, design doc,
  and a Pulse dashboard fed by real aggregations over the klams API.

## Next

- Browse surfaces: unified search workbench (the `search_unified`
  endpoint the old viewport never wired up), knowledge browser with
  facet filters (repo/machine/tag), facts + events tables with real
  pagination.
- Authors: contribution profiles with per-author activity charts.
- Live activity: auto-refreshing memory timeline (the viewport was
  manual-pull everywhere).
- Deploy story: container on kubsdb or unit on kubs0 — decide with Ken.

## Later / Ideas

- Supersede lineage viewer (no UI exists anywhere for the preferred
  correction path).
- Trust/decay surfacing (parked viewport roadmap item).
- Context-preview workbench with token budget slider (port the one
  good dark-themed screen from viewport).
- Store health page over `/metrics` (queue depth, latencies).
- Curation actions (dissent promote/discard) behind a Manage-scope
  token.
