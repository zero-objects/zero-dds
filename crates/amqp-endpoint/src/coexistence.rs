// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Bridge-coexistence loop prevention.
//!
//! Spec source: dds-amqp-1.0 §7.11 (`sec:bridge-coexistence`).
//! Endpoint and bridge share the same DDS domain with other
//! forwarding agents (other DDS-AMQP endpoints, MQTT-DDS bridges,
//! DDS web gateways). Without explicit loop prevention, an
//! already-forwarded sample re-enters through DDS and is forwarded
//! twice → exponential duplication.
//!
//! Three mechanisms:
//! 1. **Self-tag drop**: if `dds:bridge-id` contains our own
//!    `bridge_id`, the sample is silently dropped.
//! 2. **Hop cap**: `dds:bridge-hop > bridge_hop_cap` → drop.
//! 3. **Outbound stamp**: when forwarding, append our own `bridge_id`
//!    + increment `dds:bridge-hop`.
//!
//! Wired against the `dds:bridge-id`/`dds:bridge-hop` app properties
//! from [`crate::properties::app_keys`].

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use zerodds_amqp_bridge::extended_types::AmqpExtValue;

use crate::properties::app_keys;

/// Bridge identifier (RFC-4122 UUID as a 36-char canonical string).
/// Process-wide stable — spec §7.11 forbids per-topic values.
pub type BridgeId = String;

/// Spec default + maximum for the hop cap.
pub const DEFAULT_HOP_CAP: u32 = 8;
/// Spec maximum for the hop cap (operator-raisable up to this).
pub const MAX_HOP_CAP: u32 = 16;

/// Loop-prevention configuration per endpoint/bridge process.
#[derive(Debug, Clone)]
pub struct CoexistenceConfig {
    /// Our own stable UUID. Spec §7.11.
    pub bridge_id: BridgeId,
    /// Hop cap (default 8, max 16 unless explicitly overridden).
    pub hop_cap: u32,
}

impl CoexistenceConfig {
    /// Constructor with the default hop cap.
    #[must_use]
    pub fn new(bridge_id: BridgeId) -> Self {
        Self {
            bridge_id,
            hop_cap: DEFAULT_HOP_CAP,
        }
    }
}

/// Spec §7.11 — result of an inbound inspection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InboundDecision {
    /// Forward the sample onward (no loop, hop within cap).
    Forward,
    /// Self-tag detected → silently drop, increment
    /// `transfers.dropped.loop`.
    DropLoop,
    /// Hop cap exceeded → silently drop, increment
    /// `transfers.dropped.hop-cap`.
    DropHopCap,
}

/// Spec §7.11.3 (inbound drop) + §7.11.4 (hop cap) — inspect
/// incoming application properties.
///
/// `app_props` is the application-properties map (typically the
/// map body composite of the `application-properties` section).
/// Returns the routing verdict.
///
/// `bridge-id` is accepted as a comma-separated string or as
/// `AmqpExtValue::List<Str>` (spec §8.3 allows both representations
/// for history lists).
#[must_use]
pub fn inspect_inbound(cfg: &CoexistenceConfig, app_props: &AmqpExtValue) -> InboundDecision {
    // Hop cap first — fast path.
    if let Some(hop) = read_uint_key(app_props, app_keys::BRIDGE_HOP) {
        if hop > cfg.hop_cap {
            return InboundDecision::DropHopCap;
        }
    }
    // Self-tag drop: is our own bridge_id in the bridge-id list?
    if has_self_tag(app_props, &cfg.bridge_id) {
        return InboundDecision::DropLoop;
    }
    InboundDecision::Forward
}

/// Spec §7.11.2 (outbound stamp) — when forwarding, append our own
/// `bridge_id` + increment `dds:bridge-hop`.
///
/// Mutates the passed map. Creates the properties if absent.
pub fn stamp_outbound(cfg: &CoexistenceConfig, app_props: &mut AmqpExtValue) {
    if !matches!(app_props, AmqpExtValue::Map(_)) {
        // Wrong-typed properties section: overwrite with an
        // initial map (defensive behavior — the spec leaves this
        // case open, but fail-open is wrong).
        *app_props = AmqpExtValue::Map(Vec::new());
    }
    let AmqpExtValue::Map(entries) = app_props else {
        return;
    };

    // Append bridge-id.
    let key_id = AmqpExtValue::Str(app_keys::BRIDGE_ID.to_string());
    let new_id_value = match entries.iter_mut().find(|(k, _)| k == &key_id) {
        Some((_, AmqpExtValue::Str(existing))) => {
            existing.push(',');
            existing.push_str(&cfg.bridge_id);
            None
        }
        Some((_, AmqpExtValue::List(list))) => {
            list.push(AmqpExtValue::Str(cfg.bridge_id.clone()));
            None
        }
        Some((_, slot)) => {
            // Wrong type in an existing entry — overwrite.
            *slot = AmqpExtValue::Str(cfg.bridge_id.clone());
            None
        }
        None => Some(AmqpExtValue::Str(cfg.bridge_id.clone())),
    };
    if let Some(v) = new_id_value {
        entries.push((key_id, v));
    }

    // Increment bridge-hop.
    let key_hop = AmqpExtValue::Str(app_keys::BRIDGE_HOP.to_string());
    let mut bumped = false;
    for (k, v) in entries.iter_mut() {
        if k == &key_hop {
            match v {
                AmqpExtValue::Uint(n) => {
                    *n = n.saturating_add(1);
                }
                AmqpExtValue::Ulong(n) => {
                    *n = n.saturating_add(1);
                }
                _ => *v = AmqpExtValue::Uint(1),
            }
            bumped = true;
            break;
        }
    }
    if !bumped {
        entries.push((key_hop, AmqpExtValue::Uint(1)));
    }
}

// ---------------- Helpers ----------------

fn read_uint_key(app_props: &AmqpExtValue, key: &str) -> Option<u32> {
    let entries = match app_props {
        AmqpExtValue::Map(v) => v,
        _ => return None,
    };
    let want = AmqpExtValue::Str(key.to_string());
    for (k, v) in entries {
        if *k == want {
            return match v {
                AmqpExtValue::Uint(n) => Some(*n),
                AmqpExtValue::Ulong(n) => u32::try_from(*n).ok(),
                AmqpExtValue::Int(n) if *n >= 0 => u32::try_from(*n).ok(),
                _ => None,
            };
        }
    }
    None
}

fn has_self_tag(app_props: &AmqpExtValue, my_id: &str) -> bool {
    let entries = match app_props {
        AmqpExtValue::Map(v) => v,
        _ => return false,
    };
    let want = AmqpExtValue::Str(app_keys::BRIDGE_ID.to_string());
    for (k, v) in entries {
        if *k != want {
            continue;
        }
        return match v {
            AmqpExtValue::Str(s) => s.split(',').any(|tok| tok.trim() == my_id),
            AmqpExtValue::List(items) => items
                .iter()
                .any(|i| matches!(i, AmqpExtValue::Str(s) if s == my_id)),
            _ => false,
        };
    }
    false
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    fn cfg(id: &str) -> CoexistenceConfig {
        CoexistenceConfig::new(id.to_string())
    }

    fn props_with(entries: Vec<(&str, AmqpExtValue)>) -> AmqpExtValue {
        AmqpExtValue::Map(
            entries
                .into_iter()
                .map(|(k, v)| (AmqpExtValue::Str(k.to_string()), v))
                .collect(),
        )
    }

    #[test]
    fn forward_when_no_bridge_metadata() {
        let p = props_with(alloc::vec![]);
        assert_eq!(
            inspect_inbound(&cfg("self-uuid"), &p),
            InboundDecision::Forward
        );
    }

    #[test]
    fn drop_loop_on_self_tag_string() {
        let p = props_with(alloc::vec![(
            app_keys::BRIDGE_ID,
            AmqpExtValue::Str("other,self-uuid,third".into()),
        )]);
        assert_eq!(
            inspect_inbound(&cfg("self-uuid"), &p),
            InboundDecision::DropLoop
        );
    }

    #[test]
    fn drop_loop_on_self_tag_list() {
        let p = props_with(alloc::vec![(
            app_keys::BRIDGE_ID,
            AmqpExtValue::List(alloc::vec![
                AmqpExtValue::Str("other".into()),
                AmqpExtValue::Str("self-uuid".into()),
            ]),
        )]);
        assert_eq!(
            inspect_inbound(&cfg("self-uuid"), &p),
            InboundDecision::DropLoop
        );
    }

    #[test]
    fn drop_hop_cap_when_exceeded() {
        let p = props_with(alloc::vec![(app_keys::BRIDGE_HOP, AmqpExtValue::Uint(9))]);
        let mut c = cfg("self-uuid");
        c.hop_cap = 8;
        assert_eq!(inspect_inbound(&c, &p), InboundDecision::DropHopCap);
    }

    #[test]
    fn forward_at_hop_cap_limit() {
        // Hop = cap (=) is still allowed, > cap drops.
        let p = props_with(alloc::vec![(app_keys::BRIDGE_HOP, AmqpExtValue::Uint(8))]);
        let mut c = cfg("self-uuid");
        c.hop_cap = 8;
        assert_eq!(inspect_inbound(&c, &p), InboundDecision::Forward);
    }

    #[test]
    fn stamp_outbound_creates_initial_entries() {
        let mut p = AmqpExtValue::Map(Vec::new());
        stamp_outbound(&cfg("my-id"), &mut p);
        let entries = match &p {
            AmqpExtValue::Map(v) => v,
            _ => panic!(),
        };
        let id = entries
            .iter()
            .find(|(k, _)| matches!(k, AmqpExtValue::Str(s) if s == app_keys::BRIDGE_ID))
            .unwrap();
        assert_eq!(id.1, AmqpExtValue::Str("my-id".into()));
        let hop = entries
            .iter()
            .find(|(k, _)| matches!(k, AmqpExtValue::Str(s) if s == app_keys::BRIDGE_HOP))
            .unwrap();
        assert_eq!(hop.1, AmqpExtValue::Uint(1));
    }

    #[test]
    fn stamp_outbound_appends_to_existing_string_list() {
        let mut p = props_with(alloc::vec![
            (app_keys::BRIDGE_ID, AmqpExtValue::Str("first-id".into()),),
            (app_keys::BRIDGE_HOP, AmqpExtValue::Uint(2)),
        ]);
        stamp_outbound(&cfg("second-id"), &mut p);
        let entries = match &p {
            AmqpExtValue::Map(v) => v,
            _ => panic!(),
        };
        let id = entries
            .iter()
            .find(|(k, _)| matches!(k, AmqpExtValue::Str(s) if s == app_keys::BRIDGE_ID))
            .unwrap();
        assert_eq!(id.1, AmqpExtValue::Str("first-id,second-id".into()));
        let hop = entries
            .iter()
            .find(|(k, _)| matches!(k, AmqpExtValue::Str(s) if s == app_keys::BRIDGE_HOP))
            .unwrap();
        assert_eq!(hop.1, AmqpExtValue::Uint(3));
    }

    #[test]
    fn stamp_outbound_appends_to_list_form() {
        let mut p = props_with(alloc::vec![
            (
                app_keys::BRIDGE_ID,
                AmqpExtValue::List(alloc::vec![AmqpExtValue::Str("first".into())]),
            ),
            (app_keys::BRIDGE_HOP, AmqpExtValue::Uint(1)),
        ]);
        stamp_outbound(&cfg("second"), &mut p);
        let entries = match &p {
            AmqpExtValue::Map(v) => v,
            _ => panic!(),
        };
        let id = entries
            .iter()
            .find(|(k, _)| matches!(k, AmqpExtValue::Str(s) if s == app_keys::BRIDGE_ID))
            .unwrap();
        match &id.1 {
            AmqpExtValue::List(items) => {
                assert_eq!(items.len(), 2);
                assert_eq!(items[0], AmqpExtValue::Str("first".into()));
                assert_eq!(items[1], AmqpExtValue::Str("second".into()));
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn round_trip_stamp_then_inspect_drops_loop() {
        // If I stamp and then inspect myself — my own tag must
        // lead to DropLoop.
        let mut p = AmqpExtValue::Map(Vec::new());
        let c = cfg("loop-uuid");
        stamp_outbound(&c, &mut p);
        assert_eq!(inspect_inbound(&c, &p), InboundDecision::DropLoop);
    }

    #[test]
    fn other_bridges_tag_does_not_drop() {
        let mut p = AmqpExtValue::Map(Vec::new());
        stamp_outbound(&cfg("other-bridge"), &mut p);
        assert_eq!(
            inspect_inbound(&cfg("self-uuid"), &p),
            InboundDecision::Forward
        );
    }

    #[test]
    fn config_defaults_match_spec() {
        let c = CoexistenceConfig::new("x".to_string());
        assert_eq!(c.hop_cap, DEFAULT_HOP_CAP);
        assert_eq!(DEFAULT_HOP_CAP, 8);
        assert_eq!(MAX_HOP_CAP, 16);
    }
}
