<script lang="ts">
  // Authors — every agent identity that has touched the store, with
  // its contribution mix. restores_received is hardcoded 0 upstream
  // and deliberately not shown.
  import { api } from "$lib/api";
  import type { Author } from "$lib/types";
  import AgentBars from "$lib/components/AgentBars.svelte";
  import { relTime } from "$lib/format";

  let authors = $state<Author[]>([]);
  let error = $state<string | null>(null);

  $effect(() => {
    api
      .authors({ limit: 200 })
      .then((p) => (authors = p.authors))
      .catch((e) => (error = String(e)));
  });

  const rows = $derived(
    authors.map((a) => ({
      agent_name: a.agent_name,
      fact: a.counts.writes,
      knowledge: a.counts.knowledge,
      event: a.counts.events,
    })),
  );
  const sorted = $derived(
    [...authors].sort((a, b) => Date.parse(b.last_seen_at) - Date.parse(a.last_seen_at)),
  );
</script>

<svelte:head><title>klams-view · Authors</title></svelte:head>

<h1 class="text-lg font-semibold">Authors</h1>

{#if error}
  <p class="mt-4 text-sm" style="color:var(--status-critical)">✕ {error}</p>
{/if}

<section class="mt-4 rounded-lg border border-[var(--color-border)] bg-[var(--color-surface)] p-4">
  <h2 class="mb-3 text-sm font-semibold">
    Corpus share <span class="font-normal text-[var(--color-muted)]"
      >(√ scale, exact values labeled)</span
    >
  </h2>
  <AgentBars {rows} maxRows={10} log={true} />
</section>

<section
  class="mt-4 overflow-x-auto rounded-lg border border-[var(--color-border)] bg-[var(--color-surface)]"
>
  <table class="w-full text-sm">
    <thead class="text-left text-xs text-[var(--color-muted)]">
      <tr class="border-b border-[var(--color-border)]">
        <th class="px-3 py-2 font-medium">Agent</th>
        <th class="px-3 py-2 font-medium">Model</th>
        <th class="px-3 py-2 font-medium">Client</th>
        <th class="px-3 py-2 text-right font-medium">Facts</th>
        <th class="px-3 py-2 text-right font-medium">Knowledge</th>
        <th class="px-3 py-2 text-right font-medium">Events</th>
        <th class="px-3 py-2 text-right font-medium">Deletes</th>
        <th class="px-3 py-2 text-right font-medium">Last seen</th>
      </tr>
    </thead>
    <tbody class="tabular-nums">
      {#each sorted as a (a.id)}
        <tr
          class="border-b border-[var(--color-border)] last:border-0 hover:bg-[var(--color-surface-hi)]"
        >
          <td class="px-3 py-1.5">
            <a class="text-[var(--color-accent)] hover:underline" href="/authors/{a.id}"
              >{a.agent_name}</a
            >
          </td>
          <td class="px-3 py-1.5 text-xs text-[var(--color-muted)]">{a.model ?? "—"}</td>
          <td class="px-3 py-1.5 text-xs text-[var(--color-muted)]">
            {a.client_app
              ? `${a.client_app}${a.client_version ? ` ${a.client_version}` : ""}`
              : "—"}
          </td>
          <td class="px-3 py-1.5 text-right">{a.counts.writes.toLocaleString()}</td>
          <td class="px-3 py-1.5 text-right">{a.counts.knowledge.toLocaleString()}</td>
          <td class="px-3 py-1.5 text-right">{a.counts.events.toLocaleString()}</td>
          <td class="px-3 py-1.5 text-right">{a.counts.soft_deletes.toLocaleString()}</td>
          <td class="px-3 py-1.5 text-right text-xs text-[var(--color-muted)]"
            >{relTime(a.last_seen_at)}</td
          >
        </tr>
      {/each}
    </tbody>
  </table>
</section>
