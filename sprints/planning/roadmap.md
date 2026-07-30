# Roadmap

> The general plan for this project. Keep it current; detail lives in the
> sprint records.

## Now

- Deploy story: container on kubsdb or unit on kubs0 — decide with
  Ken, then a deploy skill/recipe + dedicated read-only token in
  `/etc/klams/klams.toml`.

## Next

- Knowledge browser with facet filters (repo/machine/tag/language) —
  there is still no way to browse knowledge without a query.
- Search-ranking workbench over MCP (`ScoredMemory` raw vs fused
  scores, `memory_related`) — REST doesn't expose them.
- Component/E2E tests (playwright is already proven against the app
  headlessly).

## Later / Ideas

- Supersede lineage viewer (no UI exists anywhere for the preferred
  correction path).
- Trust/decay surfacing (parked viewport roadmap item).
- Context-preview workbench with token budget slider (port the one
  good dark-themed screen from viewport).
- Store health page over `/metrics` (queue depth, latencies).
- Curation actions (dissent promote/discard) behind a Manage-scope
  token.
