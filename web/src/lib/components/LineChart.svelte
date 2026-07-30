<script lang="ts">
  // Single-series time line: 2px round-joined line, 10% area wash,
  // hairline grid, crosshair snapping to the nearest sample with a
  // tooltip. One series — the card title names it, so no legend box.
  import { compact } from "$lib/format";

  let {
    points,
    height = 160,
    color = "var(--series-1)",
    format = compact,
  }: {
    points: { t: number; v: number }[];
    height?: number;
    color?: string;
    format?: (v: number) => string;
  } = $props();

  let container: HTMLDivElement | undefined = $state();
  let width = $state(600);
  let hover: number | null = $state(null);

  $effect(() => {
    if (!container) return;
    const ro = new ResizeObserver(() => (width = container!.clientWidth));
    ro.observe(container);
    return () => ro.disconnect();
  });

  const margin = { top: 10, right: 8, bottom: 22, left: 44 };
  const plotW = $derived(Math.max(60, width - margin.left - margin.right));
  const plotH = $derived(height - margin.top - margin.bottom);

  const tMin = $derived(points.length ? points[0].t : 0);
  const tMax = $derived(points.length ? points[points.length - 1].t : 1);
  const vMax = $derived(Math.max(1, ...points.map((p) => p.v)));

  const x = (t: number) => ((t - tMin) / Math.max(1, tMax - tMin)) * plotW;
  const y = (v: number) => plotH - (v / vMax) * plotH;

  const path = $derived(
    points
      .map((p, i) => `${i === 0 ? "M" : "L"}${x(p.t).toFixed(1)},${y(p.v).toFixed(1)}`)
      .join(" "),
  );
  const area = $derived(
    points.length ? `${path} L${x(tMax).toFixed(1)},${plotH} L0,${plotH} Z` : "",
  );
  const ticks = $derived([0.5, 1].map((f) => f * vMax));

  function onmove(e: PointerEvent) {
    if (!points.length) return;
    const rect = (e.currentTarget as SVGElement).getBoundingClientRect();
    const px = e.clientX - rect.left - margin.left;
    const t = tMin + (px / plotW) * (tMax - tMin);
    let best = 0;
    let bestD = Infinity;
    points.forEach((p, i) => {
      const d = Math.abs(p.t - t);
      if (d < bestD) {
        bestD = d;
        best = i;
      }
    });
    hover = best;
  }

  const hp = $derived(hover === null ? null : points[hover]);

  function timeLabel(t: number): string {
    return new Date(t * 1000).toLocaleTimeString("en-US", { hour: "numeric", minute: "2-digit" });
  }
</script>

<div bind:this={container} class="relative">
  {#if points.length < 2}
    <div
      class="flex items-center justify-center text-xs text-[var(--color-muted)]"
      style="height:{height}px"
    >
      collecting samples…
    </div>
  {:else}
    <svg {width} {height} role="img" onpointermove={onmove} onpointerleave={() => (hover = null)}>
      <g transform="translate({margin.left},{margin.top})">
        {#each ticks as t (t)}
          <line x1="0" x2={plotW} y1={y(t)} y2={y(t)} stroke="var(--viz-grid)" stroke-width="1" />
          <text
            x="-8"
            y={y(t) + 3}
            text-anchor="end"
            class="fill-[var(--viz-muted)] text-[10px] tabular-nums">{format(t)}</text
          >
        {/each}
        <line x1="0" x2={plotW} y1={plotH} y2={plotH} stroke="var(--viz-axis)" stroke-width="1" />
        <text x="0" y={plotH + 14} class="fill-[var(--viz-muted)] text-[10px]"
          >{timeLabel(tMin)}</text
        >
        <text x={plotW} y={plotH + 14} text-anchor="end" class="fill-[var(--viz-muted)] text-[10px]"
          >{timeLabel(tMax)}</text
        >

        <path d={area} fill={color} opacity="0.1" />
        <path
          d={path}
          fill="none"
          stroke={color}
          stroke-width="2"
          stroke-linejoin="round"
          stroke-linecap="round"
        />

        {#if hp}
          <line
            x1={x(hp.t)}
            x2={x(hp.t)}
            y1="0"
            y2={plotH}
            stroke="var(--viz-axis)"
            stroke-width="1"
          />
          <circle
            cx={x(hp.t)}
            cy={y(hp.v)}
            r="4"
            fill={color}
            stroke="var(--color-surface)"
            stroke-width="2"
          />
        {/if}
      </g>
    </svg>
    {#if hp}
      <div
        class="pointer-events-none absolute top-1 z-10 rounded border border-[var(--color-border)] bg-[var(--color-surface-hi)] px-2.5 py-1.5 text-xs shadow-lg"
        style="left:{Math.min(margin.left + x(hp.t) + 8, width - 110)}px"
      >
        <span class="font-semibold tabular-nums">{format(hp.v)}</span>
        <span class="ml-1.5 text-[var(--color-muted)]">{timeLabel(hp.t)}</span>
      </div>
    {/if}
  {/if}
</div>
