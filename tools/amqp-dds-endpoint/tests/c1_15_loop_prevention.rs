#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr
)]

//! Annex C C.1.15 — Loop-Prevention.
//!
//! Spec §C.1.15: zwei Sub-Tests:
//! (a) a sample with its own bridge_id is dropped,
//!     `transfers.dropped.loop` inkrementiert.
//! (b) a sample with `dds:bridge-hop` > cap is dropped,
//!     `transfers.dropped.hop-cap` inkrementiert.

mod common;

use zerodds_amqp_bridge::extended_types::AmqpExtValue;
use zerodds_amqp_endpoint::coexistence::{
    CoexistenceConfig, InboundDecision, inspect_inbound, stamp_outbound,
};
use zerodds_amqp_endpoint::metrics::{MetricsHub, names};
use zerodds_amqp_endpoint::properties::app_keys;

fn props(entries: Vec<(&str, AmqpExtValue)>) -> AmqpExtValue {
    AmqpExtValue::Map(
        entries
            .into_iter()
            .map(|(k, v)| (AmqpExtValue::Str(k.to_string()), v))
            .collect(),
    )
}

#[test]
fn c1_15a_sample_with_own_bridge_id_is_dropped() {
    // Spec §C.1.15 (a): sample whose `dds:bridge-id` matches its own
    // contains bridge_id MUST be dropped.
    let cfg = CoexistenceConfig::new("self-bridge-uuid".to_string());
    let metrics = MetricsHub::new();

    let sample_props = props(vec![(
        app_keys::BRIDGE_ID,
        AmqpExtValue::Str("self-bridge-uuid".into()),
    )]);

    let decision = inspect_inbound(&cfg, &sample_props);
    assert_eq!(decision, InboundDecision::DropLoop);

    if decision == InboundDecision::DropLoop {
        metrics.on_dropped_loop();
    }
    assert_eq!(metrics.snapshot(names::TRANSFERS_DROPPED_LOOP), Some(1));
    assert_eq!(metrics.snapshot(names::TRANSFERS_DROPPED_HOP_CAP), Some(0));
}

#[test]
fn c1_15a_sample_with_other_bridge_id_in_list_is_dropped() {
    // A self-tag in a list of other IDs is also detected.
    let cfg = CoexistenceConfig::new("self-uuid".to_string());
    let sample_props = props(vec![(
        app_keys::BRIDGE_ID,
        AmqpExtValue::List(vec![
            AmqpExtValue::Str("first-bridge".into()),
            AmqpExtValue::Str("self-uuid".into()),
            AmqpExtValue::Str("third-bridge".into()),
        ]),
    )]);

    assert_eq!(
        inspect_inbound(&cfg, &sample_props),
        InboundDecision::DropLoop
    );
}

#[test]
fn c1_15b_sample_with_hop_above_cap_is_dropped() {
    // Spec §C.1.15 (b): hop-Counter > cap → drop.
    let mut cfg = CoexistenceConfig::new("self".to_string());
    cfg.hop_cap = 8;
    let metrics = MetricsHub::new();

    // Sample with hop=9 > cap=8.
    let sample_props = props(vec![(app_keys::BRIDGE_HOP, AmqpExtValue::Uint(9))]);
    let decision = inspect_inbound(&cfg, &sample_props);
    assert_eq!(decision, InboundDecision::DropHopCap);

    if decision == InboundDecision::DropHopCap {
        metrics.on_dropped_hop_cap();
    }
    assert_eq!(metrics.snapshot(names::TRANSFERS_DROPPED_HOP_CAP), Some(1));
    assert_eq!(metrics.snapshot(names::TRANSFERS_DROPPED_LOOP), Some(0));
}

#[test]
fn c1_15b_sample_at_exact_cap_is_forwarded() {
    // Edge case: hop=cap is still allowed; only >cap drops.
    let mut cfg = CoexistenceConfig::new("self".to_string());
    cfg.hop_cap = 8;
    let sample_props = props(vec![(app_keys::BRIDGE_HOP, AmqpExtValue::Uint(8))]);
    assert_eq!(
        inspect_inbound(&cfg, &sample_props),
        InboundDecision::Forward
    );
}

#[test]
fn c1_15_round_trip_stamp_then_inspect_self_drops() {
    // Spec §7.11.2 Outbound-Stamp → §7.11.3 Self-Tag-Drop.
    // We stamp an outbound sample and then inspect it
    // ourselves — must return DropLoop.
    let cfg = CoexistenceConfig::new("loop-uuid".to_string());
    let mut p = AmqpExtValue::Map(Vec::new());
    stamp_outbound(&cfg, &mut p);
    assert_eq!(inspect_inbound(&cfg, &p), InboundDecision::DropLoop);
}

#[test]
fn c1_15_outbound_stamp_increments_hop_counter() {
    // Spec §7.11.2: on every outbound stamp the hop counter
    // um 1 erhoeht.
    let cfg = CoexistenceConfig::new("first-bridge".to_string());
    let mut p = AmqpExtValue::Map(Vec::new());
    stamp_outbound(&cfg, &mut p);
    // Erste Stamp setzt hop=1.
    let entries = match &p {
        AmqpExtValue::Map(v) => v,
        _ => panic!(),
    };
    let hop = entries
        .iter()
        .find(|(k, _)| matches!(k, AmqpExtValue::Str(s) if s == app_keys::BRIDGE_HOP))
        .map(|(_, v)| v.clone())
        .unwrap();
    assert_eq!(hop, AmqpExtValue::Uint(1));

    // Second stamp (in the path of a multi-hop bridge forward):
    // hop=2.
    let cfg2 = CoexistenceConfig::new("second-bridge".to_string());
    stamp_outbound(&cfg2, &mut p);
    let entries = match &p {
        AmqpExtValue::Map(v) => v,
        _ => panic!(),
    };
    let hop = entries
        .iter()
        .find(|(k, _)| matches!(k, AmqpExtValue::Str(s) if s == app_keys::BRIDGE_HOP))
        .map(|(_, v)| v.clone())
        .unwrap();
    assert_eq!(hop, AmqpExtValue::Uint(2));
}

#[test]
fn c1_15_clean_sample_passes_through() {
    // A sample without bridge metadata is forwarded.
    let cfg = CoexistenceConfig::new("self".to_string());
    let p = props(vec![]);
    assert_eq!(inspect_inbound(&cfg, &p), InboundDecision::Forward);
}
