//! E2E-Multi-Bridge-Hop-Test fuer DDS-AMQP §7.11 Coexistence.
//!
//! Simuliert eine Kette von 3+ Bridges, durch die ein Sample
//! propagiert wird. Verifiziert dass:
//!
//! 1. Self-Tag-Drop greift wenn das Sample zur Ursprungs-Bridge
//!    zurueckkommt.
//! 2. Hop-Cap greift bei zu langer Kette.
//! 3. Outbound-Stamp inkrementiert `dds:bridge-hop` korrekt.
//! 4. Bridge-ID-Liste wird beim Durchlauf akkumuliert.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use zerodds_amqp_bridge::extended_types::AmqpExtValue;
use zerodds_amqp_endpoint::coexistence::{
    CoexistenceConfig, DEFAULT_HOP_CAP, InboundDecision, inspect_inbound, stamp_outbound,
};
use zerodds_amqp_endpoint::properties::app_keys;

fn cfg(id: &str) -> CoexistenceConfig {
    CoexistenceConfig::new(id.to_string())
}

fn cfg_with_cap(id: &str, cap: u32) -> CoexistenceConfig {
    CoexistenceConfig {
        bridge_id: id.to_string(),
        hop_cap: cap,
    }
}

fn empty_props() -> AmqpExtValue {
    AmqpExtValue::Map(Vec::new())
}

fn read_hop(props: &AmqpExtValue) -> Option<u32> {
    let AmqpExtValue::Map(entries) = props else {
        return None;
    };
    let key = AmqpExtValue::Str(app_keys::BRIDGE_HOP.to_string());
    for (k, v) in entries {
        if *k == key {
            return match v {
                AmqpExtValue::Uint(n) => Some(*n),
                AmqpExtValue::Ulong(n) => u32::try_from(*n).ok(),
                _ => None,
            };
        }
    }
    None
}

fn read_bridge_ids(props: &AmqpExtValue) -> Option<String> {
    let AmqpExtValue::Map(entries) = props else {
        return None;
    };
    let key = AmqpExtValue::Str(app_keys::BRIDGE_ID.to_string());
    for (k, v) in entries {
        if *k == key {
            if let AmqpExtValue::Str(s) = v {
                return Some(s.clone());
            }
        }
    }
    None
}

#[test]
fn three_bridge_linear_chain_propagates() {
    let bridge_a = cfg("bridge-A");
    let bridge_b = cfg("bridge-B");
    let bridge_c = cfg("bridge-C");

    // Bridge A startet das Sample.
    let mut props = empty_props();
    stamp_outbound(&bridge_a, &mut props);
    assert_eq!(read_hop(&props), Some(1));
    assert_eq!(read_bridge_ids(&props).as_deref(), Some("bridge-A"));

    // Bridge B inspect → Forward, dann stamp.
    assert_eq!(inspect_inbound(&bridge_b, &props), InboundDecision::Forward);
    stamp_outbound(&bridge_b, &mut props);
    assert_eq!(read_hop(&props), Some(2));
    assert_eq!(
        read_bridge_ids(&props).as_deref(),
        Some("bridge-A,bridge-B")
    );

    // Bridge C inspect → Forward, dann stamp.
    assert_eq!(inspect_inbound(&bridge_c, &props), InboundDecision::Forward);
    stamp_outbound(&bridge_c, &mut props);
    assert_eq!(read_hop(&props), Some(3));
    assert_eq!(
        read_bridge_ids(&props).as_deref(),
        Some("bridge-A,bridge-B,bridge-C")
    );
}

#[test]
fn loop_back_to_origin_drops_self_tag() {
    let bridge_a = cfg("bridge-A");
    let bridge_b = cfg("bridge-B");

    // A stamps, B forwards + stamps, dann A inspect das gleiche Sample
    // wieder → Self-Tag-Drop.
    let mut props = empty_props();
    stamp_outbound(&bridge_a, &mut props);
    assert_eq!(inspect_inbound(&bridge_b, &props), InboundDecision::Forward);
    stamp_outbound(&bridge_b, &mut props);
    // Nun zurueck nach A.
    assert_eq!(
        inspect_inbound(&bridge_a, &props),
        InboundDecision::DropLoop
    );
}

#[test]
fn hop_cap_drops_after_default_8_hops() {
    let bridges: Vec<CoexistenceConfig> = (0..15).map(|i| cfg(&format!("bridge-{i:02}"))).collect();

    let mut props = empty_props();
    let mut last_decision = InboundDecision::Forward;
    for (i, br) in bridges.iter().enumerate() {
        let decision = inspect_inbound(br, &props);
        if matches!(decision, InboundDecision::DropHopCap) {
            last_decision = decision;
            break;
        }
        // Nach DEFAULT_HOP_CAP=8 Hops + 1 Inspektion sollte gedroppt werden.
        assert!(
            i <= DEFAULT_HOP_CAP as usize,
            "expected hop-cap drop within {DEFAULT_HOP_CAP} hops"
        );
        stamp_outbound(br, &mut props);
    }
    assert_eq!(last_decision, InboundDecision::DropHopCap);
}

#[test]
fn explicit_hop_cap_3_drops_at_4th_bridge() {
    let bridges: Vec<CoexistenceConfig> = (0..6)
        .map(|i| cfg_with_cap(&format!("bridge-{i:02}"), 3))
        .collect();

    let mut props = empty_props();
    let mut visited = Vec::new();
    for br in bridges.iter() {
        let decision = inspect_inbound(br, &props);
        if matches!(decision, InboundDecision::DropHopCap) {
            break;
        }
        visited.push(br.bridge_id.clone());
        stamp_outbound(br, &mut props);
    }
    // Mit hop_cap=3 erlaubt der Filter Hops 0,1,2,3 — d.h. 4 Bridges
    // bevor der naechste inspect den Hop=4 als > 3 droppt.
    assert!(
        (3..=4).contains(&visited.len()),
        "expected 3-4 bridges before drop, got {}",
        visited.len()
    );
}

#[test]
fn hop_count_matches_chain_length() {
    let chain: Vec<CoexistenceConfig> = (0..6).map(|i| cfg(&format!("br-{i}"))).collect();
    let mut props = empty_props();
    for (i, br) in chain.iter().enumerate() {
        stamp_outbound(br, &mut props);
        assert_eq!(read_hop(&props), Some(i as u32 + 1));
    }
}

#[test]
fn diamond_topology_drops_self_loop_at_re_entry() {
    // A → B → C, dann C → A direkt (Diamond/Loop): A muss eigenes
    // bridge-id im Sample erkennen → DropLoop.
    let bridge_a = cfg("origin-A");
    let bridge_b = cfg("middle-B");
    let bridge_c = cfg("middle-C");

    let mut props = empty_props();
    stamp_outbound(&bridge_a, &mut props);
    stamp_outbound(&bridge_b, &mut props);
    stamp_outbound(&bridge_c, &mut props);

    assert_eq!(
        inspect_inbound(&bridge_a, &props),
        InboundDecision::DropLoop,
        "origin must drop on re-entry"
    );
}
