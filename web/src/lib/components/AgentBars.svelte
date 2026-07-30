<script lang="ts">
  // Horizontal stacked bars: per-agent contribution by kind. Top N
  // rows, tail folded into "Other" (never a 9th hue). Value at the
  // bar tip; per-row hover tooltip carries the kind split.
  import { KINDS, KIND_COLOR, KIND_LABEL } from "$lib/kinds";
  import type { MemoryKind } from "$lib/types";
  import { compact } from "$lib/format";

  type Row = { agent_name: string; fact: number; knowledge: number; event: number };

  let {
    rows,
    maxRows = 8,
    log = false,
  }: { rows: Row[]; maxRows?: number; log?: boolean } = $props();

  let hover: string | null = $state(null);
  let showTable = $state(false);

  const total = (r: Row) => r.fact + r.knowledge + r.event;

  const folded = $derived.by(() => {
    // Authors are per-identity and agent_name repeats across them —
    // merge by name first (also keeps the {#each} keys unique).
    const byName = new Map<string, Row>();
    for (const r of rows) {
      const prev = byName.get(r.agent_name);
      if (prev) {
        prev.fact += r.fact;
        prev.knowledge += r.knowledge;
        prev.event += r.event;
      } else {
        byName.set(r.agent_name, { ...r });
      }
    }
    const sorted = [...byName.values()]
      .filter((r) => total(r) > 0)
      .sort((a, b) => total(b) - total(a));
    if (sorted.length <= maxRows) return sorted;
    const head = sorted.slice(0, maxRows - 1);
    const tail = sorted.slice(maxRows - 1);
    head.push({
      agent_name: `Other (${tail.length})`,
      fact: tail.reduce((s, r) => s + r.fact, 0),
      knowledge: tail.reduce((s, r) => s + r.knowledge, 0),
      event: tail.reduce((s, r) => s + r.event, 0),
    });
    return head;
  });

  const max = $derived(Math.max(1, ...folded.map(total)));
  // Optional sqrt scale: scanner corpora dwarf interactive agents by
  // 1000x; sqrt keeps small contributors visible while preserving
  // order. Labeled values stay exact.
  const scale = (v: number) => (log ? Math.sqrt(v) / Math.sqrt(max) : v / max);

  function segWidths(r: Row): { kind: MemoryKind; w: number }[] {
    const t = total(r);
    if (t === 0) return [];
    const rowFrac = scale(t);
    return KINDS.filter((k) => r[k] > 0).map((k) => ({
      kind: k,
      w: rowFrac * (r[k] / t) * 100,
    }));
  }
</script>

<div>
  <div class="mb-2 flex items-center justify-between">
    <div class="flex items-center gap-4 text-xs text-[var(--color-muted)]">
      {#each KINDS as k (k)}
        <span class="inline-flex items-center gap-1.5">
          <span class="inline-block h-2.5 w-2.5 rounded-sm" style="background:{KIND_COLOR[k]}"
          ></span>
          {KIND_LABEL[k]}
        </span>
      {/each}
    </div>
    <button
      class="rounded px-2 py-0.5 text-xs text-[var(--color-muted)] hover:bg-[var(--color-surface-hi)]"
      onclick={() => (showTable = !showTable)}
    >
      {showTable ? "chart" : "table"}
    </button>
  </div>

  {#if showTable}
    <div class="max-h-72 overflow-y-auto rounded border border-[var(--color-border)]">
      <table class="w-full text-xs tabular-nums">
        <thead class="sticky top-0 bg-[var(--color-surface-hi)] text-left">
          <tr>
            <th class="px-2 py-1 font-medium">Agent</th>
            {#each KINDS as k (k)}<th class="px-2 py-1 text-right font-medium">{KIND_LABEL[k]}</th
              >{/each}
            <th class="px-2 py-1 text-right font-medium">Total</th>
          </tr>
        </thead>
        <tbody>
          {#each folded as r (r.agent_name)}
            <tr class="border-t border-[var(--color-border)]">
              <td class="px-2 py-1">{r.agent_name}</td>
              {#each KINDS as k (k)}<td class="px-2 py-1 text-right">{r[k].toLocaleString()}</td
                >{/each}
              <td class="px-2 py-1 text-right font-semibold">{total(r).toLocaleString()}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {:else}
    <div class="space-y-1.5">
      {#each folded as r (r.agent_name)}
        <div
          class="relative flex items-center gap-2"
          role="img"
          aria-label="{r.agent_name}: {total(r).toLocaleString()} memories"
          onpointerenter={() => (hover = r.agent_name)}
          onpointerleave={() => (hover = null)}
        >
          <div
            class="w-32 shrink-0 truncate text-right text-xs text-[var(--color-muted)]"
            title={r.agent_name}
          >
            {r.agent_name}
          </div>
          <div
            class="relative h-4 flex-1"
            style="opacity:{hover === null || hover === r.agent_name ? 1 : 0.55}"
          >
            <div class="flex h-full items-center gap-[2px]">
              {#each segWidths(r) as s (s.kind)}
                <div
                  class="h-full last:rounded-r"
                  style="width:{s.w}%;background:{KIND_COLOR[s.kind]}"
                ></div>
              {/each}
              <span class="pl-1.5 text-xs font-medium tabular-nums">{compact(total(r))}</span>
            </div>
          </div>
          {#if hover === r.agent_name}
            <div
              class="pointer-events-none absolute -top-1 right-0 z-10 w-40 -translate-y-full rounded border border-[var(--color-border)] bg-[var(--color-surface-hi)] px-3 py-2 text-xs shadow-lg"
            >
              <div class="mb-1 text-[var(--color-muted)]">{r.agent_name}</div>
              {#each KINDS as k (k)}
                <div class="flex items-center justify-between gap-3">
                  <span class="inline-flex items-center gap-1.5 text-[var(--color-muted)]">
                    <span class="inline-block h-0.5 w-3" style="background:{KIND_COLOR[k]}"></span>
                    {KIND_LABEL[k]}
                  </span>
                  <span class="font-semibold tabular-nums">{r[k].toLocaleString()}</span>
                </div>
              {/each}
            </div>
          {/if}
        </div>
      {/each}
    </div>
  {/if}
</div>
