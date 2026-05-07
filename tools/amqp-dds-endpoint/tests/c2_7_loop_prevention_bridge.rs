#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr
)]

//! Annex C C.2.7 — Loop-Prevention (Bridge-Profile).
//!
//! Spec §C.2.7: zwei Sub-Tests, beide MUESSEN passen — analog
//! zu C.1.15, aber aus Bridge-Profile-Sicht (Outbound + Inbound).
//!
//! (a) Sample, das die Bridge selbst stamped hat und das ueber
//!     den Broker zurueckkommt → Drop, `transfers.dropped.loop`.
//! (b) Sample mit `dds:bridge-hop > cap` → Drop,
//!     `transfers.dropped.hop-cap`.
//!
//! Wir testen das auf der Coexistence-Layer-API. Echte
//! Bridge-zu-Broker-zu-Bridge-Roundtrips brauchen DDS-Side-
//! Bridge (siehe DDS-Side-Bridge-Welle).

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
fn c2_7a_outbound_then_inbound_round_trip_drops_self() {
    // Spec §C.2.7 (a): Bridge sendet Sample (stamp_outbound),
    // Sample kommt ueber Broker zurueck (inspect_inbound). Eigene
    // bridge_id im Tag → DropLoop.
    let cfg = CoexistenceConfig::new("bridge-uuid-7a".to_string());
    let metrics = MetricsHub::new();

    // Outbound-Stamp: Bridge schickt Sample raus, fuegt eigene
    // bridge_id hinzu.
    let mut outbound = AmqpExtValue::Map(Vec::new());
    stamp_outbound(&cfg, &mut outbound);

    // Sample wandert durch den Broker und kommt als Inbound
    // zurueck. inspect_inbound MUSS DropLoop liefern.
    let decision = inspect_inbound(&cfg, &outbound);
    assert_eq!(decision, InboundDecision::DropLoop);

    if decision == InboundDecision::DropLoop {
        metrics.on_dropped_loop();
    }
    assert_eq!(metrics.snapshot(names::TRANSFERS_DROPPED_LOOP), Some(1));
}

#[test]
fn c2_7b_hop_cap_exceeded_drops_with_metric() {
    // Spec §C.2.7 (b): Sample mit hop > cap droppt + Counter.
    let mut cfg = CoexistenceConfig::new("bridge-uuid-7b".to_string());
    cfg.hop_cap = 4; // niedriger Test-Cap
    let metrics = MetricsHub::new();

    let inbound = props(vec![(app_keys::BRIDGE_HOP, AmqpExtValue::Uint(5))]);
    let decision = inspect_inbound(&cfg, &inbound);
    assert_eq!(decision, InboundDecision::DropHopCap);

    if decision == InboundDecision::DropHopCap {
        metrics.on_dropped_hop_cap();
    }
    assert_eq!(metrics.snapshot(names::TRANSFERS_DROPPED_HOP_CAP), Some(1));
}

#[test]
fn c2_7_multi_bridge_chain_terminates_at_hop_cap() {
    // Bridge-Chain-Szenario: Sample wandert durch 5 Bridges,
    // jede stamped und inkrementiert hop. Bei cap=4 droppt
    // die 5. Bridge.
    let bridges = ["b1", "b2", "b3", "b4", "b5"];
    let mut sample = AmqpExtValue::Map(Vec::new());

    // Erste 4 Bridges stempeln.
    for id in &bridges[..4] {
        let cfg = CoexistenceConfig::new((*id).into());
        stamp_outbound(&cfg, &mut sample);
    }

    // 5. Bridge inspiziert mit cap=4 → DropHopCap (hop ist
    // nach 4 Stamps = 4 = cap, also genau auf der Grenze;
    // nach Spec §7.11.4 ist hop > cap drop, hop = cap forwarded.
    // Also bei cap=3 droppt Bridge 5.
    let mut cfg = CoexistenceConfig::new(bridges[4].into());
    cfg.hop_cap = 3;
    let decision = inspect_inbound(&cfg, &sample);
    assert_eq!(decision, InboundDecision::DropHopCap);
}

#[test]
fn c2_7_foreign_bridges_in_history_dont_trigger_self_drop() {
    // Bridge sieht eine Liste anderer Bridges in der History,
    // aber nicht ihre eigene → Forward.
    let cfg = CoexistenceConfig::new("self-bridge".to_string());
    let p = props(vec![
        (
            app_keys::BRIDGE_ID,
            AmqpExtValue::List(vec![
                AmqpExtValue::Str("foreign-1".into()),
                AmqpExtValue::Str("foreign-2".into()),
                AmqpExtValue::Str("foreign-3".into()),
            ]),
        ),
        (app_keys::BRIDGE_HOP, AmqpExtValue::Uint(3)),
    ]);
    assert_eq!(inspect_inbound(&cfg, &p), InboundDecision::Forward);
}
