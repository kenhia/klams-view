//! The connection doctor (#808).
//!
//! The 2026-07-28 viewport incident (klams #739) burned an afternoon
//! because a stale bearer token presented as "green dashboard, red goo
//! everywhere else" (the token is gone — klams-view declares an
//! identity now — but a name klams does not know fails the same way): `/healthz` is unauthenticated, so reachability
//! looked fine while every authed call 401'd. klams-view moved the
//! token server-side, which removed the *user-facing* half of that
//! failure — the operator-facing half was intact until this module, so
//! a rejected identity and an unreachable `KLAMS_URL` both rendered
//! as the same undifferentiated `✕ {error}` line.
//!
//! So the doctor walks the chain one link at a time and reports each
//! link separately, with the fix named on the row that failed:
//!
//! ```text
//! config → identity → dns → tcp → tls → healthz → authed → version
//! ```
//!
//! Two rules make it worth reading:
//!
//! * **A step that was never attempted says so** (`skipped`), rather
//!   than reporting a failure it did not observe. One broken link
//!   explains the rest.
//! * **The authenticated step is the point.** `/healthz` structurally
//!   cannot tell you klams accepts our identity, so the doctor spends
//!   a real read call on it.

use crate::klams::Client;
use serde::Serialize;
use std::time::{Duration, Instant};

/// The klams release klams-view's `/api` decoders were last verified
/// against — by `just smoke-live` (#809), which drives every route
/// against a real klams. klams versions are `0.1.<sprint>`, so the
/// patch component moves every klams sprint: a patch difference is an
/// advisory ("re-run the smoke, then bump this"), a major/minor
/// difference is a contract break and reads as down.
pub const KLAMS_VERIFIED_VERSION: &str = "0.1.49";

/// Connect/read budget per step. Deliberately short: the doctor is the
/// page you open when something is already wrong, so it must answer
/// faster than the 30s client timeout the rest of `/api` uses.
const STEP_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum State {
    /// The step did what it should.
    Ok,
    /// Worth an operator's attention; not a reason to call it broken.
    /// Rolls up to `advisory`, the same word k-homelab's `bin/audit`
    /// uses for "checked, but weaker than a full verification".
    Warn,
    /// This link is broken. Rolls up to `down`.
    Fail,
    /// Not attempted, because an earlier step failed (or it does not
    /// apply — TLS on an `http://` URL).
    Skipped,
}

#[derive(Debug, Clone, Serialize)]
pub struct Check {
    /// Stable id, for tests and for the UI's keying.
    pub id: &'static str,
    pub label: &'static str,
    pub state: State,
    /// What was observed. Never a bare error string when something
    /// more specific is knowable.
    pub detail: String,
    /// What to do about it — present only when there is something to
    /// do. This is the field the incident was missing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix: Option<String>,
    pub elapsed_ms: u64,
}

impl Check {
    fn new(id: &'static str, label: &'static str, state: State, detail: impl Into<String>) -> Self {
        Self {
            id,
            label,
            state,
            detail: detail.into(),
            fix: None,
            elapsed_ms: 0,
        }
    }

    fn with_fix(mut self, fix: impl Into<String>) -> Self {
        self.fix = Some(fix.into());
        self
    }

    fn timed(mut self, started: Instant) -> Self {
        self.elapsed_ms = started.elapsed().as_millis() as u64;
        self
    }

    fn skipped(id: &'static str, label: &'static str, because: &str) -> Self {
        Self::new(
            id,
            label,
            State::Skipped,
            format!("not attempted — {because}"),
        )
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Report {
    /// `ok` | `advisory` | `down` — the rollup, so a caller that wants
    /// one word does not have to reduce the list itself.
    pub overall: &'static str,
    pub view: ViewInfo,
    pub checks: Vec<Check>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ViewInfo {
    /// klams-view's own version, so a screenshot names the build.
    pub version: &'static str,
    /// The URL being diagnosed. Never carries credentials — klams
    /// auth is a declared identity, and there is no secret to leak.
    pub klams_url: String,
    /// What `version` skew is measured against.
    pub klams_verified: &'static str,
    /// klams' reported version, when the snapshot was readable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub klams_version: Option<String>,
}

/// Walk the chain. Every step is reported; nothing short-circuits
/// silently.
pub async fn run(client: &Client) -> Report {
    let mut checks: Vec<Check> = Vec::new();
    let base = client.base().to_string();

    // ---- config: does KLAMS_URL even parse? -------------------------
    let started = Instant::now();
    let parsed = url::Url::parse(&base);
    let (scheme, host, port) = match &parsed {
        Ok(u) => {
            let scheme = u.scheme().to_string();
            let host = u.host_str().map(str::to_string);
            let port = u.port_or_known_default();
            match (&host, port) {
                (Some(h), Some(p)) => {
                    checks.push(
                        Check::new(
                            "config",
                            "KLAMS_URL parses",
                            State::Ok,
                            format!("{scheme}://{h}:{p}"),
                        )
                        .timed(started),
                    );
                    (Some(scheme), Some(h.clone()), Some(p))
                }
                _ => {
                    checks.push(
                        Check::new(
                            "config",
                            "KLAMS_URL parses",
                            State::Fail,
                            format!("`{base}` has no host or no port"),
                        )
                        .with_fix(CONFIG_FIX)
                        .timed(started),
                    );
                    (None, None, None)
                }
            }
        }
        Err(e) => {
            checks.push(
                Check::new(
                    "config",
                    "KLAMS_URL parses",
                    State::Fail,
                    format!("`{base}` is not a URL: {e}"),
                )
                .with_fix(CONFIG_FIX)
                .timed(started),
            );
            (None, None, None)
        }
    };

    // ---- identity: which name do we declare? ------------------------
    // Separate from `authed` on purpose, and it survived the move off
    // bearer tokens with its job changed rather than removed. There is
    // no longer an unset state to catch — the name always has a
    // default — so this step answers the question that replaced it:
    // *which* name are we sending? That is the one thing an operator
    // must match against klams' `[[auth.identities]]`, and it is not a
    // secret, so the doctor can simply print it.
    checks.push(Check::new(
        "identity",
        "Identity declared",
        State::Ok,
        format!(
            "sending `{}: {}` — klams must allow-list that name, read-scoped",
            crate::klams::AGENT_HEADER,
            client.agent()
        ),
    ));

    // ---- dns + tcp: reachability, before HTTP has a say -------------
    let mut addr = None;
    match (&host, port) {
        (Some(h), Some(p)) => {
            let started = Instant::now();
            match tokio::time::timeout(STEP_TIMEOUT, tokio::net::lookup_host((h.as_str(), p))).await
            {
                Ok(Ok(addrs)) => {
                    let addrs: Vec<_> = addrs.collect();
                    match addrs.first() {
                        Some(first) => {
                            addr = Some(*first);
                            let shown = addrs
                                .iter()
                                .map(|a| a.ip().to_string())
                                .collect::<Vec<_>>()
                                .join(", ");
                            checks.push(
                                Check::new(
                                    "dns",
                                    "Host resolves",
                                    State::Ok,
                                    format!("{h} → {shown}"),
                                )
                                .timed(started),
                            );
                        }
                        None => checks.push(
                            Check::new(
                                "dns",
                                "Host resolves",
                                State::Fail,
                                format!("{h} resolved to no addresses"),
                            )
                            .with_fix(DNS_FIX)
                            .timed(started),
                        ),
                    }
                }
                Ok(Err(e)) => checks.push(
                    Check::new(
                        "dns",
                        "Host resolves",
                        State::Fail,
                        format!("cannot resolve {h}: {e}"),
                    )
                    .with_fix(DNS_FIX)
                    .timed(started),
                ),
                Err(_) => checks.push(
                    Check::new(
                        "dns",
                        "Host resolves",
                        State::Fail,
                        format!("resolving {h} timed out after {}s", STEP_TIMEOUT.as_secs()),
                    )
                    .with_fix(DNS_FIX)
                    .timed(started),
                ),
            }
        }
        _ => checks.push(Check::skipped(
            "dns",
            "Host resolves",
            "KLAMS_URL is unusable",
        )),
    }

    match addr {
        Some(a) => {
            let started = Instant::now();
            match tokio::time::timeout(STEP_TIMEOUT, tokio::net::TcpStream::connect(a)).await {
                Ok(Ok(_)) => checks.push(
                    Check::new(
                        "tcp",
                        "TCP connects",
                        State::Ok,
                        format!("connected to {a}"),
                    )
                    .timed(started),
                ),
                Ok(Err(e)) => checks.push(
                    Check::new(
                        "tcp",
                        "TCP connects",
                        State::Fail,
                        format!("cannot connect to {a}: {e}"),
                    )
                    .with_fix(tcp_fix(port))
                    .timed(started),
                ),
                Err(_) => checks.push(
                    Check::new(
                        "tcp",
                        "TCP connects",
                        State::Fail,
                        format!(
                            "connecting to {a} timed out after {}s",
                            STEP_TIMEOUT.as_secs()
                        ),
                    )
                    .with_fix(tcp_fix(port))
                    .timed(started),
                ),
            }
        }
        None => checks.push(Check::skipped(
            "tcp",
            "TCP connects",
            "the host did not resolve",
        )),
    }

    let reachable = checks
        .iter()
        .find(|c| c.id == "tcp")
        .is_some_and(|c| c.state == State::Ok);
    let https = scheme.as_deref() == Some("https");

    // ---- tls + healthz ---------------------------------------------
    // One request answers both. A TLS failure surfaces as a `reqwest`
    // connect error, so it is classified out of the error chain rather
    // than reported as "klams is down" — a wrong cert and a stopped
    // service are not the same call to make.
    let mut snapshot: Option<serde_json::Value> = None;
    if !reachable {
        checks.push(Check::skipped(
            "tls",
            "TLS handshake",
            "TCP did not connect",
        ));
        checks.push(Check::skipped(
            "healthz",
            "Unauthenticated /healthz",
            "TCP did not connect",
        ));
    } else {
        let started = Instant::now();
        let probe = tokio::time::timeout(STEP_TIMEOUT, client.probe("/healthz")).await;
        match probe {
            Ok(Ok(resp)) => {
                checks.push(if https {
                    Check::new("tls", "TLS handshake", State::Ok, "certificate accepted")
                        .timed(started)
                } else {
                    Check::new(
                        "tls",
                        "TLS handshake",
                        State::Skipped,
                        "KLAMS_URL is http:// — no TLS in play",
                    )
                });
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                let parsed: Option<serde_json::Value> = serde_json::from_str(&body).ok();
                let reported = parsed
                    .as_ref()
                    .and_then(|v| v["status"].as_str())
                    .map(str::to_string);
                snapshot = parsed;
                checks.push(match (status.as_u16(), reported.as_deref()) {
                    // klams answers /healthz with the full snapshot and
                    // a 503 when degraded, so the status code alone
                    // under-reports it; the body is the truth.
                    (_, Some("Ok")) => Check::new(
                        "healthz",
                        "Unauthenticated /healthz",
                        State::Ok,
                        "klams reports Ok",
                    )
                    .timed(started),
                    (_, Some(other)) => Check::new(
                        "healthz",
                        "Unauthenticated /healthz",
                        State::Warn,
                        format!("klams reports {other}"),
                    )
                    .with_fix("this is klams' own health, not klams-view's — see the subsystem badges below, and `journalctl -u klams-service`")
                    .timed(started),
                    (code, None) => Check::new(
                        "healthz",
                        "Unauthenticated /healthz",
                        State::Fail,
                        format!("HTTP {code}, and the body is not a klams health snapshot: {}", snippet(&body)),
                    )
                    .with_fix("is KLAMS_URL pointing at klams, and not at another service (or a proxy) on that port?")
                    .timed(started),
                });
            }
            Ok(Err(e)) if https && is_tls_error(&e) => {
                checks.push(
                    Check::new(
                        "tls",
                        "TLS handshake",
                        State::Fail,
                        format!("handshake failed: {}", chain(&e)),
                    )
                    .with_fix(
                        "the port answers but the certificate did not verify — for a tailnet host \
                         use its `*.ts.net` name (that is the name the cert carries), not an IP",
                    )
                    .timed(started),
                );
                checks.push(Check::skipped(
                    "healthz",
                    "Unauthenticated /healthz",
                    "the TLS handshake failed",
                ));
            }
            Ok(Err(e)) => {
                checks.push(if https {
                    Check::new("tls", "TLS handshake", State::Ok, "certificate accepted")
                } else {
                    Check::new(
                        "tls",
                        "TLS handshake",
                        State::Skipped,
                        "KLAMS_URL is http:// — no TLS in play",
                    )
                });
                checks.push(
                    Check::new(
                        "healthz",
                        "Unauthenticated /healthz",
                        State::Fail,
                        format!("request failed: {}", chain(&e)),
                    )
                    .with_fix(tcp_fix(port))
                    .timed(started),
                );
            }
            Err(_) => {
                checks.push(Check::skipped(
                    "tls",
                    "TLS handshake",
                    "the /healthz request timed out",
                ));
                checks.push(
                    Check::new(
                        "healthz",
                        "Unauthenticated /healthz",
                        State::Fail,
                        format!(
                            "TCP connected but /healthz did not answer within {}s",
                            STEP_TIMEOUT.as_secs()
                        ),
                    )
                    .with_fix(
                        "something is listening on that port but is not answering as klams — \
                         check `systemctl status klams-service` and what else holds the port",
                    )
                    .timed(started),
                );
            }
        }
    }

    // ---- authed: the step /healthz structurally cannot make ---------
    let started = Instant::now();
    let authed = if !reachable {
        Check::skipped("authed", "Authenticated read", "TCP did not connect")
    } else {
        match tokio::time::timeout(STEP_TIMEOUT, client.probe_authed(AUTH_PROBE_PATH)).await {
            Err(_) => Check::new(
                "authed",
                "Authenticated read",
                State::Fail,
                format!(
                    "GET {AUTH_PROBE_PATH} did not answer within {}s",
                    STEP_TIMEOUT.as_secs()
                ),
            )
            .timed(started),
            Ok(Err(e)) => Check::new(
                "authed",
                "Authenticated read",
                State::Fail,
                format!("request failed: {}", chain(&e)),
            )
            .timed(started),
            Ok(Ok(resp)) => {
                let status = resp.status().as_u16();
                let body = resp.text().await.unwrap_or_default();
                let code = serde_json::from_str::<serde_json::Value>(&body)
                    .ok()
                    .and_then(|v| v["code"].as_str().map(str::to_string));
                match status {
                    200 => Check::new(
                        "authed",
                        "Authenticated read",
                        State::Ok,
                        format!(
                            "GET {AUTH_PROBE_PATH} succeeded — klams accepts `{}` and it is read-scoped",
                            client.agent()
                        ),
                    )
                    .timed(started),
                    401 => Check::new(
                        "authed",
                        "Authenticated read",
                        State::Fail,
                        format!(
                            "klams does not know the identity `{}` (401 unauthorized) — \
                             reachability is fine, so every page will show data-less errors \
                             while /healthz stays green",
                            client.agent()
                        ),
                    )
                    .with_fix(IDENTITY_REJECTED_FIX)
                    .timed(started),
                    403 => Check::new(
                        "authed",
                        "Authenticated read",
                        State::Fail,
                        format!(
                            "klams knows `{}` but it is under-scoped (403 forbidden) — \
                             klams-view needs `read`",
                            client.agent()
                        ),
                    )
                    .with_fix(IDENTITY_REJECTED_FIX)
                    .timed(started),
                    503 => Check::new(
                        "authed",
                        "Authenticated read",
                        State::Warn,
                        format!(
                            "klams is in a maintenance window (503 {}) — reads resume when it closes",
                            code.as_deref().unwrap_or("service_unavailable")
                        ),
                    )
                    .timed(started),
                    other => Check::new(
                        "authed",
                        "Authenticated read",
                        State::Fail,
                        format!(
                            "GET {AUTH_PROBE_PATH} → HTTP {other}{}: {}",
                            code.map(|c| format!(" {c}")).unwrap_or_default(),
                            snippet(&body)
                        ),
                    )
                    .timed(started),
                }
            }
        }
    };
    checks.push(authed);

    // ---- version skew ----------------------------------------------
    let klams_version = snapshot
        .as_ref()
        .and_then(|v| v["version"].as_str())
        .map(str::to_string);
    checks.push(version_check(klams_version.as_deref()));

    let overall = if checks.iter().any(|c| c.state == State::Fail) {
        "down"
    } else if checks.iter().any(|c| c.state == State::Warn) {
        "advisory"
    } else {
        "ok"
    };

    Report {
        overall,
        view: ViewInfo {
            version: env!("CARGO_PKG_VERSION"),
            klams_url: base,
            klams_verified: KLAMS_VERIFIED_VERSION,
            klams_version,
        },
        checks,
    }
}

/// The cheapest authenticated read on the klams surface: one author.
const AUTH_PROBE_PATH: &str = "/v1/authors?limit=1";

const CONFIG_FIX: &str = "set KLAMS_URL to klams' base URL (e.g. http://localhost:7777) in \
     /etc/klams-view/klams-view.env, then `systemctl restart klams-view`";

const IDENTITY_REJECTED_FIX: &str = "add a read-scoped identity row for klams-view to /etc/klams/klams.toml \
     ([[auth.identities]] with agent_name = \"klams-view\", scopes = [\"read\"]) and reload \
     klams (`systemctl reload klams-service`). Nothing is minted and nothing goes in \
     klams-view's env file — the name is the whole credential. Keep klams-view's own name \
     rather than borrowing an agent's, so it shows up as itself in klams' author views.";

fn tcp_fix(port: Option<u16>) -> String {
    format!(
        "is klams listening on port {}? `systemctl status klams-service`, then \
         `ss -lntp | grep {}`. klams binds 127.0.0.1, so a non-local KLAMS_URL needs \
         its `tailscale serve` endpoint.",
        port.map(|p| p.to_string()).unwrap_or_else(|| "?".into()),
        port.map(|p| p.to_string()).unwrap_or_else(|| "7777".into()),
    )
}

const DNS_FIX: &str = "check the host part of KLAMS_URL; for a tailnet name, \
                       `tailscale status` and that tailscaled is running";

/// Compare klams' reported version against the one klams-view's
/// decoders were verified against. klams is `0.1.<sprint>`: patch moves
/// every klams sprint (advisory), major/minor is a semver contract
/// break (down).
fn version_check(reported: Option<&str>) -> Check {
    let expected = KLAMS_VERIFIED_VERSION;
    let Some(reported) = reported else {
        return Check::skipped(
            "version",
            "klams version",
            "the /healthz snapshot was not readable",
        );
    };
    if reported == expected {
        return Check::new(
            "version",
            "klams version",
            State::Ok,
            format!("klams {reported}, the version klams-view is verified against"),
        );
    }
    match (semver3(reported), semver3(expected)) {
        (Some((rma, rmi, rp)), Some((ema, emi, ep))) if (rma, rmi) == (ema, emi) => {
            let direction = if rp > ep { "ahead of" } else { "behind" };
            Check::new(
                "version",
                "klams version",
                State::Warn,
                format!(
                    "klams {reported} is {direction} {expected}, the version klams-view's \
                     /api decoders were last verified against"
                ),
            )
            .with_fix(
                "run `just smoke-live` against this klams; if it passes, bump \
                 KLAMS_VERIFIED_VERSION in src/doctor.rs",
            )
        }
        (Some(_), Some(_)) => Check::new(
            "version",
            "klams version",
            State::Fail,
            format!(
                "klams {reported} differs from {expected} in major/minor — by semver that is \
                 a breaking API change, and /api decoders will fail before the UI explains why"
            ),
        )
        .with_fix("upgrade klams-view (or roll klams back) so the two agree; `just smoke-live` names what broke"),
        _ => Check::new(
            "version",
            "klams version",
            State::Warn,
            format!("klams reports `{reported}`, which is not a semver — cannot compare with {expected}"),
        ),
    }
}

fn semver3(v: &str) -> Option<(u64, u64, u64)> {
    // Tolerate a pre-release/build suffix; only the numeric triple is
    // compared.
    let core = v.split(['-', '+']).next()?;
    let mut it = core.split('.');
    let a = it.next()?.parse().ok()?;
    let b = it.next()?.parse().ok()?;
    let c = it.next().unwrap_or("0").parse().ok()?;
    if it.next().is_some() {
        return None;
    }
    Some((a, b, c))
}

/// Whether a `reqwest` error is a TLS failure rather than a refused or
/// dropped connection. reqwest gives no predicate for this, so the
/// source chain is the only signal available.
fn is_tls_error(e: &reqwest::Error) -> bool {
    let text = chain(e).to_lowercase();
    [
        "tls",
        "certificate",
        "handshake",
        "self-signed",
        "unknown issuer",
    ]
    .iter()
    .any(|needle| text.contains(needle))
}

/// A `reqwest` error's own message says "error sending request"; the
/// cause is always one or two `source()` hops down.
fn chain(e: &dyn std::error::Error) -> String {
    let mut parts = vec![e.to_string()];
    let mut cur = e.source();
    while let Some(c) = cur {
        parts.push(c.to_string());
        cur = c.source();
    }
    parts.join(": ")
}

fn snippet(body: &str) -> String {
    let one_line = body.split_whitespace().collect::<Vec<_>>().join(" ");
    if one_line.len() > 160 {
        format!("{}…", &one_line[..160])
    } else if one_line.is_empty() {
        "(empty body)".into()
    } else {
        one_line
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_versions_are_ok() {
        let c = version_check(Some(KLAMS_VERIFIED_VERSION));
        assert_eq!(c.state, State::Ok);
        assert!(c.fix.is_none());
    }

    #[test]
    fn patch_skew_is_an_advisory_naming_both_versions() {
        // klams releases one patch per sprint, so this is the common
        // case and must not read as an outage.
        let c = version_check(Some("0.1.99"));
        assert_eq!(c.state, State::Warn);
        assert!(c.detail.contains("0.1.99"), "{}", c.detail);
        assert!(c.detail.contains(KLAMS_VERIFIED_VERSION), "{}", c.detail);
        assert!(c.detail.contains("ahead of"), "{}", c.detail);
        assert!(c.fix.as_deref().unwrap().contains("smoke-live"));
    }

    #[test]
    fn older_klams_says_behind_rather_than_ahead() {
        let c = version_check(Some("0.1.1"));
        assert_eq!(c.state, State::Warn);
        assert!(c.detail.contains("behind"), "{}", c.detail);
    }

    #[test]
    fn minor_skew_is_a_failure_not_an_advisory() {
        assert_eq!(version_check(Some("0.2.0")).state, State::Fail);
        assert_eq!(version_check(Some("1.1.45")).state, State::Fail);
    }

    #[test]
    fn unparseable_version_is_an_advisory_not_a_crash() {
        let c = version_check(Some("nightly"));
        assert_eq!(c.state, State::Warn);
        assert!(c.detail.contains("nightly"));
    }

    #[test]
    fn absent_snapshot_skips_rather_than_guesses() {
        let c = version_check(None);
        assert_eq!(c.state, State::Skipped);
    }

    #[test]
    fn semver3_tolerates_suffixes_and_rejects_junk() {
        assert_eq!(semver3("0.1.45"), Some((0, 1, 45)));
        assert_eq!(semver3("1.2.3-rc.1"), Some((1, 2, 3)));
        assert_eq!(semver3("1.2"), Some((1, 2, 0)));
        assert_eq!(semver3("1.2.3.4"), None);
        assert_eq!(semver3("nightly"), None);
    }

    #[test]
    fn snippet_collapses_and_caps_bodies() {
        assert_eq!(snippet("  a\n  b  "), "a b");
        assert_eq!(snippet(""), "(empty body)");
        let long = snippet(&"x".repeat(500));
        assert_eq!(long.chars().count(), 161);
    }
}
