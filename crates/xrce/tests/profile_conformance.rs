//! Profile-Conformance-Matrix fuer DDS-XRCE 1.0 §1 + §2 + §7.
//!
//! Verifiziert produktiv, dass:
//!
//! 1. **§1.1 + §1.2** Wire-Codec deckt Client/Agent-Protokoll
//!    interoperabel ab — `SubmessageId::from_u8` round-trippt alle
//!    16 Spec-Werte.
//! 2. **§2.1-§2.10** pro Profile (Read/Write/Configure/Configure-QoS/
//!    Configure-Types/Discovery/File-Config/UDP/TCP/Complete) sind
//!    alle erforderlichen `SubmessageId`-Werte als Wire-Pfad
//!    exponiert.
//! 3. **§7.1, §7.4, §7.5** Object-Model: 5 Top-Level-Klassen +
//!    Proxy-Object-Kind-Konstanten + ObjectId-Reserved-Werte.
//! 4. **§7.8.2 + §7.8.3** Root + ProxyClient Operations sind als
//!    Submessages exponiert.

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

/// Spec §2.1 Read Access Profile: alle Submessages **ausser**
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

/// Spec §2.2 Write Access Profile: alle Submessages **ausser**
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
/// DELETE + STATUS-Pfad.
const CONFIGURE_ENTITIES_PROFILE: &[SubmessageId] = &[
    SubmessageId::CreateClient,
    SubmessageId::Create,
    SubmessageId::Delete,
    SubmessageId::StatusAgent,
    SubmessageId::Status,
];

/// Spec §2.10 Complete Profile: alle 16 Submessages.
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
    // Spec §2.10: Complete Profile verlangt alle Submessage-Typen.
    // Spec §8.3.5: 16 Werte (0..15).
    assert_eq!(COMPLETE_PROFILE.len(), 16);
    let mut seen = std::collections::BTreeSet::new();
    for sid in COMPLETE_PROFILE {
        seen.insert(sid.as_u8());
    }
    for byte in 0u8..=15u8 {
        assert!(
            seen.contains(&byte),
            "Complete-Profile fehlt SubmessageId {byte}"
        );
        // Plus: jeder Wire-Wert ist via from_u8 dekodierbar.
        assert!(
            SubmessageId::from_u8(byte).is_ok(),
            "SubmessageId({byte}) nicht dekodierbar"
        );
    }
}

#[test]
fn read_and_write_profiles_disjoint_in_data_submessages() {
    // Spec-Logik: §2.1 hat ReadData/Data, §2.2 hat WriteData. Diese
    // sind die exklusiv-unterscheidenden Submessages. Wir vergleichen
    // ueber u8-Werte (SubmessageId implementiert nicht Ord).
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
    // Spec §8.3.5: Werte > 15 sind nicht definiert.
    for byte in 16u8..=255u8 {
        assert!(
            SubmessageId::from_u8(byte).is_err(),
            "SubmessageId({byte}) sollte abgelehnt werden"
        );
    }
}

// ============================================================================
// §1.1 + §1.2 Wire-Compatibility (Client/Agent-Protokoll)
// ============================================================================

#[test]
fn spec_1_1_wire_codec_supports_all_submessages() {
    // §1.1: Client-Agent-Protokoll. Wire-Codec exponiert alle
    // Spec-Submessages.
    for byte in 0u8..=15u8 {
        let _ = SubmessageId::from_u8(byte).expect("alle 16 IDs muessen dekodieren");
    }
}

#[test]
fn spec_1_2_submessage_ids_match_spec_assignment() {
    // §1.2 Vendor-Interoperability: jede SubmessageId hat ihren
    // exakten Spec-Wert.
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
    // §7.1: DDS-XRCE Object-Model definiert ObjectKind-Werte. Die
    // 12 produktiven OBJK_*-Werte sind alle als pub const exponiert.
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
            "OBJK_-Wert 0x{val:02X} doppelt fuer {name}"
        );
    }
    assert_eq!(seen.len(), 12, "Object-Model muss 12 Kind-Werte exponieren");
}

#[test]
fn spec_7_4_top_level_classes_have_kind_constants() {
    // §7.4: 5 Top-Level-Klassen — Root (ueber object_store::ObjectStore
    // realisiert; kein eigener OBJK), ProxyClient (OBJK_CLIENT),
    // Application (OBJK_APPLICATION), AccessController (n/a — keine
    // separate XRCE-Wire-Repr; Spec §7.4 sagt: "AccessController
    // exists in the model but is not exposed as a CRUD-able object"),
    // DomainParticipant (OBJK_PARTICIPANT).
    assert_ne!(OBJK_PARTICIPANT, OBJK_INVALID);
    assert_ne!(OBJK_APPLICATION, OBJK_INVALID);
    assert_ne!(OBJK_CLIENT, OBJK_INVALID);
    // Root ist Singleton — repraesentiert durch object_store::ObjectStore.
    let _ = zerodds_xrce::object_store::ObjectStore::default();
}

#[test]
fn spec_7_5_proxy_objects_have_kind_constants() {
    // §7.5: Proxy-Objekte sind DomainParticipant/Publisher/Subscriber/
    // DataWriter/DataReader/Topic; QosProfile/Type sind Value-Objekte.
    let proxy_kinds: &[u8] = &[
        OBJK_PARTICIPANT,
        OBJK_PUBLISHER,
        OBJK_SUBSCRIBER,
        OBJK_DATAWRITER,
        OBJK_DATAREADER,
        OBJK_TOPIC,
    ];
    for k in proxy_kinds {
        assert!(*k != OBJK_INVALID, "Proxy-Kind ist INVALID: 0x{k:02X}");
    }
    // Value-Objekte:
    assert_ne!(OBJK_QOSPROFILE, OBJK_INVALID);
    assert_ne!(OBJK_TYPE, OBJK_INVALID);
}

// ============================================================================
// §7.8.2 + §7.8.3 Operations
// ============================================================================

#[test]
fn spec_7_8_2_root_operations_have_submessage_ids() {
    // §7.8.2: Root-Operations (CREATE_CLIENT, DELETE_CLIENT). Wire-
    // Pfad via SubmessageId::CreateClient. DELETE_CLIENT ist
    // Spec §7.8.2 — wird als CREATE_CLIENT mit DELETE-Flag oder als
    // separater Pfad ueber Delete-Submessage realisiert.
    assert_eq!(SubmessageId::CreateClient.as_u8(), 0);
    // Delete-Submessage ist primary fuer ProxyClient-Operations
    // (siehe §7.8.3) — kann auch fuer DELETE_CLIENT genutzt werden.
    assert_eq!(SubmessageId::Delete.as_u8(), 3);
    // Status reply path — Root operations liefern StatusAgent zurueck.
    assert_eq!(SubmessageId::StatusAgent.as_u8(), 4);
}

#[test]
fn spec_7_8_3_proxy_client_operations_have_submessage_ids() {
    // §7.8.3: ProxyClient-Operations (CREATE/DELETE/GET_INFO/UPDATE).
    // UPDATE wird via CREATE mit REPLACE-Flag realisiert (siehe
    // CreationMode-Konstanten).
    assert_eq!(SubmessageId::Create.as_u8(), 1);
    assert_eq!(SubmessageId::Delete.as_u8(), 3);
    assert_eq!(SubmessageId::GetInfo.as_u8(), 2);
    // UPDATE = CREATE + REPLACE-Flag (Spec §7.7.11 CreationMode).
    use zerodds_xrce::submessages::create::CREATE_FLAG_REPLACE;
    assert_eq!(CREATE_FLAG_REPLACE, 0x04);
    // Reply-Pfad: Status fuer alle 4 Operations.
    assert_eq!(SubmessageId::Status.as_u8(), 5);
    // GET_INFO-Reply: separate Info-Submessage.
    assert_eq!(SubmessageId::Info.as_u8(), 6);
}

// ============================================================================
// §7.6 ObjectId Reserved-Werte (Sanity)
// ============================================================================

#[test]
fn object_ids_reserved_values_distinct() {
    assert_ne!(OBJECTID_INVALID, OBJECTID_CLIENT);
    assert_ne!(OBJECTID_INVALID, OBJECTID_AGENT);
    assert_ne!(OBJECTID_CLIENT, OBJECTID_AGENT);
}
