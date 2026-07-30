# Roadmap

> The general plan for this project. Keep it current; detail lives in the
> sprint records.

## Now

- Nothing in flight. Sprint 002 closed out the deploy story (systemd
  unit on the klams host, `just deploy`, docs/deploy.md), the
  dedicated read-only token, and publishing the repo.

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
