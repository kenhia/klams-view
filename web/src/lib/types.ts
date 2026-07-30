// Hand-mirrored contracts of the klams-view server /api layer (see
// docs/design.md). Kept deliberately small; serde on the server side
// omits optional fields rather than sending null.

export type MemoryKind = "fact" | "knowledge" | "event";

export interface AuthorRef {
  id?: string;
  agent_name: string;
  model?: string;
  repo?: string;
}

/** Flattened PublicMemory row from /v1/memories (+ state wrapper). */
export interface MemoryRow {
  id: string;
  kind: MemoryKind;
  tags: string[];
  author: AuthorRef;
  created_at: string;
  updated_at: string;
  state?: "live" | "deleted";
  deleted_at?: string;
  // fact
  type?: string;
  payload?: unknown;
  // knowledge
  text?: string;
  source_path?: string;
  repo?: string;
  host?: string;
  heading_path?: string;
  language?: string;
  supersedes?: string;
  superseded_by?: string;
  volatility?: string;
  // event
  category?: string;
  task_id?: string;
}

export interface MemoriesPage {
  memories: MemoryRow[];
  next_cursor?: string | null;
}

export interface SubsystemHealth {
  state: "Ok" | "Degraded" | "Down";
  message?: string;
}

export interface Health {
  status: "Ok" | "Degraded" | "Down";
  queue: { depth: number; capacity: number; workers: number };
  version: string;
  uptime_seconds: number;
  maintenance?: { active: boolean };
  // postgres / qdrant / embeddings / reranker / whatever future
  // subsystems klams grows — render generically.
  [key: string]: unknown;
}

export interface AgentSummary {
  id: string;
  agent_name: string;
  model?: string | null;
  last_seen_at: string;
  facts: number;
  knowledge: number;
  events: number;
}

export interface MetricsSummary {
  queue: { depth: number | null; capacity: number | null; workers: number | null };
  writes_accepted: Record<string, number>;
  writes_failed: number;
  search_misses: Record<string, number>;
  mcp_agents: Record<string, { writes: Record<string, number>; searches: number }>;
  latency: {
    search_p50: number | null;
    search_p95: number | null;
    context_p95: number | null;
    embedding_p95: number | null;
  };
  backup: {
    last_success_unix: number | null;
    dir_writable: number | null;
    maintenance_active: number | null;
  };
}

export interface Overview {
  health: Health;
  metrics: MetricsSummary | null;
  totals: { facts: number; knowledge: number; events: number; authors: number } | null;
  agents: AgentSummary[] | null;
  recent: MemoryRow[] | null;
  configured: boolean;
}

export interface ActivityBucket {
  t: number; // unix seconds, bucket start
  fact: number;
  knowledge: number;
  event: number;
}

export interface Activity {
  since: string;
  until: string;
  bucket_hours: number;
  buckets: ActivityBucket[];
  by_author: { agent_name: string; fact: number; knowledge: number; event: number }[];
  total: number;
  truncated: boolean;
  covered_since: string | null;
}

export interface SearchHit {
  type: MemoryKind;
  id: string;
  score: number;
  preview: string;
  payload: unknown;
}

export interface SearchResults {
  query: string;
  results: SearchHit[];
  total: number;
  degraded: boolean;
}

export interface KnowledgeItem {
  id: string;
  text: string;
  content_hash: string;
  source: string;
  tags: string[];
  repo: string | null;
  file: string | null;
  machine: string | null;
  machines?: string[];
  heading_path: string | null;
  language: string | null;
  chunk_index: number | null;
  volatility?: string;
  supersedes?: string;
  superseded_by?: string;
  confidence: number;
  decay_weight: number;
  use_count: number;
  last_used_at: string | null;
  created_at: string;
  updated_at: string;
}

export interface Author {
  id: string;
  agent_name: string;
  model?: string;
  session_title?: string;
  repo?: string;
  client_app?: string;
  client_version?: string;
  created_at: string;
  last_seen_at: string;
  counts: {
    writes: number;
    knowledge: number;
    events: number;
    soft_deletes: number;
    restores_received: number;
  };
}

export interface AuthorPage {
  authors: Author[];
  next_cursor: string | null;
}

export interface HistorySample {
  t: number;
  queue_depth: number;
  writes_fact: number;
  writes_event: number;
  writes_knowledge: number;
  mcp_searches: number;
  retrieval_p95?: number;
}
