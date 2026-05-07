// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Bridge-Coexistence Loop-Prevention.
//!
//! Spec-Quelle: dds-amqp-1.0 §7.11 (`sec:bridge-coexistence`).
//! Endpoint und Bridge teilen denselben DDS-Domain mit weiteren
//! Forwarding-Agents (andere DDS-AMQP-Endpoints, MQTT-DDS-Bridges,
//! DDS-Web-Gateways). Ohne explizite Loop-Prevention re-eintritt
//! ein bereits weitergeleitetes Sample durch DDS und wird zweimal
//! forwarded → exponentielle Duplikation.
//!
//! Drei Mechanismen:
//! 1. **Self-Tag Drop**: enthaelt `dds:bridge-id` die eigene
//!    `bridge_id`, wird das Sample silently gedroppt.
//! 2. **Hop-Cap**: `dds:bridge-hop > bridge_hop_cap` → drop.
//! 3. **Outbound-Stamp**: beim Forwarden eigene `bridge_id`
//!    anhaengen + `dds:bridge-hop` inkrementieren.
//!
//! Wires gegen die `dds:bridge-id`/`dds:bridge-hop`-App-Properties
//! aus [`crate::properties::app_keys`].

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use zerodds_amqp_bridge::extended_types::AmqpExtValue;

use crate::properties::app_keys;

/// Bridge-Identifier (RFC-4122 UUID als 36-char canonical string).
/// Process-wide stabil — Spec §7.11 verbietet pro-Topic-Werte.
pub type BridgeId = String;

/// Spec-Default + Maximum fuer den Hop-Cap.
pub const DEFAULT_HOP_CAP: u32 = 8;
/// Spec-Maximum fuer den Hop-Cap (operatorisch erhoehbar bis hier).
pub const MAX_HOP_CAP: u32 = 16;

/// Konfiguration der Loop-Prevention pro Endpoint/Bridge-Prozess.
#[derive(Debug, Clone)]
pub struct CoexistenceConfig {
    /// Eigene stabile UUID. Spec §7.11.
    pub bridge_id: BridgeId,
    /// Hop-Cap (default 8, max 16 unless explicitly overridden).
    pub hop_cap: u32,
}

impl CoexistenceConfig {
    /// Konstruktor mit Default-Hop-Cap.
    #[must_use]
    pub fn new(bridge_id: BridgeId) -> Self {
        Self {
            bridge_id,
            hop_cap: DEFAULT_HOP_CAP,
        }
    }
}

/// Spec §7.11 — Resultat einer Inbound-Inspektion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InboundDecision {
    /// Sample weiter-forwarden (kein Loop, hop in cap).
    Forward,
    /// Self-Tag erkannt → silently droppen, `transfers.dropped.loop`
    /// inkrementieren.
    DropLoop,
    /// Hop-Cap ueberschritten → silently droppen,
    /// `transfers.dropped.hop-cap` inkrementieren.
    DropHopCap,
}

/// Spec §7.11.3 (Inbound Drop) + §7.11.4 (Hop-Cap) — eingehende
/// Application-Properties pruefen.
///
/// `app_props` ist die Application-Properties-Map (typischerweise
/// das Map-Body-Composite des `application-properties`-Sections).
/// Liefert das Routing-Verdikt.
///
/// `bridge-id` wird als comma-separated string oder als
/// `AmqpExtValue::List<Str>` akzeptiert (Spec §8.3 erlaubt beide
/// Repraesentationen fuer history-Listen).
#[must_use]
pub fn inspect_inbound(cfg: &CoexistenceConfig, app_props: &AmqpExtValue) -> InboundDecision {
    // Hop-Cap zuerst — schneller Pfad.
    if let Some(hop) = read_uint_key(app_props, app_keys::BRIDGE_HOP) {
        if hop > cfg.hop_cap {
            return InboundDecision::DropHopCap;
        }
    }
    // Self-Tag-Drop: liegt eigene bridge_id in der bridge-id-Liste?
    if has_self_tag(app_props, &cfg.bridge_id) {
        return InboundDecision::DropLoop;
    }
    InboundDecision::Forward
}

/// Spec §7.11.2 (Outbound Stamp) — beim Forwarden eigene
/// `bridge_id` anhaengen + `dds:bridge-hop` inkrementieren.
///
/// Mutiert die uebergebene Map. Erstellt die Properties wenn nicht
/// vorhanden.
pub fn stamp_outbound(cfg: &CoexistenceConfig, app_props: &mut AmqpExtValue) {
    if !matches!(app_props, AmqpExtValue::Map(_)) {
        // Falsch-getypte Properties-Section: ueberschreiben mit
        // initialer Map (defensives Verhalten — Spec laesst
        // diesen Fall offen, aber fail-open ist falsch).
        *app_props = AmqpExtValue::Map(Vec::new());
    }
    let AmqpExtValue::Map(entries) = app_props else {
        return;
    };

    // bridge-id anhaengen.
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
            // Falscher Typ in vorhandenem Eintrag — ueberschreiben.
            *slot = AmqpExtValue::Str(cfg.bridge_id.clone());
            None
        }
        None => Some(AmqpExtValue::Str(cfg.bridge_id.clone())),
    };
    if let Some(v) = new_id_value {
        entries.push((key_id, v));
    }

    // bridge-hop inkrementieren.
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

// ---------------- Hilfen ----------------

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
        // Hop = cap (=) ist noch erlaubt, > cap droppt.
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
        // Wenn ich stample und dann selbst inspect — der eigene
        // Tag muss zu DropLoop fuehren.
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
