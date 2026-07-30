<script lang="ts">
  // Shared control for the scanner-suppression default (see
  // $lib/agents). Off means the scanners are left out — the default
  // everywhere, because including them flattens every other agent to
  // an invisible sliver.
  let { checked = $bindable(false), names = [] }: { checked?: boolean; names?: string[] } =
    $props();

  const hint = $derived(
    names.length ? `Bulk ingest agents: ${names.join(", ")}` : "Bulk ingest agents (*-scanner)",
  );
</script>

<label class="inline-flex items-center gap-1.5 text-xs text-[var(--color-muted)]" title={hint}>
  <input type="checkbox" bind:checked class="accent-[var(--color-accent)]" />
  Include scanners
</label>
