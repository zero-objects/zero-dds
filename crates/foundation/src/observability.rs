// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Observability — structured DDS events for tracing and metrics.
//!
//! Provides a ZeroDDS-specific event vocabulary plus a lean `Sink`
//! trait through which consumers can tap events.
//!
//! ## Design goals
//!
//! 1. **Zero-Overhead by Default**: without a sink, no event object is
//!    constructed at all (`with_sink(...)` is the opt-in point).
//! 2. **Sync, allocation-light**: `Sink::record(&Event)` takes events by
//!    `&`; each sink decides for itself whether to clone/serialize.
//! 3. **Production-ready**: the bundled [`StderrJsonSink`] writes
//!    JSON lines to stderr — directly consumable by
//!    Vector/fluentd/Datadog/Loki/journald.
//! 4. **OTel bridge later**: a separate `dds-otel` crate (or a
//!    `tracing-opentelemetry` adapter in the consumer) can implement
//!    this sink trait and ship events as OTLP spans.
//!
//! ## Event model
//!
//! Events are coarse-grained: one event per endpoint lifecycle action
//! or per sample-path phase. In the hot path (e.g. per-sample latency)
//! we use **no** events — instead the atomic stats from D.4 Phase A.
//! Events are for coarse-grained telemetry, not for p99 latency
//! sampling.

#[cfg(feature = "alloc")]
use alloc::string::String;
#[cfg(feature = "alloc")]
use alloc::sync::Arc;
#[cfg(feature = "alloc")]
use alloc::vec::Vec;

#[cfg(feature = "std")]
use std::io::{self, Write};
#[cfg(feature = "std")]
use std::sync::Mutex;

/// Severity of an event. Modeled on OTel/Syslog levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Level {
    /// Normal lifecycle event (endpoint create/destroy, match).
    Info,
    /// Indication of an abnormal but non-fatal situation
    /// (discovery timeout, single drop).
    Warn,
    /// Functionally failed operation.
    Error,
}

impl Level {
    /// Lowercase spelling conforming to JSON/logfile conventions.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warn => "warn",
            Self::Error => "error",
        }
    }
}

/// Event source. Identifies the layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Component {
    /// DCPS / domain-participant path.
    Dcps,
    /// SPDP/SEDP discovery.
    Discovery,
    /// RTPS wire / reader / writer.
    Rtps,
    /// Security plugins.
    Security,
    /// Transport layer (UDP/TCP/SHM).
    Transport,
    /// User-defined sub-system (Bridges, Tools).
    User,
}

impl Component {
    /// Machine-readable label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Dcps => "dcps",
            Self::Discovery => "discovery",
            Self::Rtps => "rtps",
            Self::Security => "security",
            Self::Transport => "transport",
            Self::User => "user",
        }
    }
}

/// Structured key-value attribute. `value` is held as a String
/// — the sink decides whether/how it serializes typed.
#[cfg(feature = "alloc")]
#[derive(Debug, Clone)]
pub struct Attribute {
    /// Stable key (kebab-case recommended).
    pub key: &'static str,
    /// String value.
    pub value: String,
}

/// Event record. Produced by the DCPS runtime and plugins.
#[cfg(feature = "alloc")]
#[derive(Debug, Clone)]
pub struct Event {
    /// Severity.
    pub level: Level,
    /// Originating component.
    pub component: Component,
    /// Stable event name in `domain.event` form (e.g.
    /// `dcps.user_writer.created`, `discovery.peer.matched`).
    pub name: &'static str,
    /// Optional structured attributes.
    pub attrs: Vec<Attribute>,
}

#[cfg(feature = "alloc")]
impl Event {
    /// Constructs a new event without attributes.
    #[must_use]
    pub fn new(level: Level, component: Component, name: &'static str) -> Self {
        Self {
            level,
            component,
            name,
            attrs: Vec::new(),
        }
    }

    /// Builder form: append an attribute.
    #[must_use]
    pub fn with_attr(mut self, key: &'static str, value: impl Into<String>) -> Self {
        self.attrs.push(Attribute {
            key,
            value: value.into(),
        });
        self
    }
}

/// Sink trait: consumers implement `record` and decide where the
/// event goes (stderr, OTLP, prometheus, /dev/null).
#[cfg(feature = "alloc")]
pub trait Sink: Send + Sync {
    /// Processes an event. **Synchronous.** Sinks may write,
    /// buffer or drop — the caller waits, so please don't block
    /// (e.g. no synchronous HTTP POST from the hot path).
    fn record(&self, event: &Event);
}

/// No-op sink. The default choice when no telemetry is configured —
/// every `record` call is an immediate return.
#[cfg(feature = "alloc")]
#[derive(Debug, Clone, Copy)]
pub struct NullSink;

#[cfg(feature = "alloc")]
impl Sink for NullSink {
    fn record(&self, _event: &Event) {}
}

/// Stderr sink: writes each event as a JSON line to stderr.
/// Suited for Docker/k8s/journald pipelines with downstream
/// log sinks (Vector/fluentd/Datadog agent/Loki).
///
/// Format per line:
///
/// ```json
/// {"level":"info","component":"dcps","name":"user_writer.created","attrs":{"topic":"Foo","reliable":"true"}}
/// ```
///
/// Synchronous via `std::io::stderr()`. The mutex guards against
/// interleaved output between threads.
#[cfg(feature = "std")]
#[derive(Debug)]
pub struct StderrJsonSink {
    out: Mutex<io::Stderr>,
}

#[cfg(feature = "std")]
impl Default for StderrJsonSink {
    fn default() -> Self {
        Self {
            out: Mutex::new(io::stderr()),
        }
    }
}

#[cfg(feature = "std")]
impl StderrJsonSink {
    /// Constructor.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

#[cfg(feature = "std")]
impl Sink for StderrJsonSink {
    fn record(&self, event: &Event) {
        let line = serialize_json_line(event);
        if let Ok(mut out) = self.out.lock() {
            // Ignore errors on stderr — the sink must not torpedo
            // the app path if someone closes stderr.
            let _ = out.write_all(line.as_bytes());
            let _ = out.write_all(b"\n");
            let _ = out.flush();
        }
    }
}

/// In-memory sink for tests. Collects events in a `Mutex<Vec>`.
#[cfg(feature = "std")]
#[derive(Debug, Default)]
pub struct VecSink {
    events: Mutex<Vec<Event>>,
}

#[cfg(feature = "std")]
impl VecSink {
    /// Constructor.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Snapshot of the events collected so far.
    #[must_use]
    pub fn snapshot(&self) -> Vec<Event> {
        self.events.lock().map(|e| e.clone()).unwrap_or_default()
    }

    /// Number of events so far.
    #[must_use]
    pub fn len(&self) -> usize {
        self.events.lock().map(|e| e.len()).unwrap_or(0)
    }

    /// True if there are no events.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(feature = "std")]
impl Sink for VecSink {
    fn record(&self, event: &Event) {
        if let Ok(mut v) = self.events.lock() {
            v.push(event.clone());
        }
    }
}

// zerodds-lint: allow no_dyn_in_safe
// SharedSink needs `Arc<dyn Sink>` so consumers can inject arbitrary
// Sink implementations (StderrJsonSink, OTLP bridge, custom forwarder).
// The sinks themselves are Send+Sync; trait objects here are an
// architectural contract, not a memory-safety question.

/// Type-erased shared sink handle.
#[cfg(feature = "alloc")]
pub type SharedSink = Arc<dyn Sink>;

/// Returns a `SharedSink` that does nothing. The default choice.
#[cfg(feature = "alloc")]
#[must_use]
pub fn null_sink() -> SharedSink {
    Arc::new(NullSink)
}

// ============================================================================
// JSON serialization — minimal, without serde (foundation should stay
// dependency-free). RFC 8259 subset: strings with \"-escape, no Unicode
// escapes except \\ \" \n \r \t.
// ============================================================================

// Only used by StderrJsonSink (feature=std); the alloc-only build
// sees it as dead. Allow is cleaner than per-caller cfg.
#[cfg(feature = "alloc")]
#[allow(dead_code)]
fn serialize_json_line(event: &Event) -> String {
    let mut s = String::new();
    s.push('{');
    s.push_str("\"level\":");
    push_json_string(&mut s, event.level.as_str());
    s.push_str(",\"component\":");
    push_json_string(&mut s, event.component.as_str());
    s.push_str(",\"name\":");
    push_json_string(&mut s, event.name);
    if !event.attrs.is_empty() {
        s.push_str(",\"attrs\":{");
        for (i, a) in event.attrs.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            push_json_string(&mut s, a.key);
            s.push(':');
            push_json_string(&mut s, &a.value);
        }
        s.push('}');
    }
    s.push('}');
    s
}

#[cfg(feature = "alloc")]
#[allow(dead_code)]
fn push_json_string(out: &mut String, value: &str) {
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                // Control char → \u00XX
                let _ = core::fmt::Write::write_fmt(out, core::format_args!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn level_labels() {
        assert_eq!(Level::Info.as_str(), "info");
        assert_eq!(Level::Warn.as_str(), "warn");
        assert_eq!(Level::Error.as_str(), "error");
    }

    #[test]
    fn component_labels() {
        assert_eq!(Component::Dcps.as_str(), "dcps");
        assert_eq!(Component::Discovery.as_str(), "discovery");
        assert_eq!(Component::Rtps.as_str(), "rtps");
        assert_eq!(Component::Security.as_str(), "security");
        assert_eq!(Component::Transport.as_str(), "transport");
        assert_eq!(Component::User.as_str(), "user");
    }

    #[test]
    fn event_builder_attrs() {
        let e = Event::new(Level::Info, Component::Dcps, "user_writer.created")
            .with_attr("topic", "Foo")
            .with_attr("reliable", "true");
        assert_eq!(e.attrs.len(), 2);
        assert_eq!(e.attrs[0].key, "topic");
        assert_eq!(e.attrs[0].value, "Foo");
    }

    #[test]
    fn null_sink_is_no_op() {
        let s = NullSink;
        let e = Event::new(Level::Info, Component::Dcps, "x");
        s.record(&e); // no panic, no mutation.
    }

    #[test]
    fn vec_sink_collects() {
        let s = VecSink::new();
        s.record(&Event::new(Level::Info, Component::Dcps, "a"));
        s.record(&Event::new(Level::Warn, Component::Rtps, "b"));
        assert_eq!(s.len(), 2);
        let snap = s.snapshot();
        assert_eq!(snap[0].name, "a");
        assert_eq!(snap[1].level, Level::Warn);
    }

    #[test]
    fn serialize_json_line_basic() {
        let e = Event::new(Level::Info, Component::Dcps, "user_writer.created");
        let s = serialize_json_line(&e);
        assert_eq!(
            s,
            r#"{"level":"info","component":"dcps","name":"user_writer.created"}"#
        );
    }

    #[test]
    fn serialize_json_line_with_attrs() {
        let e = Event::new(Level::Info, Component::Dcps, "writer.created")
            .with_attr("topic", "Foo")
            .with_attr("reliable", "true");
        let s = serialize_json_line(&e);
        assert!(s.contains(r#""attrs":{"topic":"Foo","reliable":"true"}"#));
    }

    #[test]
    fn serialize_escapes_special_chars() {
        let e = Event::new(Level::Info, Component::User, "x").with_attr("k", "a\"b\\c\nd\te");
        let s = serialize_json_line(&e);
        assert!(s.contains(r#""k":"a\"b\\c\nd\te""#));
    }

    #[test]
    fn serialize_escapes_control_chars() {
        let e = Event::new(Level::Info, Component::User, "x").with_attr("k", "\x01");
        let s = serialize_json_line(&e);
        assert!(
            s.contains("\\u0001"),
            "control-char must be \\uXXXX, got: {s}"
        );
    }

    #[test]
    fn null_sink_handle_typed() {
        let h: SharedSink = null_sink();
        h.record(&Event::new(Level::Info, Component::Dcps, "x"));
    }

    #[test]
    fn vec_sink_threadsafe_smoke() {
        use std::sync::Arc as StdArc;
        use std::thread;
        let s: StdArc<VecSink> = StdArc::new(VecSink::new());
        let mut handles = Vec::new();
        for i in 0..4 {
            let s = StdArc::clone(&s);
            handles.push(thread::spawn(move || {
                for _ in 0..100 {
                    s.record(&Event::new(
                        Level::Info,
                        Component::User,
                        if i % 2 == 0 { "even" } else { "odd" },
                    ));
                }
            }));
        }
        for h in handles {
            h.join().unwrap();
        }
        assert_eq!(s.len(), 400);
    }

    #[test]
    fn stderr_json_sink_does_not_panic() {
        // Smoke: writing to stderr should never panic.
        let s = StderrJsonSink::new();
        s.record(&Event::new(Level::Info, Component::Dcps, "stderr.smoke"));
    }
}
