<script lang="ts">
  // Time-bucketed stacked columns by memory kind. Mark spec: columns
  // <= 24px, 4px rounded data-end (top of stack only), 2px surface
  // gaps between segments, hairline solid gridlines, legend always
  // (3 series), per-mark hover with a full-band hit target and an
  // all-series tooltip. A table twin is one toggle away.
  import type { ActivityBucket, MemoryKind } from "$lib/types";
  import { KINDS, KIND_COLOR, KIND_LABEL } from "$lib/kinds";
  import { compact, shortDate } from "$lib/format";

  let {
    buckets,
    bucketHours,
    kinds = KINDS,
    height = 240,
  }: {
    buckets: ActivityBucket[];
    bucketHours: number;
    kinds?: MemoryKind[];
    height?: number;
  } = $props();

  let container: HTMLDivElement | undefined = $state();
  let width = $state(640);
  let showTable = $state(false);
  let hover: number | null = $state(null);

  $effect(() => {
    if (!container) return;
    const ro = new ResizeObserver(() => (width = container!.clientWidth));
    ro.observe(container);
    return () => ro.disconnect();
  });

  const margin = { top: 12, right: 8, bottom: 26, left: 44 };
  const plotW = $derived(Math.max(80, width - margin.left - margin.right));
  const plotH = $derived(height - margin.top - margin.bottom);

  const maxTotal = $derived(
    Math.max(1, ...buckets.map((b) => kinds.reduce((s, k) => s + b[k], 0))),
  );
  // Clean ceiling: 1/2/5 × 10^k
  const yMax = $derived.by(() => {
    const pow = Math.pow(10, Math.floor(Math.log10(maxTotal)));
    for (const m of [1, 2, 5, 10]) {
      if (m * pow >= maxTotal) return m * pow;
    }
    return 10 * pow;
  });
  const ticks = $derived([0.25, 0.5, 0.75, 1].map((f) => f * yMax));

  const band = $derived(plotW / Math.max(1, buckets.length));
  const colW = $derived(Math.max(2, Math.min(24, band - 2)));

  function segs(b: ActivityBucket) {
    // Bottom-up stack with 2px surface gaps; the top visible segment
    // gets the rounded data-end.
    const out: { kind: MemoryKind; y: number; h: number; top: boolean }[] = [];
    let yCursor = plotH;
    const visible = kinds.filter((k) => b[k] > 0);
    for (const kind of [...KINDS].reverse()) {
      if (!visible.includes(kind)) continue;
      const h = (b[kind] / yMax) * plotH;
      const gap = out.length > 0 ? 2 : 0;
      yCursor -= h + gap;
      out.push({ kind, y: yCursor, h, top: false });
    }
    if (out.length > 0) out[out.length - 1].top = true;
    return out;
  }

  function topPath(x: number, y: number, w: number, h: number): string {
    const r = Math.min(4, w / 2, h);
    return `M${x},${y + h} L${x},${y + r} Q${x},${y} ${x + r},${y} L${x + w - r},${y} Q${x + w},${y} ${x + w},${y + r} L${x + w},${y + h} Z`;
  }

  // ~6 x labels max
  const labelEvery = $derived(Math.max(1, Math.ceil(buckets.length / 6)));

  const hovered = $derived(hover === null ? null : buckets[hover]);
  const tooltipLeft = $derived(
    hover === null ? 0 : Math.min(margin.left + hover * band + band / 2, width - 170),
  );
</script>

<div bind:this={container} class="relative">
  <div class="mb-2 flex items-center justify-between">
    <div class="flex items-center gap-4 text-xs text-[var(--color-muted)]">
      {#each kinds as k (k)}
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
    <div class="max-h-64 overflow-y-auto rounded border border-[var(--color-border)]">
      <table class="w-full text-xs tabular-nums">
        <thead class="sticky top-0 bg-[var(--color-surface-hi)] text-left">
          <tr>
            <th class="px-2 py-1 font-medium">Bucket</th>
            {#each kinds as k (k)}<th class="px-2 py-1 text-right font-medium">{KIND_LABEL[k]}</th
              >{/each}
          </tr>
        </thead>
        <tbody>
          {#each buckets as b (b.t)}
            <tr class="border-t border-[var(--color-border)]">
              <td class="px-2 py-1">{shortDate(b.t, bucketHours)}</td>
              {#each kinds as k (k)}<td class="px-2 py-1 text-right">{b[k].toLocaleString()}</td
                >{/each}
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {:else}
    <svg {width} {height} role="img" aria-label="Memory writes over time by kind">
      <g transform="translate({margin.left},{margin.top})">
        {#each ticks as t (t)}
          <line
            x1="0"
            x2={plotW}
            y1={plotH - (t / yMax) * plotH}
            y2={plotH - (t / yMax) * plotH}
            stroke="var(--viz-grid)"
            stroke-width="1"
          />
          <text
            x="-8"
            y={plotH - (t / yMax) * plotH + 3}
            text-anchor="end"
            class="fill-[var(--viz-muted)] text-[10px] tabular-nums">{compact(t)}</text
          >
        {/each}
        <line x1="0" x2={plotW} y1={plotH} y2={plotH} stroke="var(--viz-axis)" stroke-width="1" />

        {#each buckets as b, i (b.t)}
          {@const x = i * band + (band - colW) / 2}
          <g opacity={hover === null || hover === i ? 1 : 0.55}>
            {#each segs(b) as s (s.kind)}
              {#if s.top}
                <path d={topPath(x, s.y, colW, s.h)} fill={KIND_COLOR[s.kind]} />
              {:else}
                <rect {x} y={s.y} width={colW} height={s.h} fill={KIND_COLOR[s.kind]} />
              {/if}
            {/each}
          </g>
          {#if i % labelEvery === 0}
            <text
              x={i * band + band / 2}
              y={plotH + 16}
              text-anchor="middle"
              class="fill-[var(--viz-muted)] text-[10px]">{shortDate(b.t, bucketHours)}</text
            >
          {/if}
          <!-- full-band hit target, bigger than the marks -->
          <rect
            x={i * band}
            y="0"
            width={band}
            height={plotH}
            fill="transparent"
            role="presentation"
            onpointerenter={() => (hover = i)}
            onpointerleave={() => (hover = null)}
          />
        {/each}
      </g>
    </svg>

    {#if hovered}
      <div
        class="pointer-events-none absolute top-2 z-10 w-40 rounded border border-[var(--color-border)] bg-[var(--color-surface-hi)] px-3 py-2 text-xs shadow-lg"
        style="left:{tooltipLeft}px"
      >
        <div class="mb-1 text-[var(--color-muted)]">{shortDate(hovered.t, bucketHours)}</div>
        {#each kinds as k (k)}
          <div class="flex items-center justify-between gap-3">
            <span class="inline-flex items-center gap-1.5 text-[var(--color-muted)]">
              <span class="inline-block h-0.5 w-3" style="background:{KIND_COLOR[k]}"></span>
              {KIND_LABEL[k]}
            </span>
            <span class="font-semibold tabular-nums">{hovered[k].toLocaleString()}</span>
          </div>
        {/each}
      </div>
    {/if}
  {/if}
</div>
