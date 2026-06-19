//! E2E multi-bridge-hop test for DDS-AMQP §7.11 coexistence.
//!
//! Simulates a chain of 3+ bridges through which a sample is
//! propagated. Verifies that:
//!
//! 1. Self-tag drop takes effect when the sample returns to the
//!    originating bridge.
//! 2. The hop cap takes effect for an overly long chain.
//! 3. The outbound stamp increments `dds:bridge-hop` correctly.
//! 4. The bridge ID list is accumulated along the traversal.

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

    // Bridge A starts the sample.
    let mut props = empty_props();
    stamp_outbound(&bridge_a, &mut props);
    assert_eq!(read_hop(&props), Some(1));
    assert_eq!(read_bridge_ids(&props).as_deref(), Some("bridge-A"));

    // Bridge B inspect → Forward, then stamp.
    assert_eq!(inspect_inbound(&bridge_b, &props), InboundDecision::Forward);
    stamp_outbound(&bridge_b, &mut props);
    assert_eq!(read_hop(&props), Some(2));
    assert_eq!(
        read_bridge_ids(&props).as_deref(),
        Some("bridge-A,bridge-B")
    );

    // Bridge C inspect → Forward, then stamp.
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

    // A stamps, B forwards + stamps, then A inspects the same sample
    // again → self-tag drop.
    let mut props = empty_props();
    stamp_outbound(&bridge_a, &mut props);
    assert_eq!(inspect_inbound(&bridge_b, &props), InboundDecision::Forward);
    stamp_outbound(&bridge_b, &mut props);
    // Now back to A.
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
        // After DEFAULT_HOP_CAP=8 hops + 1 inspection it should be dropped.
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
    // With hop_cap=3 the filter allows hops 0,1,2,3 — i.e. 4 bridges
    // before the next inspect drops hop=4 as > 3.
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
    // A → B → C, then C → A directly (diamond/loop): A must recognize
    // its own bridge-id in the sample → DropLoop.
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
