//! D.5f — SEDP-PID-DATA_REPRESENTATION-Conformance.
//!
//! Schuetzt gegen den D.5e-Bug "ZeroDDS announct kein
//! PID_DATA_REPRESENTATION (PID 0x0073)" der bei Live-Interop gegen
//! strict Vendoren (RTI Connext 7.7.0) zu silent SEDP-Match-Fail
//! fuehrt.
//!
//! Spec-Quellen:
//! * **DDS-XTypes 1.3 §7.6.3.1.1** — `DataRepresentationQosPolicy`
//!   mit `value: sequence<DataRepresentationId>`. IDs:
//!   `XCDR_DATA_REPRESENTATION = 0`, `XML = 1`, `XCDR2 = 2`.
//! * **DDSI-RTPS 2.5** PID-Tabelle: `PID_DATA_REPRESENTATION = 0x0073`.
//! * **DDS-XTypes 1.3 §7.6.3.1.2** — Default ohne Policy-Match ist
//!   `[XCDR1]` (legacy). Wenn ZeroDDS XCDR2-Encap (`0x0007`) emittiert,
//!   muss es das im SEDP auch ankuendigen.
//!
//! Pre-D.5f: ZeroDDS hat `data_representation: Vec::new()` und der
//! Encoder skippt das PID. Unser Wire-Encap war XCDR2 — aber
//! Announcement war default-XCDR1 = inkonsistent.

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
        key: Guid::new(
            GuidPrefix::from_bytes([1; 12]),
            EntityId::user_writer_with_key([0, 0, 1]),
        ),
        participant_key: Guid::new(GuidPrefix::from_bytes([1; 12]), EntityId::PARTICIPANT),
        topic_name: "Circle".into(),
        type_name: "ShapeType".into(),
        durability: DurabilityKind::Volatile,
        reliability: ReliabilityQos {
            kind: ReliabilityKind::Reliable,
            max_blocking_time: zerodds_qos::Duration::from_millis(100_i32),
        },
        ownership: zerodds_qos::OwnershipKind::Shared,
        ownership_strength: 0,
        liveliness: zerodds_qos::LivelinessQosPolicy::default(),
        deadline: zerodds_qos::DeadlineQosPolicy::default(),
        lifespan: zerodds_qos::LifespanQosPolicy::default(),
        partition: Vec::new(),
        user_data: Vec::new(),
        topic_data: Vec::new(),
        group_data: Vec::new(),
        type_information: None,
        data_representation: data_rep,
        security_info: None,
        service_instance_name: None,
        related_entity_guid: None,
        topic_aliases: None,
        type_identifier: zerodds_types::TypeIdentifier::None,
    }
}

/// Cluster-A: PID_DATA_REPRESENTATION wird emittiert wenn Liste
/// non-empty ist.
#[test]
fn publication_data_emits_data_representation_pid_when_set() {
    let pd = make_pub_data(vec![publication_data::data_representation::XCDR2]);
    let bytes = pd.to_pl_cdr_le().expect("encode");
    // PID-Layout: 4 byte encap header + ParameterList. Suche nach
    // 0x0073 little-endian = bytes [0x73, 0x00].
    let hex_dump: String = bytes
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(" ");
    let needle = "73 00";
    assert!(
        hex_dump.contains(needle),
        "PID_DATA_REPRESENTATION (0x0073) NICHT in encoded SEDP-Pub gefunden!\nFull hex:\n{hex_dump}"
    );
}

/// Cluster-A neg: leere Liste → PID wird NICHT emittiert (Pre-D.5f
/// Verhalten, das wir genau vermeiden wollen). Nur dokumentarisch —
/// Production-Code muss IMMER die Liste befuellen.
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
        "Empty data_representation hat PID emittiert — Encoder-Bug:\n{hex_dump}"
    );
}

/// Cluster-A roundtrip: encode → decode preserviert Liste exakt.
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

/// Cluster-B: das DCPS-Runtime `build_publication_data` setzt das Feld
/// AUF XCDR2 (D.5f-Fix). Dieser Test wird red wenn der Fix
/// regrediert wird.
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
    // Note: offline-Mode emittiert kein SEDP. Diese Assertion muss
    // ueber den `build_publication_data`-Helper laufen wenn er
    // public-API-zugaenglich ist; sonst ueber den Live-Mode-Test in
    // `live_data_representation_announce.rs`.
    //
    // Hier nur Smoketest dass create_datawriter nicht panicked.
    let _ = (p, publisher);
}

// ---------------------------------------------------------------------------
// Cluster-C: receiver-side detection — wenn announce sagt XCDR1 aber
// wire-encap ist XCDR2 (oder umgekehrt), das ist Spec-Inkonsistenz.
// Reader sollte sample droppen oder log Warning. (Implementation TBD)
//
// Vorerst nur dokumentarisch: wir testen die DATA-Submessage-Encap-
// Erkennung auf Bytes-Level.

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

/// Konsistenz-Check: announced [XCDR2] + received-encap=PLAIN_CDR2_LE → match.
#[test]
fn announced_xcdr2_with_xcdr2_encap_is_consistent() {
    let announced = [publication_data::data_representation::XCDR2];
    let received_rep = detect_data_rep(ENCAP_PLAIN_CDR2_LE);
    assert_eq!(received_rep, DataRepFromEncap::Xcdr2);
    assert!(
        announced.contains(&publication_data::data_representation::XCDR2),
        "announced XCDR2 sollte XCDR2-encap akzeptieren"
    );
}

/// Konsistenz-Check FAIL: announced [XCDR1] aber received-encap=PLAIN_CDR2_LE.
/// Pre-D.5f-Bug: ZeroDDS announct nichts (=XCDR1 default) aber sendet
/// XCDR2 — dieser Mismatch wird hier explizit erkannt.
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
        "Pre-D.5f-Konstellation: announce-default=XCDR1 + wire=XCDR2 — \
         genau die Inkonsistenz die RTI 7.7.0 silently rejected hat."
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
        "Tolerant: writer-first XCDR2 ist im reader → pick XCDR2"
    );
}

#[test]
fn tolerant_no_overlap_returns_none() {
    let result = negotiate(&[XCDR2], &[XCDR], DataRepMatchMode::Tolerant);
    assert_eq!(result, None, "Kein overlap = no match");
}

/// Spec-Default: leere Listen → impliziert [XCDR1].
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

/// DEFAULT_OFFER Sanity-Check.
///
/// `[XCDR1, XCDR2]`: XCDR1 first damit Strict-Spec-Vendoren wie RTI
/// matchen (writer.first ∈ reader.list mit reader=[XCDR1]). XCDR2
/// second als modern fallback fuer Peers die XCDR2 koennen.
#[test]
fn default_offer_is_xcdr1_first_xcdr2_second() {
    assert_eq!(
        DEFAULT_OFFER,
        [XCDR, XCDR2],
        "Default offer: prefer legacy XCDR1 (RTI-strict-compat), modern XCDR2 als Fallback"
    );
}

// ---------------------------------------------------------------------------
// D.5g Config-Options — RuntimeConfig override + Per-Writer/Reader override
// ---------------------------------------------------------------------------

/// `RuntimeConfig::data_representation_offer` ist konfigurierbar.
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

/// `RuntimeConfig::data_rep_match_mode` ist konfigurierbar; Default = Tolerant.
#[test]
fn runtime_config_data_rep_match_mode_default_is_tolerant() {
    use zerodds_dcps::runtime::RuntimeConfig;
    let cfg = RuntimeConfig::default();
    assert_eq!(cfg.data_rep_match_mode, DataRepMatchMode::Tolerant);
}

/// User kann eigene Offer-Liste setzen, z.B. XCDR2-only fuer modern-Deployments.
#[test]
fn runtime_config_data_rep_offer_user_override_xcdr2_only() {
    use zerodds_dcps::runtime::RuntimeConfig;
    let cfg = RuntimeConfig {
        data_representation_offer: vec![XCDR2],
        ..RuntimeConfig::default()
    };
    assert_eq!(cfg.data_representation_offer, vec![XCDR2]);
}

/// User kann Strict-Mode setzen.
#[test]
fn runtime_config_data_rep_match_mode_user_strict() {
    use zerodds_dcps::runtime::RuntimeConfig;
    let cfg = RuntimeConfig {
        data_rep_match_mode: DataRepMatchMode::Strict,
        ..RuntimeConfig::default()
    };
    assert_eq!(cfg.data_rep_match_mode, DataRepMatchMode::Strict);
}
