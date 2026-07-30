<script lang="ts">
  // Explore — the unified search workbench (POST /memory/search: the
  // endpoint the old viewport never exposed). Score bars show the
  // fused RRF ranking; knowledge hits open the full item.
  import { api } from "$lib/api";
  import type { MemoryKind, MemoryRow, SearchHit, SearchResults } from "$lib/types";
  import { KIND_COLOR, KIND_LABEL, KINDS } from "$lib/kinds";
  import Drawer from "$lib/components/Drawer.svelte";
  import MemoryDetail from "$lib/components/MemoryDetail.svelte";

  let query = $state("");
  let types = $state<Record<MemoryKind, boolean>>({ knowledge: true, fact: true, event: true });
  let tag = $state("");
  let repo = $state("");
  let host = $state("");
  let topK = $state(20);
  let results = $state<SearchResults | null>(null);
  let searching = $state(false);
  let error = $state<string | null>(null);
  let selected = $state<MemoryRow | null>(null);

  async function run(e?: Event) {
    e?.preventDefault();
    if (!query.trim()) return;
    searching = true;
    error = null;
    try {
      const filters: Record<string, string> = {};
      if (tag.trim()) filters.tag = tag.trim();
      if (repo.trim()) filters.repo = repo.trim();
      if (host.trim()) filters.host = host.trim();
      const selectedTypes = KINDS.filter((k) => types[k]);
      results = await api.search({
        query,
        types: selectedTypes.length === KINDS.length ? undefined : selectedTypes,
        filters: Object.keys(filters).length ? filters : undefined,
        top_k: topK,
      });
    } catch (e2) {
      error = String(e2);
    } finally {
      searching = false;
    }
  }

  function open(hit: SearchHit) {
    // MemoryDetail fetches the full item for knowledge; fact/event
    // hits only carry their payload here.
    selected = {
      id: hit.id,
      kind: hit.type,
      tags: [],
      author: { agent_name: "—" },
      created_at: "",
      updated_at: "",
      payload: hit.payload,
    };
  }

  const maxScore = $derived(results ? Math.max(0.001, ...results.results.map((r) => r.score)) : 1);
</script>

<svelte:head><title>klams-view · Explore</title></svelte:head>

<h1 class="text-lg font-semibold">Explore</h1>
<p class="mt-1 text-sm text-[var(--color-muted)]">
  Hybrid search across the whole store — dense vectors + full-text, fused.
</p>

<form class="mt-4 flex flex-wrap items-center gap-2" onsubmit={run}>
  <input
    class="w-full max-w-xl rounded border border-[var(--color-border)] bg-[var(--color-surface)] px-3 py-1.5 text-sm placeholder:text-[var(--viz-muted)] focus:outline-2 focus:outline-[var(--color-accent)]"
    placeholder="what do the agents know about…"
    bind:value={query}
  />
  <button
    class="rounded bg-[var(--color-accent-soft)] px-4 py-1.5 text-sm font-medium hover:opacity-90 disabled:opacity-50"
    disabled={searching || !query.trim()}
    type="submit"
  >
    {searching ? "searching…" : "Search"}
  </button>
</form>

<div class="mt-2 flex flex-wrap items-center gap-x-4 gap-y-2 text-xs">
  {#each KINDS as k (k)}
    <label class="inline-flex items-center gap-1.5">
      <input type="checkbox" bind:checked={types[k]} class="accent-[var(--color-accent)]" />
      <span class="inline-block h-2.5 w-2.5 rounded-sm" style="background:{KIND_COLOR[k]}"></span>
      {KIND_LABEL[k]}
    </label>
  {/each}
  <input
    class="w-28 rounded border border-[var(--color-border)] bg-[var(--color-surface)] px-2 py-1"
    placeholder="tag"
    bind:value={tag}
  />
  <input
    class="w-28 rounded border border-[var(--color-border)] bg-[var(--color-surface)] px-2 py-1"
    placeholder="repo"
    bind:value={repo}
  />
  <input
    class="w-28 rounded border border-[var(--color-border)] bg-[var(--color-surface)] px-2 py-1"
    placeholder="host"
    bind:value={host}
  />
  <label class="inline-flex items-center gap-1.5 text-[var(--color-muted)]">
    top
    <input
      type="number"
      min="1"
      max="100"
      bind:value={topK}
      class="w-16 rounded border border-[var(--color-border)] bg-[var(--color-surface)] px-2 py-1"
    />
  </label>
</div>

{#if error}
  <p class="mt-4 text-sm" style="color:var(--status-critical)">✕ {error}</p>
{/if}

{#if results}
  <div class="mt-4">
    {#if results.degraded}
      <p class="mb-2 text-xs" style="color:var(--status-serious)">
        ▲ degraded — one retrieval source was unavailable; ranking is partial
      </p>
    {/if}
    <p class="mb-2 text-xs text-[var(--color-muted)]">{results.results.length} results</p>
    <ol class="space-y-1">
      {#each results.results as hit, i (hit.id)}
        <li>
          <button
            class="block w-full rounded px-2 py-2 text-left hover:bg-[var(--color-surface-hi)]"
            onclick={() => open(hit)}
          >
            <div class="flex items-center gap-2">
              <span class="w-5 shrink-0 text-right text-xs text-[var(--viz-muted)] tabular-nums"
                >{i + 1}</span
              >
              <span
                class="inline-block h-2.5 w-2.5 shrink-0 rounded-sm"
                style="background:{KIND_COLOR[hit.type]}"
                title={hit.type}
              ></span>
              <div
                class="h-1.5 w-24 shrink-0 overflow-hidden rounded-full bg-[var(--color-surface-hi)]"
              >
                <div
                  class="h-full rounded-full"
                  style="width:{(hit.score / maxScore) * 100}%;background:{KIND_COLOR[hit.type]}"
                ></div>
              </div>
              <span class="shrink-0 text-xs text-[var(--color-muted)] tabular-nums"
                >{hit.score.toFixed(3)}</span
              >
              <span class="truncate text-sm">{hit.preview}</span>
            </div>
          </button>
        </li>
      {/each}
    </ol>
  </div>
{/if}

<Drawer
  open={selected !== null}
  onclose={() => (selected = null)}
  title={selected ? `${selected.kind} · ${selected.id.slice(0, 8)}…` : ""}
>
  {#if selected}
    <MemoryDetail m={selected} />
  {/if}
</Drawer>
