// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Builtin DataTagging-Plugin (OMG DDS-Security 1.2 §12).
//!
//! Implementiert das [`zerodds_security::DataTaggingPlugin`]-SPI als
//! produktiven Builtin. Tags sind Application-Level-Labels
//! (Classification-Marker, Sensitivity, etc.), die per Endpoint-GUID
//! verwaltet, ueber SEDP via `PID_PROPERTY_LIST` propagiert und
//! Subscriber-seitig auf Match geprueft werden.
//!
//! # Wire-Pfad
//!
//! Tags werden als `WireProperty`-Eintraege mit Namespace-Prefix
//! `dds.sec.data_tags.` in die existierende [`WirePropertyList`]
//! eingebettet — d.h. wir reuten den bereits in SPDP/SEDP propagierten
//! `PID_PROPERTY_LIST`-Parameter, anstatt einen neuen PID einzufuehren.
//! Das passt zu Cyclones/RTI-Verhalten, das Tags ebenfalls via
//! `PID_PROPERTY_LIST` traegt (Cyclone DDS Security §8 doc).
//!
//! # Match-Predicate
//!
//! Default-Predicate ist Subset-Match:
//! * Subscriber ohne Tags → akzeptiert jeden Publisher (Wildcard).
//! * Subscriber mit Tags → jeder Tag (name+value) muss exakt im
//!   Publisher-Tag-Set vorkommen.
//! * Unknown Tag-Name auf Subscriber-Seite, der nicht beim Publisher
//!   existiert → Reject.
//!
//! Spec-konform und einfach genug fuer NGVA/FACE-Pilot-Setups; komplexe
//! Predicates (range/regex) werden via Custom-`DataTaggingPlugin`
//! abgedeckt.
//!
//! # Beispiel
//!
//! ```no_run
//! use zerodds_security::data_tagging::{DataTag, DataTaggingPlugin};
//! use zerodds_security_runtime::data_tagging::BuiltinDataTaggingPlugin;
//!
//! let mut plugin = BuiltinDataTaggingPlugin::new();
//! let pub_guid = [0xAA; 16];
//! plugin.set_tags(
//!     pub_guid,
//!     vec![DataTag { name: "classification".into(), value: "secret".into() }],
//! );
//!
//! let sub_tags = vec![DataTag { name: "classification".into(), value: "secret".into() }];
//! assert!(BuiltinDataTaggingPlugin::tags_match(&plugin.get_tags(pub_guid), &sub_tags));
//! ```
//!
//! zerodds-lint: allow no_dyn_in_safe
//! (Plugin-Trait-Object via `Box<dyn DataTaggingPlugin>`.)

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use zerodds_rtps::property_list::{WireProperty, WirePropertyList};
use zerodds_security::data_tagging::{DataTag, DataTaggingPlugin};

/// Property-Namespace fuer Tag-Wire-Encoding. Jeder Tag erscheint als
/// ein `WireProperty` mit `name = TAG_PROPERTY_PREFIX + tag.name`,
/// `value = tag.value`. Andere Properties (auth-class, suite-list, …)
/// bleiben unangetastet.
pub const TAG_PROPERTY_PREFIX: &str = "dds.sec.data_tags.";

/// Builtin DataTagging-Plugin fuer Spec §12.
#[derive(Debug, Default)]
pub struct BuiltinDataTaggingPlugin {
    tags: BTreeMap<[u8; 16], Vec<DataTag>>,
}

impl BuiltinDataTaggingPlugin {
    /// Konstruktor — Plugin startet ohne registrierte Endpoints.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Default Subset-Match-Predicate (siehe Modul-Doku).
    #[must_use]
    pub fn tags_match(publisher: &[DataTag], subscriber: &[DataTag]) -> bool {
        if subscriber.is_empty() {
            return true;
        }
        subscriber.iter().all(|s| publisher.iter().any(|p| p == s))
    }

    /// Encodiert eine Tag-Liste als `WireProperty`-Sequenz zur Aufnahme
    /// in eine [`WirePropertyList`]. Tags mit doppeltem Namen werden
    /// stabil in Eingabe-Reihenfolge geschrieben — die `last value
    /// wins`-Semantik der PropertyList wird damit konsistent.
    #[must_use]
    pub fn encode_tags(tags: &[DataTag]) -> Vec<WireProperty> {
        tags.iter()
            .map(|t| {
                let mut name = String::with_capacity(TAG_PROPERTY_PREFIX.len() + t.name.len());
                name.push_str(TAG_PROPERTY_PREFIX);
                name.push_str(&t.name);
                WireProperty::new(name, t.value.clone())
            })
            .collect()
    }

    /// Filtert eine [`WirePropertyList`] nach Tag-Properties (Prefix-
    /// Match) und liefert die de-prefixed Tag-Liste. Andere Properties
    /// werden ignoriert.
    #[must_use]
    pub fn decode_tags(list: &WirePropertyList) -> Vec<DataTag> {
        list.entries
            .iter()
            .filter_map(|p| {
                p.name.strip_prefix(TAG_PROPERTY_PREFIX).map(|n| DataTag {
                    name: n.to_string(),
                    value: p.value.clone(),
                })
            })
            .collect()
    }
}

impl DataTaggingPlugin for BuiltinDataTaggingPlugin {
    fn set_tags(&mut self, endpoint_guid: [u8; 16], tags: Vec<DataTag>) {
        if tags.is_empty() {
            self.tags.remove(&endpoint_guid);
        } else {
            self.tags.insert(endpoint_guid, tags);
        }
    }

    fn get_tags(&self, endpoint_guid: [u8; 16]) -> Vec<DataTag> {
        self.tags.get(&endpoint_guid).cloned().unwrap_or_default()
    }

    fn plugin_class_id(&self) -> &str {
        "DDS:Tagging:Builtin"
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    fn tag(name: &str, value: &str) -> DataTag {
        DataTag {
            name: name.into(),
            value: value.into(),
        }
    }

    #[test]
    fn set_get_roundtrip() {
        let mut p = BuiltinDataTaggingPlugin::new();
        let g = [0xAA; 16];
        p.set_tags(g, vec![tag("classification", "secret")]);
        assert_eq!(p.get_tags(g), vec![tag("classification", "secret")]);
    }

    #[test]
    fn unknown_endpoint_returns_empty() {
        let p = BuiltinDataTaggingPlugin::new();
        assert!(p.get_tags([0xCC; 16]).is_empty());
    }

    #[test]
    fn set_empty_clears_existing() {
        let mut p = BuiltinDataTaggingPlugin::new();
        let g = [0xBB; 16];
        p.set_tags(g, vec![tag("a", "1")]);
        p.set_tags(g, Vec::new());
        assert!(p.get_tags(g).is_empty());
    }

    #[test]
    fn match_empty_subscriber_is_wildcard() {
        // Subscriber ohne Tags akzeptiert jeden Publisher — auch ohne Tags.
        assert!(BuiltinDataTaggingPlugin::tags_match(&[], &[]));
        assert!(BuiltinDataTaggingPlugin::tags_match(
            &[tag("classification", "secret")],
            &[],
        ));
    }

    #[test]
    fn match_subset_passes() {
        let publisher = vec![
            tag("classification", "secret"),
            tag("releasability", "nato"),
        ];
        let subscriber = vec![tag("classification", "secret")];
        assert!(BuiltinDataTaggingPlugin::tags_match(
            &publisher,
            &subscriber
        ));
    }

    #[test]
    fn match_full_set_passes() {
        let publisher = vec![
            tag("classification", "secret"),
            tag("releasability", "nato"),
        ];
        let subscriber = publisher.clone();
        assert!(BuiltinDataTaggingPlugin::tags_match(
            &publisher,
            &subscriber
        ));
    }

    #[test]
    fn match_missing_required_tag_rejects() {
        let publisher = vec![tag("classification", "secret")];
        let subscriber = vec![tag("releasability", "nato")];
        assert!(!BuiltinDataTaggingPlugin::tags_match(
            &publisher,
            &subscriber
        ));
    }

    #[test]
    fn match_value_mismatch_rejects() {
        // Subscriber will "secret", Publisher hat "topsecret" — Reject.
        let publisher = vec![tag("classification", "topsecret")];
        let subscriber = vec![tag("classification", "secret")];
        assert!(!BuiltinDataTaggingPlugin::tags_match(
            &publisher,
            &subscriber
        ));
    }

    #[test]
    fn match_unknown_subscriber_tag_rejects() {
        // Subscriber fordert Tag-Name den Publisher gar nicht setzt.
        let publisher = vec![tag("classification", "secret")];
        let subscriber = vec![tag("project", "alpha")];
        assert!(!BuiltinDataTaggingPlugin::tags_match(
            &publisher,
            &subscriber
        ));
    }

    #[test]
    fn empty_publisher_with_required_subscriber_rejects() {
        // Publisher ohne Tags + Subscriber mit Anforderung → Reject.
        let subscriber = vec![tag("classification", "secret")];
        assert!(!BuiltinDataTaggingPlugin::tags_match(&[], &subscriber));
    }

    #[test]
    fn encode_tags_uses_namespace_prefix() {
        let tags = vec![tag("classification", "secret")];
        let wire = BuiltinDataTaggingPlugin::encode_tags(&tags);
        assert_eq!(wire.len(), 1);
        assert_eq!(wire[0].name, "dds.sec.data_tags.classification");
        assert_eq!(wire[0].value, "secret");
    }

    #[test]
    fn decode_tags_skips_non_tag_properties() {
        let mut list = WirePropertyList::new();
        list.push(WireProperty::new(
            "dds.sec.auth.plugin_class",
            "DDS:Auth:PKI-DH:1.2",
        ));
        list.push(WireProperty::new(
            "dds.sec.data_tags.classification",
            "secret",
        ));
        list.push(WireProperty::new(
            "zerodds.sec.supported_suites",
            "AES_128_GCM",
        ));
        list.push(WireProperty::new("dds.sec.data_tags.releasability", "nato"));
        let decoded = BuiltinDataTaggingPlugin::decode_tags(&list);
        assert_eq!(
            decoded,
            vec![
                tag("classification", "secret"),
                tag("releasability", "nato"),
            ]
        );
    }

    #[test]
    fn wire_roundtrip_via_property_list() {
        // encode_tags → WirePropertyList → CDR-bytes → WirePropertyList
        // → decode_tags muss die Tag-Liste byte-genau reproduzieren.
        let tags = vec![
            tag("classification", "secret"),
            tag("releasability", "nato"),
        ];
        let mut list = WirePropertyList::new();
        for w in BuiltinDataTaggingPlugin::encode_tags(&tags) {
            list.push(w);
        }
        let bytes = list.encode(true).expect("encode");
        let decoded_list = WirePropertyList::decode(&bytes, true).expect("decode");
        let decoded_tags = BuiltinDataTaggingPlugin::decode_tags(&decoded_list);
        assert_eq!(decoded_tags, tags);
    }

    #[test]
    fn plugin_is_object_safe_via_dyn_trait() {
        // Sanity-Check: via Box<dyn DataTaggingPlugin> bedienbar.
        let mut boxed: alloc::boxed::Box<dyn DataTaggingPlugin> =
            alloc::boxed::Box::new(BuiltinDataTaggingPlugin::new());
        boxed.set_tags([1; 16], vec![tag("a", "b")]);
        assert_eq!(boxed.get_tags([1; 16]), vec![tag("a", "b")]);
        assert_eq!(boxed.plugin_class_id(), "DDS:Tagging:Builtin");
    }

    #[test]
    fn plugin_class_id_matches_spec_format() {
        let p = BuiltinDataTaggingPlugin::new();
        // Spec §12.0: Class-Id-Konvention "DDS:<Service>:<Variant>".
        assert_eq!(p.plugin_class_id(), "DDS:Tagging:Builtin");
    }
}
