// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-security-runtime` with `zerodds-rtps`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-rtps`:
//! - `wire_types::EntityId` and the secure builtin entity ids it matches on
//!   (`src/builtin_attributes.rs`);
//! - `endpoint_security_info::{EndpointSecurityInfo, attrs, plugin_attrs}`
//!   and `participant_security_info::{ParticipantSecurityInfo,
//!   ParticipantSecurityAttributesMaskExt, attrs, plugin_attrs}`, whose mask
//!   bits it sets from the governance (`src/plugin_attributes.rs`,
//!   `src/endpoint.rs`), with `is_valid`;
//! - `security_algo_info::symmetric_bit::{AES128, AES256}`;
//! - `property_list::{WireProperty, WirePropertyList, WireDataTags}`: `new`,
//!   `push`, `get`, the `entries` it filters, and the `tags`
//!   (`src/caps_wire.rs`, `src/data_tagging.rs`).
//!
//! Oracle: DDS-Security 1.2 §7.4.7.1 Table 9 (secure builtin entity ids),
//! §9.4.2.5 Table 34 and §9.4.2.8 Table 38 (attribute masks), §10.4.2.4
//! Table 67 and §10.4.2.6 Table 69 (plugin masks), §8.1 (CBIT_AES128_GCM =
//! 1 << 0, CBIT_AES256_GCM = 1 << 1).
#![allow(clippy::unwrap_used)]

use zerodds_rtps::endpoint_security_info::{EndpointSecurityInfo, attrs, plugin_attrs};
use zerodds_rtps::participant_security_info::{
    ParticipantSecurityAttributesMaskExt, ParticipantSecurityInfo, attrs as participant_attrs,
    plugin_attrs as participant_plugin_attrs,
};
use zerodds_rtps::property_list::{WireDataTags, WireProperty, WirePropertyList};
use zerodds_rtps::security_algo_info::symmetric_bit;
use zerodds_rtps::wire_types::EntityId;

#[test]
fn security_7_4_7_1_secure_builtin_entity_ids() {
    let table: [(EntityId, [u8; 4]); 16] = [
        (
            EntityId::SEDP_BUILTIN_PUBLICATIONS_SECURE_WRITER,
            [0xff, 0x00, 0x03, 0xc2],
        ),
        (
            EntityId::SEDP_BUILTIN_PUBLICATIONS_SECURE_READER,
            [0xff, 0x00, 0x03, 0xc7],
        ),
        (
            EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_SECURE_WRITER,
            [0xff, 0x00, 0x04, 0xc2],
        ),
        (
            EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_SECURE_READER,
            [0xff, 0x00, 0x04, 0xc7],
        ),
        (
            EntityId::BUILTIN_PARTICIPANT_MESSAGE_SECURE_WRITER,
            [0xff, 0x02, 0x00, 0xc2],
        ),
        (
            EntityId::BUILTIN_PARTICIPANT_MESSAGE_SECURE_READER,
            [0xff, 0x02, 0x00, 0xc7],
        ),
        (
            EntityId::BUILTIN_PARTICIPANT_STATELESS_MESSAGE_WRITER,
            [0x00, 0x02, 0x01, 0xc3],
        ),
        (
            EntityId::BUILTIN_PARTICIPANT_STATELESS_MESSAGE_READER,
            [0x00, 0x02, 0x01, 0xc4],
        ),
        (
            EntityId::BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_WRITER,
            [0xff, 0x02, 0x02, 0xc3],
        ),
        (
            EntityId::BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_READER,
            [0xff, 0x02, 0x02, 0xc4],
        ),
        (
            EntityId::SPDP_RELIABLE_BUILTIN_PARTICIPANTS_SECURE_WRITER,
            [0xff, 0x01, 0x01, 0xc2],
        ),
        (
            EntityId::SPDP_RELIABLE_BUILTIN_PARTICIPANTS_SECURE_READER,
            [0xff, 0x01, 0x01, 0xc7],
        ),
        (EntityId::TL_SVC_REQ_SECURE_WRITER, [0xff, 0x03, 0x00, 0xc3]),
        (EntityId::TL_SVC_REQ_SECURE_READER, [0xff, 0x03, 0x00, 0xc4]),
        (
            EntityId::TL_SVC_REPLY_SECURE_WRITER,
            [0xff, 0x03, 0x01, 0xc3],
        ),
        (
            EntityId::TL_SVC_REPLY_SECURE_READER,
            [0xff, 0x03, 0x01, 0xc4],
        ),
    ];
    for (id, bytes) in table {
        assert_eq!(id.to_bytes(), bytes, "{id:?}");
        assert_eq!(EntityId::from_bytes(bytes), id);
    }
}

#[test]
fn security_9_4_2_8_and_10_4_2_6_endpoint_mask_bits() {
    assert_eq!(attrs::IS_READ_PROTECTED, 1 << 0);
    assert_eq!(attrs::IS_WRITE_PROTECTED, 1 << 1);
    assert_eq!(attrs::IS_DISCOVERY_PROTECTED, 1 << 2);
    assert_eq!(attrs::IS_SUBMESSAGE_PROTECTED, 1 << 3);
    assert_eq!(attrs::IS_PAYLOAD_PROTECTED, 1 << 4);
    assert_eq!(attrs::IS_KEY_PROTECTED, 1 << 5);
    assert_eq!(attrs::IS_LIVELINESS_PROTECTED, 1 << 6);
    assert_eq!(attrs::IS_VALID, 1 << 31);
    assert_eq!(plugin_attrs::IS_SUBMESSAGE_ENCRYPTED, 1 << 0);
    assert_eq!(plugin_attrs::IS_PAYLOAD_ENCRYPTED, 1 << 1);
    assert_eq!(plugin_attrs::IS_SUBMESSAGE_ORIGIN_AUTHENTICATED, 1 << 2);
    assert_eq!(plugin_attrs::IS_VALID, 1 << 31);
}

#[test]
fn security_9_4_2_5_and_10_4_2_4_participant_mask_bits() {
    assert_eq!(participant_attrs::IS_RTPS_AXK_PROTECTED, 1 << 0);
    assert_eq!(participant_attrs::IS_DISCOVERY_PROTECTED, 1 << 1);
    assert_eq!(participant_attrs::IS_LIVELINESS_PROTECTED, 1 << 2);
    assert_eq!(participant_attrs::IS_KEY_REVISION_ENABLED, 1 << 3);
    assert_eq!(participant_attrs::IS_RTPS_PSK_PROTECTED, 1 << 4);
    assert_eq!(participant_attrs::IS_VALID, 1 << 31);
    assert_eq!(participant_plugin_attrs::IS_RTPS_AXK_ENCRYPTED, 1 << 0);
    assert_eq!(participant_plugin_attrs::IS_DISCOVERY_ENCRYPTED, 1 << 1);
    assert_eq!(participant_plugin_attrs::IS_LIVELINESS_ENCRYPTED, 1 << 2);
    assert_eq!(
        participant_plugin_attrs::IS_RTPS_ORIGIN_AUTHENTICATED,
        1 << 3
    );
    assert_eq!(
        participant_plugin_attrs::IS_DISCOVERY_ORIGIN_AUTHENTICATED,
        1 << 4
    );
    assert_eq!(
        participant_plugin_attrs::IS_LIVELINESS_ORIGIN_AUTHENTICATED,
        1 << 5
    );
    assert_eq!(participant_plugin_attrs::IS_RTPS_PSK_ENCRYPTED, 1 << 6);
    assert_eq!(participant_plugin_attrs::IS_VALID, 1 << 31);
}

#[test]
fn an_info_is_valid_only_with_the_valid_bit_in_both_masks() {
    let both = EndpointSecurityInfo {
        endpoint_security_attributes: attrs::IS_VALID | attrs::IS_SUBMESSAGE_PROTECTED,
        plugin_endpoint_security_attributes: plugin_attrs::IS_VALID,
    };
    assert!(both.is_valid());
    let one = EndpointSecurityInfo {
        endpoint_security_attributes: attrs::IS_VALID,
        plugin_endpoint_security_attributes: 0,
    };
    assert!(!one.is_valid());
    let participant = ParticipantSecurityInfo {
        participant_security_attributes: participant_attrs::IS_VALID,
        plugin_participant_security_attributes: participant_plugin_attrs::IS_VALID,
        optional_attributes: ParticipantSecurityAttributesMaskExt::default(),
    };
    assert!(participant.is_valid());
    assert!(!ParticipantSecurityInfo::default().is_valid());
}

#[test]
fn security_8_1_symmetric_cipher_bits() {
    assert_eq!(symmetric_bit::AES128, 1 << 0, "CBIT_AES128_GCM");
    assert_eq!(symmetric_bit::AES256, 1 << 1, "CBIT_AES256_GCM");
}

#[test]
fn a_property_list_finds_by_name_and_keeps_what_was_pushed() {
    let mut list = WirePropertyList::new();
    assert!(list.is_empty());
    list.push(WireProperty::new("dds.sec.caps", "1"));
    list.push(WireProperty::new("other", "x"));
    assert_eq!(list.len(), 2);
    assert_eq!(list.get("dds.sec.caps"), Some("1"));
    assert_eq!(list.get("missing"), None);
    // `remove_by_key` in the runtime filters `entries` directly.
    list.entries.retain(|entry| entry.name != "dds.sec.caps");
    assert_eq!(list.get("dds.sec.caps"), None);
    let tags = WireDataTags {
        tags: vec![WireProperty::new("classification", "secret")],
    };
    assert_eq!(tags.tags[0].name, "classification");
    assert_eq!(tags.tags[0].value, "secret");
    assert!(WireDataTags::default().is_empty());
}
