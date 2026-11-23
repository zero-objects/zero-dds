// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The DDS statuses of a topic the daemon reports to the broker — Spec §6.
//!
//! Deadline: a missed DEADLINE of a topic's DataReader (a DDS writer was
//! late, REQUESTED_DEADLINE_MISSED) or DataWriter (the MQTT publishers were
//! late, OFFERED_DEADLINE_MISSED) is a retained PUBLISH on
//! `<own_topic_prefix>/<client_id>/deadline_missed/<slug>`. Liveliness: a
//! matched DDS writer of a topic's DataReader that loses its liveliness (the
//! `not_alive_count` of LIVELINESS_CHANGED grows) is a PUBLISH on
//! `<own_topic_prefix>/<client_id>/liveliness_lost/<slug>`. `<slug>` is the
//! slug of the DDS topic name (Spec §5.1); `<own_topic_prefix>` is
//! `mqtt.own_topic_prefix`, default `$zerodds` (Spec §3). Both carry a JSON
//! document with Payload Format Indicator 1 and Content Type
//! `application/json`, QoS 1 (the broker's Maximum QoS when that is 0).

use std::string::String;
use std::vec::Vec;

use zerodds_dcps::runtime::DcpsRuntime;
use zerodds_rtps::wire_types::EntityId;

use super::client::{ClientError, MqttClient};
use super::config::default_mqtt_slug;
use crate::property_set::PropertySet;

/// The statuses of one topic and what was reported of them.
#[derive(Debug, Clone)]
struct Watched {
    dds_name: String,
    slug: String,
    reader: Option<EntityId>,
    writer: Option<EntityId>,
    reader_missed: u64,
    writer_missed: u64,
    not_alive: u64,
}

/// One report to the broker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    /// The topic name of the PUBLISH.
    pub topic: String,
    /// Its JSON document.
    pub json: String,
    /// Whether it is retained.
    pub retain: bool,
}

/// The watched topics of one connection.
#[derive(Debug, Clone, Default)]
pub struct StatusWatch {
    /// `mqtt.own_topic_prefix` (Spec §3).
    own_prefix: String,
    prefix: Option<String>,
    topics: Vec<Watched>,
}

impl StatusWatch {
    /// The topics with `deadline` set or a DataReader, of the connection
    /// of `client_id` (`None`: an identifier with `+` or `#`, nothing is
    /// reported), their reports under `own_prefix` (`mqtt.own_topic_prefix`).
    /// The reported counts start at the runtime's.
    #[must_use]
    pub fn new(
        runtime: &DcpsRuntime,
        own_prefix: &str,
        client_id: &str,
        topics: &[(String, Option<EntityId>, Option<EntityId>)],
    ) -> Self {
        let prefix = prefix_of(own_prefix, client_id);
        let topics = topics
            .iter()
            .map(|(dds_name, reader, writer)| Watched {
                dds_name: dds_name.clone(),
                slug: default_mqtt_slug(dds_name),
                reader: *reader,
                writer: *writer,
                reader_missed: reader
                    .map_or(0, |r| runtime.user_reader_requested_deadline_missed(r)),
                writer_missed: writer.map_or(0, |w| runtime.user_writer_offered_deadline_missed(w)),
                not_alive: reader.map_or(0, |r| runtime.user_reader_liveliness_status(r).2),
            })
            .collect();
        Self {
            own_prefix: own_prefix.to_string(),
            prefix,
            topics,
        }
    }

    /// The reports from now on go under `client_id`: the identifier of a
    /// new connection, which the broker may have assigned anew (Spec §6).
    pub fn set_client_id(&mut self, client_id: &str) {
        self.prefix = prefix_of(&self.own_prefix, client_id);
    }

    /// What changed since the last call, as the PUBLISH packets that
    /// report it.
    #[must_use]
    pub fn changes(&mut self, runtime: &DcpsRuntime) -> Vec<Report> {
        let Some(prefix) = self.prefix.clone() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for t in &mut self.topics {
            if let Some(r) = t.reader {
                let missed = runtime.user_reader_requested_deadline_missed(r);
                if missed > t.reader_missed {
                    t.reader_missed = missed;
                    out.push(deadline_report(
                        &prefix,
                        t,
                        "reader",
                        missed,
                        runtime.user_reader_missed_deadline_instance(r),
                    ));
                }
                let (_, alive, not_alive) = runtime.user_reader_liveliness_status(r);
                if not_alive > t.not_alive {
                    out.push(Report {
                        topic: format!("{prefix}/liveliness_lost/{}", t.slug),
                        json: format!(
                            "{{\"dds_topic\":{},\"alive_count\":{alive},\"not_alive_count\":{not_alive}}}",
                            json_string(&t.dds_name)
                        ),
                        retain: false,
                    });
                }
                t.not_alive = not_alive;
            }
            if let Some(w) = t.writer {
                let missed = runtime.user_writer_offered_deadline_missed(w);
                if missed > t.writer_missed {
                    t.writer_missed = missed;
                    out.push(deadline_report(
                        &prefix,
                        t,
                        "writer",
                        missed,
                        runtime.user_writer_missed_deadline_instance(w),
                    ));
                }
            }
        }
        out
    }
}

/// `<own_prefix>/<client_id>` (`$zerodds/<client_id>` by default); `None`
/// for an empty identifier or one with `+`, `#` or NUL, which cannot be a
/// topic level.
fn prefix_of(own_prefix: &str, client_id: &str) -> Option<String> {
    (!client_id.is_empty() && !client_id.contains(['+', '#', '\0']))
        .then(|| format!("{own_prefix}/{client_id}"))
}

fn deadline_report(
    prefix: &str,
    t: &Watched,
    endpoint: &str,
    total: u64,
    instance: Option<[u8; 16]>,
) -> Report {
    let key = instance.map_or_else(
        || "null".to_string(),
        |k| {
            let mut hex = String::with_capacity(34);
            hex.push('"');
            for b in k {
                use std::fmt::Write as _;
                let _ = write!(hex, "{b:02x}");
            }
            hex.push('"');
            hex
        },
    );
    Report {
        topic: format!("{prefix}/deadline_missed/{}", t.slug),
        json: format!(
            "{{\"dds_topic\":{},\"endpoint\":\"{endpoint}\",\"total_count\":{total},\"key_hash\":{key}}}",
            json_string(&t.dds_name)
        ),
        retain: true,
    }
}

/// `s` as a JSON string, quoted.
fn json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => {
                use std::fmt::Write as _;
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Sends `report` on `client`: QoS 1 (the broker's Maximum QoS when that is
/// 0), Payload Format Indicator 1, Content Type `application/json`.
///
/// # Errors
/// What [`MqttClient::publish`] returns.
pub fn send(client: &mut MqttClient, report: &Report) -> Result<(), ClientError> {
    let props = PropertySet {
        payload_format_indicator: Some(1),
        content_type: Some("application/json".to_string()),
        ..PropertySet::default()
    };
    let qos = client.broker_limits().maximum_qos.min(1);
    client.publish(
        &report.topic,
        report.json.as_bytes(),
        qos,
        report.retain,
        &props,
    )
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    /// Spec §6 Deadline: the retained report names the DDS topic, the
    /// endpoint, the count and the instance, on the slug of the topic.
    #[test]
    fn mqttbridge_6_the_deadline_report() {
        let t = Watched {
            dds_name: "Robot::Arm \"1\"".into(),
            slug: default_mqtt_slug("Robot::Arm \"1\""),
            reader: None,
            writer: None,
            reader_missed: 0,
            writer_missed: 0,
            not_alive: 0,
        };
        let r = deadline_report("$zerodds/b", &t, "reader", 3, Some([0xab; 16]));
        assert_eq!(r.topic, "$zerodds/b/deadline_missed/robot/arm__1_");
        assert_eq!(
            r.json,
            r#"{"dds_topic":"Robot::Arm \"1\"","endpoint":"reader","total_count":3,"key_hash":"abababababababababababababababab"}"#
        );
        assert!(r.retain);
        let w = deadline_report("$zerodds/b", &t, "writer", 1, None);
        assert!(
            w.json
                .ends_with(r#""endpoint":"writer","total_count":1,"key_hash":null}"#)
        );
    }

    /// Spec §3, §6: the reports go under `mqtt.own_topic_prefix`, also
    /// after a new client identifier; an identifier with a wildcard has
    /// none.
    #[test]
    fn mqttbridge_6_the_reports_go_under_the_own_topic_prefix() {
        assert_eq!(prefix_of("$zerodds", "b").as_deref(), Some("$zerodds/b"));
        assert_eq!(
            prefix_of("plant/zerodds", "b").as_deref(),
            Some("plant/zerodds/b")
        );
        assert_eq!(prefix_of("zerodds", "a+b"), None);
        let mut watch = StatusWatch {
            own_prefix: "zerodds".into(),
            prefix: prefix_of("zerodds", "one"),
            topics: Vec::new(),
        };
        assert_eq!(watch.prefix.as_deref(), Some("zerodds/one"));
        watch.set_client_id("two");
        assert_eq!(watch.prefix.as_deref(), Some("zerodds/two"));
    }
}
