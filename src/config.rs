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
    /// The identity klams-view declares to klams, sent as
    /// `X-Homelab-Agent`. `KLAMS_AGENT`, default `klams-view`.
    ///
    /// Not a secret (program korg:2440): klams allow-lists the name in
    /// `[[auth.identities]]` and keeps it read-scoped, so there is no
    /// value to leak and no "unconfigured" state to degrade into. The
    /// override exists so the negative path stays testable — a name
    /// klams does not know must still 401.
    pub klams_agent: String,
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
        let klams_agent = std::env::var("KLAMS_AGENT")
            .ok()
            .filter(|a| !a.is_empty())
            .unwrap_or_else(|| "klams-view".into());
        Ok(Self {
            listen_addr,
            static_dir,
            klams_url,
            klams_agent,
        })
    }
}
