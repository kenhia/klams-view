//! The `/api` layer the SPA talks to: passthroughs to klams read
//! endpoints plus the aggregations klams doesn't provide (corpus
//! totals, activity bucketing, prometheus parsing, sampler history).
//!
//! Contract gotchas this layer absorbs so the UI never sees them are
//! catalogued in docs/design.md.

use crate::klams::{Client, Relay};
use crate::metrics::{self, History};
use axum::extract::{Path, Query, RawQuery, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct Inner {
    pub klams: Client,
    pub history: RwLock<History>,
}

#[derive(Clone)]
pub struct AppState(pub Arc<Inner>);

impl AppState {
    pub fn new(cfg: &crate::config::Config) -> anyhow::Result<Self> {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()?;
        Ok(Self(Arc::new(Inner {
            klams: Client::new(http, &cfg.klams_url, cfg.klams_token.clone()),
            history: RwLock::new(History::new()),
        })))
    }
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/status", get(status))
        .route("/overview", get(overview))
        .route("/activity", get(activity))
        .route("/memories", get(memories))
        .route("/authors", get(authors))
        .route("/authors/{id}", get(author))
        .route("/authors/{id}/memories", get(author_memories))
        .route("/search", post(search))
        .route("/knowledge/{id}", get(knowledge))
        .route("/health", get(health))
        .route("/metrics/summary", get(metrics_summary))
        .route("/metrics/history", get(metrics_history))
        .with_state(state)
}

/// Run the metrics sampler until the process exits.
pub async fn sampler(state: AppState, interval: std::time::Duration) {
    let mut tick = tokio::time::interval(interval);
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tick.tick().await;
        match state.0.klams.metrics_text().await {
            Ok(text) => {
                let fams = metrics::parse(&text);
                let t = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0);
                state
                    .0
                    .history
                    .write()
                    .await
                    .push(metrics::sample_from(&fams, t));
            }
            Err(err) => tracing::warn!(%err, "metrics sample failed"),
        }
    }
}

// ---- plumbing ------------------------------------------------------

fn relay_response(r: Relay) -> Response {
    (r.status, [(header::CONTENT_TYPE, r.content_type)], r.body).into_response()
}

fn err(status: StatusCode, code: &str, message: impl std::fmt::Display) -> Response {
    (
        status,
        Json(json!({ "code": code, "message": message.to_string() })),
    )
        .into_response()
}

fn upstream_err(e: anyhow::Error) -> Response {
    if e.to_string().contains("KLAMS_TOKEN not configured") {
        err(StatusCode::SERVICE_UNAVAILABLE, "unconfigured", e)
    } else {
        err(StatusCode::BAD_GATEWAY, "upstream_error", e)
    }
}

async fn passthrough(state: &AppState, path: &str, query: Option<String>) -> Response {
    match state
        .0
        .klams
        .relay_get(path, query.as_deref().unwrap_or(""))
        .await
    {
        Ok(r) => relay_response(r),
        Err(e) => upstream_err(e),
    }
}

// ---- passthrough routes --------------------------------------------

async fn memories(State(state): State<AppState>, RawQuery(q): RawQuery) -> Response {
    passthrough(&state, "/v1/memories", q).await
}

async fn authors(State(state): State<AppState>, RawQuery(q): RawQuery) -> Response {
    passthrough(&state, "/v1/authors", q).await
}

async fn author(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    passthrough(&state, &format!("/v1/authors/{id}"), None).await
}

async fn author_memories(
    State(state): State<AppState>,
    Path(id): Path<String>,
    RawQuery(q): RawQuery,
) -> Response {
    passthrough(&state, &format!("/v1/authors/{id}/memories"), q).await
}

async fn knowledge(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    passthrough(&state, &format!("/memory/knowledge/{id}"), None).await
}

async fn search(State(state): State<AppState>, Json(body): Json<Value>) -> Response {
    match state.0.klams.relay_post("/memory/search", body).await {
        Ok(r) => relay_response(r),
        Err(e) => upstream_err(e),
    }
}

async fn health(State(state): State<AppState>) -> Response {
    match state.0.klams.healthz().await {
        Ok(v) => Json(v).into_response(),
        Err(e) => err(StatusCode::BAD_GATEWAY, "upstream_error", e),
    }
}

// ---- status --------------------------------------------------------

async fn status(State(state): State<AppState>) -> Response {
    let klams = if !state.0.klams.has_token() {
        "unconfigured"
    } else {
        match state.0.klams.healthz().await {
            Ok(_) => "ok",
            Err(_) => "unreachable",
        }
    };
    Json(json!({ "view": "ok", "klams": klams })).into_response()
}

// ---- aggregations --------------------------------------------------

/// Everything Pulse needs in one call.
async fn overview(State(state): State<AppState>) -> Response {
    let k = &state.0.klams;
    let (healthz, metrics_text, authors, recent) =
        tokio::join!(k.healthz(), k.metrics_text(), walk_authors(k), async {
            if k.has_token() {
                k.get_json("/v1/memories", "limit=20").await.map(Some)
            } else {
                Ok(None)
            }
        });

    let summary = metrics_text.map(|t| summarize(&metrics::parse(&t))).ok();

    let (totals, agents) = match authors {
        Ok(list) => {
            let mut facts = 0i64;
            let mut knowledge = 0i64;
            let mut events = 0i64;
            let mut agents = Vec::new();
            for a in &list {
                let c = &a["counts"];
                facts += c["writes"].as_i64().unwrap_or(0);
                knowledge += c["knowledge"].as_i64().unwrap_or(0);
                events += c["events"].as_i64().unwrap_or(0);
                agents.push(json!({
                    "id": a["id"],
                    "agent_name": a["agent_name"],
                    "model": a.get("model"),
                    "last_seen_at": a["last_seen_at"],
                    "facts": c["writes"],
                    "knowledge": c["knowledge"],
                    "events": c["events"],
                }));
            }
            (
                Some(json!({
                    "facts": facts, "knowledge": knowledge, "events": events,
                    "authors": list.len(),
                })),
                Some(agents),
            )
        }
        Err(_) => (None, None),
    };

    Json(json!({
        "health": healthz.unwrap_or_else(|_| json!({ "status": "Down" })),
        "metrics": summary,
        "totals": totals,
        "agents": agents,
        "recent": recent.ok().flatten().map(|v| v["memories"].clone()),
        "configured": k.has_token(),
    }))
    .into_response()
}

/// All authors, cursor-walked to exhaustion (cap 10 pages = 2000).
async fn walk_authors(k: &Client) -> anyhow::Result<Vec<Value>> {
    let mut out = Vec::new();
    let mut cursor: Option<String> = None;
    for _ in 0..10 {
        let q = match &cursor {
            Some(c) => format!("limit=200&cursor={c}"),
            None => "limit=200".into(),
        };
        let page = k.get_json("/v1/authors", &q).await?;
        if let Some(items) = page["authors"].as_array() {
            out.extend(items.iter().cloned());
        }
        cursor = page["next_cursor"].as_str().map(String::from);
        if cursor.is_none() {
            break;
        }
    }
    Ok(out)
}

#[derive(Deserialize)]
struct ActivityParams {
    since: Option<String>,
    until: Option<String>,
    kinds: Option<String>,
    authors: Option<String>,
    state: Option<String>,
    /// "hour" or "day"; default: hour for spans <= 3 days.
    bucket: Option<String>,
    /// Scanner corpora outweigh interactive agents ~1000:1, so the UI
    /// asks for them to be left out by default. Defaults to `true`
    /// here — a bare `/api/activity` still reports everything.
    include_scanners: Option<bool>,
}

/// Bulk ingest agents (`klams-scanner`, `kai-scanner`, …) are named by
/// convention, and there is no upstream flag distinguishing them.
fn is_scanner(agent_name: &str) -> bool {
    agent_name.ends_with("-scanner")
}

/// Time-bucketed counts by kind over `/v1/memories`, walked
/// server-side. Also returns per-author counts for the window.
async fn activity(State(state): State<AppState>, Query(p): Query<ActivityParams>) -> Response {
    let k = &state.0.klams;
    let now = chrono::Utc::now();
    let until = match p.until.as_deref().map(chrono::DateTime::parse_from_rfc3339) {
        Some(Ok(t)) => t.with_timezone(&chrono::Utc),
        Some(Err(e)) => return err(StatusCode::BAD_REQUEST, "bad_until", e),
        None => now,
    };
    let since = match p.since.as_deref().map(chrono::DateTime::parse_from_rfc3339) {
        Some(Ok(t)) => t.with_timezone(&chrono::Utc),
        Some(Err(e)) => return err(StatusCode::BAD_REQUEST, "bad_since", e),
        None => until - chrono::Duration::hours(24),
    };
    let span = until - since;
    let bucket_hours: i64 = match p.bucket.as_deref() {
        Some("day") => 24,
        Some("hour") => 1,
        Some(other) => return err(StatusCode::BAD_REQUEST, "bad_bucket", other),
        None if span <= chrono::Duration::days(3) => 1,
        None => 24,
    };

    let mut base = format!(
        "since={}&until={}&limit=200",
        urlenc(&since.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)),
        urlenc(&until.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)),
    );
    if let Some(kinds) = &p.kinds {
        base.push_str(&format!("&kinds={}", urlenc(kinds)));
    }
    if let Some(authors) = &p.authors {
        base.push_str(&format!("&authors={}", urlenc(authors)));
    }
    if let Some(st) = &p.state {
        base.push_str(&format!("&state={}", urlenc(st)));
    }

    // bucket start (unix secs) -> per-kind counts
    let mut buckets: BTreeMap<i64, [i64; 3]> = BTreeMap::new();
    // agent_name -> per-kind counts for the window
    let mut by_author: BTreeMap<String, [i64; 3]> = BTreeMap::new();
    let mut total = 0i64;
    let mut truncated = false;
    // The cursor walks newest-first, so a capped walk loses the OLD
    // end of the window: track the oldest row actually seen and
    // report it, so the UI can mark the early buckets as partial.
    let mut oldest_seen: Option<i64> = None;
    let mut cursor: Option<String> = None;
    const MAX_PAGES: usize = 100;
    // Note the walk still *pages* over scanner rows — klams has no
    // exclude-author filter — so the MAX_PAGES cap is unchanged by
    // this; only what lands in the buckets differs.
    let include_scanners = p.include_scanners.unwrap_or(true);

    for page_no in 0..=MAX_PAGES {
        if page_no == MAX_PAGES {
            truncated = true;
            break;
        }
        let q = match &cursor {
            Some(c) => format!("{base}&cursor={c}"),
            None => base.clone(),
        };
        let page = match k.get_json("/v1/memories", &q).await {
            Ok(p) => p,
            Err(e) => return upstream_err(e),
        };
        for m in page["memories"].as_array().into_iter().flatten() {
            let slot = match m["kind"].as_str() {
                Some("fact") => 0,
                Some("knowledge") => 1,
                Some("event") => 2,
                _ => continue,
            };
            let Some(ts) = m["created_at"]
                .as_str()
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            else {
                continue;
            };
            let secs = ts.timestamp();
            // Coverage is a property of the walk, not of the filter,
            // so scanner rows still move `oldest_seen`.
            oldest_seen = Some(oldest_seen.map_or(secs, |o: i64| o.min(secs)));
            let agent = m["author"]["agent_name"].as_str().unwrap_or("unknown");
            if !include_scanners && is_scanner(agent) {
                continue;
            }
            let width = bucket_hours * 3600;
            buckets.entry(secs - secs.rem_euclid(width)).or_default()[slot] += 1;
            by_author.entry(agent.to_string()).or_default()[slot] += 1;
            total += 1;
        }
        cursor = page["next_cursor"].as_str().map(String::from);
        if cursor.is_none() {
            break;
        }
    }

    // Fill empty buckets so the chart's time axis is linear — a
    // sparse map would compress quiet gaps and misstate the timeline.
    let width = bucket_hours * 3600;
    let first = since.timestamp() - since.timestamp().rem_euclid(width);
    let mut t = first;
    let mut filled = 0;
    while t <= until.timestamp() && filled < 800 {
        buckets.entry(t).or_default();
        t += width;
        filled += 1;
    }

    let buckets: Vec<Value> = buckets
        .iter()
        .map(|(t, [f, kn, e])| json!({ "t": t, "fact": f, "knowledge": kn, "event": e }))
        .collect();
    let authors: Vec<Value> = by_author
        .iter()
        .map(|(a, [f, kn, e])| json!({ "agent_name": a, "fact": f, "knowledge": kn, "event": e }))
        .collect();

    Json(json!({
        "since": since.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        "until": until.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        "bucket_hours": bucket_hours,
        "buckets": buckets,
        "by_author": authors,
        "total": total,
        "truncated": truncated,
        "covered_since": truncated
            .then(|| oldest_seen.and_then(|s| chrono::DateTime::from_timestamp(s, 0))
                .map(|t| t.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)))
            .flatten(),
    }))
    .into_response()
}

fn urlenc(s: &str) -> String {
    // Query-string percent-encoding for the few reserved chars that
    // appear in RFC3339 timestamps and CSV lists.
    s.replace('%', "%25")
        .replace('+', "%2B")
        .replace(':', "%3A")
        .replace(',', "%2C")
}

// ---- metrics -------------------------------------------------------

fn summarize(fams: &metrics::Families) -> Value {
    let writes = metrics::sum_by(fams, "klams_writes_accepted_total", "type");
    let misses = metrics::sum_by(fams, "klams_search_misses_total", "reason");

    // agent_name -> {kind -> writes, searches}
    let mut agents: BTreeMap<String, Value> = BTreeMap::new();
    if let Some(lines) = fams.get("klams_mcp_writes_total") {
        for l in lines {
            let (Some(agent), Some(kind)) = (l.labels.get("agent_name"), l.labels.get("kind"))
            else {
                continue;
            };
            let entry = agents
                .entry(agent.clone())
                .or_insert_with(|| json!({ "writes": {}, "searches": 0.0 }));
            entry["writes"][kind] = json!(entry["writes"][kind].as_f64().unwrap_or(0.0) + l.value);
        }
    }
    for (agent, n) in metrics::sum_by(fams, "klams_mcp_search_total", "agent_name") {
        let entry = agents
            .entry(agent)
            .or_insert_with(|| json!({ "writes": {}, "searches": 0.0 }));
        entry["searches"] = json!(n);
    }

    json!({
        "queue": {
            "depth": metrics::gauge(fams, "klams_queue_depth"),
            "capacity": metrics::gauge(fams, "klams_queue_capacity"),
            "workers": metrics::gauge(fams, "klams_workers_active"),
        },
        "writes_accepted": writes,
        "writes_failed": metrics::sum_all(fams, "klams_writes_failed_total"),
        "search_misses": misses,
        "mcp_agents": agents,
        "latency": {
            "search_p50": metrics::quantile(fams, "klams_retrieval_duration_seconds", "0.5", Some(("op", "search"))),
            "search_p95": metrics::quantile(fams, "klams_retrieval_duration_seconds", "0.95", Some(("op", "search"))),
            "context_p95": metrics::quantile(fams, "klams_retrieval_duration_seconds", "0.95", Some(("op", "context"))),
            "embedding_p95": metrics::quantile(fams, "klams_embedding_latency_seconds", "0.95", None),
        },
        "backup": {
            "last_success_unix": metrics::gauge(fams, "klams_backup_last_success_timestamp_seconds"),
            "dir_writable": metrics::gauge(fams, "klams_backup_dir_writable"),
            "maintenance_active": metrics::gauge(fams, "klams_maintenance_mode_active"),
        },
    })
}

async fn metrics_summary(State(state): State<AppState>) -> Response {
    match state.0.klams.metrics_text().await {
        Ok(text) => Json(summarize(&metrics::parse(&text))).into_response(),
        Err(e) => err(StatusCode::BAD_GATEWAY, "upstream_error", e),
    }
}

async fn metrics_history(State(state): State<AppState>) -> Response {
    let snap = state.0.history.read().await.snapshot();
    Json(json!({ "samples": snap })).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scanners_are_recognised_by_name_suffix() {
        assert!(is_scanner("klams-scanner"));
        assert!(is_scanner("kai-scanner"));
        // Interactive agents, including ones that merely mention it.
        assert!(!is_scanner("claude"));
        assert!(!is_scanner("klams-mind"));
        assert!(!is_scanner("scanner"));
        assert!(!is_scanner("scanner-repair"));
    }

    #[test]
    fn urlenc_escapes_timestamps_and_csv_lists() {
        assert_eq!(urlenc("2026-07-30T18:00:00Z"), "2026-07-30T18%3A00%3A00Z");
        assert_eq!(urlenc("fact,knowledge"), "fact%2Cknowledge");
    }
}
