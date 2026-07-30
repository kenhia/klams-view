<script lang="ts">
  let status = $state<{ view: string; klams: string } | null>(null);
  let error = $state<string | null>(null);

  $effect(() => {
    fetch("/api/status")
      .then((r) => r.json())
      .then((j) => (status = j))
      .catch((e) => (error = String(e)));
  });
</script>

<h1 class="text-xl font-semibold">Pulse</h1>
<p class="mt-2 text-[var(--color-muted)]">Scaffold shell — the dashboard lands here.</p>
{#if status}
  <p class="mt-4 text-sm">
    view: <span class="text-[var(--color-data)]">{status.view}</span> · klams:
    <span class="text-[var(--color-data)]">{status.klams}</span>
  </p>
{:else if error}
  <p class="mt-4 text-sm text-red-400">{error}</p>
{/if}
