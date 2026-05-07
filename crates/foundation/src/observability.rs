// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Observability — strukturierte DDS-Events fuer Tracing und Metriken.
//!
//! Liefert eine ZeroDDS-spezifische Event-Sprache plus einen
//! schlanken `Sink`-Trait, ueber den Konsumenten Events abgreifen
//! koennen.
//!
//! ## Design-Ziele
//!
//! 1. **Zero-Overhead by Default**: ohne Sink wird gar kein Event-Objekt
//!    konstruiert (`with_sink(...)` ist die Opt-In-Stelle).
//! 2. **Sync, allocation-light**: `Sink::record(&Event)` nimmt Events per
//!    `&`, jeder Sink entscheidet selber ob er klont/serialisiert.
//! 3. **Production-tauglich**: der mitgelieferte [`StderrJsonSink`]
//!    schreibt JSON-Lines auf stderr — direkt verarbeitbar von
//!    Vector/fluentd/Datadog/Loki/journald.
//! 4. **OTel-Bruecke spaeter**: ein eigener Crate `dds-otel` (oder ein
//!    `tracing-opentelemetry`-Adapter im Konsumenten) kann diesen
//!    Sink-Trait implementieren und Events als OTLP-Spans schicken.
//!
//! ## Event-Modell
//!
//! Events sind grobgranular: ein Event pro Endpoint-Lifecycle-Aktion
//! oder pro Sample-Pfad-Phase. Im Hot-Path (z.B. pro-Sample-Latency)
//! benutzen wir **keine** Events — stattdessen die atomaren Stats
//! aus D.4 Phase A. Events sind fuer Coarse-Grained-Telemetry, nicht
//! fuer p99-Latenz-Sampling.

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

/// Schweregrad eines Events. An OTel/Syslog-Levels angelehnt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Level {
    /// Normaler Lifecycle-Event (Endpoint create/destroy, match).
    Info,
    /// Hinweis auf abnormale aber nicht-fatale Situation
    /// (Discovery-Timeout, einzelner Drop).
    Warn,
    /// Funktional fehlgeschlagene Operation.
    Error,
}

impl Level {
    /// JSON-/Logfile-konforme Klein-Schreibweise.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warn => "warn",
            Self::Error => "error",
        }
    }
}

/// Event-Quelle. Identifiziert die Schicht.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Component {
    /// DCPS / Domain-Participant-Pfad.
    Dcps,
    /// SPDP/SEDP Discovery.
    Discovery,
    /// RTPS-Wire / Reader / Writer.
    Rtps,
    /// Security-Plugins.
    Security,
    /// Transport-Layer (UDP/TCP/SHM).
    Transport,
    /// User-defined sub-system (Bridges, Tools).
    User,
}

impl Component {
    /// Maschinenlesbares Label.
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

/// Strukturiertes Key-Value-Attribut. `value` ist als String gehalten
/// — der Sink entscheidet, ob/wie er typisiert serialisiert.
#[cfg(feature = "alloc")]
#[derive(Debug, Clone)]
pub struct Attribute {
    /// Stable-Schluessel (kebab-case empfohlen).
    pub key: &'static str,
    /// String-Value.
    pub value: String,
}

/// Event-Datensatz. Wird von der DCPS-Runtime und Plugins erzeugt.
#[cfg(feature = "alloc")]
#[derive(Debug, Clone)]
pub struct Event {
    /// Schweregrad.
    pub level: Level,
    /// Verursacher-Komponente.
    pub component: Component,
    /// Stable-Event-Name in `domain.event`-Form (z.B.
    /// `dcps.user_writer.created`, `discovery.peer.matched`).
    pub name: &'static str,
    /// Optionale strukturierte Attribute.
    pub attrs: Vec<Attribute>,
}

#[cfg(feature = "alloc")]
impl Event {
    /// Konstruiert ein neues Event ohne Attribute.
    #[must_use]
    pub fn new(level: Level, component: Component, name: &'static str) -> Self {
        Self {
            level,
            component,
            name,
            attrs: Vec::new(),
        }
    }

    /// Builder-Form: ein Attribut anhaengen.
    #[must_use]
    pub fn with_attr(mut self, key: &'static str, value: impl Into<String>) -> Self {
        self.attrs.push(Attribute {
            key,
            value: value.into(),
        });
        self
    }
}

/// Sink-Trait: Konsumenten implementieren `record` und entscheiden,
/// wohin das Event geht (stderr, OTLP, prometheus, /dev/null).
#[cfg(feature = "alloc")]
pub trait Sink: Send + Sync {
    /// Verarbeitet ein Event. **Synchron.** Sinks duerfen schreiben,
    /// puffern oder droppen — der Aufrufer wartet, also bitte nicht
    /// blockieren (z.B. nicht synchrones HTTP-POST aus dem Hot-Path).
    fn record(&self, event: &Event);
}

/// No-op-Sink. Default-Wahl wenn keine Telemetry konfiguriert ist —
/// jeder `record`-Call ist ein direkter Return.
#[cfg(feature = "alloc")]
#[derive(Debug, Clone, Copy)]
pub struct NullSink;

#[cfg(feature = "alloc")]
impl Sink for NullSink {
    fn record(&self, _event: &Event) {}
}

/// Stderr-Sink: schreibt jedes Event als JSON-Line auf stderr.
/// Geeignet fuer Docker/k8s/journald-Pipelines mit nachgelagerten
/// Loglinks (Vector/fluentd/Datadog-Agent/Loki).
///
/// Format pro Zeile:
///
/// ```json
/// {"level":"info","component":"dcps","name":"user_writer.created","attrs":{"topic":"Foo","reliable":"true"}}
/// ```
///
/// Synchron ueber `std::io::stderr()`. Mutex schuetzt vor
/// interleaved-Output zwischen Threads.
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
    /// Konstruktor.
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
            // Errors auf stderr ignorieren — Sink darf den App-Pfad
            // nicht torpedieren wenn jemand stderr schliesst.
            let _ = out.write_all(line.as_bytes());
            let _ = out.write_all(b"\n");
            let _ = out.flush();
        }
    }
}

/// In-Memory-Sink fuer Tests. Sammelt Events in einem `Mutex<Vec>`.
#[cfg(feature = "std")]
#[derive(Debug, Default)]
pub struct VecSink {
    events: Mutex<Vec<Event>>,
}

#[cfg(feature = "std")]
impl VecSink {
    /// Konstruktor.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Snapshot der bisher gesammelten Events.
    #[must_use]
    pub fn snapshot(&self) -> Vec<Event> {
        self.events.lock().map(|e| e.clone()).unwrap_or_default()
    }

    /// Anzahl Events bisher.
    #[must_use]
    pub fn len(&self) -> usize {
        self.events.lock().map(|e| e.len()).unwrap_or(0)
    }

    /// True wenn keine Events.
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
// SharedSink benoetigt `Arc<dyn Sink>` damit Konsumenten beliebige
// Sink-Implementations injizieren koennen (StderrJsonSink, OTLP-Bridge,
// custom Forwarder). Die Sinks selbst sind Send+Sync; trait-objects
// hier sind ein Architektur-Vertrag, keine Speicher-Sicherheits-Frage.

/// Type-erased shared Sink-Handle.
#[cfg(feature = "alloc")]
pub type SharedSink = Arc<dyn Sink>;

/// Liefert einen `SharedSink`, der nichts macht. Default-Wahl.
#[cfg(feature = "alloc")]
#[must_use]
pub fn null_sink() -> SharedSink {
    Arc::new(NullSink)
}

// ============================================================================
// JSON-Serialisierung — minimal, ohne serde (foundation soll dep-frei
// bleiben). RFC 8259 Subset: Strings mit \"-Escape, keine Unicode-
// Eskapaden ausser \\ \" \n \r \t.
// ============================================================================

// Wird nur vom StderrJsonSink (feature=std) genutzt; alloc-only-
// Build erkennt sie als dead. Allow ist sauberer als pro-Aufrufer-cfg.
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
        s.record(&e); // kein Panic, keine Mutation.
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
        // Smoke: schreiben auf stderr soll niemals panicen.
        let s = StderrJsonSink::new();
        s.record(&Event::new(Level::Info, Component::Dcps, "stderr.smoke"));
    }
}
