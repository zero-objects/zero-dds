// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Builtin DataTagging plugin (OMG DDS-Security 1.2 §9.7, Table 49 `DDS:Tagging:DDS_Discovery`).
//!
//! Implements the [`zerodds_security::DataTaggingPlugin`] SPI as a
//! production builtin. Tags are application-level labels
//! (classification markers, sensitivity, etc.) that are managed per
//! endpoint GUID, propagated over SEDP via `PID_DATA_TAGS` (0x1003,
//! §7.5.1.7, §7.5.1.8), and checked for a match on the subscriber side.
//!
//! # Wire path
//!
//! The tags of an endpoint are its `DataTags`: a `TagSeq` of (name, value)
//! string pairs, the value of `PID_DATA_TAGS` ([`WireDataTags`]).
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
use alloc::vec::Vec;

use zerodds_rtps::property_list::{WireDataTags, WireProperty};
use zerodds_security::data_tagging::{DataTag, DataTaggingPlugin};

/// The plugin class id of the builtin DataTagging plugin
/// (DDS-Security 1.2 §10.1, Table 49).
pub const DATA_TAGGING_PLUGIN_CLASS_ID: &str = "DDS:Tagging:DDS_Discovery";

/// Builtin DataTagging plugin `DDS:Tagging:DDS_Discovery` (§10.1, Table 49).
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

    /// Encodes a tag list as the `DataTags` of `PID_DATA_TAGS`, in input order.
    #[must_use]
    pub fn encode_tags(tags: &[DataTag]) -> WireDataTags {
        WireDataTags {
            tags: tags
                .iter()
                .map(|t| WireProperty::new(t.name.clone(), t.value.clone()))
                .collect(),
        }
    }

    /// The tag list of the `DataTags` of a peer's endpoint.
    #[must_use]
    pub fn decode_tags(tags: &WireDataTags) -> Vec<DataTag> {
        tags.tags
            .iter()
            .map(|t| DataTag {
                name: t.name.clone(),
                value: t.value.clone(),
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
        DATA_TAGGING_PLUGIN_CLASS_ID
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
        // Subscriber wants "secret", publisher has "topsecret" — reject.
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

    /// §7.3.21, §7.5.1.7: the tags of an endpoint are its `DataTags`, a
    /// `TagSeq` of (name, value) pairs, and they cross the wire as the value of
    /// `PID_DATA_TAGS` in the order given.
    #[test]
    fn tags_cross_the_wire_as_data_tags() {
        let tags = vec![
            tag("classification", "secret"),
            tag("releasability", "nato"),
        ];
        let wire = BuiltinDataTaggingPlugin::encode_tags(&tags);
        assert_eq!(wire.tags.len(), 2);
        assert_eq!(wire.tags[0].name, "classification");
        assert_eq!(wire.tags[0].value, "secret");
        let bytes = wire.encode(true).expect("encode");
        let decoded = WireDataTags::decode(&bytes, true).expect("decode");
        assert_eq!(BuiltinDataTaggingPlugin::decode_tags(&decoded), tags);
    }

    #[test]
    fn plugin_is_object_safe_via_dyn_trait() {
        // Sanity-Check: via Box<dyn DataTaggingPlugin> bedienbar.
        let mut boxed: alloc::boxed::Box<dyn DataTaggingPlugin> =
            alloc::boxed::Box::new(BuiltinDataTaggingPlugin::new());
        boxed.set_tags([1; 16], vec![tag("a", "b")]);
        assert_eq!(boxed.get_tags([1; 16]), vec![tag("a", "b")]);
        assert_eq!(boxed.plugin_class_id(), "DDS:Tagging:DDS_Discovery");
    }

    #[test]
    fn plugin_class_id_matches_spec_format() {
        let p = BuiltinDataTaggingPlugin::new();
        // Spec §12.0: class-id convention "DDS:<Service>:<Variant>".
        assert_eq!(p.plugin_class_id(), "DDS:Tagging:DDS_Discovery");
    }
}
