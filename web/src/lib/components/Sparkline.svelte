<script lang="ts">
  // 12-point-ish trend beside a stat value: de-emphasis hue for the
  // line, accent dot (with surface ring) on the current point.
  let {
    data,
    width = 96,
    height = 28,
  }: { data: number[]; width?: number; height?: number } = $props();

  const pad = 3;
  const pts = $derived.by(() => {
    const min = Math.min(...data);
    const max = Math.max(...data);
    const span = max - min || 1;
    return data.map((v, i) => ({
      x: pad + (i * (width - 2 * pad)) / (data.length - 1),
      y: height - pad - ((v - min) / span) * (height - 2 * pad),
    }));
  });
  const path = $derived(pts.map((p, i) => `${i === 0 ? "M" : "L"}${p.x},${p.y}`).join(" "));
  const last = $derived(pts[pts.length - 1]);
</script>

<svg {width} {height} aria-hidden="true" class="shrink-0">
  <path
    d={path}
    fill="none"
    stroke="var(--viz-muted)"
    stroke-width="2"
    stroke-linejoin="round"
    stroke-linecap="round"
  />
  <circle
    cx={last.x}
    cy={last.y}
    r="4"
    fill="var(--color-accent)"
    stroke="var(--color-surface)"
    stroke-width="2"
  />
</svg>
