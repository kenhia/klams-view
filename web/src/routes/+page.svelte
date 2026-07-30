<script lang="ts">
  // Pulse — the dashboard. One /api/overview call renders the top;
  // the activity chart re-fetches per selected range. Auto-refreshes
  // every 60s, holding the previous render (no skeleton flash).
  import { api, sinceHoursAgo } from "$lib/api";
  import type { Activity, MemoryRow, Overview, SubsystemHealth } from "$lib/types";
  import StatTile from "$lib/components/StatTile.svelte";
  import StackedColumns from "$lib/components/StackedColumns.svelte";
  import AgentBars from "$lib/components/AgentBars.svelte";
  import HealthBadge from "$lib/components/HealthBadge.svelte";
  import MemoryRowView from "$lib/components/MemoryRow.svelte";
  import MemoryDetail from "$lib/components/MemoryDetail.svelte";
  import Drawer from "$lib/components/Drawer.svelte";
  import TimeRange from "$lib/components/TimeRange.svelte";
  import ScannerToggle from "$lib/components/ScannerToggle.svelte";
  import { relTime, uptime } from "$lib/format";
  import { scannerNames } from "$lib/agents";

  let overview = $state<Overview | null>(null);
  let activity = $state<Activity | null>(null);
  let sparks = $state<{ searches: number[]; queue: number[] }>({ searches: [], queue: [] });
  let rangeHours = $state(24);
  // Off by default: both panels below are fed by one /api/activity
  // call, and with scanners in it they are both a single bar.
  let includeScanners = $state(false);
  let error = $state<string | null>(null);
  let loading = $state(false);
  let selected = $state<MemoryRow | null>(null);

  async function refresh() {
    loading = true;
    try {
      const since = sinceHoursAgo(rangeHours);
      const [o, a, h] = await Promise.all([
        api.overview(),
        api.activity({ since, include_scanners: includeScanners }),
        api.metricsHistory().catch(() => ({ samples: [] })),
      ]);
      overview = o;
      activity = a;
      const tail = h.samples.slice(-24);
      sparks = {
        searches: tail.map((s) => s.mcp_searches),
        queue: tail.map((s) => s.queue_depth),
      };
      error = null;
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    void rangeHours;
    void includeScanners;
    void refresh();
  });
  $effect(() => {
    const t = setInterval(refresh, 60_000);
    return () => clearInterval(t);
  });

  const subsystems = $derived.by(() => {
    if (!overview) return [];
    const known = ["postgres", "qdrant", "embeddings", "reranker"];
    return known
      .map((k) => [k, overview!.health[k] as SubsystemHealth | undefined] as const)
      .filter(([, v]) => v && typeof v === "object" && "state" in v)
      .map(([k, v]) => ({ name: k, state: (v as SubsystemHealth).state }));
  });

  const backupAge = $derived(overview?.metrics?.backup?.last_success_unix ?? null);
  // Named for the toggle's tooltip; the exclusion itself is applied
  // server-side, so this is presentation only.
  const scanners = $derived(scannerNames((overview?.agents ?? []).map((a) => a.agent_name)));
</script>

<svelte:head><title>klams-view · Pulse</title></svelte:head>

{#if error}
  <div
    class="mb-4 rounded border border-[var(--status-critical)] bg-[var(--color-surface)] px-3 py-2 text-sm"
  >
    <span style="color:var(--status-critical)">✕</span>
    {error}
  </div>
{/if}

{#if overview}
  <div class="transition-opacity" class:opacity-60={loading}>
    <!-- health strip -->
    <div class="flex flex-wrap items-center gap-2">
      <HealthBadge state={overview.health.status} label="klams" />
      {#each subsystems as s (s.name)}
        <HealthBadge state={s.state} label={s.name} />
      {/each}
      <span class="ml-auto text-xs text-[var(--color-muted)]">
        v{overview.health.version} · up {uptime(overview.health.uptime_seconds)} · queue
        {overview.health.queue.depth}/{overview.health.queue.capacity} · backup
        {backupAge ? relTime(backupAge) : "—"}
      </span>
    </div>

    {#if !overview.configured}
      <div
        class="mt-4 rounded border border-[var(--status-serious)] bg-[var(--color-surface)] px-3 py-2 text-sm"
      >
        <span style="color:var(--status-serious)">▲</span>
        KLAMS_TOKEN is not configured — showing public health and metrics only.
      </div>
    {/if}

    <!-- stat tiles -->
    {#if overview.totals}
      <div class="mt-4 grid grid-cols-2 gap-3 md:grid-cols-3 xl:grid-cols-6">
        <StatTile label="Knowledge" value={overview.totals.knowledge} />
        <StatTile label="Facts" value={overview.totals.facts} />
        <StatTile label="Events" value={overview.totals.events} />
        <StatTile label="Authors" value={overview.totals.authors} />
        <StatTile
          label="MCP searches"
          value={Object.values(overview.metrics?.mcp_agents ?? {}).reduce(
            (s, a) => s + a.searches,
            0,
          )}
          sub="since service start"
          spark={sparks.searches}
        />
        <StatTile
          label="Queue depth"
          value={overview.health.queue.depth}
          sub="{overview.health.queue.workers} workers"
          spark={sparks.queue}
        />
      </div>
    {/if}

    <!-- activity -->
    <section
      class="mt-6 rounded-lg border border-[var(--color-border)] bg-[var(--color-surface)] p-4"
    >
      <div class="mb-3 flex flex-wrap items-center justify-between gap-2">
        <h2 class="text-sm font-semibold">
          Writes over time
          {#if !includeScanners}<span class="font-normal text-[var(--color-muted)]"
              >(scanners hidden)</span
            >{/if}
        </h2>
        <div class="flex flex-wrap items-center gap-4">
          <ScannerToggle bind:checked={includeScanners} names={scanners} />
          <TimeRange value={rangeHours} onchange={(h) => (rangeHours = h)} />
        </div>
      </div>
      {#if activity}
        {#if activity.truncated}
          <p class="mb-2 text-xs" style="color:var(--status-serious)">
            ▲ large window: counts before {activity.covered_since
              ? relTime(activity.covered_since)
              : "the cap"} are not included
          </p>
        {/if}
        <StackedColumns buckets={activity.buckets} bucketHours={activity.bucket_hours} />
      {/if}
    </section>

    <div class="mt-6 grid gap-6 lg:grid-cols-2">
      <section class="rounded-lg border border-[var(--color-border)] bg-[var(--color-surface)] p-4">
        <h2 class="mb-3 text-sm font-semibold">
          Agents in window <span class="font-normal text-[var(--color-muted)]"
            >(√ scale, exact values labeled)</span
          >
        </h2>
        {#if activity}
          <AgentBars rows={activity.by_author} log={true} />
        {/if}
      </section>

      <section class="rounded-lg border border-[var(--color-border)] bg-[var(--color-surface)] p-4">
        <h2 class="mb-3 text-sm font-semibold">Latest writes</h2>
        {#if overview.recent}
          <div class="-mx-2">
            {#each overview.recent as m (m.id)}
              <MemoryRowView {m} onopen={(row) => (selected = row)} />
            {/each}
          </div>
        {:else}
          <p class="text-xs text-[var(--color-muted)]">needs a configured token</p>
        {/if}
      </section>
    </div>
  </div>
{:else if !error}
  <p class="text-sm text-[var(--color-muted)]">loading…</p>
{/if}

<Drawer
  open={selected !== null}
  onclose={() => (selected = null)}
  title={selected ? `${selected.kind} · ${selected.author.agent_name}` : ""}
>
  {#if selected}
    <MemoryDetail m={selected} />
  {/if}
</Drawer>
