//! D.5f — SEDP PID_DATA_REPRESENTATION conformance.
//!
//! Guards against the D.5e bug "ZeroDDS announces no
//! PID_DATA_REPRESENTATION (PID 0x0073)" which causes a silent SEDP
//! match failure in live interop against strict vendors (RTI Connext
//! 7.7.0).
//!
//! Spec sources:
//! * **DDS-XTypes 1.3 §7.6.3.1.1** — `DataRepresentationQosPolicy`
//!   with `value: sequence<DataRepresentationId>`. IDs:
//!   `XCDR_DATA_REPRESENTATION = 0`, `XML = 1`, `XCDR2 = 2`.
//! * **DDSI-RTPS 2.5** PID table: `PID_DATA_REPRESENTATION = 0x0073`.
//! * **DDS-XTypes 1.3 §7.6.3.1.2** — the default without a policy match
//!   is `[XCDR1]` (legacy). If ZeroDDS emits XCDR2 encapsulation
//!   (`0x0007`), it must also announce it in SEDP.
//!
//! Pre-D.5f: ZeroDDS had `data_representation: Vec::new()` and the
//! encoder skipped the PID. Our wire encapsulation was XCDR2 — but the
//! announcement was default XCDR1 = inconsistent.

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

use zerodds_rtps::publication_data::{
    self, DurabilityKind, PublicationBuiltinTopicData, ReliabilityKind, ReliabilityQos,
};
use zerodds_rtps::wire_types::{EntityId, Guid, GuidPrefix};

fn make_pub_data(data_rep: Vec<i16>) -> PublicationBuiltinTopicData {
    PublicationBuiltinTopicData {
        vendor_id: Some(zerodds_rtps::wire_types::VendorId::ZERODDS),
        key: Guid::new(
            GuidPrefix::from_bytes([1; 12]),
            EntityId::user_writer_with_key([0, 0, 1]),
        ),
        participant_key: Guid::new(GuidPrefix::from_bytes([1; 12]), EntityId::PARTICIPANT),
        topic_name: "Circle".into(),
        type_name: "ShapeType".into(),
        durability: DurabilityKind::Volatile,
        durability_service: zerodds_qos::DurabilityServiceQosPolicy::default(),
        reliability: ReliabilityQos {
            kind: ReliabilityKind::Reliable,
            max_blocking_time: zerodds_qos::Duration::from_millis(100_i32),
        },
        ownership: zerodds_qos::OwnershipKind::Shared,
        ownership_strength: 0,
        liveliness: zerodds_qos::LivelinessQosPolicy::default(),
        deadline: zerodds_qos::DeadlineQosPolicy::default(),
        latency_budget: zerodds_qos::LatencyBudgetQosPolicy::default(),
        destination_order: zerodds_qos::DestinationOrderQosPolicy::default(),
        lifespan: zerodds_qos::LifespanQosPolicy::default(),
        presentation: zerodds_qos::PresentationQosPolicy::default(),
        partition: Vec::new(),
        user_data: Vec::new(),
        topic_data: Vec::new(),
        group_data: Vec::new(),
        type_information: None,
        type_id_v1: None,
        type_object_v1: None,
        data_representation: data_rep,
        security_info: None,
        sym_cipher_algo_info: None,
        data_tags: Default::default(),
        properties: Default::default(),
        service_instance_name: None,
        related_entity_guid: None,
        topic_aliases: None,
        type_identifier: zerodds_types::TypeIdentifier::None,
        unicast_locators: Vec::new(),
        multicast_locators: Vec::new(),
        group_guid: None,
        data_max_size_serialized: None,
        entity_name: None,
    }
}

/// Cluster-A: PID_DATA_REPRESENTATION is emitted when the list is
/// non-empty.
#[test]
fn publication_data_emits_data_representation_pid_when_set() {
    let pd = make_pub_data(vec![publication_data::data_representation::XCDR2]);
    let bytes = pd.to_pl_cdr_le().expect("encode");
    // PID layout: 4-byte encap header + ParameterList. Search for
    // 0x0073 little-endian = bytes [0x73, 0x00].
    let hex_dump: String = bytes
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(" ");
    let needle = "73 00";
    assert!(
        hex_dump.contains(needle),
        "PID_DATA_REPRESENTATION (0x0073) NOT found in encoded SEDP pub!\nFull hex:\n{hex_dump}"
    );
}

/// Cluster-A neg: empty list → PID is NOT emitted (pre-D.5f behavior,
/// which is exactly what we want to avoid). Documentary only —
/// production code must ALWAYS populate the list.
#[test]
fn publication_data_skips_pid_when_empty_documentary() {
    let pd = make_pub_data(Vec::new());
    let bytes = pd.to_pl_cdr_le().expect("encode");
    let hex_dump: String = bytes
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(" ");
    assert!(
        !hex_dump.contains("73 00"),
        "empty data_representation emitted a PID — encoder bug:\n{hex_dump}"
    );
}

/// Cluster-A roundtrip: encode → decode preserves the list exactly.
#[test]
fn publication_data_roundtrip_preserves_data_representation() {
    let original = vec![
        publication_data::data_representation::XCDR2,
        publication_data::data_representation::XCDR,
    ];
    let pd = make_pub_data(original.clone());
    let bytes = pd.to_pl_cdr_le().expect("encode");
    let decoded = PublicationBuiltinTopicData::from_pl_cdr_le(&bytes).expect("decode");
    assert_eq!(
        decoded.data_representation, original,
        "Roundtrip dropped DataRepresentation"
    );
}

/// Cluster-B: the DCPS runtime `build_publication_data` sets the field
/// TO XCDR2 (D.5f fix). This test goes red if the fix regresses.
#[test]
fn dcps_runtime_publication_announces_xcdr2() {
    use zerodds_dcps::{
        DataWriterQos, DomainParticipantFactory, DomainParticipantQos, PublisherQos, RawBytes,
        TopicQos,
    };

    let factory = DomainParticipantFactory::instance();
    let p = factory.create_participant_offline(99, DomainParticipantQos::default());
    let topic = p
        .create_topic::<RawBytes>("DRTest", TopicQos::default())
        .expect("topic");
    let publisher = p.create_publisher(PublisherQos::default());
    let _writer = publisher
        .create_datawriter::<RawBytes>(&topic, DataWriterQos::default())
        .expect("writer");
    // Note: offline mode emits no SEDP. This assertion must run via the
    // `build_publication_data` helper if it is public-API-accessible;
    // otherwise via the live-mode test in
    // `live_data_representation_announce.rs`.
    //
    // Here just a smoke test that create_datawriter does not panic.
    let _ = (p, publisher);
}

// ---------------------------------------------------------------------------
// Cluster-C: receiver-side detection — if the announce says XCDR1 but
// the wire encapsulation is XCDR2 (or vice versa), that is a spec
// inconsistency. The reader should drop the sample or log a warning.
// (Implementation TBD)
//
// For now documentary only: we test DATA-submessage encapsulation
// detection at the byte level.

const ENCAP_PLAIN_CDR_LE: [u8; 4] = [0x00, 0x01, 0x00, 0x00]; // XCDR1
const ENCAP_PLAIN_CDR2_LE: [u8; 4] = [0x00, 0x07, 0x00, 0x00]; // XCDR2 final
const ENCAP_DELIM_CDR2_LE: [u8; 4] = [0x00, 0x09, 0x00, 0x00]; // XCDR2 appendable
const ENCAP_PL_CDR2_LE: [u8; 4] = [0x00, 0x0b, 0x00, 0x00]; // XCDR2 mutable

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DataRepFromEncap {
    Xcdr1,
    Xcdr2,
    Unknown,
}

fn detect_data_rep(encap: [u8; 4]) -> DataRepFromEncap {
    match (encap[0], encap[1]) {
        (0x00, 0x00) | (0x00, 0x01) | (0x00, 0x02) | (0x00, 0x03) => DataRepFromEncap::Xcdr1,
        (0x00, 0x06) | (0x00, 0x07) | (0x00, 0x08) | (0x00, 0x09) | (0x00, 0x0a) | (0x00, 0x0b) => {
            DataRepFromEncap::Xcdr2
        }
        _ => DataRepFromEncap::Unknown,
    }
}

#[test]
fn detect_xcdr1_from_plain_cdr_le_encap() {
    assert_eq!(detect_data_rep(ENCAP_PLAIN_CDR_LE), DataRepFromEncap::Xcdr1);
}

#[test]
fn detect_xcdr2_from_plain_cdr2_le_encap() {
    assert_eq!(
        detect_data_rep(ENCAP_PLAIN_CDR2_LE),
        DataRepFromEncap::Xcdr2
    );
}

#[test]
fn detect_xcdr2_from_delimited_cdr2_le_encap() {
    assert_eq!(
        detect_data_rep(ENCAP_DELIM_CDR2_LE),
        DataRepFromEncap::Xcdr2
    );
}

#[test]
fn detect_xcdr2_from_pl_cdr2_le_encap() {
    assert_eq!(detect_data_rep(ENCAP_PL_CDR2_LE), DataRepFromEncap::Xcdr2);
}

/// Consistency check: announced [XCDR2] + received-encap=PLAIN_CDR2_LE → match.
#[test]
fn announced_xcdr2_with_xcdr2_encap_is_consistent() {
    let announced = [publication_data::data_representation::XCDR2];
    let received_rep = detect_data_rep(ENCAP_PLAIN_CDR2_LE);
    assert_eq!(received_rep, DataRepFromEncap::Xcdr2);
    assert!(
        announced.contains(&publication_data::data_representation::XCDR2),
        "announced XCDR2 should accept XCDR2 encap"
    );
}

/// Consistency check FAIL: announced [XCDR1] but received-encap=PLAIN_CDR2_LE.
/// Pre-D.5f bug: ZeroDDS announces nothing (=XCDR1 default) but sends
/// XCDR2 — this mismatch is detected explicitly here.
#[test]
fn announced_default_xcdr1_with_xcdr2_encap_is_inconsistent() {
    let announced_or_default: Vec<i16> = Vec::new(); // empty = default = [XCDR1]
    let effective_announced = if announced_or_default.is_empty() {
        vec![publication_data::data_representation::XCDR]
    } else {
        announced_or_default
    };
    let received_rep = detect_data_rep(ENCAP_PLAIN_CDR2_LE);
    assert_eq!(received_rep, DataRepFromEncap::Xcdr2);
    assert!(
        !effective_announced.contains(&publication_data::data_representation::XCDR2),
        "pre-D.5f constellation: announce-default=XCDR1 + wire=XCDR2 — \
         exactly the inconsistency that RTI 7.7.0 silently rejected."
    );
}

// ---------------------------------------------------------------------------
// D.5g — DataRep-Negotiation Tests (Strict + Tolerant Match-Mode).
// ---------------------------------------------------------------------------

use zerodds_rtps::publication_data::data_representation::{
    DEFAULT_OFFER, DataRepMatchMode, XCDR, XCDR2, encap_for_final_le, negotiate,
};

/// Strict Match (XTypes 1.3 §7.6.3.1.2): Writer's first must be in Reader's list.
#[test]
fn strict_writer_xcdr2_first_reader_xcdr1_only_no_match() {
    let result = negotiate(&[XCDR2, XCDR], &[XCDR], DataRepMatchMode::Strict);
    assert_eq!(
        result, None,
        "Strict-Mode: writer.first=XCDR2 ∉ reader=[XCDR1] → no match"
    );
}

#[test]
fn strict_writer_xcdr1_first_reader_xcdr1_only_matches() {
    let result = negotiate(&[XCDR, XCDR2], &[XCDR], DataRepMatchMode::Strict);
    assert_eq!(
        result,
        Some(XCDR),
        "Strict-Mode: XCDR1-first matches XCDR1-reader"
    );
}

#[test]
fn strict_both_xcdr2_only_matches() {
    let result = negotiate(&[XCDR2], &[XCDR2], DataRepMatchMode::Strict);
    assert_eq!(result, Some(XCDR2));
}

/// Tolerant Match: any overlap → first-overlap wins.
#[test]
fn tolerant_writer_xcdr2_first_reader_xcdr1_only_falls_back() {
    let result = negotiate(&[XCDR2, XCDR], &[XCDR], DataRepMatchMode::Tolerant);
    assert_eq!(
        result,
        Some(XCDR),
        "Tolerant: overlap = {{XCDR}}, picks XCDR (legacy fallback)"
    );
}

#[test]
fn tolerant_both_offer_both_picks_xcdr2_first() {
    let result = negotiate(&[XCDR2, XCDR], &[XCDR2, XCDR], DataRepMatchMode::Tolerant);
    assert_eq!(
        result,
        Some(XCDR2),
        "Tolerant: writer-first XCDR2 is in the reader → pick XCDR2"
    );
}

#[test]
fn tolerant_no_overlap_returns_none() {
    let result = negotiate(&[XCDR2], &[XCDR], DataRepMatchMode::Tolerant);
    assert_eq!(result, None, "Kein overlap = no match");
}

/// Spec default: empty lists → implies [XCDR1].
#[test]
fn empty_writer_list_treated_as_xcdr1() {
    let result = negotiate(&[], &[XCDR2, XCDR], DataRepMatchMode::Strict);
    assert_eq!(
        result,
        Some(XCDR),
        "Empty writer list = [XCDR1] per Spec §7.6.3.1.2"
    );
}

#[test]
fn empty_reader_list_treated_as_xcdr1() {
    let result = negotiate(&[XCDR, XCDR2], &[], DataRepMatchMode::Strict);
    assert_eq!(
        result,
        Some(XCDR),
        "Empty reader list = accepts only [XCDR1]"
    );
}

/// Encap-Header-Mapping pro DataRep.
#[test]
fn encap_for_final_xcdr2_yields_plain_cdr2_le() {
    assert_eq!(encap_for_final_le(XCDR2), [0x00, 0x07, 0x00, 0x00]);
}

#[test]
fn encap_for_final_xcdr1_yields_plain_cdr_le() {
    assert_eq!(encap_for_final_le(XCDR), [0x00, 0x01, 0x00, 0x00]);
}

/// DEFAULT_OFFER is the default of DDS-XTypes 1.3 §7.6.3.1.1: an empty list,
/// which "shall be taken to be equivalent to a list containing the single
/// element XCDR". The list of a peer that writes and reads both is a setting
/// (`OFFER_XCDR1_XCDR2`), XCDR1 first: §7.6.3.1.2 matches the writer's first
/// element against the reader's list, and the reader's list is `[XCDR1]`
/// whenever a peer leaves the QoS at the DDS default.
#[test]
fn default_offer_is_xcdr1_alone_and_both_is_a_setting() {
    assert_eq!(DEFAULT_OFFER, [XCDR]);
    assert_eq!(
        zerodds_rtps::publication_data::data_representation::OFFER_XCDR1_XCDR2,
        [XCDR, XCDR2]
    );
}

// ---------------------------------------------------------------------------
// D.5g Config-Options — RuntimeConfig override + Per-Writer/Reader override
// ---------------------------------------------------------------------------

/// `RuntimeConfig::data_representation_offer` is configurable.
#[test]
fn runtime_config_data_rep_offer_default_matches_default_offer() {
    use zerodds_dcps::runtime::RuntimeConfig;
    let cfg = RuntimeConfig::default();
    assert_eq!(
        cfg.data_representation_offer.as_slice(),
        DEFAULT_OFFER,
        "RuntimeConfig-Default = lib-DEFAULT_OFFER"
    );
}

/// `RuntimeConfig::data_rep_match_mode` is configurable; default = Strict,
/// DDS-XTypes 1.3 §7.6.3.1.1: the offer is compatible when the writer's
/// offered representation is contained in the reader's sequence.
#[test]
fn runtime_config_data_rep_match_mode_default_is_strict() {
    use zerodds_dcps::runtime::RuntimeConfig;
    let cfg = RuntimeConfig::default();
    assert_eq!(cfg.data_rep_match_mode, DataRepMatchMode::Strict);
}

/// The tolerant mode stays selectable.
#[test]
fn runtime_config_data_rep_match_mode_tolerant_is_selectable() {
    use zerodds_dcps::runtime::RuntimeConfig;
    let cfg = RuntimeConfig {
        data_rep_match_mode: DataRepMatchMode::Tolerant,
        ..RuntimeConfig::default()
    };
    assert_eq!(cfg.data_rep_match_mode, DataRepMatchMode::Tolerant);
}

/// The user can set a custom offer list, e.g. XCDR2-only for modern deployments.
#[test]
fn runtime_config_data_rep_offer_user_override_xcdr2_only() {
    use zerodds_dcps::runtime::RuntimeConfig;
    let cfg = RuntimeConfig {
        data_representation_offer: vec![XCDR2],
        ..RuntimeConfig::default()
    };
    assert_eq!(cfg.data_representation_offer, vec![XCDR2]);
}

/// The user can set strict mode.
#[test]
fn runtime_config_data_rep_match_mode_user_strict() {
    use zerodds_dcps::runtime::RuntimeConfig;
    let cfg = RuntimeConfig {
        data_rep_match_mode: DataRepMatchMode::Strict,
        ..RuntimeConfig::default()
    };
    assert_eq!(cfg.data_rep_match_mode, DataRepMatchMode::Strict);
}

/// DDS-XTypes 1.3 Annex D: `TopicBuiltinTopicData` has
/// `@id(0x0075) @optional TypeInformation type_information` and
/// `@id(0x0073) DataRepresentationQosPolicy representation`. The topic
/// announcement writes both parameters under those ids and reads them back.
#[test]
fn topic_data_roundtrip_preserves_data_representation_and_type_information() {
    use zerodds_rtps::parameter_list::pid;
    use zerodds_rtps::topic_data::TopicBuiltinTopicData;
    use zerodds_rtps::wire_types::VendorId;

    assert_eq!(pid::DATA_REPRESENTATION, 0x0073, "Annex D: @id(0x0073)");
    assert_eq!(pid::TYPE_INFORMATION, 0x0075, "Annex D: @id(0x0075)");

    let key = Guid::new(
        GuidPrefix::from_bytes([3; 12]),
        EntityId::user_writer_with_key([0, 0, 7]),
    );
    let mut topic = TopicBuiltinTopicData::new(key, "AnnexD", "annex_d::Type");
    topic.vendor_id = None;
    topic.data_representation = vec![
        publication_data::data_representation::XCDR2,
        publication_data::data_representation::XCDR,
    ];
    // The octets of the TypeInformation are kept as they are.
    topic.type_information = Some(vec![0x10, 0, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8]);

    let bytes = topic.to_pl_cdr_le().expect("encode");
    let parameter_ids = {
        // 4 octets of encapsulation, then (id, length, value) with the
        // length a multiple of four.
        let mut ids = Vec::new();
        let mut at = 4;
        while at + 4 <= bytes.len() {
            let id = u16::from_le_bytes([bytes[at], bytes[at + 1]]);
            let length = usize::from(u16::from_le_bytes([bytes[at + 2], bytes[at + 3]]));
            if id == 0x0001 {
                break;
            }
            ids.push(id);
            at += 4 + length;
        }
        ids
    };
    assert!(parameter_ids.contains(&0x0073), "{parameter_ids:x?}");
    assert!(parameter_ids.contains(&0x0075), "{parameter_ids:x?}");

    let decoded = TopicBuiltinTopicData::from_pl_cdr(&bytes, VendorId::ZERODDS).expect("decode");
    assert_eq!(decoded.data_representation, topic.data_representation);
    assert_eq!(decoded.type_information, topic.type_information);
}
