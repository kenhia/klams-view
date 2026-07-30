//! The `/api` layer: server-side client for klams plus the
//! aggregations the klams API doesn't provide. Placeholder while the
//! contract work lands; every route 501s except `/api/status`.

use axum::{Json, Router, extract::State, http::StatusCode, routing::get};
use std::sync::Arc;

pub struct Inner {
    pub http: reqwest::Client,
    pub klams_url: String,
    pub klams_token: Option<String>,
}

#[derive(Clone)]
pub struct AppState(pub Arc<Inner>);

impl AppState {
    pub fn new(cfg: &crate::config::Config) -> anyhow::Result<Self> {
        Ok(Self(Arc::new(Inner {
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()?,
            klams_url: cfg.klams_url.clone(),
            klams_token: cfg.klams_token.clone(),
        })))
    }
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/status", get(status))
        .with_state(state)
}

/// Reachability probe: is klams-view up, and can it see klams?
async fn status(State(state): State<AppState>) -> (StatusCode, Json<serde_json::Value>) {
    let klams = match &state.0.klams_token {
        None => "unconfigured",
        Some(_) => {
            let url = format!("{}/healthz", state.0.klams_url);
            match state.0.http.get(&url).send().await {
                Ok(r) if r.status().is_success() => "ok",
                Ok(_) | Err(_) => "unreachable",
            }
        }
    };
    (
        StatusCode::OK,
        Json(serde_json::json!({ "view": "ok", "klams": klams })),
    )
}
