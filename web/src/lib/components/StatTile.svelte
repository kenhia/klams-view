<script lang="ts">
  import { compact } from "$lib/format";
  import Sparkline from "./Sparkline.svelte";

  let {
    label,
    value,
    sub,
    spark,
  }: {
    label: string;
    value: number | string;
    sub?: string;
    spark?: number[];
  } = $props();
</script>

<div class="rounded-lg border border-[var(--color-border)] bg-[var(--color-surface)] px-4 py-3">
  <div class="text-xs text-[var(--color-muted)]">{label}</div>
  <div class="mt-1 flex items-end justify-between gap-3">
    <div class="text-2xl font-semibold">
      {typeof value === "number" ? compact(value) : value}
    </div>
    {#if spark && spark.length > 1}
      <Sparkline data={spark} />
    {/if}
  </div>
  {#if sub}
    <div class="mt-1 text-xs text-[var(--color-muted)]">{sub}</div>
  {/if}
</div>
