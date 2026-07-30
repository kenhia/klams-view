/** 1284 -> "1,284"; 12900 -> "12.9K"; 4200000 -> "4.2M". */
export function compact(n: number): string {
  if (!Number.isFinite(n)) return "—";
  const abs = Math.abs(n);
  if (abs >= 1_000_000) return `${trim1(n / 1_000_000)}M`;
  if (abs >= 10_000) return `${trim1(n / 1_000)}K`;
  return n.toLocaleString("en-US");
}

function trim1(x: number): string {
  const s = x.toFixed(1);
  return s.endsWith(".0") ? s.slice(0, -2) : s;
}

export function relTime(iso: string | number | null | undefined): string {
  if (iso === null || iso === undefined) return "never";
  const t = typeof iso === "number" ? iso * 1000 : Date.parse(iso);
  if (Number.isNaN(t)) return "—";
  const s = Math.round((Date.now() - t) / 1000);
  if (s < 0) return "now";
  if (s < 60) return `${s}s ago`;
  if (s < 3600) return `${Math.floor(s / 60)}m ago`;
  if (s < 86400) return `${Math.floor(s / 3600)}h ago`;
  if (s < 86400 * 30) return `${Math.floor(s / 86400)}d ago`;
  return new Date(t).toISOString().slice(0, 10);
}

export function shortDate(unixSecs: number, bucketHours: number): string {
  const d = new Date(unixSecs * 1000);
  if (bucketHours < 24) {
    return d.toLocaleString("en-US", { month: "short", day: "numeric", hour: "numeric" });
  }
  return d.toLocaleDateString("en-US", { month: "short", day: "numeric" });
}

export function uptime(seconds: number): string {
  if (seconds < 3600) return `${Math.floor(seconds / 60)}m`;
  if (seconds < 86400)
    return `${Math.floor(seconds / 3600)}h ${Math.floor((seconds % 3600) / 60)}m`;
  return `${Math.floor(seconds / 86400)}d ${Math.floor((seconds % 86400) / 3600)}h`;
}

export function ms(seconds: number | null | undefined): string {
  if (seconds === null || seconds === undefined) return "—";
  return seconds >= 1 ? `${seconds.toFixed(2)}s` : `${Math.round(seconds * 1000)}ms`;
}
