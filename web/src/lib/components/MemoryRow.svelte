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

<!--
  #807: the author name is a link to its author page, everywhere a memory
  renders one. An <a> cannot live inside a <button>, and this has to be a
  real link (middle-click, copy-link, keyboard) — so the whole-row click
  target is a sibling layer underneath the content rather than a wrapper
  around it.

  `author.id` is Option<Uuid> upstream (absent only when the author could
  not be resolved, which is also when agent_name reads "unknown"), and
  Explore synthesises rows with no author at all — so the plain-text
  fallback is a real case, not defensive padding.
-->
<div class="relative rounded hover:bg-[var(--color-surface-hi)]">
  {#if onopen}
    <button
      class="absolute inset-0 h-full w-full cursor-pointer"
      onclick={() => onopen?.(m)}
      aria-label="Open {m.kind} detail"
    ></button>
  {/if}
  <div class="pointer-events-none relative flex items-baseline gap-2 px-2 py-1.5">
    <span
      class="mt-0.5 inline-block h-2.5 w-2.5 shrink-0 self-center rounded-sm"
      style="background:{KIND_COLOR[m.kind]}"
      title={m.kind}
    ></span>
    {#if m.author.id}
      <a
        href="/authors/{m.author.id}"
        class="pointer-events-auto shrink-0 text-xs font-medium hover:text-[var(--color-accent)] hover:underline"
        title="Author profile: {m.author.agent_name}"
      >
        {m.author.agent_name}
      </a>
    {:else}
      <span class="shrink-0 text-xs font-medium">{m.author.agent_name}</span>
    {/if}
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
</div>
