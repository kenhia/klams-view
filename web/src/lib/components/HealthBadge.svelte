<script lang="ts">
  // Status is never color alone: icon + label always.
  let { state, label }: { state: string; label: string } = $props();

  const styles: Record<string, { color: string; icon: string }> = {
    Ok: { color: "var(--status-good)", icon: "✓" },
    Degraded: { color: "var(--status-serious)", icon: "▲" },
    Down: { color: "var(--status-critical)", icon: "✕" },
  };
  const s = $derived(styles[state] ?? { color: "var(--viz-muted)", icon: "?" });
</script>

<span
  class="inline-flex items-center gap-1.5 rounded-full border border-[var(--color-border)] bg-[var(--color-surface-hi)] px-2.5 py-0.5 text-xs"
>
  <span style="color:{s.color}" aria-hidden="true">{s.icon}</span>
  <span>{label}</span>
  <span class="text-[var(--color-muted)]">{state}</span>
</span>
