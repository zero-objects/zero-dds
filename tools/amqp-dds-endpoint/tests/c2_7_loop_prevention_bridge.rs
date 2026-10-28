#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr
)]

//! Annex C C.2.7 — Loop-Prevention (Bridge-Profile).
//!
//! Spec §C.2.7: zwei Sub-Tests, beide MUESSEN passen — analog
//! to C.1.15, but from the bridge profile perspective (outbound + inbound).
//!
//! (a) a sample the bridge stamped itself and that comes back
//!     comes back from the broker → drop, `transfers.dropped.loop`.
//! (b) sample with `dds:bridge-hop > cap` → drop,
//!     `transfers.dropped.hop-cap`.
//!
//! We test this on the coexistence layer API. Real
//! bridge-to-broker-to-bridge roundtrips need a DDS-side
//! bridge (see the DDS-side-bridge wave).

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
    // Spec §C.2.7 (a): the bridge sends a sample (stamp_outbound),
    // Sample comes back via the broker (inspect_inbound). Its own
    // bridge_id im Tag → DropLoop.
    let cfg = CoexistenceConfig::new("bridge-uuid-7a".to_string());
    let metrics = MetricsHub::new();

    // Outbound stamp: bridge sends the sample out, adds its own
    // bridge_id hinzu.
    let mut outbound = AmqpExtValue::Map(Vec::new());
    stamp_outbound(&cfg, &mut outbound);

    // Sample travels through the broker and arrives as inbound
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
    // Spec §C.2.7 (b): sample with hop > cap drops + counter.
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
    // Bridge chain scenario: sample travels through 5 bridges,
    // each stamps and increments hop. At cap=4 it drops
    // die 5. Bridge.
    let bridges = ["b1", "b2", "b3", "b4", "b5"];
    let mut sample = AmqpExtValue::Map(Vec::new());

    // Erste 4 Bridges stempeln.
    for id in &bridges[..4] {
        let cfg = CoexistenceConfig::new((*id).into());
        stamp_outbound(&cfg, &mut sample);
    }

    // 5th bridge inspects with cap=4 → DropHopCap (hop is
    // after 4 stamps = 4 = cap, i.e. exactly on the boundary;
    // per spec §7.11.4, hop > cap is drop, hop = cap forwarded.
    // So at cap=3, bridge 5 drops.
    let mut cfg = CoexistenceConfig::new(bridges[4].into());
    cfg.hop_cap = 3;
    let decision = inspect_inbound(&cfg, &sample);
    assert_eq!(decision, InboundDecision::DropHopCap);
}

#[test]
fn c2_7_foreign_bridges_in_history_dont_trigger_self_drop() {
    // The bridge sees a list of other bridges in the history,
    // but not its own → forward.
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
