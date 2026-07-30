<script lang="ts">
  import type { Snippet } from "svelte";

  let {
    open,
    onclose,
    title,
    children,
  }: { open: boolean; onclose: () => void; title: string; children: Snippet } = $props();

  function onkeydown(e: KeyboardEvent) {
    if (e.key === "Escape") onclose();
  }
</script>

<svelte:window {onkeydown} />

{#if open}
  <div class="fixed inset-0 z-50 flex justify-end">
    <button class="absolute inset-0 bg-black/40" aria-label="Close" onclick={onclose}></button>
    <aside
      class="relative z-10 flex h-full w-full max-w-xl flex-col border-l border-[var(--color-border)] bg-[var(--color-surface)] shadow-2xl"
    >
      <header
        class="flex items-center justify-between border-b border-[var(--color-border)] px-4 py-3"
      >
        <h2 class="truncate text-sm font-semibold">{title}</h2>
        <button
          class="rounded px-2 py-1 text-sm text-[var(--color-muted)] hover:bg-[var(--color-surface-hi)]"
          onclick={onclose}>✕</button
        >
      </header>
      <div class="min-h-0 flex-1 overflow-y-auto p-4">
        {@render children()}
      </div>
    </aside>
  </div>
{/if}
