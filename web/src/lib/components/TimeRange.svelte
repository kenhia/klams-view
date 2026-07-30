<script lang="ts">
  // Preset date-range rows, date-range-first per the dataviz filter
  // spec. Emits an hours-back span; 30d is the klams window cap.
  let { value = 24, onchange }: { value?: number; onchange: (hours: number) => void } = $props();

  const presets = [
    { hours: 24, label: "24h" },
    { hours: 72, label: "3d" },
    { hours: 24 * 7, label: "7d" },
    { hours: 24 * 30, label: "30d" },
  ];
</script>

<div class="inline-flex overflow-hidden rounded border border-[var(--color-border)]">
  {#each presets as p (p.hours)}
    <button
      class="px-3 py-1 text-xs transition-colors hover:bg-[var(--color-surface-hi)]"
      class:bg-[var(--color-accent-soft)]={value === p.hours}
      class:font-semibold={value === p.hours}
      onclick={() => onchange(p.hours)}
    >
      {p.label}
    </button>
  {/each}
</div>
