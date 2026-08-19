# Roadmap

> The general plan for this project. Keep it current; detail lives in the
> sprint records.

## Now

- Sprint 003 closed klams-view's entire open backlog: the connection
  doctor on `/api/status` + `/health` (#808), two test layers over the
  `/api` contract (#809 — hermetic in CI, live via `just smoke-live`),
  the author residuals (#807), and publish-then-deploy through the
  homelab package store (#1013).

## Next

- Merge klams' kind segments into one `created_at` timeline for the
  author memory list (#1448) — `/v1/authors/{id}/memories` serves
  postgres-backed kinds before knowledge, so an author whose recent
  writes are all knowledge gets a first page with none of them.
- Knowledge browser with facet filters (repo/machine/tag/language) —
  there is still no way to browse knowledge without a query.
- Search-ranking workbench over MCP (`ScoredMemory` raw vs fused
  scores, `memory_related`) — REST doesn't expose them.
- Component/E2E tests (playwright is already proven against the app
  headlessly). Sprint 003 covered the server-to-klams contract from
  both sides; this is the browser side, still wanted.

## Later / Ideas

- Supersede lineage viewer (no UI exists anywhere for the preferred
  correction path).
- Trust/decay surfacing (parked viewport roadmap item).
- Context-preview workbench with token budget slider (port the one
  good dark-themed screen from viewport).
- Store health page over `/metrics` (queue depth, latencies).
- Curation actions (dissent promote/discard) behind a Manage-scope
  token.
