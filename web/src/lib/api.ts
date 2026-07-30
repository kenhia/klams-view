import type {
  Activity,
  Author,
  AuthorPage,
  Health,
  HistorySample,
  KnowledgeItem,
  MemoriesPage,
  MetricsSummary,
  Overview,
  SearchResults,
} from "./types";

export class ApiError extends Error {
  code: string;
  status: number;
  constructor(status: number, code: string, message: string) {
    super(message);
    this.code = code;
    this.status = status;
  }
}

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const res = await fetch(path, init);
  const body = await res.json().catch(() => ({}));
  if (!res.ok) {
    // Two upstream error envelopes exist: ApiError {code,message} and
    // the maintenance one {error, retry_after_seconds}.
    const code = body.code ?? body.error ?? "error";
    throw new ApiError(res.status, code, body.message ?? body.error ?? res.statusText);
  }
  return body as T;
}

function qs(params: Record<string, string | number | undefined>): string {
  const p = new URLSearchParams();
  for (const [k, v] of Object.entries(params)) {
    if (v !== undefined && v !== "") p.set(k, String(v));
  }
  const s = p.toString();
  return s ? `?${s}` : "";
}

/**
 * `since` for an hours-back window, with a one-minute safety margin:
 * the server (and klams) stamp `until = now` at *their* clocks, a
 * beat after the browser computes `since` — without the margin a
 * full 30-day preset lands a hair over klams' 30-day window cap and
 * 400s with `window_too_large`.
 */
export function sinceHoursAgo(hours: number): string {
  return new Date(Date.now() - hours * 3600_000 + 60_000).toISOString();
}

export const api = {
  overview: () => request<Overview>("/api/overview"),
  activity: (params: {
    since?: string;
    until?: string;
    kinds?: string;
    authors?: string;
    state?: string;
    bucket?: string;
  }) => request<Activity>(`/api/activity${qs(params)}`),
  memories: (params: {
    since?: string;
    until?: string;
    kinds?: string;
    authors?: string;
    state?: string;
    limit?: number;
    cursor?: string;
  }) => request<MemoriesPage>(`/api/memories${qs(params)}`),
  authors: (params: { limit?: number; cursor?: string } = {}) =>
    request<AuthorPage>(`/api/authors${qs(params)}`),
  author: (id: string) => request<Author>(`/api/authors/${id}`),
  authorMemories: (id: string, params: { limit?: number; cursor?: string; kinds?: string } = {}) =>
    request<MemoriesPage>(`/api/authors/${id}/memories${qs(params)}`),
  search: (body: {
    query: string;
    types?: string[];
    filters?: Record<string, string>;
    top_k?: number;
  }) =>
    request<SearchResults>("/api/search", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(body),
    }),
  knowledge: (id: string) => request<KnowledgeItem>(`/api/knowledge/${id}`),
  health: () => request<Health>("/api/health"),
  metricsSummary: () => request<MetricsSummary>("/api/metrics/summary"),
  metricsHistory: () => request<{ samples: HistorySample[] }>("/api/metrics/history"),
};
