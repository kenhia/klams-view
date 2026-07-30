// Kind identity is fixed app-wide (docs/design.md): the three kinds
// are the all-pairs-safe series trio and always wear the same slots.
import type { MemoryKind } from "./types";

export const KINDS: MemoryKind[] = ["knowledge", "fact", "event"];

export const KIND_COLOR: Record<MemoryKind, string> = {
  knowledge: "var(--series-1)",
  fact: "var(--series-2)",
  event: "var(--series-3)",
};

export const KIND_LABEL: Record<MemoryKind, string> = {
  knowledge: "Knowledge",
  fact: "Facts",
  event: "Events",
};
