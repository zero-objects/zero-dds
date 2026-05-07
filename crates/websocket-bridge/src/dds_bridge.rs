// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! WebSocket↔DDS-Topic-Bridge.
//!
//! Wire-Format JSON (Text-Frames):
//!
//! Subscribe (Client → Server):
//! ```json
//! {"op": "subscribe", "topic": "Trade", "id": "sub-1"}
//! ```
//!
//! Unsubscribe:
//! ```json
//! {"op": "unsubscribe", "topic": "Trade", "id": "sub-1"}
//! ```
//!
//! Publish (Client → Server):
//! ```json
//! {"op": "publish", "topic": "Trade", "data": {"symbol": "AAPL", "price": 200.5}}
//! ```
//!
//! Notification (Server → Client):
//! ```json
//! {"op": "notify", "topic": "Trade", "data": {...}, "subscription_id": "sub-1"}
//! ```

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// Bridge-Operation aus dem JSON-Frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BridgeOp {
    /// Subscribe to topic.
    Subscribe {
        /// Topic-Name.
        topic: String,
        /// Optional client-side subscription-id (echoed in notifications).
        id: Option<String>,
    },
    /// Unsubscribe.
    Unsubscribe {
        /// Topic-Name.
        topic: String,
        /// Subscription-id.
        id: Option<String>,
    },
    /// Publish a sample.
    Publish {
        /// Topic-Name.
        topic: String,
        /// Raw JSON-encoded sample (Caller-Layer mappt zu IDL-Type).
        data: String,
    },
}

/// Notification an Subscriber.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notification {
    /// Topic-Name.
    pub topic: String,
    /// Sample-JSON.
    pub data: String,
    /// Optional Subscription-Id (echo).
    pub subscription_id: Option<String>,
}

/// Bridge-Fehler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BridgeError {
    /// JSON nicht parsbar.
    BadJson(String),
    /// `op`-Feld fehlt oder unbekannt.
    UnknownOp(String),
    /// `topic`-Feld fehlt oder leer.
    MissingTopic,
    /// Daten-Feld fehlt bei `publish`.
    MissingData,
}

impl core::fmt::Display for BridgeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::BadJson(s) => write!(f, "bad json: {s}"),
            Self::UnknownOp(s) => write!(f, "unknown op: {s}"),
            Self::MissingTopic => f.write_str("missing topic"),
            Self::MissingData => f.write_str("missing data"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for BridgeError {}

/// Minimal-JSON-Parser fuer das Bridge-Wire-Format. Erkennt Top-Level-
/// Objekt mit String/Object-Werten — keinen vollen JSON-Stack
/// (Nested-Objekte landen als Roh-String im `data`-Feld).
fn parse_top_level_object(input: &str) -> Result<BTreeMap<String, String>, BridgeError> {
    let s = input.trim();
    let s = s
        .strip_prefix('{')
        .ok_or_else(|| BridgeError::BadJson("expected `{`".into()))?;
    let s = s
        .strip_suffix('}')
        .ok_or_else(|| BridgeError::BadJson("expected `}`".into()))?;
    let mut out = BTreeMap::new();
    let mut chars = s.char_indices().peekable();
    while let Some((_, c)) = chars.peek().copied() {
        if c.is_whitespace() || c == ',' {
            chars.next();
            continue;
        }
        let key = parse_json_string(&mut chars, s)?;
        // expect ':'
        skip_ws_to(&mut chars);
        match chars.next() {
            Some((_, ':')) => {}
            _ => return Err(BridgeError::BadJson("expected `:`".into())),
        }
        skip_ws_to(&mut chars);
        let value = parse_json_value(&mut chars, s)?;
        out.insert(key, value);
    }
    Ok(out)
}

fn skip_ws_to(chars: &mut core::iter::Peekable<core::str::CharIndices<'_>>) {
    while let Some((_, c)) = chars.peek().copied() {
        if c.is_whitespace() {
            chars.next();
        } else {
            break;
        }
    }
}

fn parse_json_string(
    chars: &mut core::iter::Peekable<core::str::CharIndices<'_>>,
    src: &str,
) -> Result<String, BridgeError> {
    skip_ws_to(chars);
    match chars.next() {
        Some((_, '"')) => {}
        _ => return Err(BridgeError::BadJson("expected `\"`".into())),
    }
    let start = chars
        .peek()
        .map(|(i, _)| *i)
        .ok_or_else(|| BridgeError::BadJson("eof".into()))?;
    let mut end = start;
    while let Some((i, c)) = chars.next() {
        if c == '"' {
            return Ok(src[start..i].to_string());
        }
        if c == '\\' {
            chars.next(); // skip escaped char
        }
        end = i + c.len_utf8();
    }
    let _ = end;
    Err(BridgeError::BadJson("unterminated string".into()))
}

fn parse_json_value(
    chars: &mut core::iter::Peekable<core::str::CharIndices<'_>>,
    src: &str,
) -> Result<String, BridgeError> {
    skip_ws_to(chars);
    match chars.peek().map(|(_, c)| *c) {
        Some('"') => parse_json_string(chars, src),
        Some('{') => parse_json_object_to_string(chars, src),
        Some('[') => parse_json_array_to_string(chars, src),
        _ => parse_json_scalar(chars, src),
    }
}

fn parse_json_object_to_string(
    chars: &mut core::iter::Peekable<core::str::CharIndices<'_>>,
    src: &str,
) -> Result<String, BridgeError> {
    let start = chars
        .peek()
        .map(|(i, _)| *i)
        .ok_or_else(|| BridgeError::BadJson("eof".into()))?;
    let mut depth = 0i32;
    while let Some((i, c)) = chars.next() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Ok(src[start..i + 1].to_string());
                }
            }
            '"' => {
                // skip whole string
                while let Some((_, sc)) = chars.next() {
                    if sc == '"' {
                        break;
                    }
                    if sc == '\\' {
                        chars.next();
                    }
                }
            }
            _ => {}
        }
    }
    Err(BridgeError::BadJson("unterminated object".into()))
}

fn parse_json_array_to_string(
    chars: &mut core::iter::Peekable<core::str::CharIndices<'_>>,
    src: &str,
) -> Result<String, BridgeError> {
    let start = chars
        .peek()
        .map(|(i, _)| *i)
        .ok_or_else(|| BridgeError::BadJson("eof".into()))?;
    let mut depth = 0i32;
    while let Some((i, c)) = chars.next() {
        match c {
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    return Ok(src[start..i + 1].to_string());
                }
            }
            '"' => {
                while let Some((_, sc)) = chars.next() {
                    if sc == '"' {
                        break;
                    }
                    if sc == '\\' {
                        chars.next();
                    }
                }
            }
            _ => {}
        }
    }
    Err(BridgeError::BadJson("unterminated array".into()))
}

fn parse_json_scalar(
    chars: &mut core::iter::Peekable<core::str::CharIndices<'_>>,
    src: &str,
) -> Result<String, BridgeError> {
    let start = chars
        .peek()
        .map(|(i, _)| *i)
        .ok_or_else(|| BridgeError::BadJson("eof".into()))?;
    let mut end = start;
    while let Some((i, c)) = chars.peek().copied() {
        if c == ',' || c == '}' || c.is_whitespace() {
            break;
        }
        end = i + c.len_utf8();
        chars.next();
    }
    Ok(src[start..end].to_string())
}

/// Parst ein WebSocket-Text-Frame in eine Bridge-Operation.
///
/// # Errors
/// Siehe [`BridgeError`].
pub fn parse_op(text: &str) -> Result<BridgeOp, BridgeError> {
    let map = parse_top_level_object(text)?;
    let op = map
        .get("op")
        .ok_or_else(|| BridgeError::UnknownOp("(missing)".into()))?;
    let topic = map
        .get("topic")
        .filter(|s| !s.is_empty())
        .ok_or(BridgeError::MissingTopic)?
        .clone();
    let id = map.get("id").cloned();
    match op.as_str() {
        "subscribe" => Ok(BridgeOp::Subscribe { topic, id }),
        "unsubscribe" => Ok(BridgeOp::Unsubscribe { topic, id }),
        "publish" => {
            let data = map.get("data").ok_or(BridgeError::MissingData)?.clone();
            Ok(BridgeOp::Publish { topic, data })
        }
        other => Err(BridgeError::UnknownOp(other.to_string())),
    }
}

/// Render eine Notification zu einem JSON-Text-Frame.
#[must_use]
pub fn render_notification(n: &Notification) -> String {
    let mut s = alloc::format!(
        "{{\"op\":\"notify\",\"topic\":\"{}\",\"data\":{}",
        json_escape(&n.topic),
        n.data
    );
    if let Some(id) = &n.subscription_id {
        s.push_str(&alloc::format!(
            ",\"subscription_id\":\"{}\"",
            json_escape(id)
        ));
    }
    s.push('}');
    s
}

fn json_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Subscription-Registry. Pro WebSocket-Connection ein Set von Topics.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SubscriptionRegistry {
    by_connection: BTreeMap<u64, BTreeMap<String, Option<String>>>,
}

impl SubscriptionRegistry {
    /// Konstruktor.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Subscribe.
    pub fn subscribe(&mut self, conn_id: u64, topic: String, sub_id: Option<String>) {
        self.by_connection
            .entry(conn_id)
            .or_default()
            .insert(topic, sub_id);
    }

    /// Unsubscribe.
    pub fn unsubscribe(&mut self, conn_id: u64, topic: &str) -> bool {
        self.by_connection
            .get_mut(&conn_id)
            .map(|set| set.remove(topic).is_some())
            .unwrap_or(false)
    }

    /// Connection schliessen — entfernt alle ihre Subscriptions.
    pub fn drop_connection(&mut self, conn_id: u64) {
        self.by_connection.remove(&conn_id);
    }

    /// Liste aller Connections, die einen Topic abonniert haben.
    /// Liefert `(conn_id, optional sub_id)`-Tupel.
    #[must_use]
    pub fn subscribers_of(&self, topic: &str) -> Vec<(u64, Option<String>)> {
        let mut out = Vec::new();
        for (&cid, subs) in &self.by_connection {
            if let Some(sub_id) = subs.get(topic) {
                out.push((cid, sub_id.clone()));
            }
        }
        out
    }

    /// Anzahl Connections.
    #[must_use]
    pub fn connection_count(&self) -> usize {
        self.by_connection.len()
    }

    /// Anzahl Subscriptions ueber alle Connections.
    #[must_use]
    pub fn subscription_count(&self) -> usize {
        self.by_connection.values().map(BTreeMap::len).sum()
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn parse_subscribe_frame() {
        let r = parse_op(r#"{"op":"subscribe","topic":"T","id":"s1"}"#).unwrap();
        assert!(matches!(r, BridgeOp::Subscribe { .. }));
        if let BridgeOp::Subscribe { topic, id } = r {
            assert_eq!(topic, "T");
            assert_eq!(id, Some("s1".into()));
        }
    }

    #[test]
    fn parse_unsubscribe_frame() {
        let r = parse_op(r#"{"op":"unsubscribe","topic":"T"}"#).unwrap();
        assert!(matches!(r, BridgeOp::Unsubscribe { .. }));
    }

    #[test]
    fn parse_publish_frame_with_object_data() {
        let r = parse_op(r#"{"op":"publish","topic":"T","data":{"x":1,"y":"z"}}"#).unwrap();
        if let BridgeOp::Publish { data, .. } = r {
            assert_eq!(data, r#"{"x":1,"y":"z"}"#);
        } else {
            panic!("expected publish");
        }
    }

    #[test]
    fn parse_publish_with_array_data() {
        let r = parse_op(r#"{"op":"publish","topic":"T","data":[1,2,3]}"#).unwrap();
        if let BridgeOp::Publish { data, .. } = r {
            assert_eq!(data, "[1,2,3]");
        } else {
            panic!("expected publish");
        }
    }

    #[test]
    fn missing_topic_rejected() {
        assert_eq!(
            parse_op(r#"{"op":"subscribe"}"#),
            Err(BridgeError::MissingTopic)
        );
    }

    #[test]
    fn unknown_op_rejected() {
        assert!(matches!(
            parse_op(r#"{"op":"explode","topic":"T"}"#),
            Err(BridgeError::UnknownOp(_))
        ));
    }

    #[test]
    fn missing_data_in_publish_rejected() {
        assert_eq!(
            parse_op(r#"{"op":"publish","topic":"T"}"#),
            Err(BridgeError::MissingData)
        );
    }

    #[test]
    fn render_notification_round_trip() {
        let n = Notification {
            topic: "T".into(),
            data: r#"{"x":1}"#.into(),
            subscription_id: Some("s1".into()),
        };
        let s = render_notification(&n);
        assert!(s.contains(r#""op":"notify""#));
        assert!(s.contains(r#""topic":"T""#));
        assert!(s.contains(r#""data":{"x":1}"#));
        assert!(s.contains(r#""subscription_id":"s1""#));
    }

    #[test]
    fn registry_subscribe_unsubscribe_round_trip() {
        let mut r = SubscriptionRegistry::new();
        r.subscribe(1, "Trade".into(), Some("s1".into()));
        r.subscribe(2, "Trade".into(), None);
        r.subscribe(1, "Quote".into(), None);
        assert_eq!(r.subscription_count(), 3);
        let subs = r.subscribers_of("Trade");
        assert_eq!(subs.len(), 2);
        assert!(r.unsubscribe(1, "Trade"));
        assert_eq!(r.subscribers_of("Trade").len(), 1);
    }

    #[test]
    fn drop_connection_removes_all_subs() {
        let mut r = SubscriptionRegistry::new();
        r.subscribe(1, "A".into(), None);
        r.subscribe(1, "B".into(), None);
        r.drop_connection(1);
        assert_eq!(r.subscription_count(), 0);
    }

    #[test]
    fn unsubscribe_unknown_returns_false() {
        let mut r = SubscriptionRegistry::new();
        assert!(!r.unsubscribe(1, "X"));
    }

    #[test]
    fn json_escape_handles_quote_and_backslash() {
        assert_eq!(json_escape(r#"a"b\c"#), r#"a\"b\\c"#);
    }
}
