<script lang="ts">
  // Detail body for a memory. Knowledge ids get the full KnowledgeItem
  // (the richest shape on the REST surface) with supersede-chain
  // navigation; facts/events show their row payload.
  import { api } from "$lib/api";
  import type { KnowledgeItem, MemoryRow } from "$lib/types";
  import { relTime } from "$lib/format";

  let { m }: { m: MemoryRow } = $props();

  let item = $state<KnowledgeItem | null>(null);
  let error = $state<string | null>(null);
  let currentId = $state<string>("");

  async function load(id: string) {
    currentId = id;
    item = null;
    error = null;
    try {
      item = await api.knowledge(id);
    } catch (e) {
      error = String(e);
    }
  }

  $effect(() => {
    if (m.kind === "knowledge") void load(m.id);
  });

  const meta = $derived.by(() => {
    if (!item) return [];
    return [
      ["repo", item.repo],
      ["file", item.file],
      ["machine", [item.machine, ...(item.machines ?? [])].filter(Boolean).join(", ") || null],
      ["heading", item.heading_path],
      ["language", item.language],
      ["chunk", item.chunk_index?.toString() ?? null],
      ["volatility", item.volatility ?? null],
      ["source", item.source],
      ["confidence", item.confidence?.toFixed(2)],
      ["decay weight", item.decay_weight?.toFixed(2)],
      ["use count", item.use_count?.toString()],
      ["last used", item.last_used_at ? relTime(item.last_used_at) : "never"],
      ["created", relTime(item.created_at)],
      ["updated", relTime(item.updated_at)],
    ].filter(([, v]) => v !== null && v !== undefined && v !== "") as [string, string][];
  });
</script>

{#if m.kind === "knowledge"}
  {#if error}
    <p class="text-sm text-[var(--status-critical)]">{error}</p>
  {:else if !item}
    <p class="text-sm text-[var(--color-muted)]">loading…</p>
  {:else}
    {#if item.superseded_by || item.supersedes}
      <div class="mb-3 flex flex-wrap gap-2 text-xs">
        {#if item.superseded_by}
          <button
            class="rounded border border-[var(--status-serious)] px-2 py-1 text-[var(--status-serious)] hover:bg-[var(--color-surface-hi)]"
            onclick={() => load(item!.superseded_by!)}
          >
            ▲ superseded — view replacement
          </button>
        {/if}
        {#if item.supersedes}
          <button
            class="rounded border border-[var(--color-border)] px-2 py-1 text-[var(--color-muted)] hover:bg-[var(--color-surface-hi)]"
            onclick={() => load(item!.supersedes!)}
          >
            ↩ supersedes an earlier memory — view it
          </button>
        {/if}
      </div>
    {/if}
    {#if item.tags.length}
      <div class="mb-3 flex flex-wrap gap-1.5">
        {#each item.tags as tag (tag)}
          <span
            class="rounded-full bg-[var(--color-surface-hi)] px-2 py-0.5 text-xs text-[var(--color-muted)]"
            >{tag}</span
          >
        {/each}
      </div>
    {/if}
    <pre
      class="mb-4 rounded bg-[var(--color-bg)] p-3 text-xs leading-relaxed whitespace-pre-wrap">{item.text}</pre>
    <dl class="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-xs">
      {#each meta as [k, v] (k)}
        <dt class="text-[var(--color-muted)]">{k}</dt>
        <dd class="break-all">{v}</dd>
      {/each}
      <dt class="text-[var(--color-muted)]">id</dt>
      <dd class="font-mono text-[10px] break-all">{currentId}</dd>
    </dl>
  {/if}
{:else}
  <dl class="mb-3 grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-xs">
    <dt class="text-[var(--color-muted)]">kind</dt>
    <dd>{m.kind}{m.kind === "event" ? ` · ${m.category}` : m.type ? ` · ${m.type}` : ""}</dd>
    <dt class="text-[var(--color-muted)]">author</dt>
    <dd>{m.author.agent_name}{m.author.model ? ` (${m.author.model})` : ""}</dd>
    <dt class="text-[var(--color-muted)]">created</dt>
    <dd>{new Date(m.created_at).toLocaleString()}</dd>
    <dt class="text-[var(--color-muted)]">id</dt>
    <dd class="font-mono text-[10px] break-all">{m.id}</dd>
  </dl>
  <pre
    class="rounded bg-[var(--color-bg)] p-3 text-xs leading-relaxed whitespace-pre-wrap">{JSON.stringify(
      m.payload,
      null,
      2,
    )}</pre>
{/if}
