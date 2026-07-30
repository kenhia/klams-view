//! Thin server-side client for the klams HTTP surface. Read scope
//! only; `/healthz` and `/metrics` are public upstream and need no
//! token.

use anyhow::Context;
use axum::http::StatusCode;

pub struct Client {
    http: reqwest::Client,
    base: String,
    token: Option<String>,
}

/// A relayed upstream response: status + content type + raw body.
pub struct Relay {
    pub status: StatusCode,
    pub content_type: String,
    pub body: bytes::Bytes,
}

impl Client {
    pub fn new(http: reqwest::Client, base: &str, token: Option<String>) -> Self {
        Self {
            http,
            base: base.trim_end_matches('/').to_string(),
            token,
        }
    }

    pub fn has_token(&self) -> bool {
        self.token.is_some()
    }

    fn authed(&self, req: reqwest::RequestBuilder) -> anyhow::Result<reqwest::RequestBuilder> {
        let token = self
            .token
            .as_ref()
            .context("KLAMS_TOKEN not configured — /api routes that read the store need it")?;
        Ok(req.bearer_auth(token))
    }

    /// GET an authed klams path, relaying status/body verbatim.
    pub async fn relay_get(&self, path: &str, query: &str) -> anyhow::Result<Relay> {
        let url = if query.is_empty() {
            format!("{}{path}", self.base)
        } else {
            format!("{}{path}?{query}", self.base)
        };
        let resp = self.authed(self.http.get(&url))?.send().await?;
        Self::relay(resp).await
    }

    /// POST JSON to an authed klams path, relaying status/body.
    pub async fn relay_post(&self, path: &str, body: serde_json::Value) -> anyhow::Result<Relay> {
        let url = format!("{}{path}", self.base);
        let resp = self
            .authed(self.http.post(&url))?
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

    /// GET an authed path and parse JSON (for server-side aggregation).
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

    /// Public healthz — no token. klams returns the full snapshot with
    /// a 503 status when degraded, so accept any status and parse.
    pub async fn healthz(&self) -> anyhow::Result<serde_json::Value> {
        let url = format!("{}/healthz", self.base);
        let body = self.http.get(&url).send().await?.bytes().await?;
        Ok(serde_json::from_slice(&body)
            .unwrap_or_else(|_| serde_json::json!({ "status": "Down" })))
    }

    /// Public prometheus text — no token.
    pub async fn metrics_text(&self) -> anyhow::Result<String> {
        let url = format!("{}/metrics", self.base);
        let resp = self.http.get(&url).send().await?.error_for_status()?;
        Ok(resp.text().await?)
    }
}
