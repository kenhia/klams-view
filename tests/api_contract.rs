//! Contract tests for the `/api` aggregation layer (#809, CI half).
//!
//! `just check` used to be fmt/clippy/`cargo test` + svelte-check +
//! prettier + an SPA build, and **nothing exercised `/api/*` against a
//! klams at all**. That is the layer which decodes klams' response
//! shapes, so it is where a contract change shows up first — and a unit
//! test of `is_scanner` structurally cannot see it.
//!
//! This half runs in CI with no docker, no network and no klams: a stub
//! klams on a loopback port serves the shapes, and the *real* `/api`
//! router talks to it over real HTTP through the real `reqwest` client.
//! The other half (`just smoke-live`, `scripts/smoke-live.sh`) points
//! the same routes at a real klams and is the one that catches skew in
//! the *upstream*; this one catches regressions in klams-view.
//!
//! **Fixtures are synthetic, not captured.** Their shapes were derived
//! from `klams-types` (`PublicMemory`, `PublicAuthor`, `PublicAuthorRef`,
//! the health snapshot) and verified field-by-field against a live
//! klams 0.1.45 — but the repo is public (sprint 002), so no real
//! memory text, path or host lands in it.

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tower::ServiceExt;

// ---- the stub klams -------------------------------------------------

const GOOD_TOKEN: &str = "stub-read-token";
/// The stub reports the version klams-view is verified against, so the
/// doctor's skew step is `ok` here and skew is tested on its own.
const STUB_VERSION: &str = klams_view::doctor::KLAMS_VERIFIED_VERSION;

const AUTHOR_A: &str = "019f0000-0000-7000-8000-00000000000a";
const AUTHOR_SCANNER: &str = "019f0000-0000-7000-8000-00000000000b";

/// Every request the stub saw, as `METHOD path?query`. The assertion
/// that klams-view forwards a filter *upstream* needs this — a response
/// that merely looks right can be right by accident.
#[derive(Clone, Default)]
struct Seen(Arc<Mutex<Vec<String>>>);

impl Seen {
    fn record(&self, line: String) {
        self.0.lock().unwrap().push(line);
    }
    fn matching(&self, needle: &str) -> Vec<String> {
        self.0
            .lock()
            .unwrap()
            .iter()
            .filter(|l| l.contains(needle))
            .cloned()
            .collect()
    }
}

#[derive(Clone)]
struct Stub {
    seen: Seen,
    /// Rows the memories endpoints serve, newest first (klams' order).
    memories: Arc<Vec<Value>>,
}

fn health_snapshot() -> Value {
    json!({
        "status": "Ok",
        "postgres": { "state": "Ok" },
        "qdrant": { "state": "Ok" },
        "embeddings": { "state": "Ok" },
        "reranker": { "state": "Ok" },
        "queue": { "depth": 0, "capacity": 1024, "workers": 4 },
        "version": STUB_VERSION,
        "uptime_seconds": 196_236,
        "maintenance": { "active": false },
    })
}

/// Prometheus text in klams' actual dialect — note histograms render as
/// summaries (`quantile` label, no `_bucket`), which docs/design.md
/// lists as a contract gotcha this layer absorbs.
fn metrics_text() -> &'static str {
    "\
# HELP klams_queue_depth Queue depth
# TYPE klams_queue_depth gauge
klams_queue_depth 3
klams_queue_capacity 1024
klams_workers_active 4
klams_writes_accepted_total{type=\"knowledge\"} 61860
klams_writes_accepted_total{type=\"fact\"} 12
klams_writes_failed_total{reason=\"embed\"} 2
klams_search_misses_total{reason=\"zero_hit\"} 5
klams_search_misses_total{reason=\"low_score\"} 7
klams_mcp_writes_total{agent_name=\"claude\",kind=\"knowledge\"} 9
klams_mcp_writes_total{agent_name=\"claude\",kind=\"fact\"} 4
klams_mcp_search_total{agent_name=\"claude\"} 31
klams_retrieval_duration_seconds{op=\"search\",quantile=\"0.5\"} 0.021
klams_retrieval_duration_seconds{op=\"search\",quantile=\"0.95\"} 0.088
klams_retrieval_duration_seconds{op=\"context\",quantile=\"0.95\"} 0.14
klams_embedding_latency_seconds{quantile=\"0.95\"} 0.032
klams_backup_last_success_timestamp_seconds 1787000000
klams_backup_dir_writable 1
klams_maintenance_mode_active 0
"
}

/// Two authors, one of them a scanner — the ~1000:1 corpus asymmetry
/// that every per-agent view has to filter, in miniature.
fn authors() -> Value {
    json!({
        "authors": [
            {
                "id": AUTHOR_A,
                "agent_name": "claude",
                "model": "claude-opus-5",
                "session_title": "a session",
                "repo": "klams-view",
                "client_app": "claude-code",
                "client_version": "2.0.0",
                "created_at": "2026-08-01T00:00:00Z",
                "last_seen_at": "2026-08-19T06:00:00Z",
                "counts": { "writes": 4, "knowledge": 9, "events": 2,
                            "soft_deletes": 1, "restores_received": 0 },
            },
            {
                "id": AUTHOR_SCANNER,
                "agent_name": "kai-scanner",
                "session_title": "bulk ingest",
                "client_app": "klams-service",
                "created_at": "2026-07-13T00:00:00Z",
                "last_seen_at": "2026-08-19T06:53:00Z",
                "counts": { "writes": 0, "knowledge": 61_860, "events": 0,
                            "soft_deletes": 0, "restores_received": 0 },
            },
        ],
        "next_cursor": Value::Null,
    })
}

/// `PublicMemory` is flattened, absent-not-null for optionals, and its
/// `author` is a `PublicAuthorRef` carrying an `id` — which is what
/// makes the memory → author jump (#807) possible client-side.
fn memory_rows() -> Vec<Value> {
    // Timestamps are deliberately spread across three hours and two
    // days so bucketing has something to get wrong.
    vec![
        json!({
            "id": "01a00000-0000-7000-8000-000000000001",
            "kind": "knowledge",
            "author": { "id": AUTHOR_SCANNER, "agent_name": "kai-scanner" },
            "created_at": "2026-08-19T06:30:00Z",
            "updated_at": "2026-08-19T06:30:00Z",
            "tags": [],
            "text": "synthetic knowledge chunk",
            "heading_path": "Doc > Section",
            "repo": "example",
            "host": "example-host",
            "language": "markdown",
            "chunk_index": 0,
            "content_hash": "0".repeat(64),
        }),
        json!({
            "id": "01a00000-0000-7000-8000-000000000002",
            "kind": "fact",
            "author": { "id": AUTHOR_A, "agent_name": "claude" },
            "created_at": "2026-08-19T06:10:00Z",
            "updated_at": "2026-08-19T06:10:00Z",
            "tags": ["t"],
            "type": "Preference",
            "payload": { "k": "v" },
        }),
        json!({
            "id": "01a00000-0000-7000-8000-000000000003",
            "kind": "event",
            "author": { "id": AUTHOR_A, "agent_name": "claude" },
            "created_at": "2026-08-19T05:05:00Z",
            "updated_at": "2026-08-19T05:05:00Z",
            "tags": [],
            "category": "deploy",
            "payload": { "n": 1 },
        }),
        json!({
            "id": "01a00000-0000-7000-8000-000000000004",
            "kind": "knowledge",
            "author": { "id": AUTHOR_A, "agent_name": "claude" },
            "created_at": "2026-08-18T09:00:00Z",
            "updated_at": "2026-08-18T09:00:00Z",
            "tags": [],
            "text": "another synthetic chunk",
            "state": "deleted",
            "deleted_at": "2026-08-18T10:00:00Z",
        }),
    ]
}

fn unauthorized() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(json!({ "code": "unauthorized", "message": "missing or invalid bearer token" })),
    )
        .into_response()
}

fn require_token(headers: &HeaderMap) -> bool {
    headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v == format!("Bearer {GOOD_TOKEN}"))
}

async fn stub_healthz(State(s): State<Stub>) -> Response {
    s.seen.record("GET /healthz".into());
    Json(health_snapshot()).into_response()
}

async fn stub_metrics(State(s): State<Stub>) -> Response {
    s.seen.record("GET /metrics".into());
    metrics_text().into_response()
}

async fn stub_memories(
    State(s): State<Stub>,
    headers: HeaderMap,
    Query(q): Query<HashMap<String, String>>,
) -> Response {
    s.seen.record(format!("GET /v1/memories?{}", flatten(&q)));
    if !require_token(&headers) {
        return unauthorized();
    }
    // Honour the filters klams-view forwards and the aggregation
    // depends on. `authors` is a CSV of author UUIDs upstream, not
    // agent names — that distinction is #807's per-author activity, and
    // a stub that ignored it would let a wrong forward pass.
    let want: Option<Vec<&str>> = q.get("authors").map(|csv| csv.split(',').collect());
    let bound = |key: &str| -> Option<chrono::DateTime<chrono::Utc>> {
        q.get(key)
            .and_then(|v| chrono::DateTime::parse_from_rfc3339(v).ok())
            .map(|t| t.with_timezone(&chrono::Utc))
    };
    let (since, until) = (bound("since"), bound("until"));
    let rows: Vec<Value> = s
        .memories
        .iter()
        .filter(|m| match &want {
            Some(ids) => m["author"]["id"]
                .as_str()
                .is_some_and(|id| ids.contains(&id)),
            None => true,
        })
        .filter(|m| {
            let Some(t) = m["created_at"]
                .as_str()
                .and_then(|v| chrono::DateTime::parse_from_rfc3339(v).ok())
            else {
                return true;
            };
            let t = t.with_timezone(&chrono::Utc);
            since.is_none_or(|s| t >= s) && until.is_none_or(|u| t <= u)
        })
        .cloned()
        .collect();
    Json(json!({ "memories": rows, "next_cursor": Value::Null })).into_response()
}

async fn stub_authors(State(s): State<Stub>, headers: HeaderMap) -> Response {
    s.seen.record("GET /v1/authors".into());
    if !require_token(&headers) {
        return unauthorized();
    }
    Json(authors()).into_response()
}

async fn stub_author(
    State(s): State<Stub>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    s.seen.record(format!("GET /v1/authors/{id}"));
    if !require_token(&headers) {
        return unauthorized();
    }
    match authors()["authors"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["id"] == id.as_str())
    {
        Some(a) => Json(a.clone()).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({ "code": "not_found", "message": "no such author" })),
        )
            .into_response(),
    }
}

async fn stub_author_memories(
    State(s): State<Stub>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    s.seen.record(format!("GET /v1/authors/{id}/memories"));
    if !require_token(&headers) {
        return unauthorized();
    }
    let rows: Vec<Value> = s
        .memories
        .iter()
        .filter(|m| m["author"]["id"] == id.as_str())
        .cloned()
        .collect();
    Json(json!({ "memories": rows, "next_cursor": Value::Null })).into_response()
}

async fn stub_knowledge(
    State(s): State<Stub>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    s.seen.record(format!("GET /memory/knowledge/{id}"));
    if !require_token(&headers) {
        return unauthorized();
    }
    Json(json!({
        "id": id,
        "text": "synthetic knowledge chunk",
        "content_hash": "0".repeat(64),
        "source": "Scanner",
        "tags": [],
        "repo": "example",
        "file": "docs/example.md",
        "machine": "example-host",
        "heading_path": "Doc > Section",
        "language": "markdown",
        "chunk_index": 0,
        "confidence": 0.8,
        "decay_weight": 1.0,
        "use_count": 3,
        "last_used_at": Value::Null,
        "created_at": "2026-08-19T06:30:00Z",
        "updated_at": "2026-08-19T06:30:00Z",
    }))
    .into_response()
}

async fn stub_search(State(s): State<Stub>, headers: HeaderMap, body: String) -> Response {
    s.seen.record("POST /memory/search".into());
    if !require_token(&headers) {
        return unauthorized();
    }
    let q = serde_json::from_str::<Value>(&body)
        .ok()
        .and_then(|v| v["query"].as_str().map(str::to_string))
        .unwrap_or_default();
    Json(json!({
        "query": q,
        "results": [{
            "type": "knowledge",
            "id": "01a00000-0000-7000-8000-000000000001",
            "score": 0.71,
            "preview": "synthetic knowledge chunk",
            "payload": { "text": "synthetic knowledge chunk" },
        }],
        "total": 1,
        "degraded": false,
    }))
    .into_response()
}

fn flatten(q: &HashMap<String, String>) -> String {
    let mut pairs: Vec<String> = q.iter().map(|(k, v)| format!("{k}={v}")).collect();
    pairs.sort();
    pairs.join("&")
}

/// Boot a stub klams on an ephemeral loopback port and return
/// `(base_url, seen)`. The task is left running for the test's lifetime.
async fn spawn_stub() -> (String, Seen) {
    let seen = Seen::default();
    let stub = Stub {
        seen: seen.clone(),
        memories: Arc::new(memory_rows()),
    };
    let app = Router::new()
        .route("/healthz", get(stub_healthz))
        .route("/metrics", get(stub_metrics))
        .route("/v1/memories", get(stub_memories))
        .route("/v1/authors", get(stub_authors))
        .route("/v1/authors/{id}", get(stub_author))
        .route("/v1/authors/{id}/memories", get(stub_author_memories))
        .route("/memory/knowledge/{id}", get(stub_knowledge))
        .route("/memory/search", post(stub_search))
        .with_state(stub);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (format!("http://{addr}"), seen)
}

// ---- the klams-view side --------------------------------------------

fn config(klams_url: &str, token: Option<&str>) -> klams_view::config::Config {
    klams_view::config::Config {
        listen_addr: "127.0.0.1:0".into(),
        static_dir: None,
        klams_url: klams_url.to_string(),
        klams_token: token.map(str::to_string),
    }
}

/// The same nesting `main.rs` builds, so the paths under test are the
/// paths the SPA calls.
fn view_router(cfg: &klams_view::config::Config) -> Router {
    let state = klams_view::api::AppState::new(cfg).unwrap();
    Router::new().nest("/api", klams_view::api::router(state))
}

async fn api_get(app: &Router, uri: &str) -> (StatusCode, Value) {
    let req = axum::http::Request::builder()
        .uri(uri)
        .body(axum::body::Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, value)
}

async fn api_post(app: &Router, uri: &str, body: Value) -> (StatusCode, Value) {
    let req = axum::http::Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json")
        .body(axum::body::Body::from(body.to_string()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

/// A stub klams plus a klams-view router wired to it, with a good token.
async fn wired() -> (Router, Seen) {
    let (base, seen) = spawn_stub().await;
    (view_router(&config(&base, Some(GOOD_TOKEN))), seen)
}

// ---- passthroughs ---------------------------------------------------

#[tokio::test]
async fn memories_authors_and_knowledge_relay_upstream_shapes_verbatim() {
    let (app, _) = wired().await;

    let (status, body) = api_get(&app, "/api/memories?limit=200").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["memories"].as_array().unwrap().len(), 4);
    // Flattened content, absent-not-null, and the author ref carries an
    // id — the three facts the frontend types are written against.
    let first = &body["memories"][0];
    assert_eq!(first["kind"], "knowledge");
    assert!(first["text"].is_string());
    assert!(first.get("payload").is_none());
    assert!(first["author"]["id"].is_string());

    let (status, body) = api_get(&app, "/api/authors?limit=200").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["authors"].as_array().unwrap().len(), 2);

    let (status, body) = api_get(&app, &format!("/api/authors/{AUTHOR_A}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["agent_name"], "claude");
    assert_eq!(body["counts"]["knowledge"], 9);

    let (status, body) = api_get(&app, &format!("/api/authors/{AUTHOR_A}/memories?limit=50")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["memories"].as_array().unwrap().len(), 3);

    let (status, body) = api_get(&app, "/api/knowledge/01a00000-0000-7000-8000-000000000001").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["content_hash"].is_string());
    assert_eq!(body["source"], "Scanner");
}

#[tokio::test]
async fn search_posts_through_and_relays_the_result_envelope() {
    let (app, seen) = wired().await;
    let (status, body) =
        api_post(&app, "/api/search", json!({ "query": "chunk", "top_k": 5 })).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["query"], "chunk");
    assert_eq!(body["total"], 1);
    assert_eq!(body["degraded"], false);
    assert_eq!(seen.matching("POST /memory/search").len(), 1);
}

#[tokio::test]
async fn a_404_from_klams_stays_a_404_rather_than_becoming_a_502() {
    // Passthroughs relay status verbatim; the drawer's "no such id"
    // must not read as "klams is unreachable".
    let (app, _) = wired().await;
    let (status, body) = api_get(&app, "/api/authors/019f0000-0000-7000-8000-0000000000ff").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["code"], "not_found");
}

#[tokio::test]
async fn a_rejected_token_relays_401_and_names_it_unauthorized() {
    // The headline case (#808/klams #739): the token is wrong, not the
    // connection. The passthrough must not flatten it to a 502, or the
    // UI is back to one undifferentiated error string.
    let (base, _) = spawn_stub().await;
    let app = view_router(&config(&base, Some("wrong-token")));
    let (status, body) = api_get(&app, "/api/memories?limit=1").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["code"], "unauthorized");
}

#[tokio::test]
async fn no_token_configured_is_503_unconfigured_not_a_panic() {
    let (base, _) = spawn_stub().await;
    let app = view_router(&config(&base, None));
    let (status, body) = api_get(&app, "/api/memories?limit=1").await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["code"], "unconfigured");
}

#[tokio::test]
async fn an_unreachable_klams_is_a_502_upstream_error() {
    // Port 1 on loopback: nothing listens, and nothing will.
    let app = view_router(&config("http://127.0.0.1:1", Some(GOOD_TOKEN)));
    let (status, body) = api_get(&app, "/api/memories?limit=1").await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert_eq!(body["code"], "upstream_error");
}

// ---- aggregations ---------------------------------------------------

#[tokio::test]
async fn overview_sums_authors_and_carries_health_metrics_and_recent() {
    let (app, _) = wired().await;
    let (status, body) = api_get(&app, "/api/overview").await;
    assert_eq!(status, StatusCode::OK);

    assert_eq!(body["configured"], true);
    assert_eq!(body["health"]["status"], "Ok");
    assert_eq!(body["health"]["version"], STUB_VERSION);

    // Totals are summed from `counts`, and `writes` is the fact count —
    // the one field name that does not match its meaning upstream.
    assert_eq!(body["totals"]["facts"], 4);
    assert_eq!(body["totals"]["knowledge"], 61_869);
    assert_eq!(body["totals"]["events"], 2);
    assert_eq!(body["totals"]["authors"], 2);

    let agents = body["agents"].as_array().unwrap();
    assert_eq!(agents.len(), 2);
    assert_eq!(agents[0]["agent_name"], "claude");
    assert_eq!(agents[0]["facts"], 4);
    assert!(agents[0]["id"].is_string());

    assert_eq!(body["recent"].as_array().unwrap().len(), 4);

    // Metrics come from the prometheus text, quantiles included.
    assert_eq!(body["metrics"]["queue"]["depth"], 3.0);
    assert_eq!(body["metrics"]["writes_accepted"]["knowledge"], 61_860.0);
    assert_eq!(body["metrics"]["latency"]["search_p95"], 0.088);
}

#[tokio::test]
async fn overview_survives_an_unreachable_klams_with_nulls_not_an_error() {
    // Pulse has to render the outage; the unit `[Unit]` comment on the
    // systemd file promises exactly this, and only a live-ish test can
    // check it.
    let app = view_router(&config("http://127.0.0.1:1", Some(GOOD_TOKEN)));
    let (status, body) = api_get(&app, "/api/overview").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["health"]["status"], "Down");
    assert!(body["metrics"].is_null());
    assert!(body["totals"].is_null());
    assert!(body["agents"].is_null());
    assert!(body["recent"].is_null());
}

#[tokio::test]
async fn activity_buckets_by_kind_and_fills_the_quiet_hours() {
    let (app, _) = wired().await;
    let (status, body) = api_get(
        &app,
        "/api/activity?since=2026-08-19T04:00:00Z&until=2026-08-19T07:00:00Z&bucket=hour",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["bucket_hours"], 1);
    assert_eq!(body["truncated"], false);

    let buckets = body["buckets"].as_array().unwrap();
    // 04:00..07:00 inclusive of both ends = 4 hourly buckets, present
    // even where nothing was written: a sparse map would compress the
    // quiet hour and misstate the timeline.
    assert_eq!(buckets.len(), 4);
    let at = |hour: &str| -> &Value {
        let want = chrono::DateTime::parse_from_rfc3339(hour)
            .unwrap()
            .timestamp();
        buckets
            .iter()
            .find(|b| b["t"].as_i64() == Some(want))
            .unwrap_or_else(|| panic!("no bucket at {hour}"))
    };
    assert_eq!(at("2026-08-19T04:00:00Z")["knowledge"], 0);
    assert_eq!(at("2026-08-19T05:00:00Z")["event"], 1);
    assert_eq!(at("2026-08-19T06:00:00Z")["fact"], 1);
    assert_eq!(at("2026-08-19T06:00:00Z")["knowledge"], 1);

    // by_author is keyed by agent_name (the chart's label), while the
    // upstream filter is keyed by id — the two are not interchangeable.
    let by_author = body["by_author"].as_array().unwrap();
    assert!(
        by_author
            .iter()
            .any(|a| a["agent_name"] == "kai-scanner" && a["knowledge"] == 1)
    );
}

#[tokio::test]
async fn activity_include_scanners_false_drops_scanner_rows_from_the_counts() {
    let (app, _) = wired().await;
    let (_, all) = api_get(
        &app,
        "/api/activity?since=2026-08-19T04:00:00Z&until=2026-08-19T07:00:00Z&bucket=hour",
    )
    .await;
    let (_, filtered) = api_get(
        &app,
        "/api/activity?since=2026-08-19T04:00:00Z&until=2026-08-19T07:00:00Z&bucket=hour\
         &include_scanners=false",
    )
    .await;

    assert_eq!(all["total"], 3);
    assert_eq!(filtered["total"], 2);
    let names: Vec<&str> = filtered["by_author"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["agent_name"].as_str().unwrap())
        .collect();
    assert_eq!(names, vec!["claude"]);
}

#[tokio::test]
async fn activity_forwards_an_author_filter_upstream_as_a_uuid_csv() {
    // #807's per-author activity strip rides on this: the aggregation
    // already computes windowed counts, and the author filter is the
    // whole of the server-side work. Assert the *upstream* request, not
    // just the response — klams takes author UUIDs here, not names.
    let (app, seen) = wired().await;
    let (status, body) = api_get(
        &app,
        &format!(
            "/api/activity?since=2026-08-19T04:00:00Z&until=2026-08-19T07:00:00Z\
             &bucket=hour&authors={AUTHOR_A}&include_scanners=true"
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["total"], 2, "only claude's two rows are in the window");
    let names: Vec<&str> = body["by_author"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["agent_name"].as_str().unwrap())
        .collect();
    assert_eq!(names, vec!["claude"]);

    let upstream = seen.matching("/v1/memories");
    assert!(!upstream.is_empty(), "no upstream memories call was made");
    assert!(
        upstream
            .iter()
            .all(|l| l.contains(&format!("authors={AUTHOR_A}"))),
        "the author filter was not forwarded: {upstream:?}"
    );
}

#[tokio::test]
async fn activity_rejects_a_bad_window_and_a_bad_bucket_with_400s() {
    let (app, _) = wired().await;
    let (status, body) = api_get(&app, "/api/activity?since=yesterday").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["code"], "bad_since");

    let (status, body) = api_get(&app, "/api/activity?bucket=fortnight").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["code"], "bad_bucket");
}

#[tokio::test]
async fn health_and_metrics_summary_and_history_answer_the_operator_page() {
    let (app, _) = wired().await;

    let (status, body) = api_get(&app, "/api/health").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "Ok");
    // Subsystems are rendered generically by shape, so their presence
    // as `{state}` objects is the contract, not their names.
    assert_eq!(body["postgres"]["state"], "Ok");
    assert_eq!(body["queue"]["capacity"], 1024);

    let (status, body) = api_get(&app, "/api/metrics/summary").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["writes_failed"], 2.0);
    assert_eq!(body["search_misses"]["zero_hit"], 5.0);
    assert_eq!(body["mcp_agents"]["claude"]["searches"], 31.0);
    assert_eq!(body["mcp_agents"]["claude"]["writes"]["knowledge"], 9.0);
    assert_eq!(body["backup"]["dir_writable"], 1.0);

    // The sampler is not running in-test, so history is empty — but the
    // envelope must still be `{samples: []}`, which is what the chart
    // reduces over.
    let (status, body) = api_get(&app, "/api/metrics/history").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["samples"].as_array().unwrap().len(), 0);
}

// ---- the doctor (#808), end to end ---------------------------------

fn check<'a>(report: &'a Value, id: &str) -> &'a Value {
    report["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == id)
        .unwrap_or_else(|| panic!("no `{id}` check in the report"))
}

#[tokio::test]
async fn doctor_reports_every_link_ok_against_a_healthy_klams() {
    let (app, _) = wired().await;
    let (status, body) = api_get(&app, "/api/status").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["overall"], "ok");
    assert_eq!(body["view"]["klams_version"], STUB_VERSION);
    for id in [
        "config", "token", "dns", "tcp", "healthz", "authed", "version",
    ] {
        assert_eq!(check(&body, id)["state"], "ok", "check `{id}`");
    }
    // http:// upstream — the TLS step says so rather than claiming ok.
    assert_eq!(check(&body, "tls")["state"], "skipped");
    // Nothing is wrong, so nothing offers a fix.
    assert!(
        body["checks"]
            .as_array()
            .unwrap()
            .iter()
            .all(|c| c.get("fix").is_none())
    );
}

#[tokio::test]
async fn doctor_separates_a_rejected_token_from_an_unreachable_klams() {
    // This pair IS #808. Same undifferentiated `✕ {error}` before;
    // two different rows, two different fixes, now.
    let (base, _) = spawn_stub().await;
    let rejected = view_router(&config(&base, Some("wrong-token")));
    let (_, body) = api_get(&rejected, "/api/status").await;
    assert_eq!(body["overall"], "down");
    // Reachability is green — that is the trap, stated out loud.
    assert_eq!(check(&body, "tcp")["state"], "ok");
    assert_eq!(check(&body, "healthz")["state"], "ok");
    let authed = check(&body, "authed");
    assert_eq!(authed["state"], "fail");
    assert!(
        authed["detail"].as_str().unwrap().contains("401"),
        "{}",
        authed["detail"]
    );
    assert!(
        authed["fix"]
            .as_str()
            .unwrap()
            .contains("read-scoped grant"),
        "the fix must name the action, not the symptom"
    );

    let dead = view_router(&config("http://127.0.0.1:1", Some(GOOD_TOKEN)));
    let (_, body) = api_get(&dead, "/api/status").await;
    assert_eq!(body["overall"], "down");
    assert_eq!(check(&body, "tcp")["state"], "fail");
    // Everything downstream says it was never attempted rather than
    // reporting a failure it did not observe.
    for id in ["tls", "healthz", "authed", "version"] {
        assert_eq!(check(&body, id)["state"], "skipped", "check `{id}`");
    }
}

#[tokio::test]
async fn doctor_reports_a_missing_token_as_configuration_not_rejection() {
    let (base, _) = spawn_stub().await;
    let app = view_router(&config(&base, None));
    let (_, body) = api_get(&app, "/api/status").await;
    assert_eq!(check(&body, "token")["state"], "fail");
    assert_eq!(check(&body, "authed")["state"], "skipped");
    assert!(
        check(&body, "token")["fix"]
            .as_str()
            .unwrap()
            .contains("KLAMS_TOKEN")
    );
}

#[tokio::test]
async fn doctor_reports_an_unparseable_klams_url_without_probing() {
    let app = view_router(&config("not a url", Some(GOOD_TOKEN)));
    let (status, body) = api_get(&app, "/api/status").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["overall"], "down");
    assert_eq!(check(&body, "config")["state"], "fail");
    assert_eq!(check(&body, "dns")["state"], "skipped");
    assert_eq!(check(&body, "tcp")["state"], "skipped");
}
