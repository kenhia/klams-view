<script lang="ts">
  import "../app.css";
  import { page } from "$app/stores";

  let { children } = $props();

  const nav = [
    { href: "/", label: "Pulse" },
    { href: "/explore", label: "Explore" },
    { href: "/activity", label: "Activity" },
    { href: "/authors", label: "Authors" },
    { href: "/health", label: "Health" },
  ];

  function active(href: string, path: string): boolean {
    if (href === "/") return path === "/";
    return path === href || path.startsWith(href + "/");
  }
</script>

<div class="min-h-screen">
  <header class="sticky top-0 z-40 border-b border-[var(--color-border)] bg-[var(--color-surface)]">
    <nav class="mx-auto flex max-w-[90rem] flex-wrap items-center gap-1 px-4 py-2">
      <a href="/" class="mr-4 text-lg font-semibold tracking-tight text-[var(--color-accent)]">
        klams-view
      </a>
      {#each nav as item (item.href)}
        <a
          href={item.href}
          class="rounded px-3 py-1.5 text-sm transition-colors hover:bg-[var(--color-surface-hi)]"
          class:bg-[var(--color-surface-hi)]={active(item.href, $page.url.pathname)}
        >
          {item.label}
        </a>
      {/each}
    </nav>
  </header>
  <main class="mx-auto max-w-[90rem] px-4 py-6">
    {@render children()}
  </main>
</div>
