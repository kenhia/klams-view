<script lang="ts">
  // Activity — time-windowed browse of the unified memory stream.
  // One filter row scopes both the chart and the table; the table
  // pages with the (working) /v1/memories cursor; auto-refresh is
  // opt-in and holds the frame while refetching.
  import { api, sinceHoursAgo } from "$lib/api";
  import type { Activity, Author, MemoriesPage, MemoryKind, MemoryRow } from "$lib/types";
  import { KIND_COLOR, KIND_LABEL, KINDS } from "$lib/kinds";
  import StackedColumns from "$lib/components/StackedColumns.svelte";
  import TimeRange from "$lib/components/TimeRange.svelte";
  import MemoryRowView from "$lib/components/MemoryRow.svelte";
  import MemoryDetail from "$lib/components/MemoryDetail.svelte";
  import Drawer from "$lib/components/Drawer.svelte";
  import { relTime } from "$lib/format";

  let rangeHours = $state(24);
  let kinds = $state<Record<MemoryKind, boolean>>({ knowledge: true, fact: true, event: true });
  let authorId = $state("");
  let memState = $state("live");
  let live = $state(false);

  let authors = $state<Author[]>([]);
  let activity = $state<Activity | null>(null);
  let rows = $state<MemoryRow[]>([]);
  let cursor = $state<string | null>(null);
  let loading = $state(false);
  let error = $state<string | null>(null);
  let selected = $state<MemoryRow | null>(null);

  const kindsCsv = $derived(KINDS.filter((k) => kinds[k]).join(",") || "fact,knowledge,event");
  const visibleKinds = $derived(KINDS.filter((k) => kinds[k]));

  async function refresh() {
    loading = true;
    try {
      const since = sinceHoursAgo(rangeHours);
      const params = {
        since,
        kinds: kindsCsv,
        authors: authorId || undefined,
        state: memState,
      };
      const [a, m]: [Activity, MemoriesPage] = await Promise.all([
        api.activity(params),
        api.memories({ ...params, limit: 50 }),
      ]);
      activity = a;
      rows = m.memories;
      cursor = m.next_cursor ?? null;
      error = null;
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  async function more() {
    if (!cursor) return;
    const since = sinceHoursAgo(rangeHours);
    const m = await api.memories({
      since,
      kinds: kindsCsv,
      authors: authorId || undefined,
      state: memState,
      limit: 50,
      cursor,
    });
    rows = [...rows, ...m.memories];
    cursor = m.next_cursor ?? null;
  }

  $effect(() => {
    void rangeHours;
    void kindsCsv;
    void authorId;
    void memState;
    void refresh();
  });
  $effect(() => {
    if (!live) return;
    const t = setInterval(refresh, 30_000);
    return () => clearInterval(t);
  });
  $effect(() => {
    api
      .authors({ limit: 200 })
      .then((p) => (authors = p.authors))
      .catch(() => {});
  });
</script>

<svelte:head><title>klams-view · Activity</title></svelte:head>

<h1 class="text-lg font-semibold">Activity</h1>

<!-- one filter row scoping everything below -->
<div class="mt-3 flex flex-wrap items-center gap-x-4 gap-y-2 text-xs">
  <TimeRange value={rangeHours} onchange={(h) => (rangeHours = h)} />
  {#each KINDS as k (k)}
    <label class="inline-flex items-center gap-1.5">
      <input type="checkbox" bind:checked={kinds[k]} class="accent-[var(--color-accent)]" />
      <span class="inline-block h-2.5 w-2.5 rounded-sm" style="background:{KIND_COLOR[k]}"></span>
      {KIND_LABEL[k]}
    </label>
  {/each}
  <select
    bind:value={authorId}
    class="rounded border border-[var(--color-border)] bg-[var(--color-surface)] px-2 py-1"
  >
    <option value="">all authors</option>
    {#each authors as a (a.id)}
      <option value={a.id}>{a.agent_name}{a.model ? ` · ${a.model}` : ""}</option>
    {/each}
  </select>
  <select
    bind:value={memState}
    class="rounded border border-[var(--color-border)] bg-[var(--color-surface)] px-2 py-1"
  >
    <option value="live">live</option>
    <option value="deleted">deleted</option>
    <option value="all">all</option>
  </select>
  <label class="inline-flex items-center gap-1.5 text-[var(--color-muted)]">
    <input type="checkbox" bind:checked={live} class="accent-[var(--color-accent)]" />
    auto-refresh 30s
  </label>
</div>

{#if error}
  <p class="mt-4 text-sm" style="color:var(--status-critical)">✕ {error}</p>
{/if}

<div class="transition-opacity" class:opacity-60={loading}>
  {#if activity}
    <section
      class="mt-4 rounded-lg border border-[var(--color-border)] bg-[var(--color-surface)] p-4"
    >
      {#if activity.truncated}
        <p class="mb-2 text-xs" style="color:var(--status-serious)">
          ▲ large window: counts before {activity.covered_since
            ? relTime(activity.covered_since)
            : "the cap"} are not included
        </p>
      {/if}
      <StackedColumns
        buckets={activity.buckets}
        bucketHours={activity.bucket_hours}
        kinds={visibleKinds}
      />
      <p class="mt-2 text-xs text-[var(--color-muted)]">
        {activity.total.toLocaleString()} memories in window
      </p>
    </section>
  {/if}

  <section
    class="mt-4 rounded-lg border border-[var(--color-border)] bg-[var(--color-surface)] p-2"
  >
    {#each rows as m (m.id)}
      <MemoryRowView {m} onopen={(row) => (selected = row)} />
    {:else}
      <p class="px-2 py-3 text-xs text-[var(--color-muted)]">nothing in this window</p>
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
</div>

<Drawer
  open={selected !== null}
  onclose={() => (selected = null)}
  title={selected ? `${selected.kind} · ${selected.author.agent_name}` : ""}
>
  {#if selected}
    <MemoryDetail m={selected} />
  {/if}
</Drawer>
