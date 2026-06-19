// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Builtin DataTagging plugin (OMG DDS-Security 1.2 §12).
//!
//! Implements the [`zerodds_security::DataTaggingPlugin`] SPI as a
//! production builtin. Tags are application-level labels
//! (classification markers, sensitivity, etc.) that are managed per
//! endpoint GUID, propagated over SEDP via `PID_PROPERTY_LIST`, and
//! checked for a match on the subscriber side.
//!
//! # Wire path
//!
//! Tags are embedded as `WireProperty` entries with the namespace prefix
//! `dds.sec.data_tags.` into the existing [`WirePropertyList`]
//! — i.e. we reuse the `PID_PROPERTY_LIST` parameter already propagated in
//! SPDP/SEDP, instead of introducing a new PID.
//! This matches the Cyclone/RTI behavior, which also carries tags via
//! `PID_PROPERTY_LIST` (Cyclone DDS Security §8 doc).
//!
//! # Match predicate
//!
//! The default predicate is a subset match:
//! * subscriber without tags → accepts any publisher (wildcard).
//! * subscriber with tags → every tag (name+value) must appear exactly in
//!   the publisher tag set.
//! * an unknown tag name on the subscriber side that does not exist at the
//!   publisher → reject.
//!
//! Spec-conform and simple enough for NGVA/FACE pilot setups; complex
//! predicates (range/regex) are covered via a custom `DataTaggingPlugin`.
//!
//! # Example
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
//! (plugin trait object via `Box<dyn DataTaggingPlugin>`.)

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use zerodds_rtps::property_list::{WireProperty, WirePropertyList};
use zerodds_security::data_tagging::{DataTag, DataTaggingPlugin};

/// Property namespace for tag wire encoding. Each tag appears as
/// a `WireProperty` with `name = TAG_PROPERTY_PREFIX + tag.name`,
/// `value = tag.value`. Other properties (auth-class, suite-list, …)
/// stay untouched.
pub const TAG_PROPERTY_PREFIX: &str = "dds.sec.data_tags.";

/// Builtin DataTagging plugin for spec §12.
#[derive(Debug, Default)]
pub struct BuiltinDataTaggingPlugin {
    tags: BTreeMap<[u8; 16], Vec<DataTag>>,
}

impl BuiltinDataTaggingPlugin {
    /// Constructor — the plugin starts with no registered endpoints.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Default subset-match predicate (see module docs).
    #[must_use]
    pub fn tags_match(publisher: &[DataTag], subscriber: &[DataTag]) -> bool {
        if subscriber.is_empty() {
            return true;
        }
        subscriber.iter().all(|s| publisher.iter().any(|p| p == s))
    }

    /// Encodes a tag list as a `WireProperty` sequence for inclusion
    /// in a [`WirePropertyList`]. Tags with a duplicate name are
    /// written stably in input order — making the `last value
    /// wins` semantics of the PropertyList consistent.
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

    /// Filters a [`WirePropertyList`] for tag properties (prefix
    /// match) and returns the de-prefixed tag list. Other properties
    /// are ignored.
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
        // A subscriber without tags accepts any publisher — even without tags.
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
        // Subscriber requires a tag name the publisher does not set at all.
        let publisher = vec![tag("classification", "secret")];
        let subscriber = vec![tag("project", "alpha")];
        assert!(!BuiltinDataTaggingPlugin::tags_match(
            &publisher,
            &subscriber
        ));
    }

    #[test]
    fn empty_publisher_with_required_subscriber_rejects() {
        // Publisher without tags + subscriber with a requirement → reject.
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
        // → decode_tags must reproduce the tag list byte-exactly.
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
        // Spec §12.0: class-id convention "DDS:<Service>:<Variant>".
        assert_eq!(p.plugin_class_id(), "DDS:Tagging:Builtin");
    }
}
