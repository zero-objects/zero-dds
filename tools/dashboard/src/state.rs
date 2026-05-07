// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Daten-Modell des Dashboards. Konsumenten fuellen es aus DcpsRuntime,
//! der Server serialisiert es zu JSON.

use std::sync::Mutex;

/// Live-State des Dashboards. Mutex-geschuetzt, der Server liest
/// und Konsumenten schreiben.
pub struct DashboardState {
    inner: Mutex<Inner>,
}

#[derive(Default)]
struct Inner {
    topics: Vec<TopicInfo>,
    participants: Vec<ParticipantInfo>,
    histograms: Vec<HistogramSnapshot>,
    edges: Vec<DiscoveryEdge>,
    recording: RecordingStatus,
}

/// Eintrag in der Topic-Liste.
#[derive(Clone, Debug, PartialEq)]
pub struct TopicInfo {
    /// Topic-Name (DDS-Wire-Form, ggf. mit `rt/`-Prefix).
    pub name: String,
    /// Type-Name.
    pub type_name: String,
    /// Anzahl matched Publisher.
    pub publishers: u32,
    /// Anzahl matched Subscriber.
    pub subscribers: u32,
    /// Letzte gemessene Sample-Rate (samples/s, EMA).
    pub sample_rate_hz: f64,
}

/// Participant-Info.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParticipantInfo {
    /// 16-Byte GUID-Prefix als Hex.
    pub guid_prefix_hex: String,
    /// Logischer Name.
    pub name: String,
    /// Domain-ID.
    pub domain_id: u32,
    /// Vendor-ID-Hex (z.B. "01.0F" fuer ZeroDDS).
    pub vendor_id_hex: String,
}

/// Histogram-Snapshot fuer das Dashboard.
#[derive(Clone, Debug)]
pub struct HistogramSnapshot {
    /// Logischer Name.
    pub name: String,
    /// Anzahl aller Records.
    pub count: u64,
    /// Mittelwert (ns).
    pub mean_ns: u64,
    /// Min/Max.
    pub min_ns: u64,
    /// Max-Wert (ns).
    pub max_ns: u64,
    /// p50/p99 — Aufrufer berechnet (z.B. aus hdrhistogram).
    pub p50_ns: u64,
    /// p99 als ns.
    pub p99_ns: u64,
}

/// Edge im Discovery-Graph: Pub→Sub-Match auf einem Topic.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscoveryEdge {
    /// GUID-Hex des Pub-Endpoints.
    pub from_guid: String,
    /// GUID-Hex des Sub-Endpoints.
    pub to_guid: String,
    /// Topic-Name.
    pub topic: String,
}

/// Knoten im Discovery-Graph (Endpoint).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscoveryNode {
    /// GUID-Hex.
    pub guid: String,
    /// Display-Name.
    pub label: String,
    /// "publisher" | "subscriber" | "participant".
    pub kind: String,
}

/// Recording-Status.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RecordingStatus {
    /// `true` wenn aktuell aufgezeichnet wird.
    pub active: bool,
    /// Pfad der Output-Datei (wenn aktiv).
    pub output_path: Option<String>,
    /// Anzahl bisher geschriebener Frames.
    pub frames: u64,
}

impl DashboardState {
    /// Leerer State.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(Inner::default()),
        }
    }

    /// Setzt die komplette Topic-Liste (ueberschreibt).
    pub fn set_topics(&self, topics: Vec<TopicInfo>) {
        if let Ok(mut g) = self.inner.lock() {
            g.topics = topics;
        }
    }

    /// Setzt die Participant-Liste.
    pub fn set_participants(&self, ps: Vec<ParticipantInfo>) {
        if let Ok(mut g) = self.inner.lock() {
            g.participants = ps;
        }
    }

    /// Setzt die Histogramme.
    pub fn set_histograms(&self, hs: Vec<HistogramSnapshot>) {
        if let Ok(mut g) = self.inner.lock() {
            g.histograms = hs;
        }
    }

    /// Setzt die Discovery-Edges.
    pub fn set_edges(&self, e: Vec<DiscoveryEdge>) {
        if let Ok(mut g) = self.inner.lock() {
            g.edges = e;
        }
    }

    /// Setzt den Recording-Status.
    pub fn set_recording(&self, r: RecordingStatus) {
        if let Ok(mut g) = self.inner.lock() {
            g.recording = r;
        }
    }

    /// Phase-B: Inject ueber JSON-Body (POST /api/inject/topics).
    /// Erwartet das gleiche Schema wie die GET-Antwort.
    pub fn inject_topics_json(&self, body: &str) -> Result<usize, String> {
        let v = parse_json(body)?;
        let arr = v.as_array().ok_or("topics must be array")?;
        let mut topics = Vec::with_capacity(arr.len());
        for o in arr {
            topics.push(TopicInfo {
                name: o.get_str("name").unwrap_or("").into(),
                type_name: o.get_str("type_name").unwrap_or("").into(),
                publishers: o.get_u32("publishers").unwrap_or(0),
                subscribers: o.get_u32("subscribers").unwrap_or(0),
                sample_rate_hz: o.get_f64("sample_rate_hz").unwrap_or(0.0),
            });
        }
        let n = topics.len();
        self.set_topics(topics);
        Ok(n)
    }

    /// Phase-B: Inject von Participants.
    pub fn inject_participants_json(&self, body: &str) -> Result<usize, String> {
        let v = parse_json(body)?;
        let arr = v.as_array().ok_or("participants must be array")?;
        let mut ps = Vec::with_capacity(arr.len());
        for o in arr {
            ps.push(ParticipantInfo {
                guid_prefix_hex: o.get_str("guid_prefix_hex").unwrap_or("").into(),
                name: o.get_str("name").unwrap_or("").into(),
                domain_id: o.get_u32("domain_id").unwrap_or(0),
                vendor_id_hex: o.get_str("vendor_id_hex").unwrap_or("").into(),
            });
        }
        let n = ps.len();
        self.set_participants(ps);
        Ok(n)
    }

    /// Phase-B: Inject von Histograms.
    pub fn inject_histograms_json(&self, body: &str) -> Result<usize, String> {
        let v = parse_json(body)?;
        let arr = v.as_array().ok_or("histograms must be array")?;
        let mut hs = Vec::with_capacity(arr.len());
        for o in arr {
            hs.push(HistogramSnapshot {
                name: o.get_str("name").unwrap_or("").into(),
                count: o.get_u64("count").unwrap_or(0),
                mean_ns: o.get_u64("mean_ns").unwrap_or(0),
                min_ns: o.get_u64("min_ns").unwrap_or(0),
                max_ns: o.get_u64("max_ns").unwrap_or(0),
                p50_ns: o.get_u64("p50_ns").unwrap_or(0),
                p99_ns: o.get_u64("p99_ns").unwrap_or(0),
            });
        }
        let n = hs.len();
        self.set_histograms(hs);
        Ok(n)
    }

    /// Liest topics-JSON.
    #[must_use]
    pub fn topics_json(&self) -> String {
        let Ok(g) = self.inner.lock() else {
            return "[]".into();
        };
        let mut out = String::from("[");
        for (i, t) in g.topics.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            out.push_str(r#"{"name":""#);
            json_escape(&mut out, &t.name);
            out.push_str(r#"","type_name":""#);
            json_escape(&mut out, &t.type_name);
            out.push_str(&format!(
                r#"","publishers":{},"subscribers":{},"sample_rate_hz":{:.3}}}"#,
                t.publishers, t.subscribers, t.sample_rate_hz
            ));
        }
        out.push(']');
        out
    }

    /// Liest participants-JSON.
    #[must_use]
    pub fn participants_json(&self) -> String {
        let Ok(g) = self.inner.lock() else {
            return "[]".into();
        };
        let mut out = String::from("[");
        for (i, p) in g.participants.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            out.push_str(r#"{"guid_prefix_hex":""#);
            json_escape(&mut out, &p.guid_prefix_hex);
            out.push_str(r#"","name":""#);
            json_escape(&mut out, &p.name);
            out.push_str(r#"","vendor_id_hex":""#);
            json_escape(&mut out, &p.vendor_id_hex);
            out.push_str(&format!(r#"","domain_id":{}}}"#, p.domain_id));
        }
        out.push(']');
        out
    }

    /// Liest histograms-JSON.
    #[must_use]
    pub fn histograms_json(&self) -> String {
        let Ok(g) = self.inner.lock() else {
            return "[]".into();
        };
        let mut out = String::from("[");
        for (i, h) in g.histograms.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            out.push_str(r#"{"name":""#);
            json_escape(&mut out, &h.name);
            out.push_str(&format!(
                r#"","count":{},"mean_ns":{},"min_ns":{},"max_ns":{},"p50_ns":{},"p99_ns":{}}}"#,
                h.count, h.mean_ns, h.min_ns, h.max_ns, h.p50_ns, h.p99_ns
            ));
        }
        out.push(']');
        out
    }

    /// Liest discovery-graph-JSON.
    #[must_use]
    pub fn graph_json(&self) -> String {
        let Ok(g) = self.inner.lock() else {
            return r#"{"nodes":[],"edges":[]}"#.into();
        };
        // Nodes ergeben sich aus Participants (immer).
        let mut nodes = String::from(r#"{"nodes":["#);
        for (i, p) in g.participants.iter().enumerate() {
            if i > 0 {
                nodes.push(',');
            }
            nodes.push_str(r#"{"guid":""#);
            json_escape(&mut nodes, &p.guid_prefix_hex);
            nodes.push_str(r#"","label":""#);
            json_escape(&mut nodes, &p.name);
            nodes.push_str(r#"","kind":"participant"}"#);
        }
        nodes.push_str(r#"],"edges":["#);
        for (i, e) in g.edges.iter().enumerate() {
            if i > 0 {
                nodes.push(',');
            }
            nodes.push_str(r#"{"from_guid":""#);
            json_escape(&mut nodes, &e.from_guid);
            nodes.push_str(r#"","to_guid":""#);
            json_escape(&mut nodes, &e.to_guid);
            nodes.push_str(r#"","topic":""#);
            json_escape(&mut nodes, &e.topic);
            nodes.push_str(r#""}"#);
        }
        nodes.push_str("]}");
        nodes
    }

    /// Liest recording-JSON.
    #[must_use]
    pub fn recording_json(&self) -> String {
        let Ok(g) = self.inner.lock() else {
            return r#"{"active":false,"frames":0}"#.into();
        };
        let path = match &g.recording.output_path {
            Some(p) => format!(r#""{}""#, escape_value(p)),
            None => "null".into(),
        };
        format!(
            r#"{{"active":{},"output_path":{},"frames":{}}}"#,
            g.recording.active, path, g.recording.frames
        )
    }
}

impl Default for DashboardState {
    fn default() -> Self {
        Self::new()
    }
}

// ---- Mini-JSON-Parser (kein serde-Dep) ----
//
// Reicht fuer das Inject-Schema: Array of Objects mit String/Num-
// Feldern. Identisch zur Logik in tools/interop-matrix/src/parser.rs,
// hier inline weil wir einen separaten Top-Level-Use-Case haben.

#[derive(Clone, Debug)]
#[allow(dead_code)] // Bool/Null werden parsed aber nicht weiter inspiziert.
enum Val {
    Str(String),
    Num(String),
    Bool(bool),
    Null,
    Array(Vec<Val>),
    Object(Vec<(String, Val)>),
}

impl Val {
    fn as_array(&self) -> Option<&[Val]> {
        if let Self::Array(a) = self {
            Some(a)
        } else {
            None
        }
    }
    fn as_object(&self) -> Option<&[(String, Val)]> {
        if let Self::Object(o) = self {
            Some(o)
        } else {
            None
        }
    }
    fn get_str(&self, k: &str) -> Option<&str> {
        let kv = self.as_object()?.iter().find(|(n, _)| n == k)?;
        if let Val::Str(s) = &kv.1 {
            Some(s)
        } else {
            None
        }
    }
    fn get_u32(&self, k: &str) -> Option<u32> {
        let kv = self.as_object()?.iter().find(|(n, _)| n == k)?;
        if let Val::Num(s) = &kv.1 {
            s.parse().ok()
        } else {
            None
        }
    }
    fn get_u64(&self, k: &str) -> Option<u64> {
        let kv = self.as_object()?.iter().find(|(n, _)| n == k)?;
        if let Val::Num(s) = &kv.1 {
            s.parse().ok()
        } else {
            None
        }
    }
    fn get_f64(&self, k: &str) -> Option<f64> {
        let kv = self.as_object()?.iter().find(|(n, _)| n == k)?;
        if let Val::Num(s) = &kv.1 {
            s.parse().ok()
        } else {
            None
        }
    }
}

fn parse_json(body: &str) -> Result<Val, String> {
    let mut p = JsonParser {
        s: body.as_bytes(),
        i: 0,
    };
    p.skip_ws();
    p.parse_value()
}

struct JsonParser<'a> {
    s: &'a [u8],
    i: usize,
}

impl JsonParser<'_> {
    fn skip_ws(&mut self) {
        while self.i < self.s.len() && self.s[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
    }
    fn parse_value(&mut self) -> Result<Val, String> {
        self.skip_ws();
        if self.i >= self.s.len() {
            return Err("unexpected eof".into());
        }
        match self.s[self.i] {
            b'{' => self.parse_object(),
            b'[' => self.parse_array(),
            b'"' => self.parse_string().map(Val::Str),
            b't' | b'f' => self.parse_bool(),
            b'n' => self.parse_null(),
            b'-' | b'0'..=b'9' => self.parse_num(),
            other => Err(format!("unexpected byte 0x{other:02x} at {}", self.i)),
        }
    }
    fn parse_object(&mut self) -> Result<Val, String> {
        self.i += 1; // {
        self.skip_ws();
        let mut out = Vec::new();
        if self.i < self.s.len() && self.s[self.i] == b'}' {
            self.i += 1;
            return Ok(Val::Object(out));
        }
        loop {
            self.skip_ws();
            let key = self.parse_string()?;
            self.skip_ws();
            if self.i >= self.s.len() || self.s[self.i] != b':' {
                return Err("expected ':'".into());
            }
            self.i += 1;
            let val = self.parse_value()?;
            out.push((key, val));
            self.skip_ws();
            if self.i < self.s.len() && self.s[self.i] == b',' {
                self.i += 1;
                continue;
            }
            if self.i < self.s.len() && self.s[self.i] == b'}' {
                self.i += 1;
                return Ok(Val::Object(out));
            }
            return Err("expected ',' or '}'".into());
        }
    }
    fn parse_array(&mut self) -> Result<Val, String> {
        self.i += 1;
        self.skip_ws();
        let mut out = Vec::new();
        if self.i < self.s.len() && self.s[self.i] == b']' {
            self.i += 1;
            return Ok(Val::Array(out));
        }
        loop {
            out.push(self.parse_value()?);
            self.skip_ws();
            if self.i < self.s.len() && self.s[self.i] == b',' {
                self.i += 1;
                continue;
            }
            if self.i < self.s.len() && self.s[self.i] == b']' {
                self.i += 1;
                return Ok(Val::Array(out));
            }
            return Err("expected ',' or ']'".into());
        }
    }
    fn parse_string(&mut self) -> Result<String, String> {
        if self.s[self.i] != b'"' {
            return Err("expected string".into());
        }
        self.i += 1;
        let mut out = String::new();
        while self.i < self.s.len() {
            let c = self.s[self.i];
            if c == b'"' {
                self.i += 1;
                return Ok(out);
            }
            if c == b'\\' && self.i + 1 < self.s.len() {
                let esc = self.s[self.i + 1];
                self.i += 2;
                match esc {
                    b'"' => out.push('"'),
                    b'\\' => out.push('\\'),
                    b'/' => out.push('/'),
                    b'n' => out.push('\n'),
                    b't' => out.push('\t'),
                    b'r' => out.push('\r'),
                    other => out.push(other as char),
                }
            } else {
                out.push(c as char);
                self.i += 1;
            }
        }
        Err("unterminated string".into())
    }
    fn parse_bool(&mut self) -> Result<Val, String> {
        if self.s[self.i..].starts_with(b"true") {
            self.i += 4;
            Ok(Val::Bool(true))
        } else if self.s[self.i..].starts_with(b"false") {
            self.i += 5;
            Ok(Val::Bool(false))
        } else {
            Err("expected bool".into())
        }
    }
    fn parse_null(&mut self) -> Result<Val, String> {
        if self.s[self.i..].starts_with(b"null") {
            self.i += 4;
            Ok(Val::Null)
        } else {
            Err("expected null".into())
        }
    }
    fn parse_num(&mut self) -> Result<Val, String> {
        let start = self.i;
        if self.s[self.i] == b'-' {
            self.i += 1;
        }
        while self.i < self.s.len() {
            let c = self.s[self.i];
            if c.is_ascii_digit() || c == b'.' || c == b'e' || c == b'E' || c == b'+' || c == b'-' {
                self.i += 1;
            } else {
                break;
            }
        }
        Ok(Val::Num(
            String::from_utf8_lossy(&self.s[start..self.i]).into_owned(),
        ))
    }
}

fn json_escape(out: &mut String, s: &str) {
    for c in s.chars() {
        match c {
            '"' => out.push_str(r#"\""#),
            '\\' => out.push_str(r"\\"),
            '\n' => out.push_str(r"\n"),
            '\r' => out.push_str(r"\r"),
            '\t' => out.push_str(r"\t"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
}

fn escape_value(s: &str) -> String {
    let mut out = String::new();
    json_escape(&mut out, s);
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // tests duerfen unwrap nutzen.
mod tests {
    use super::*;

    #[test]
    fn empty_state_topics_is_empty_array() {
        let s = DashboardState::new();
        assert_eq!(s.topics_json(), "[]");
        assert_eq!(s.participants_json(), "[]");
        assert_eq!(s.histograms_json(), "[]");
        assert_eq!(s.graph_json(), r#"{"nodes":[],"edges":[]}"#);
    }

    #[test]
    fn topics_serializes_fields() {
        let s = DashboardState::new();
        s.set_topics(vec![TopicInfo {
            name: "/chatter".into(),
            type_name: "std_msgs::msg::String".into(),
            publishers: 1,
            subscribers: 2,
            sample_rate_hz: 10.5,
        }]);
        let j = s.topics_json();
        assert!(j.contains(r#""name":"/chatter""#));
        assert!(j.contains(r#""publishers":1"#));
        assert!(j.contains(r#""subscribers":2"#));
        assert!(j.contains(r#""sample_rate_hz":10.500"#));
    }

    #[test]
    fn participants_serializes_guid() {
        let s = DashboardState::new();
        s.set_participants(vec![ParticipantInfo {
            guid_prefix_hex: "010203040506070809".into(),
            name: "talker".into(),
            domain_id: 0,
            vendor_id_hex: "01.0F".into(),
        }]);
        let j = s.participants_json();
        assert!(j.contains(r#""guid_prefix_hex":"010203040506070809""#));
        assert!(j.contains(r#""domain_id":0"#));
    }

    #[test]
    fn graph_combines_nodes_and_edges() {
        let s = DashboardState::new();
        s.set_participants(vec![
            ParticipantInfo {
                guid_prefix_hex: "a".into(),
                name: "talker".into(),
                domain_id: 0,
                vendor_id_hex: "x".into(),
            },
            ParticipantInfo {
                guid_prefix_hex: "b".into(),
                name: "listener".into(),
                domain_id: 0,
                vendor_id_hex: "x".into(),
            },
        ]);
        s.set_edges(vec![DiscoveryEdge {
            from_guid: "a".into(),
            to_guid: "b".into(),
            topic: "/chatter".into(),
        }]);
        let j = s.graph_json();
        assert!(j.contains(r#""label":"talker""#));
        assert!(j.contains(r#""kind":"participant""#));
        assert!(j.contains(r#""topic":"/chatter""#));
    }

    #[test]
    fn json_escape_handles_special_chars() {
        let mut s = String::new();
        json_escape(&mut s, "a\"b\nc");
        assert_eq!(s, r#"a\"b\nc"#);
    }

    #[test]
    fn recording_status_json() {
        let s = DashboardState::new();
        s.set_recording(RecordingStatus {
            active: true,
            output_path: Some("/tmp/x.zddsrec".into()),
            frames: 42,
        });
        let j = s.recording_json();
        assert!(j.contains(r#""active":true"#));
        assert!(j.contains(r#""output_path":"/tmp/x.zddsrec""#));
        assert!(j.contains(r#""frames":42"#));
    }
}
