//! klams-view — web viewer and dashboard for the klams memory service.
//!
//! One binary, korg-style: serves the built SvelteKit SPA and an
//! `/api/*` aggregation layer that talks to the klams HTTP API
//! server-side. klams-view declares its identity here, never in the
//! browser.

use anyhow::Context;
use axum::{Router, routing::get};
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;

use klams_view::{api, config};

/// `--version` / `-V`, answered before anything else can fail.
///
/// Sprint 003 (#1013): `install-from-store.sh` asserts that a fetched
/// binary reports the version it was published as — the one check no
/// checksum can make, and the signal k-homelab's version floors read. So
/// this has to work on a host with no config, no `.env`, no reachable
/// klams and no bundle. Output matches clap's `<name> <version>` so
/// `awk '{print $NF}'` readers keep working.
///
/// klams learned this the hard way in its own sprint 042: its
/// `--version` ran after config resolution, so on exactly the hosts
/// being provisioned it exited non-zero with a config error instead of
/// printing a version.
fn version_early_out() -> bool {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("--version" | "-V") => {
            println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
            true
        }
        _ => false,
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    if version_early_out() {
        return Ok(());
    }

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "klams_view=info,tower_http=info".into()),
        )
        .init();

    let cfg = config::Config::from_env()?;
    let state = api::AppState::new(&cfg)?;

    tokio::spawn(api::sampler(
        state.clone(),
        std::time::Duration::from_secs(60),
    ));

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
