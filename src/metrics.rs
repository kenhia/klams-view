//! Prometheus text parsing and the in-memory metrics history sampler.
//!
//! klams's `/metrics` renders histograms as summaries (`quantile`
//! label, no `_bucket`) — the parser here is a plain line parser that
//! is enough for gauges, counters, and quantile lines.

use serde::Serialize;
use std::collections::{BTreeMap, HashMap, VecDeque};

/// One parsed sample line: labels + value.
#[derive(Debug, Clone)]
pub struct Line {
    pub labels: HashMap<String, String>,
    pub value: f64,
}

/// name -> samples. Untyped; callers know which family they want.
pub type Families = HashMap<String, Vec<Line>>;

pub fn parse(text: &str) -> Families {
    let mut out: Families = HashMap::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (name_labels, value) = match line.rsplit_once(' ') {
            Some(pair) => pair,
            None => continue,
        };
        // A trailing timestamp would make `value` non-numeric; klams
        // doesn't emit them, so a plain parse failure just skips.
        let value: f64 = match value.parse() {
            Ok(v) => v,
            Err(_) => continue,
        };
        let (name, labels) = match name_labels.split_once('{') {
            Some((n, rest)) => (n, parse_labels(rest.trim_end_matches('}'))),
            None => (name_labels, HashMap::new()),
        };
        out.entry(name.to_string())
            .or_default()
            .push(Line { labels, value });
    }
    out
}

fn parse_labels(s: &str) -> HashMap<String, String> {
    // klams label values (agent names, kinds, endpoints) never contain
    // escaped quotes or commas, so split-on-`",` is sufficient.
    let mut labels = HashMap::new();
    for part in s.split("\",") {
        if let Some((k, v)) = part.split_once("=\"") {
            labels.insert(k.trim().to_string(), v.trim_end_matches('"').to_string());
        }
    }
    labels
}

/// Sum every sample of a family, optionally keyed by one label.
pub fn sum_by(fams: &Families, name: &str, label: &str) -> BTreeMap<String, f64> {
    let mut out = BTreeMap::new();
    if let Some(lines) = fams.get(name) {
        for l in lines {
            let key = l.labels.get(label).cloned().unwrap_or_default();
            *out.entry(key).or_insert(0.0) += l.value;
        }
    }
    out
}

pub fn sum_all(fams: &Families, name: &str) -> f64 {
    fams.get(name)
        .map(|ls| ls.iter().map(|l| l.value).sum())
        .unwrap_or(0.0)
}

pub fn gauge(fams: &Families, name: &str) -> Option<f64> {
    fams.get(name).and_then(|ls| ls.first()).map(|l| l.value)
}

/// One quantile of a summary-rendered histogram (e.g. "0.95"),
/// filtered by an extra label when given.
pub fn quantile(fams: &Families, name: &str, q: &str, filter: Option<(&str, &str)>) -> Option<f64> {
    fams.get(name)?.iter().find_map(|l| {
        let matches_q = l.labels.get("quantile").map(String::as_str) == Some(q);
        let matches_f = filter.is_none_or(|(k, v)| l.labels.get(k).map(String::as_str) == Some(v));
        (matches_q && matches_f && l.value.is_finite()).then_some(l.value)
    })
}

/// A point in the sampler's ring buffer. Counters are cumulative —
/// the frontend charts deltas between consecutive samples.
#[derive(Debug, Clone, Serialize)]
pub struct Sample {
    /// Unix seconds.
    pub t: i64,
    pub queue_depth: f64,
    /// Cumulative writes accepted, by type (fact/event/knowledge).
    pub writes_fact: f64,
    pub writes_event: f64,
    pub writes_knowledge: f64,
    /// Cumulative MCP searches across all agents.
    pub mcp_searches: f64,
    /// p95 search retrieval latency, seconds (absent while no traffic).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retrieval_p95: Option<f64>,
}

/// Ring buffer of samples; ~24h at the 60s interval.
pub struct History {
    samples: VecDeque<Sample>,
    cap: usize,
}

impl History {
    pub fn new() -> Self {
        Self {
            samples: VecDeque::new(),
            cap: 1440,
        }
    }

    pub fn push(&mut self, s: Sample) {
        if self.samples.len() == self.cap {
            self.samples.pop_front();
        }
        self.samples.push_back(s);
    }

    pub fn snapshot(&self) -> Vec<Sample> {
        self.samples.iter().cloned().collect()
    }
}

pub fn sample_from(fams: &Families, t: i64) -> Sample {
    let writes = sum_by(fams, "klams_writes_accepted_total", "type");
    Sample {
        t,
        queue_depth: gauge(fams, "klams_queue_depth").unwrap_or(0.0),
        writes_fact: writes.get("fact").copied().unwrap_or(0.0),
        writes_event: writes.get("event").copied().unwrap_or(0.0),
        writes_knowledge: writes.get("knowledge").copied().unwrap_or(0.0),
        mcp_searches: sum_all(fams, "klams_mcp_search_total"),
        retrieval_p95: quantile(
            fams,
            "klams_retrieval_duration_seconds",
            "0.95",
            Some(("op", "search")),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEXT: &str = r#"
# HELP klams_queue_depth queue depth
# TYPE klams_queue_depth gauge
klams_queue_depth 3
klams_writes_accepted_total{type="fact"} 10
klams_writes_accepted_total{type="knowledge"} 32
klams_mcp_writes_total{agent_name="claude",model="fable-5",kind="knowledge"} 7
klams_mcp_writes_total{agent_name="copilot",model="gpt-6",kind="fact"} 2
klams_mcp_search_total{agent_name="claude",model="fable-5"} 41
klams_retrieval_duration_seconds{op="search",transport="mcp",quantile="0.95"} 0.031
"#;

    #[test]
    fn parses_gauges_counters_and_quantiles() {
        let fams = parse(TEXT);
        assert_eq!(gauge(&fams, "klams_queue_depth"), Some(3.0));
        let by_type = sum_by(&fams, "klams_writes_accepted_total", "type");
        assert_eq!(by_type.get("fact"), Some(&10.0));
        assert_eq!(by_type.get("knowledge"), Some(&32.0));
        let by_agent = sum_by(&fams, "klams_mcp_writes_total", "agent_name");
        assert_eq!(by_agent.get("claude"), Some(&7.0));
        assert_eq!(by_agent.get("copilot"), Some(&2.0));
        assert_eq!(sum_all(&fams, "klams_mcp_search_total"), 41.0);
        assert_eq!(
            quantile(
                &fams,
                "klams_retrieval_duration_seconds",
                "0.95",
                Some(("op", "search"))
            ),
            Some(0.031)
        );
    }

    #[test]
    fn history_ring_caps() {
        let mut h = History::new();
        for i in 0..2000 {
            h.push(sample_from(&parse(TEXT), i));
        }
        let snap = h.snapshot();
        assert_eq!(snap.len(), 1440);
        assert_eq!(snap.last().unwrap().t, 1999);
    }
}
