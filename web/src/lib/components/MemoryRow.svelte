<script lang="ts">
  import type { MemoryRow } from "$lib/types";
  import { KIND_COLOR } from "$lib/kinds";
  import { relTime } from "$lib/format";

  let { m, onopen }: { m: MemoryRow; onopen?: (m: MemoryRow) => void } = $props();

  function summary(m: MemoryRow): string {
    if (m.kind === "knowledge") {
      const head = m.heading_path ? `${m.heading_path} — ` : "";
      return head + (m.text ?? "").slice(0, 160);
    }
    if (m.kind === "event")
      return `${m.category ?? "event"}: ${JSON.stringify(m.payload).slice(0, 140)}`;
    return `${m.type ?? "fact"}: ${JSON.stringify(m.payload).slice(0, 140)}`;
  }
</script>

<button
  class="block w-full rounded px-2 py-1.5 text-left hover:bg-[var(--color-surface-hi)] disabled:cursor-default"
  onclick={() => onopen?.(m)}
  disabled={!onopen}
>
  <div class="flex items-baseline gap-2">
    <span
      class="mt-0.5 inline-block h-2.5 w-2.5 shrink-0 self-center rounded-sm"
      style="background:{KIND_COLOR[m.kind]}"
      title={m.kind}
    ></span>
    <span class="shrink-0 text-xs font-medium">{m.author.agent_name}</span>
    {#if m.state === "deleted"}
      <span
        class="shrink-0 rounded bg-[var(--color-surface-hi)] px-1 text-[10px] text-[var(--status-serious)]"
        >deleted</span
      >
    {/if}
    <span class="truncate text-xs text-[var(--color-muted)]">{summary(m)}</span>
    <span class="ml-auto shrink-0 text-[10px] text-[var(--viz-muted)]">{relTime(m.created_at)}</span
    >
  </div>
</button>
