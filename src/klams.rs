//! Thin server-side client for the klams HTTP surface. Read scope
//! only; `/healthz` and `/metrics` are public upstream and need no
//! auth at all.
//!
//! klams-view authenticates by *declaring who it is* — the
//! `X-Homelab-Agent` header, allow-listed read-scoped in klams'
//! `[[auth.identities]]` (program korg:2440). The name is not a
//! secret, so unlike the bearer token it replaced there is no
//! unconfigured state: every request carries an identity, and a name
//! klams does not know is refused at klams.

use axum::http::StatusCode;

/// klams' identity header. The name it carries is allow-listed
/// server-side; the header itself is not a credential.
pub const AGENT_HEADER: &str = "X-Homelab-Agent";

pub struct Client {
    http: reqwest::Client,
    base: String,
    agent: String,
}

/// A relayed upstream response: status + content type + raw body.
pub struct Relay {
    pub status: StatusCode,
    pub content_type: String,
    pub body: bytes::Bytes,
}

impl Client {
    pub fn new(http: reqwest::Client, base: &str, agent: String) -> Self {
        Self {
            http,
            base: base.trim_end_matches('/').to_string(),
            agent,
        }
    }

    /// The identity this client declares. The doctor reports it so an
    /// operator can match it against klams' `[[auth.identities]]`
    /// without reading either config file.
    pub fn agent(&self) -> &str {
        &self.agent
    }

    /// The klams base URL, normalised (no trailing slash). The doctor
    /// needs it to resolve/connect step by step rather than letting one
    /// `reqwest` error stand in for the whole chain.
    pub fn base(&self) -> &str {
        &self.base
    }

    /// Unauthenticated GET with the transport error left UNwrapped —
    /// `anyhow` would erase the `reqwest::Error` the doctor classifies
    /// (connect vs timeout vs TLS).
    pub async fn probe(&self, path: &str) -> Result<reqwest::Response, reqwest::Error> {
        self.http.get(format!("{}{path}", self.base)).send().await
    }

    /// Identified GET, same contract. Always attempted: there is no
    /// "not configured" case to skip for, so the doctor's authed step
    /// always has a real result to report.
    pub async fn probe_authed(&self, path: &str) -> Result<reqwest::Response, reqwest::Error> {
        self.identified(self.http.get(format!("{}{path}", self.base)))
            .send()
            .await
    }

    fn identified(&self, req: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        req.header(AGENT_HEADER, &self.agent)
    }

    /// GET an identified klams path, relaying status/body verbatim.
    pub async fn relay_get(&self, path: &str, query: &str) -> anyhow::Result<Relay> {
        let url = if query.is_empty() {
            format!("{}{path}", self.base)
        } else {
            format!("{}{path}?{query}", self.base)
        };
        let resp = self.identified(self.http.get(&url)).send().await?;
        Self::relay(resp).await
    }

    /// POST JSON to an identified klams path, relaying status/body.
    pub async fn relay_post(&self, path: &str, body: serde_json::Value) -> anyhow::Result<Relay> {
        let url = format!("{}{path}", self.base);
        let resp = self
            .identified(self.http.post(&url))
            .json(&body)
            .send()
            .await?;
        Self::relay(resp).await
    }

    async fn relay(resp: reqwest::Response) -> anyhow::Result<Relay> {
        let status = StatusCode::from_u16(resp.status().as_u16())?;
        let content_type = resp
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("application/json")
            .to_string();
        let body = resp.bytes().await?;
        Ok(Relay {
            status,
            content_type,
            body,
        })
    }

    /// GET an identified path and parse JSON (for server-side aggregation).
    pub async fn get_json(&self, path: &str, query: &str) -> anyhow::Result<serde_json::Value> {
        let relay = self.relay_get(path, query).await?;
        if !relay.status.is_success() {
            anyhow::bail!(
                "klams GET {path} -> {}: {}",
                relay.status,
                String::from_utf8_lossy(&relay.body)
            );
        }
        Ok(serde_json::from_slice(&relay.body)?)
    }

    /// Public healthz — no identity needed. klams returns the full snapshot with
    /// a 503 status when degraded, so accept any status and parse.
    pub async fn healthz(&self) -> anyhow::Result<serde_json::Value> {
        let url = format!("{}/healthz", self.base);
        let body = self.http.get(&url).send().await?.bytes().await?;
        Ok(serde_json::from_slice(&body)
            .unwrap_or_else(|_| serde_json::json!({ "status": "Down" })))
    }

    /// Public prometheus text — no identity needed.
    pub async fn metrics_text(&self) -> anyhow::Result<String> {
        let url = format!("{}/metrics", self.base);
        let resp = self.http.get(&url).send().await?.error_for_status()?;
        Ok(resp.text().await?)
    }
}
