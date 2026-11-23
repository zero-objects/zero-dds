// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The OTLP spans of the daemon (Spec §8.3), sent through
//! `zerodds-observability-otlp` when `OTEL_EXPORTER_OTLP_ENDPOINT` is set.
//!
//! Every PUBLISH is one trace whose root is `mqtt.publish`:
//!
//! * a PUBLISH of the daemon (DDS → MQTT, `SpanKind::Client`): from taking
//!   the sample from the DataReader to the end of its roundtrip with the
//!   broker — the socket write under QoS 0, the PUBACK under QoS 1, the
//!   PUBCOMP under QoS 2; a refusal of the broker (a Reason Code of 0x80 or
//!   above) or a connection lost before the acknowledgement is status
//!   error.
//! * a PUBLISH of the broker (MQTT → DDS, `SpanKind::Server`): from reading
//!   it to the end of its handling; each DDS topic it was written into is a
//!   child `dds.write` (`SpanKind::Client`) covering the DataWriter's write,
//!   status error when DDS refused the sample.

use std::string::{String, ToString};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::vec::Vec;

use zerodds_foundation::observability::Attribute;
use zerodds_foundation::tracing::{Span, SpanContext, SpanId, SpanKind, SpanStatus, TraceId};
use zerodds_observability_otlp::OtlpExporter;

/// Sends the spans of the PUBLISH packets to an OTLP exporter; without one
/// it sends nothing and costs nothing.
#[derive(Clone, Default)]
pub struct Tracer {
    exporter: Option<Arc<OtlpExporter>>,
}

/// A PUBLISH of the daemon (DDS → MQTT), its times in nanoseconds since the
/// Unix epoch.
#[derive(Debug, Clone, Default)]
pub struct PublishOut {
    /// The sample was taken from the DataReader.
    pub start_ns: u64,
    /// The MQTT topic.
    pub mqtt_topic: String,
    /// The DDS topic of the configuration.
    pub dds_topic: String,
    /// The QoS of the PUBLISH.
    pub qos: u8,
    /// The payload octets.
    pub bytes: usize,
    /// The packet identifier (QoS 1 and 2).
    pub packet_id: Option<u16>,
}

/// One DDS write of a PUBLISH of the broker.
#[derive(Debug, Clone, Default)]
pub struct DdsWrite {
    /// The DDS topic.
    pub dds_topic: String,
    /// The write began.
    pub start_ns: u64,
    /// It returned.
    pub end_ns: u64,
    /// DDS refused the sample: why.
    pub refused: Option<String>,
}

/// A PUBLISH of the broker (MQTT → DDS).
#[derive(Debug, Clone, Default)]
pub struct PublishIn {
    /// It was read.
    pub start_ns: u64,
    /// Its handling ended.
    pub end_ns: u64,
    /// The MQTT topic.
    pub mqtt_topic: String,
    /// The QoS of the PUBLISH.
    pub qos: u8,
    /// The payload octets.
    pub bytes: usize,
    /// The DDS writes, one per DDS topic it was routed to.
    pub writes: Vec<DdsWrite>,
    /// It was not written: why (no subscribed topic, the ACL).
    pub refused: Option<String>,
}

impl Tracer {
    /// A tracer sending to `exporter`.
    #[must_use]
    pub fn new(exporter: Option<Arc<OtlpExporter>>) -> Self {
        Self { exporter }
    }

    /// Whether spans go anywhere.
    #[must_use]
    pub fn enabled(&self) -> bool {
        self.exporter.is_some()
    }

    /// The span of a PUBLISH of the daemon whose roundtrip ended at
    /// `end_ns`; `failure` when it did not end with a success.
    pub fn publish_out(&self, x: &PublishOut, end_ns: u64, failure: Option<&str>) {
        if let Some(exporter) = &self.exporter {
            exporter.add_span(publish_out_span(x, end_ns, failure));
        }
    }

    /// The spans of a PUBLISH of the broker.
    pub fn publish_in(&self, x: &PublishIn) {
        if let Some(exporter) = &self.exporter {
            for span in publish_in_spans(x) {
                exporter.add_span(span);
            }
        }
    }
}

fn attribute(key: &'static str, value: impl ToString) -> Attribute {
    Attribute {
        key,
        value: value.to_string(),
    }
}

/// The span of a PUBLISH of the daemon: `mqtt.publish`, a root.
#[must_use]
pub fn publish_out_span(x: &PublishOut, end_ns: u64, failure: Option<&str>) -> Span {
    let mut attributes = vec![
        attribute("mqtt.topic", &x.mqtt_topic),
        attribute("mqtt.qos", x.qos),
        attribute("mqtt.message.bytes", x.bytes),
        attribute("dds.topic", &x.dds_topic),
    ];
    if let Some(pid) = x.packet_id {
        attributes.push(attribute("mqtt.packet_id", pid));
    }
    Span {
        context: SpanContext::new_root(new_trace_id(), new_span_id()),
        name: "mqtt.publish".to_string(),
        kind: SpanKind::Client,
        start_unix_ns: x.start_ns,
        end_unix_ns: end_ns.max(x.start_ns),
        status: if failure.is_some() {
            SpanStatus::Error
        } else {
            SpanStatus::Ok
        },
        status_description: failure.map(str::to_string),
        attributes,
    }
}

/// The spans of a PUBLISH of the broker: the children `dds.write`, then the
/// root `mqtt.publish`.
#[must_use]
pub fn publish_in_spans(x: &PublishIn) -> Vec<Span> {
    let root = SpanContext::new_root(new_trace_id(), new_span_id());
    let mut out: Vec<Span> = x
        .writes
        .iter()
        .map(|w| Span {
            context: SpanContext::child_of(&root, new_span_id()),
            name: "dds.write".to_string(),
            kind: SpanKind::Client,
            start_unix_ns: w.start_ns,
            end_unix_ns: w.end_ns.max(w.start_ns),
            status: if w.refused.is_some() {
                SpanStatus::Error
            } else {
                SpanStatus::Ok
            },
            status_description: w.refused.clone(),
            attributes: vec![attribute("dds.topic", &w.dds_topic)],
        })
        .collect();
    let failure = x
        .refused
        .clone()
        .or_else(|| x.writes.iter().find_map(|w| w.refused.clone()));
    out.push(Span {
        context: root,
        name: "mqtt.publish".to_string(),
        kind: SpanKind::Server,
        start_unix_ns: x.start_ns,
        end_unix_ns: x.end_ns.max(x.start_ns),
        status: if failure.is_some() {
            SpanStatus::Error
        } else {
            SpanStatus::Ok
        },
        status_description: failure,
        attributes: vec![
            attribute("mqtt.topic", &x.mqtt_topic),
            attribute("mqtt.qos", x.qos),
            attribute("mqtt.message.bytes", x.bytes),
        ],
    });
    out
}

/// Nanoseconds since the Unix epoch.
#[must_use]
pub fn unix_ns() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_nanos()).unwrap_or(u64::MAX))
}

/// The time since the Unix epoch of a moment `since` before now.
#[must_use]
pub fn unix_ns_since(since: std::time::Instant) -> u64 {
    let elapsed = u64::try_from(since.elapsed().as_nanos()).unwrap_or(u64::MAX);
    unix_ns().saturating_sub(elapsed)
}

/// Mixed into the span ids: a collector only needs to tell them apart.
static SPAN_COUNTER: AtomicU64 = AtomicU64::new(1);

fn new_trace_id() -> TraceId {
    let mut bytes = [0u8; 16];
    if getrandom::getrandom(&mut bytes).is_err() || bytes.iter().all(|b| *b == 0) {
        bytes = u128::from(unix_ns()).max(1).to_be_bytes();
    }
    TraceId(bytes)
}

fn new_span_id() -> SpanId {
    let n = SPAN_COUNTER.fetch_add(1, Ordering::Relaxed);
    let mixed = n ^ (u64::from(std::process::id()).rotate_left(32));
    let mut bytes = mixed.to_be_bytes();
    if bytes.iter().all(|b| *b == 0) {
        bytes[7] = 1;
    }
    SpanId(bytes)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    fn value<'a>(span: &'a Span, key: &str) -> Option<&'a str> {
        span.attributes
            .iter()
            .find(|a| a.key == key)
            .map(|a| a.value.as_str())
    }

    fn out() -> PublishOut {
        PublishOut {
            start_ns: 100,
            mqtt_topic: "plant/temp".into(),
            dds_topic: "Plant::Temp".into(),
            qos: 1,
            bytes: 12,
            packet_id: Some(7),
        }
    }

    /// Spec §8.3: a PUBLISH of the daemon is the root `mqtt.publish`, kind
    /// client, from the sample to the end of its roundtrip, with the topic,
    /// the QoS, the octets, the DDS topic and the packet identifier.
    #[test]
    fn mqttbridge_8_3_a_publish_of_the_daemon_is_a_root_span_to_its_acknowledgement() {
        let span = publish_out_span(&out(), 900, None);
        assert_eq!(span.name, "mqtt.publish");
        assert!(matches!(span.kind, SpanKind::Client));
        assert!(span.context.parent_span_id.is_none());
        assert_eq!((span.start_unix_ns, span.end_unix_ns), (100, 900));
        assert!(matches!(span.status, SpanStatus::Ok));
        assert_eq!(value(&span, "mqtt.topic"), Some("plant/temp"));
        assert_eq!(value(&span, "mqtt.qos"), Some("1"));
        assert_eq!(value(&span, "mqtt.message.bytes"), Some("12"));
        assert_eq!(value(&span, "dds.topic"), Some("Plant::Temp"));
        assert_eq!(value(&span, "mqtt.packet_id"), Some("7"));
        let qos0 = publish_out_span(
            &PublishOut {
                qos: 0,
                packet_id: None,
                ..out()
            },
            150,
            None,
        );
        assert_eq!(value(&qos0, "mqtt.packet_id"), None);
    }

    /// Spec §8.3: a refusal of the broker is status error with its reason.
    #[test]
    fn mqttbridge_8_3_a_refused_publish_is_status_error() {
        let span = publish_out_span(&out(), 900, Some("PUBACK 0x87"));
        assert!(matches!(span.status, SpanStatus::Error));
        assert_eq!(span.status_description.as_deref(), Some("PUBACK 0x87"));
    }

    /// Spec §8.3: a PUBLISH of the broker is the root `mqtt.publish`, kind
    /// server, with one child `dds.write` per DDS topic it was written
    /// into, in the trace of the root; a write DDS refused is status error
    /// on the child and on the root.
    #[test]
    fn mqttbridge_8_3_a_publish_of_the_broker_has_a_dds_write_child_per_topic() {
        let x = PublishIn {
            start_ns: 100,
            end_ns: 900,
            mqtt_topic: "plant/cmd".into(),
            qos: 2,
            bytes: 5,
            writes: vec![
                DdsWrite {
                    dds_topic: "Cmd".into(),
                    start_ns: 200,
                    end_ns: 300,
                    refused: None,
                },
                DdsWrite {
                    dds_topic: "CmdLog".into(),
                    start_ns: 400,
                    end_ns: 500,
                    refused: Some("dds write: rejected".into()),
                },
            ],
            refused: None,
        };
        let spans = publish_in_spans(&x);
        let names: Vec<&str> = spans.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["dds.write", "dds.write", "mqtt.publish"]);
        let root = &spans[2];
        assert!(matches!(root.kind, SpanKind::Server));
        assert!(root.context.parent_span_id.is_none());
        for child in &spans[..2] {
            assert!(matches!(child.kind, SpanKind::Client));
            assert_eq!(child.context.trace_id, root.context.trace_id);
            assert_eq!(child.context.parent_span_id, Some(root.context.span_id));
            assert_ne!(child.context.span_id, root.context.span_id);
        }
        assert_eq!((spans[0].start_unix_ns, spans[0].end_unix_ns), (200, 300));
        assert_eq!(value(&spans[1], "dds.topic"), Some("CmdLog"));
        assert!(matches!(spans[0].status, SpanStatus::Ok));
        assert!(matches!(spans[1].status, SpanStatus::Error));
        assert!(matches!(root.status, SpanStatus::Error));
        assert_eq!(value(root, "mqtt.qos"), Some("2"));
    }

    /// Spec §8.3: a PUBLISH that reached no DDS topic is the root alone,
    /// status error with the reason.
    #[test]
    fn mqttbridge_8_3_a_publish_written_nowhere_is_the_root_alone() {
        let spans = publish_in_spans(&PublishIn {
            start_ns: 1,
            end_ns: 2,
            mqtt_topic: "x".into(),
            refused: Some("matches no subscribed topic".into()),
            ..PublishIn::default()
        });
        assert_eq!(spans.len(), 1);
        assert!(matches!(spans[0].status, SpanStatus::Error));
    }

    /// Spec §8.3: without `OTEL_EXPORTER_OTLP_ENDPOINT` there is no
    /// exporter: the tracer is off and sends nothing.
    #[test]
    fn mqttbridge_8_3_a_tracer_without_exporter_is_off() {
        let t = Tracer::new(None);
        assert!(!t.enabled());
        t.publish_out(&out(), 2, None);
        t.publish_in(&PublishIn::default());
        assert!(Tracer::default().exporter.is_none());
    }
}
