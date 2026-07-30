<script lang="ts">
  import { page } from "$app/stores";
  import { api } from "$lib/api";
  import type { Author, MemoryKind, MemoryRow } from "$lib/types";
  import { KIND_COLOR, KIND_LABEL, KINDS } from "$lib/kinds";
  import MemoryRowView from "$lib/components/MemoryRow.svelte";
  import MemoryDetail from "$lib/components/MemoryDetail.svelte";
  import Drawer from "$lib/components/Drawer.svelte";
  import { relTime } from "$lib/format";

  let author = $state<Author | null>(null);
  let rows = $state<MemoryRow[]>([]);
  let cursor = $state<string | null>(null);
  let kinds = $state<Record<MemoryKind, boolean>>({ knowledge: true, fact: true, event: true });
  let error = $state<string | null>(null);
  let selected = $state<MemoryRow | null>(null);

  const id = $derived($page.params.id ?? "");
  const kindsCsv = $derived(KINDS.filter((k) => kinds[k]).join(","));

  $effect(() => {
    api
      .author(id)
      .then((a) => (author = a))
      .catch((e) => (error = String(e)));
  });

  $effect(() => {
    void kindsCsv;
    api
      .authorMemories(id, { limit: 50, kinds: kindsCsv || undefined })
      .then((m) => {
        rows = m.memories;
        cursor = m.next_cursor ?? null;
      })
      .catch((e) => (error = String(e)));
  });

  async function more() {
    if (!cursor) return;
    const m = await api.authorMemories(id, { limit: 50, cursor, kinds: kindsCsv || undefined });
    rows = [...rows, ...m.memories];
    cursor = m.next_cursor ?? null;
  }

  const profile = $derived.by(() => {
    if (!author) return [];
    return [
      ["model", author.model],
      ["session", author.session_title],
      ["repo", author.repo],
      ["client", author.client_app ? `${author.client_app} ${author.client_version ?? ""}` : null],
      ["first seen", relTime(author.created_at)],
      ["last seen", relTime(author.last_seen_at)],
    ].filter(([, v]) => v) as [string, string][];
  });
</script>

<svelte:head><title>klams-view · {author?.agent_name ?? "author"}</title></svelte:head>

<a href="/authors" class="text-xs text-[var(--color-muted)] hover:underline">← authors</a>

{#if error}
  <p class="mt-4 text-sm" style="color:var(--status-critical)">✕ {error}</p>
{:else if author}
  <div class="mt-2 flex flex-wrap items-baseline gap-x-6 gap-y-2">
    <h1 class="text-lg font-semibold">{author.agent_name}</h1>
    <dl class="flex flex-wrap gap-x-6 gap-y-1 text-xs text-[var(--color-muted)]">
      {#each profile as [k, v] (k)}
        <div>
          <dt class="inline">{k}:</dt>
          <dd class="inline text-[var(--color-text)]">{v}</dd>
        </div>
      {/each}
    </dl>
  </div>

  <div class="mt-3 flex flex-wrap gap-4 text-sm tabular-nums">
    <span
      ><span
        class="inline-block h-2.5 w-2.5 rounded-sm align-baseline"
        style="background:{KIND_COLOR.fact}"
      ></span>
      {author.counts.writes.toLocaleString()} facts</span
    >
    <span
      ><span
        class="inline-block h-2.5 w-2.5 rounded-sm align-baseline"
        style="background:{KIND_COLOR.knowledge}"
      ></span>
      {author.counts.knowledge.toLocaleString()} knowledge</span
    >
    <span
      ><span
        class="inline-block h-2.5 w-2.5 rounded-sm align-baseline"
        style="background:{KIND_COLOR.event}"
      ></span>
      {author.counts.events.toLocaleString()} events</span
    >
    <span class="text-[var(--color-muted)]"
      >{author.counts.soft_deletes.toLocaleString()} soft deletes</span
    >
  </div>

  <div class="mt-4 flex gap-4 text-xs">
    {#each KINDS as k (k)}
      <label class="inline-flex items-center gap-1.5">
        <input type="checkbox" bind:checked={kinds[k]} class="accent-[var(--color-accent)]" />
        <span class="inline-block h-2.5 w-2.5 rounded-sm" style="background:{KIND_COLOR[k]}"></span>
        {KIND_LABEL[k]}
      </label>
    {/each}
  </div>

  <section
    class="mt-3 rounded-lg border border-[var(--color-border)] bg-[var(--color-surface)] p-2"
  >
    {#each rows as m (m.id)}
      <MemoryRowView {m} onopen={(row) => (selected = row)} />
    {:else}
      <p class="px-2 py-3 text-xs text-[var(--color-muted)]">no memories</p>
    {/each}
    {#if cursor}
      <button
        class="m-2 rounded border border-[var(--color-border)] px-3 py-1 text-xs hover:bg-[var(--color-surface-hi)]"
        onclick={more}
      >
        load more
      </button>
    {/if}
  </section>
{/if}

<Drawer
  open={selected !== null}
  onclose={() => (selected = null)}
  title={selected ? `${selected.kind} · ${author?.agent_name ?? ""}` : ""}
>
  {#if selected}
    <MemoryDetail m={selected} />
  {/if}
</Drawer>
