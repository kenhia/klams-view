//! klams-view — web viewer and dashboard for the klams memory service.
//!
//! One binary, korg-style: serves the built SvelteKit SPA and an
//! `/api/*` aggregation layer that talks to the klams HTTP API
//! server-side. The klams bearer token lives here, never in the
//! browser.

use anyhow::Context;
use axum::{Router, routing::get};
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;

mod api;
mod config;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "klams_view=info,tower_http=info".into()),
        )
        .init();

    let cfg = config::Config::from_env()?;
    let state = api::AppState::new(&cfg)?;

    let app = Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .nest("/api", api::router(state))
        .layer(TraceLayer::new_for_http());

    // SPA fallback: unknown paths serve the client-side-routed shell
    // with a 200 (ServeDir::fallback, not not_found_service — the
    // latter would stamp a 404 on deep links).
    let app = match &cfg.static_dir {
        Some(dir) => {
            let index = dir.join("index.html");
            app.fallback_service(ServeDir::new(dir).fallback(ServeFile::new(index)))
        }
        None => app,
    };

    let listener = tokio::net::TcpListener::bind(&cfg.listen_addr)
        .await
        .with_context(|| format!("binding {}", cfg.listen_addr))?;
    tracing::info!(addr = %cfg.listen_addr, "klams-view listening");
    axum::serve(listener, app).await?;
    Ok(())
}
