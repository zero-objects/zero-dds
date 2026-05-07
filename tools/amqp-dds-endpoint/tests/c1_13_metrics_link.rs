#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr
)]

//! Annex C C.1.13 — Metrics Link.
//!
//! Spec §C.1.13: Receiver-Link auf `$metrics` produziert pro
//! Mandatory-Metric eine AMQP-Message mit Map-Body
//! `{name, value, unit, timestamp}`.
//!
//! Wir verifizieren das ohne den vollen Receiver-Wire-Roundtrip
//! (der ist Daemon-Wiring): wir testen den Producer-Pfad
//! `metrics_snapshot(hub, now_ms)` direkt.

mod common;

use zerodds_amqp_bridge::extended_types::AmqpExtValue;
use zerodds_amqp_endpoint::management::metrics_snapshot;
use zerodds_amqp_endpoint::metrics::{MANDATORY_METRIC_NAMES, MetricsHub, names};

#[test]
fn c1_13_metrics_snapshot_emits_one_sample_per_mandatory_metric() {
    let hub = MetricsHub::new();
    // Etwas Traffic erzeugen, sodass Counter > 0 sind.
    hub.on_connection_open();
    hub.on_transfer_received();
    hub.on_dropped_loop();
    hub.on_topic_added();
    hub.on_rpc_timeout();

    let samples = metrics_snapshot(&hub, 1_700_000_000_000);

    // Alle 14 Mandatory-Metrics MUESSEN als Sample vorhanden sein.
    assert_eq!(
        samples.len(),
        MANDATORY_METRIC_NAMES.len(),
        "expected {} samples, got {}",
        MANDATORY_METRIC_NAMES.len(),
        samples.len()
    );

    // Jedes Sample MUSS Spec-konformes Map-Body-Format haben:
    // {name, value, unit, timestamp}.
    let mut found_names = std::collections::HashSet::new();
    for sample in &samples {
        let entries = match sample {
            AmqpExtValue::Map(v) => v,
            _ => panic!("sample not a map"),
        };
        let keys: Vec<String> = entries
            .iter()
            .map(|(k, _)| match k {
                AmqpExtValue::Str(s) => s.clone(),
                _ => panic!("map key not Str"),
            })
            .collect();
        for required in ["name", "value", "unit", "timestamp"] {
            assert!(
                keys.contains(&required.to_string()),
                "missing key '{required}' in sample"
            );
        }
        // Name extrahieren und sammeln.
        let name_value = entries
            .iter()
            .find(|(k, _)| matches!(k, AmqpExtValue::Str(s) if s == "name"))
            .map(|(_, v)| v.clone())
            .unwrap();
        if let AmqpExtValue::Str(s) = name_value {
            found_names.insert(s);
        }
    }

    // Alle Mandatory-Metric-Namen MUESSEN im Sample-Stream
    // erscheinen (Spec §7.9.2.1).
    for required in MANDATORY_METRIC_NAMES {
        assert!(
            found_names.contains(required),
            "metric '{required}' missing from snapshot"
        );
    }
}

#[test]
fn c1_13_metrics_carry_traffic_counter_values() {
    let hub = MetricsHub::new();
    hub.on_connection_open();
    hub.on_connection_open(); // 2 active
    hub.on_transfer_received(); // 1 received
    hub.on_transfer_received();
    hub.on_transfer_received(); // 3 received total

    let samples = metrics_snapshot(&hub, 0);

    let active = extract_value(&samples, names::CONNECTIONS_ACTIVE).unwrap();
    let received = extract_value(&samples, names::TRANSFERS_RECEIVED).unwrap();
    assert_eq!(active, 2);
    assert_eq!(received, 3);
}

#[test]
fn c1_13_metrics_units_are_spec_symbols() {
    let hub = MetricsHub::new();
    let samples = metrics_snapshot(&hub, 0);
    // transfers.rate hat Unit `per-second`, alle anderen `count`.
    for sample in samples {
        let entries = match sample {
            AmqpExtValue::Map(v) => v,
            _ => panic!(),
        };
        let name = entries
            .iter()
            .find(|(k, _)| matches!(k, AmqpExtValue::Str(s) if s == "name"))
            .map(|(_, v)| v.clone())
            .unwrap();
        let unit = entries
            .iter()
            .find(|(k, _)| matches!(k, AmqpExtValue::Str(s) if s == "unit"))
            .map(|(_, v)| v.clone())
            .unwrap();
        let name_str = match name {
            AmqpExtValue::Str(s) => s,
            _ => panic!(),
        };
        let unit_str = match unit {
            AmqpExtValue::Symbol(s) => s,
            _ => panic!("unit must be symbol"),
        };
        let expected = if name_str == "transfers.rate" {
            "per-second"
        } else {
            "count"
        };
        assert_eq!(unit_str, expected, "metric '{name_str}' wrong unit");
    }
}

fn extract_value(samples: &[AmqpExtValue], metric_name: &str) -> Option<i64> {
    for s in samples {
        let entries = match s {
            AmqpExtValue::Map(v) => v,
            _ => continue,
        };
        let name = entries
            .iter()
            .find(|(k, _)| matches!(k, AmqpExtValue::Str(s) if s == "name"))
            .map(|(_, v)| v.clone())?;
        if name == AmqpExtValue::Str(metric_name.to_string()) {
            let v = entries
                .iter()
                .find(|(k, _)| matches!(k, AmqpExtValue::Str(s) if s == "value"))
                .map(|(_, v)| v.clone())?;
            if let AmqpExtValue::Long(n) = v {
                return Some(n);
            }
        }
    }
    None
}
