// Bulk ingest agents ("klams-scanner", "kai-scanner", …) hold ~1000x
// the corpus of every interactive agent combined, so every per-agent
// chart is a scanner chart unless they are left out. There is no
// upstream flag for this — the naming convention is the signal, and
// the server-side aggregation agrees (see `is_scanner` in src/api.rs).
export function isScanner(agentName: string): boolean {
  return agentName.endsWith("-scanner");
}

/** The scanners among `names`, deduped, for labeling the toggle. */
export function scannerNames(names: string[]): string[] {
  return [...new Set(names.filter(isScanner))].sort();
}
