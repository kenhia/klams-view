<script lang="ts">
  // The connection doctor panel (#808). Renders /api/status one link at
  // a time, with the fix attached to the row that failed.
  //
  // Collapsed to a single line when every link passes — the operator
  // page should not spend a third of its height telling you nothing is
  // wrong — and self-expanding the moment one does not. That is the
  // whole point: the klams #739 afternoon was spent with a green
  // /healthz and no line anywhere saying "the token is being rejected".
  import type { DoctorReport } from "$lib/types";

  let { report }: { report: DoctorReport } = $props();

  // Status is never color alone: icon + word, same rule as HealthBadge.
  const STATE: Record<string, { color: string; icon: string; word: string }> = {
    ok: { color: "var(--status-good)", icon: "✓", word: "ok" },
    warn: { color: "var(--status-serious)", icon: "▲", word: "advisory" },
    fail: { color: "var(--status-critical)", icon: "✕", word: "failed" },
    skipped: { color: "var(--viz-muted)", icon: "–", word: "skipped" },
  };

  const OVERALL: Record<string, { color: string; icon: string; text: string }> = {
    ok: { color: "var(--status-good)", icon: "✓", text: "klams connection healthy" },
    advisory: { color: "var(--status-serious)", icon: "▲", text: "klams connection — advisory" },
    down: { color: "var(--status-critical)", icon: "✕", text: "klams connection broken" },
  };

  // Opening on anything other than "ok" is the behaviour, not a
  // preference — but the toggle still wins once a human touches it.
  let userOpen = $state<boolean | null>(null);
  const open = $derived(userOpen ?? report.overall !== "ok");

  const o = $derived(OVERALL[report.overall] ?? OVERALL.down);
  const failing = $derived(report.checks.filter((c) => c.state === "fail" || c.state === "warn"));
  const passing = $derived(report.checks.filter((c) => c.state === "ok").length);
  // A skipped link is not a link that failed to pass — TLS on an http://
  // URL is "does not apply", and folding it into a denominator reads as
  // one short of clean forever.
  const skipped = $derived(report.checks.filter((c) => c.state === "skipped").length);
  const elapsed = $derived(report.checks.reduce((s, c) => s + c.elapsed_ms, 0));
</script>

<section
  class="mt-4 rounded-lg border bg-[var(--color-surface)]"
  style="border-color:{report.overall === 'ok' ? 'var(--color-border)' : o.color}"
>
  <div class="flex flex-wrap items-baseline gap-x-3 gap-y-1 px-3 py-2">
    <span style="color:{o.color}" aria-hidden="true">{o.icon}</span>
    <h2 class="text-sm font-semibold">{o.text}</h2>
    <span class="text-xs text-[var(--color-muted)]">
      {passing} ok{#if skipped}
        · {skipped} n/a{/if}{#if failing.length}
        · {failing.map((c) => c.label.toLowerCase()).join(", ")}{/if}
      · {report.view.klams_url}
      {#if report.view.klams_version}· klams {report.view.klams_version}{/if}
      · {elapsed}ms
    </span>
    <button
      class="ml-auto rounded px-2 py-0.5 text-xs text-[var(--color-muted)] hover:bg-[var(--color-surface-hi)]"
      onclick={() => (userOpen = !open)}
      aria-expanded={open}
    >
      {open ? "hide chain" : "show chain"}
    </button>
  </div>

  {#if open}
    <ol class="border-t border-[var(--color-border)]">
      {#each report.checks as c (c.id)}
        {@const s = STATE[c.state] ?? STATE.skipped}
        <li
          class="flex flex-wrap items-baseline gap-x-2 gap-y-0.5 border-b border-[var(--color-border)] px-3 py-1.5 text-xs last:border-0"
          class:opacity-60={c.state === "skipped"}
        >
          <span style="color:{s.color}" aria-hidden="true">{s.icon}</span>
          <span class="w-52 shrink-0 font-medium">{c.label}</span>
          <span class="sr-only">{s.word}</span>
          <span class="min-w-40 flex-1 text-[var(--color-muted)]">{c.detail}</span>
          <span class="shrink-0 tabular-nums text-[var(--viz-muted)]">
            {c.elapsed_ms > 0 ? `${c.elapsed_ms}ms` : ""}
          </span>
          {#if c.fix}
            <p
              class="w-full pl-6 text-[var(--color-text)]"
              style="border-left:2px solid {s.color}; margin-left:0.25rem"
            >
              <span class="font-medium" style="color:{s.color}">fix:</span>
              {c.fix}
            </p>
          {/if}
        </li>
      {/each}
    </ol>
    <p class="px-3 py-2 text-[10px] text-[var(--color-muted)]">
      klams-view {report.view.version}, verified against klams {report.view.klams_verified}. The
      authenticated step is the one <code>/healthz</code> structurally cannot make — an unauthorized token
      leaves reachability green and every read failing.
    </p>
  {/if}
</section>
