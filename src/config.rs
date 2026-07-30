//! Configuration — env vars only, `.env`-friendly (the kproject
//! harness gitignores `.env`; `just run` sources it).

use std::path::PathBuf;

pub struct Config {
    /// Address to listen on. `KLAMS_VIEW_ADDR`, default `127.0.0.1:7779`.
    pub listen_addr: String,
    /// Built SPA directory. `KLAMS_VIEW_STATIC`, default `web/build`
    /// if it exists, else none (API-only, for dev where `vite dev`
    /// serves the frontend and proxies `/api`).
    pub static_dir: Option<PathBuf>,
    /// Base URL of the klams service. `KLAMS_URL`, default
    /// `http://localhost:7777`.
    pub klams_url: String,
    /// Bearer token for klams. `KLAMS_TOKEN`, required for API routes
    /// to work; startup proceeds without it so the shell can be
    /// smoke-tested.
    pub klams_token: Option<String>,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        let listen_addr =
            std::env::var("KLAMS_VIEW_ADDR").unwrap_or_else(|_| "127.0.0.1:7779".into());
        let static_dir = match std::env::var("KLAMS_VIEW_STATIC") {
            Ok(s) if s.is_empty() => None,
            Ok(s) => Some(PathBuf::from(s)),
            Err(_) => {
                let default = PathBuf::from("web/build");
                default.is_dir().then_some(default)
            }
        };
        let klams_url =
            std::env::var("KLAMS_URL").unwrap_or_else(|_| "http://localhost:7777".into());
        let klams_token = std::env::var("KLAMS_TOKEN").ok().filter(|t| !t.is_empty());
        if klams_token.is_none() {
            tracing::warn!("KLAMS_TOKEN not set — /api routes will return 503");
        }
        Ok(Self {
            listen_addr,
            static_dir,
            klams_url,
            klams_token,
        })
    }
}
