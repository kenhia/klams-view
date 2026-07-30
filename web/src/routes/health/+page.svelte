<script lang="ts">
  // Health — the operator page: subsystem states, queue, latency,
  // write/search counters, backup freshness, and the sampler's
  // history lines (in-memory upstream of this page; resets with the
  // klams-view process).
  import { api } from "$lib/api";
  import type { Health, HistorySample, MetricsSummary, SubsystemHealth } from "$lib/types";
  import HealthBadge from "$lib/components/HealthBadge.svelte";
  import LineChart from "$lib/components/LineChart.svelte";
  import StatTile from "$lib/components/StatTile.svelte";
  import { compact, ms, relTime, uptime } from "$lib/format";

  let health = $state<Health | null>(null);
  let summary = $state<MetricsSummary | null>(null);
  let samples = $state<HistorySample[]>([]);
  let error = $state<string | null>(null);
  let loading = $state(false);

  async function refresh() {
    loading = true;
    try {
      const [h, s, hist] = await Promise.all([
        api.health(),
        api.metricsSummary(),
        api.metricsHistory(),
      ]);
      health = h;
      summary = s;
      samples = hist.samples;
      error = null;
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    void refresh();
    const t = setInterval(refresh, 30_000);
    return () => clearInterval(t);
  });

  const subsystems = $derived.by(() => {
    if (!health) return [];
    return Object.entries(health)
      .filter(([, v]) => v && typeof v === "object" && "state" in (v as object))
      .map(([k, v]) => ({ name: k, ...(v as SubsystemHealth) }));
  });

  const queuePoints = $derived(samples.map((s) => ({ t: s.t, v: s.queue_depth })));
  // Counters are cumulative; chart the per-minute delta.
  const writeRate = $derived.by(() => {
    const out: { t: number; v: number }[] = [];
    for (let i = 1; i < samples.length; i++) {
      const cur = samples[i];
      const prev = samples[i - 1];
      const d =
        cur.writes_fact +
        cur.writes_event +
        cur.writes_knowledge -
        (prev.writes_fact + prev.writes_event + prev.writes_knowledge);
      out.push({ t: cur.t, v: Math.max(0, d) });
    }
    return out;
  });
  const searchRate = $derived.by(() => {
    const out: { t: number; v: number }[] = [];
    for (let i = 1; i < samples.length; i++) {
      out.push({
        t: samples[i].t,
        v: Math.max(0, samples[i].mcp_searches - samples[i - 1].mcp_searches),
      });
    }
    return out;
  });

  const missTotal = $derived(
    Object.values(summary?.search_misses ?? {}).reduce((a, b) => a + b, 0),
  );
</script>

<svelte:head><title>klams-view · Health</title></svelte:head>

<h1 class="text-lg font-semibold">Health</h1>

{#if error}
  <p class="mt-4 text-sm" style="color:var(--status-critical)">✕ {error}</p>
{/if}

{#if health && summary}
  <div class="transition-opacity" class:opacity-60={loading}>
    <div class="mt-3 flex flex-wrap items-center gap-2">
      <HealthBadge state={health.status} label="klams" />
      {#each subsystems as s (s.name)}
        <HealthBadge state={s.state} label={s.name} />
      {/each}
      <span class="ml-auto text-xs text-[var(--color-muted)]">
        v{health.version} · up {uptime(health.uptime_seconds)}
        {#if health.maintenance?.active}· <span style="color:var(--status-serious)"
            >▲ maintenance window</span
          >{/if}
      </span>
    </div>

    <div class="mt-4 grid grid-cols-2 gap-3 md:grid-cols-3 xl:grid-cols-6">
      <StatTile
        label="Queue"
        value="{health.queue.depth}/{health.queue.capacity}"
        sub="{health.queue.workers} workers"
      />
      <StatTile
        label="Writes accepted"
        value={Object.values(summary.writes_accepted).reduce((a, b) => a + b, 0)}
        sub="failed: {compact(summary.writes_failed)}"
      />
      <StatTile
        label="Search p95"
        value={ms(summary.latency.search_p95)}
        sub="p50 {ms(summary.latency.search_p50)}"
      />
      <StatTile label="Context p95" value={ms(summary.latency.context_p95)} />
      <StatTile label="Search misses" value={missTotal} sub="zero-hit + low-score" />
      <StatTile
        label="Last backup"
        value={summary.backup.last_success_unix ? relTime(summary.backup.last_success_unix) : "—"}
        sub={summary.backup.dir_writable === 0 ? "▲ backup dir not writable" : "dir writable"}
      />
    </div>

    <div class="mt-6 grid gap-6 lg:grid-cols-3">
      <section class="rounded-lg border border-[var(--color-border)] bg-[var(--color-surface)] p-4">
        <h2 class="mb-2 text-sm font-semibold">Queue depth</h2>
        <LineChart points={queuePoints} color="var(--series-7)" />
      </section>
      <section class="rounded-lg border border-[var(--color-border)] bg-[var(--color-surface)] p-4">
        <h2 class="mb-2 text-sm font-semibold">Writes / min</h2>
        <LineChart points={writeRate} color="var(--series-1)" />
      </section>
      <section class="rounded-lg border border-[var(--color-border)] bg-[var(--color-surface)] p-4">
        <h2 class="mb-2 text-sm font-semibold">MCP searches / min</h2>
        <LineChart points={searchRate} color="var(--series-3)" />
      </section>
    </div>

    <section
      class="mt-6 overflow-x-auto rounded-lg border border-[var(--color-border)] bg-[var(--color-surface)]"
    >
      <h2 class="px-3 pt-3 text-sm font-semibold">
        MCP usage by agent <span class="font-normal text-[var(--color-muted)]"
          >(since service start)</span
        >
      </h2>
      <table class="mt-2 w-full text-sm">
        <thead class="text-left text-xs text-[var(--color-muted)]">
          <tr class="border-b border-[var(--color-border)]">
            <th class="px-3 py-2 font-medium">Agent</th>
            <th class="px-3 py-2 text-right font-medium">Searches</th>
            <th class="px-3 py-2 text-right font-medium">Fact writes</th>
            <th class="px-3 py-2 text-right font-medium">Knowledge writes</th>
            <th class="px-3 py-2 text-right font-medium">Event writes</th>
          </tr>
        </thead>
        <tbody class="tabular-nums">
          {#each Object.entries(summary.mcp_agents) as [agent, a] (agent)}
            <tr class="border-b border-[var(--color-border)] last:border-0">
              <td class="px-3 py-1.5">{agent}</td>
              <td class="px-3 py-1.5 text-right">{a.searches.toLocaleString()}</td>
              <td class="px-3 py-1.5 text-right">{(a.writes.fact ?? 0).toLocaleString()}</td>
              <td class="px-3 py-1.5 text-right">{(a.writes.knowledge ?? 0).toLocaleString()}</td>
              <td class="px-3 py-1.5 text-right">{(a.writes.event ?? 0).toLocaleString()}</td>
            </tr>
          {:else}
            <tr
              ><td class="px-3 py-2 text-xs text-[var(--color-muted)]" colspan="5"
                >no MCP traffic since service start</td
              ></tr
            >
          {/each}
        </tbody>
      </table>
    </section>
  </div>
{:else if !error}
  <p class="mt-4 text-sm text-[var(--color-muted)]">loading…</p>
{/if}
