#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr
)]

//! Annex C C.1.14 — Audit Link.
//!
//! Spec §C.1.14: receiver link on `$audit` produces a stream
//! of audit events. A second client's SASL success produces
//! `link.attach.success` audit record with `subject_name` field.

mod common;

use zerodds_amqp_bridge::extended_types::AmqpExtValue;
use zerodds_amqp_endpoint::management::{AuditEvent, AuditProducer, audit_event_sample};

#[test]
fn c1_14_link_attach_success_event_carries_subject() {
    // Spec §C.1.14: receiver on $audit; after
    // SASL auth + link attach an event with subject_name arrives.
    let event = AuditEvent::LinkAttached {
        subject: "alice".into(),
        link: "L1".into(),
        address: "Sensor".into(),
    };
    let sample = audit_event_sample(event, 1_700_000_000_000);
    let entries = match sample {
        AmqpExtValue::Map(v) => v,
        _ => panic!(),
    };

    // Spec-Pflicht: event-type, timestamp, subject.
    let event_type = entries
        .iter()
        .find(|(k, _)| matches!(k, AmqpExtValue::Str(s) if s == "event-type"))
        .map(|(_, v)| v.clone())
        .unwrap();
    assert_eq!(
        event_type,
        AmqpExtValue::Symbol("link.attach.success".into())
    );

    let subject = entries
        .iter()
        .find(|(k, _)| matches!(k, AmqpExtValue::Str(s) if s == "subject"))
        .map(|(_, v)| v.clone())
        .unwrap();
    assert_eq!(subject, AmqpExtValue::Str("alice".into()));
}

#[test]
fn c1_14_audit_producer_streams_events_in_order() {
    let mut prod = AuditProducer::new(8);
    prod.push(
        AuditEvent::ConnectionOpened {
            subject: "alice".into(),
            remote: "1.2.3.4:5".into(),
        },
        100,
    );
    prod.push(
        AuditEvent::SaslSuccess {
            subject: "alice".into(),
            mechanism: "EXTERNAL".into(),
        },
        200,
    );
    prod.push(
        AuditEvent::LinkAttached {
            subject: "alice".into(),
            link: "L".into(),
            address: "Sensor".into(),
        },
        300,
    );

    let samples = prod.drain_samples();
    assert_eq!(samples.len(), 3);

    let types: Vec<String> = samples
        .iter()
        .map(|s| {
            let entries = match s {
                AmqpExtValue::Map(v) => v,
                _ => panic!(),
            };
            let v = entries
                .iter()
                .find(|(k, _)| matches!(k, AmqpExtValue::Str(s) if s == "event-type"))
                .map(|(_, v)| v.clone())
                .unwrap();
            match v {
                AmqpExtValue::Symbol(s) => s,
                _ => panic!(),
            }
        })
        .collect();

    // Spec C.1.14 ordnet: connection.opened → sasl.success →
    // link.attach.success.
    assert_eq!(types[0], "connection.opened");
    assert_eq!(types[1], "sasl.success");
    assert_eq!(types[2], "link.attach.success");
}

#[test]
fn c1_14_unauthorized_event_carries_resource_field() {
    // Spec §C.1.14 + §10.3.3 — on an AccessControl reject
    // an `access.unauthorized` event is emitted that contains subject +
    // Resource-Address fuehrt.
    let event = AuditEvent::Unauthorized {
        subject: "eve".into(),
        resource: "RestrictedTopic".into(),
    };
    let sample = audit_event_sample(event, 0);
    let entries = match sample {
        AmqpExtValue::Map(v) => v,
        _ => panic!(),
    };
    let resource = entries
        .iter()
        .find(|(k, _)| matches!(k, AmqpExtValue::Str(s) if s == "resource"))
        .map(|(_, v)| v.clone())
        .unwrap();
    assert_eq!(resource, AmqpExtValue::Str("RestrictedTopic".into()));
    let event_type = entries
        .iter()
        .find(|(k, _)| matches!(k, AmqpExtValue::Str(s) if s == "event-type"))
        .map(|(_, v)| v.clone())
        .unwrap();
    assert_eq!(
        event_type,
        AmqpExtValue::Symbol("access.unauthorized".into())
    );
}

#[test]
fn c1_14_ringbuffer_evicts_oldest_on_overflow() {
    // §sec:audit-channel: audit is a read-only stream, no
    // persistenter Trail. Capacity-bounded queue verdraengt
    // aelteste Events.
    let mut prod = AuditProducer::new(2);
    prod.push(
        AuditEvent::ConnectionOpened {
            subject: "first".into(),
            remote: "x".into(),
        },
        1,
    );
    prod.push(
        AuditEvent::ConnectionOpened {
            subject: "second".into(),
            remote: "y".into(),
        },
        2,
    );
    prod.push(
        AuditEvent::ConnectionOpened {
            subject: "third".into(),
            remote: "z".into(),
        },
        3,
    );
    assert_eq!(prod.len(), 2);
    let samples = prod.drain_samples();
    let subjects: Vec<String> = samples
        .iter()
        .map(|s| {
            let entries = match s {
                AmqpExtValue::Map(v) => v,
                _ => panic!(),
            };
            let v = entries
                .iter()
                .find(|(k, _)| matches!(k, AmqpExtValue::Str(s) if s == "subject"))
                .map(|(_, v)| v.clone())
                .unwrap();
            match v {
                AmqpExtValue::Str(s) => s,
                _ => panic!(),
            }
        })
        .collect();
    // 'first' was evicted; 'second' and 'third' remain.
    assert!(!subjects.contains(&"first".to_string()));
    assert!(subjects.contains(&"second".to_string()));
    assert!(subjects.contains(&"third".to_string()));
}
