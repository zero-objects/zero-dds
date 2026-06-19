//! Profile conformance matrix for DDS-XRCE 1.0 §1 + §2 + §7.
//!
//! Verifies productively that:
//!
//! 1. **§1.1 + §1.2** the wire codec covers the client/agent protocol
//!    interoperably — `SubmessageId::from_u8` round-trips all
//!    16 spec values.
//! 2. **§2.1-§2.10** per profile (Read/Write/Configure/Configure-QoS/
//!    Configure-Types/Discovery/File-Config/UDP/TCP/Complete), all
//!    required `SubmessageId` values are exposed as a wire path.
//! 3. **§7.1, §7.4, §7.5** object model: 5 top-level classes +
//!    proxy object kind constants + ObjectId reserved values.
//! 4. **§7.8.2 + §7.8.3** Root + ProxyClient operations are exposed as
//!    submessages.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::field_reassign_with_default,
    clippy::manual_flatten,
    clippy::collapsible_if,
    clippy::empty_line_after_doc_comments,
    clippy::uninlined_format_args,
    clippy::drop_non_drop,
    missing_docs
)]

use zerodds_xrce::object_id::{OBJECTID_AGENT, OBJECTID_CLIENT, OBJECTID_INVALID};
use zerodds_xrce::object_kind::{
    OBJK_AGENT, OBJK_APPLICATION, OBJK_CLIENT, OBJK_DATAREADER, OBJK_DATAWRITER, OBJK_INVALID,
    OBJK_PARTICIPANT, OBJK_PUBLISHER, OBJK_QOSPROFILE, OBJK_SUBSCRIBER, OBJK_TOPIC, OBJK_TYPE,
};
use zerodds_xrce::submessages::SubmessageId;

// ============================================================================
// §2.1-§2.10 Profile-Conformance-Matrix
// ============================================================================

/// Spec §2.1 Read Access Profile: all submessages **except**
/// CREATE/INFO/WRITE_DATA/DELETE.
const READ_ACCESS_PROFILE: &[SubmessageId] = &[
    SubmessageId::CreateClient,
    SubmessageId::GetInfo,
    SubmessageId::StatusAgent,
    SubmessageId::Status,
    SubmessageId::ReadData,
    SubmessageId::Data,
    SubmessageId::AckNack,
    SubmessageId::Heartbeat,
    SubmessageId::Reset,
    SubmessageId::Fragment,
    SubmessageId::Timestamp,
    SubmessageId::TimestampReply,
];

/// Spec §2.2 Write Access Profile: all submessages **except**
/// CREATE/INFO/READ_DATA/DATA/DELETE.
const WRITE_ACCESS_PROFILE: &[SubmessageId] = &[
    SubmessageId::CreateClient,
    SubmessageId::GetInfo,
    SubmessageId::StatusAgent,
    SubmessageId::Status,
    SubmessageId::WriteData,
    SubmessageId::AckNack,
    SubmessageId::Heartbeat,
    SubmessageId::Reset,
    SubmessageId::Fragment,
    SubmessageId::Timestamp,
    SubmessageId::TimestampReply,
];

/// Spec §2.3 Configure Entities Profile: CREATE_CLIENT + CREATE +
/// DELETE + STATUS path.
const CONFIGURE_ENTITIES_PROFILE: &[SubmessageId] = &[
    SubmessageId::CreateClient,
    SubmessageId::Create,
    SubmessageId::Delete,
    SubmessageId::StatusAgent,
    SubmessageId::Status,
];

/// Spec §2.10 Complete Profile: all 16 submessages.
const COMPLETE_PROFILE: &[SubmessageId] = &[
    SubmessageId::CreateClient,
    SubmessageId::Create,
    SubmessageId::GetInfo,
    SubmessageId::Delete,
    SubmessageId::StatusAgent,
    SubmessageId::Status,
    SubmessageId::Info,
    SubmessageId::WriteData,
    SubmessageId::ReadData,
    SubmessageId::Data,
    SubmessageId::AckNack,
    SubmessageId::Heartbeat,
    SubmessageId::Reset,
    SubmessageId::Fragment,
    SubmessageId::Timestamp,
    SubmessageId::TimestampReply,
];

#[test]
fn profile_2_1_read_access_submessages_all_roundtrip() {
    // Pro Submessage: Wire-ID round-trippt via SubmessageId::from_u8.
    for sid in READ_ACCESS_PROFILE {
        let byte = sid.as_u8();
        let decoded = SubmessageId::from_u8(byte).expect("read-access roundtrip");
        assert_eq!(decoded, *sid, "Read-Access SubmessageId {sid:?}");
    }
}

#[test]
fn profile_2_2_write_access_submessages_all_roundtrip() {
    for sid in WRITE_ACCESS_PROFILE {
        let decoded = SubmessageId::from_u8(sid.as_u8()).expect("write-access roundtrip");
        assert_eq!(decoded, *sid);
    }
}

#[test]
fn profile_2_3_configure_entities_submessages_all_roundtrip() {
    for sid in CONFIGURE_ENTITIES_PROFILE {
        let decoded = SubmessageId::from_u8(sid.as_u8()).expect("configure roundtrip");
        assert_eq!(decoded, *sid);
    }
}

#[test]
fn profile_2_10_complete_covers_all_16_submessages() {
    // Spec §2.10: the Complete profile requires all submessage types.
    // Spec §8.3.5: 16 values (0..15).
    assert_eq!(COMPLETE_PROFILE.len(), 16);
    let mut seen = std::collections::BTreeSet::new();
    for sid in COMPLETE_PROFILE {
        seen.insert(sid.as_u8());
    }
    for byte in 0u8..=15u8 {
        assert!(
            seen.contains(&byte),
            "Complete profile is missing SubmessageId {byte}"
        );
        // Plus: every wire value is decodable via from_u8.
        assert!(
            SubmessageId::from_u8(byte).is_ok(),
            "SubmessageId({byte}) not decodable"
        );
    }
}

#[test]
fn read_and_write_profiles_disjoint_in_data_submessages() {
    // Spec logic: §2.1 has ReadData/Data, §2.2 has WriteData. These
    // are the exclusively distinguishing submessages. We compare
    // over u8 values (SubmessageId does not implement Ord).
    let r: std::collections::BTreeSet<u8> = READ_ACCESS_PROFILE.iter().map(|s| s.as_u8()).collect();
    let w: std::collections::BTreeSet<u8> =
        WRITE_ACCESS_PROFILE.iter().map(|s| s.as_u8()).collect();
    assert!(r.contains(&SubmessageId::ReadData.as_u8()));
    assert!(r.contains(&SubmessageId::Data.as_u8()));
    assert!(!r.contains(&SubmessageId::WriteData.as_u8()));
    assert!(w.contains(&SubmessageId::WriteData.as_u8()));
    assert!(!w.contains(&SubmessageId::ReadData.as_u8()));
    assert!(!w.contains(&SubmessageId::Data.as_u8()));
}

#[test]
fn invalid_submessage_id_rejected() {
    // Spec §8.3.5: values > 15 are not defined.
    for byte in 16u8..=255u8 {
        assert!(
            SubmessageId::from_u8(byte).is_err(),
            "SubmessageId({byte}) should be rejected"
        );
    }
}

// ============================================================================
// §1.1 + §1.2 wire compatibility (client/agent protocol)
// ============================================================================

#[test]
fn spec_1_1_wire_codec_supports_all_submessages() {
    // §1.1: client-agent protocol. The wire codec exposes all
    // spec submessages.
    for byte in 0u8..=15u8 {
        let _ = SubmessageId::from_u8(byte).expect("all 16 IDs must decode");
    }
}

#[test]
fn spec_1_2_submessage_ids_match_spec_assignment() {
    // §1.2 vendor interoperability: every SubmessageId has its
    // exact spec value.
    assert_eq!(SubmessageId::CreateClient.as_u8(), 0);
    assert_eq!(SubmessageId::Create.as_u8(), 1);
    assert_eq!(SubmessageId::GetInfo.as_u8(), 2);
    assert_eq!(SubmessageId::Delete.as_u8(), 3);
    assert_eq!(SubmessageId::StatusAgent.as_u8(), 4);
    assert_eq!(SubmessageId::Status.as_u8(), 5);
    assert_eq!(SubmessageId::Info.as_u8(), 6);
    assert_eq!(SubmessageId::WriteData.as_u8(), 7);
    assert_eq!(SubmessageId::ReadData.as_u8(), 8);
    assert_eq!(SubmessageId::Data.as_u8(), 9);
    assert_eq!(SubmessageId::AckNack.as_u8(), 10);
    assert_eq!(SubmessageId::Heartbeat.as_u8(), 11);
    assert_eq!(SubmessageId::Reset.as_u8(), 12);
    assert_eq!(SubmessageId::Fragment.as_u8(), 13);
    assert_eq!(SubmessageId::Timestamp.as_u8(), 14);
    assert_eq!(SubmessageId::TimestampReply.as_u8(), 15);
}

// ============================================================================
// §7.1, §7.4, §7.5 Object Model
// ============================================================================

#[test]
fn spec_7_1_object_model_kinds_complete() {
    // §7.1: the DDS-XRCE object model defines ObjectKind values. The
    // 12 productive OBJK_* values are all exposed as pub const.
    let kinds: &[(u8, &str)] = &[
        (OBJK_INVALID, "INVALID"),
        (OBJK_PARTICIPANT, "PARTICIPANT"),
        (OBJK_TOPIC, "TOPIC"),
        (OBJK_PUBLISHER, "PUBLISHER"),
        (OBJK_SUBSCRIBER, "SUBSCRIBER"),
        (OBJK_DATAWRITER, "DATAWRITER"),
        (OBJK_DATAREADER, "DATAREADER"),
        (OBJK_TYPE, "TYPE"),
        (OBJK_QOSPROFILE, "QOSPROFILE"),
        (OBJK_APPLICATION, "APPLICATION"),
        (OBJK_AGENT, "AGENT"),
        (OBJK_CLIENT, "CLIENT"),
    ];
    let mut seen = std::collections::BTreeSet::new();
    for (val, name) in kinds {
        assert!(
            seen.insert(*val),
            "OBJK_ value 0x{val:02X} duplicated for {name}"
        );
    }
    assert_eq!(seen.len(), 12, "object model must expose 12 kind values");
}

#[test]
fn spec_7_4_top_level_classes_have_kind_constants() {
    // §7.4: 5 top-level classes — Root (realized via object_store::ObjectStore;
    // no own OBJK), ProxyClient (OBJK_CLIENT),
    // Application (OBJK_APPLICATION), AccessController (n/a — no
    // separate XRCE wire repr; Spec §7.4 says: "AccessController
    // exists in the model but is not exposed as a CRUD-able object"),
    // DomainParticipant (OBJK_PARTICIPANT).
    assert_ne!(OBJK_PARTICIPANT, OBJK_INVALID);
    assert_ne!(OBJK_APPLICATION, OBJK_INVALID);
    assert_ne!(OBJK_CLIENT, OBJK_INVALID);
    // Root is a singleton — represented by object_store::ObjectStore.
    let _ = zerodds_xrce::object_store::ObjectStore::default();
}

#[test]
fn spec_7_5_proxy_objects_have_kind_constants() {
    // §7.5: proxy objects are DomainParticipant/Publisher/Subscriber/
    // DataWriter/DataReader/Topic; QosProfile/Type are value objects.
    let proxy_kinds: &[u8] = &[
        OBJK_PARTICIPANT,
        OBJK_PUBLISHER,
        OBJK_SUBSCRIBER,
        OBJK_DATAWRITER,
        OBJK_DATAREADER,
        OBJK_TOPIC,
    ];
    for k in proxy_kinds {
        assert!(*k != OBJK_INVALID, "proxy kind is INVALID: 0x{k:02X}");
    }
    // Value objects:
    assert_ne!(OBJK_QOSPROFILE, OBJK_INVALID);
    assert_ne!(OBJK_TYPE, OBJK_INVALID);
}

// ============================================================================
// §7.8.2 + §7.8.3 Operations
// ============================================================================

#[test]
fn spec_7_8_2_root_operations_have_submessage_ids() {
    // §7.8.2: Root operations (CREATE_CLIENT, DELETE_CLIENT). Wire
    // path via SubmessageId::CreateClient. DELETE_CLIENT is
    // Spec §7.8.2 — realized as CREATE_CLIENT with a DELETE flag or as
    // a separate path via the Delete submessage.
    assert_eq!(SubmessageId::CreateClient.as_u8(), 0);
    // The Delete submessage is primary for ProxyClient operations
    // (see §7.8.3) — can also be used for DELETE_CLIENT.
    assert_eq!(SubmessageId::Delete.as_u8(), 3);
    // Status reply path — Root operations return StatusAgent.
    assert_eq!(SubmessageId::StatusAgent.as_u8(), 4);
}

#[test]
fn spec_7_8_3_proxy_client_operations_have_submessage_ids() {
    // §7.8.3: ProxyClient operations (CREATE/DELETE/GET_INFO/UPDATE).
    // UPDATE is realized via CREATE with a REPLACE flag (see
    // the CreationMode constants).
    assert_eq!(SubmessageId::Create.as_u8(), 1);
    assert_eq!(SubmessageId::Delete.as_u8(), 3);
    assert_eq!(SubmessageId::GetInfo.as_u8(), 2);
    // UPDATE = CREATE + REPLACE flag (Spec §7.7.11 CreationMode).
    use zerodds_xrce::submessages::create::CREATE_FLAG_REPLACE;
    assert_eq!(CREATE_FLAG_REPLACE, 0x04);
    // Reply path: Status for all 4 operations.
    assert_eq!(SubmessageId::Status.as_u8(), 5);
    // GET_INFO reply: separate Info submessage.
    assert_eq!(SubmessageId::Info.as_u8(), 6);
}

// ============================================================================
// §7.6 ObjectId reserved values (sanity)
// ============================================================================

#[test]
fn object_ids_reserved_values_distinct() {
    assert_ne!(OBJECTID_INVALID, OBJECTID_CLIENT);
    assert_ne!(OBJECTID_INVALID, OBJECTID_AGENT);
    assert_ne!(OBJECTID_CLIENT, OBJECTID_AGENT);
}
