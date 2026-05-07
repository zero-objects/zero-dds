// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Properties + Application-Properties Producer.
//!
//! Spec-Quellen:
//! * dds-amqp-1.0 §8.2 — Properties-Section (`message-id`,
//!   `group-id`, `creation-time`, `absolute-expiry-time`, ...).
//! * dds-amqp-1.0 §8.3 — Application-Properties (`dds:*` keys).
//!
//! Dieses Modul liefert die normativ vorgeschriebenen Belegungs-
//! Funktionen und stellt sicher, dass der per-Sample
//! `message-id`-Identifier (24 Byte: writer-GUID || RTPS-seqnum)
//! statt des per-Instanz `InstanceHandle_t` benutzt wird (siehe
//! Spec §8.2 Rationale).

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use zerodds_amqp_bridge::extended_types::AmqpExtValue;

use crate::keyhash;

/// Spec §8.2 — DDS-Sample-Operation, die in der App-Property
/// `dds:operation` codiert wird.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DdsOperation {
    /// Reines Datum-Sample (Default wenn `dds:operation` fehlt).
    #[default]
    Write,
    /// `register_instance`.
    Register,
    /// `unregister_instance`.
    Unregister,
    /// `dispose`.
    Dispose,
}

impl DdsOperation {
    /// String-Repraesentation gemaess Spec §7.7.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Write => "write",
            Self::Register => "register",
            Self::Unregister => "unregister",
            Self::Dispose => "dispose",
        }
    }

    /// Inverse-Decode aus `dds:operation`-String.
    ///
    /// # Errors
    /// `Err(input)` bei unbekanntem Wert (Spec §11.2 →
    /// `amqp:not-implemented`).
    pub fn parse(s: &str) -> Result<Self, &str> {
        match s {
            "write" => Ok(Self::Write),
            "register" => Ok(Self::Register),
            "unregister" => Ok(Self::Unregister),
            "dispose" => Ok(Self::Dispose),
            other => Err(other),
        }
    }
}

/// Eingabe-Daten fuer den Properties-Producer pro Sample.
#[derive(Debug, Clone)]
pub struct SampleHeader {
    /// RTPS Writer-GUID (16 Byte).
    pub writer_guid: [u8; 16],
    /// RTPS Sequence-Number.
    pub seqnum: u64,
    /// DDS `Time_t.sec * 1000 + nanosec/1_000_000` (ms-Praezision).
    pub source_timestamp_ms: i64,
    /// Sub-Millisekunden-Anteil aus `Time_t.nanosec % 1_000_000`.
    pub source_nsec_remainder: u32,
    /// XCDR2-KeyHash-Bytes (Source fuer §7.6.1 group-id);
    /// `None` bei unkeyed Topic (group-id wird weggelassen).
    pub keyhash: Option<Vec<u8>>,
    /// `InstanceHandle_t` (16 Byte) — wandert auf
    /// `dds:instance-handle` App-Property statt auf `message-id`.
    pub instance_handle: [u8; 16],
    /// Optionaler verbleibender LIFESPAN-Rest in Millisekunden;
    /// wenn `Some`, fuettert `absolute-expiry-time`.
    pub lifespan_remaining_ms: Option<i64>,
    /// DDS-Sample-Operation (Default `Write`).
    pub operation: DdsOperation,
    /// X-Types-TypeIdentifier in voller 14-Byte-Hex-Form;
    /// PFLICHT bei `descriptor_form = DESC_TRUNCATED` (Spec
    /// §7.2.1.3), sonst `None`.
    pub type_id_hex: Option<String>,
    /// DDS Domain-Id.
    pub domain_id: u32,
    /// DDS Partition-QoS (sequence; `vec![]` = Default-Partition).
    pub partitions: Vec<String>,
}

/// Spec §8.2 — `message-id` als 24-Byte-binary erzeugen.
///
/// Format: `writer_guid (16B BE) || seqnum (8B BE)`. Eindeutig
/// pro Sample (verhindert Broker-Dedup-Falle wie z.B. Service Bus
/// `EnableDuplicateDetection`, vgl. §8.2 Rationale).
#[must_use]
pub fn message_id(writer_guid: [u8; 16], seqnum: u64) -> Vec<u8> {
    let mut out = Vec::with_capacity(24);
    out.extend_from_slice(&writer_guid);
    out.extend_from_slice(&seqnum.to_be_bytes());
    out
}

/// Hex-encode 16 oder 24 Byte fuer JSON-Mode-Surface (§8.1.2).
#[must_use]
pub fn hex_lower(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = core::fmt::Write::write_fmt(&mut out, core::format_args!("{b:02x}"));
    }
    out
}

/// Spec §8.2 — produzierte Properties-Section-Felder.
///
/// Wir liefern eine Sub-Struktur mit den von dieser Spec
/// belegten Feldern; der Caller (Endpoint-Daemon) baut daraus
/// das `MessageSection::Properties`-AMQP-list-Composite.
#[derive(Debug, Clone)]
pub struct ProducedProperties {
    /// `message-id` als binary(24).
    pub message_id: Vec<u8>,
    /// `creation-time` (8 Byte BE Timestamp ms-since-epoch).
    pub creation_time_ms: i64,
    /// `absolute-expiry-time` (Timestamp ms) — `None` wenn LIFESPAN
    /// nicht konfiguriert.
    pub absolute_expiry_time_ms: Option<i64>,
    /// `group-id` (SHA-256-Hex-Digest aus KeyHash) — `None` bei
    /// unkeyed Topic.
    pub group_id: Option<String>,
}

/// Spec §8.2 — Properties-Section aus Sample-Header bauen.
///
/// Liefert die normativ belegten Felder. `to`, `subject`,
/// `reply-to`, `correlation-id`, `content-type`,
/// `content-encoding`, `group-sequence`, `reply-to-group-id`,
/// `user-id` sind optional und Caller-Sache (subject z.B.
/// applikations-spezifischer Routing-Key).
#[must_use]
pub fn produce_properties(hdr: &SampleHeader) -> ProducedProperties {
    ProducedProperties {
        message_id: message_id(hdr.writer_guid, hdr.seqnum),
        creation_time_ms: hdr.source_timestamp_ms,
        absolute_expiry_time_ms: hdr
            .lifespan_remaining_ms
            .map(|rem| hdr.source_timestamp_ms.saturating_add(rem)),
        group_id: hdr.keyhash.as_deref().map(keyhash::group_id),
    }
}

/// Spec §8.3 — Standard-`dds:*`-App-Property-Keys.
pub mod app_keys {
    /// `dds:nsec` — sub-Millisekunden-Anteil von `Time_t`.
    pub const NSEC: &str = "dds:nsec";
    /// `dds:partition` — DDS Partition-QoS (list-of-string oder string).
    pub const PARTITION: &str = "dds:partition";
    /// `dds:domain-id` — DDS Domain-Id (Integer).
    pub const DOMAIN_ID: &str = "dds:domain-id";
    /// `dds:type-id` — XTypes TypeIdentifier hex (PFLICHT bei TRUNCATED).
    pub const TYPE_ID: &str = "dds:type-id";
    /// `dds:source-guid` — Originating-Endpoint-GUID hex.
    pub const SOURCE_GUID: &str = "dds:source-guid";
    /// `dds:lifespan-ms` — Rest-LIFESPAN in Millisekunden.
    pub const LIFESPAN_MS: &str = "dds:lifespan-ms";
    /// `dds:sample-state` — read / not-read.
    pub const SAMPLE_STATE: &str = "dds:sample-state";
    /// `dds:view-state` — new / not-new.
    pub const VIEW_STATE: &str = "dds:view-state";
    /// `dds:instance-state` — alive / not-alive-disposed / not-alive-no-writers.
    pub const INSTANCE_STATE: &str = "dds:instance-state";
    /// `dds:operation` — write / register / unregister / dispose.
    pub const OPERATION: &str = "dds:operation";
    /// `dds:bridge-id` — Liste durchgelaufener Bridge-UUIDs (Loop-Prevention).
    pub const BRIDGE_ID: &str = "dds:bridge-id";
    /// `dds:bridge-hop` — Hop-Counter (Loop-Prevention).
    pub const BRIDGE_HOP: &str = "dds:bridge-hop";
    /// `dds:instance-handle` — DDS InstanceHandle_t als binary(16).
    pub const INSTANCE_HANDLE: &str = "dds:instance-handle";
}

// ============================================================
// §7.2.1.3 — Receiver-Side Type-ID Collision Inspector
// ============================================================

/// Spec §7.2.1.3 — Resultat einer Type-ID-Inspektion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeIdCheck {
    /// `dds:type-id` matched die lokal erwartete TypeIdentifier-
    /// Hex-Form. Decode kann fortgesetzt werden.
    Match,
    /// `dds:type-id` fehlt (Sender hat `descriptor_form = DESC_FULL`
    /// benutzt). Decode kann fortgesetzt werden.
    Absent,
    /// `dds:type-id` matched NICHT die erwartete Form — Hash-
    /// Truncation-Kollision detektiert. Receiver muss den Transfer
    /// mit `amqp:decode-error` rejecten (Spec §7.2.1.3).
    Mismatch {
        /// `dds:type-id`-Wert aus der Application-Property.
        received: String,
        /// Lokal erwartete TypeIdentifier-Hex-Form.
        expected: String,
    },
}

/// Spec §7.2.1.3 — Receiver-Side Inspektor fuer
/// Hash-Truncation-Kollisionen.
///
/// Wenn der Sender mit `descriptor_form = DESC_TRUNCATED` arbeitet
/// (default), MUSS er die volle 14-Byte-TypeIdentifier-Hex-Form
/// als `dds:type-id`-Application-Property mitgeben. Der Receiver
/// vergleicht das gegen die lokal-bekannte Type-Form. Bei
/// Mismatch ist ein Hash-Truncation-Kollisionspaar entdeckt
/// worden — Sample MUSS rejected werden.
///
/// Liefert [`TypeIdCheck::Match`] wenn property gesetzt und
/// matcht; [`TypeIdCheck::Absent`] wenn nicht gesetzt
/// (DESC_FULL-Pfad); [`TypeIdCheck::Mismatch`] wenn gesetzt
/// und nicht matcht.
#[must_use]
pub fn inspect_dds_type_id(app_props: &AmqpExtValue, expected_hex: &str) -> TypeIdCheck {
    let entries = match app_props {
        AmqpExtValue::Map(v) => v,
        _ => return TypeIdCheck::Absent,
    };
    let want = AmqpExtValue::Str(app_keys::TYPE_ID.to_string());
    for (k, v) in entries {
        if *k == want {
            let received = match v {
                AmqpExtValue::Str(s) => s.clone(),
                AmqpExtValue::Symbol(s) => s.clone(),
                AmqpExtValue::Binary(b) => hex_lower(b),
                _ => return TypeIdCheck::Absent,
            };
            return if received.eq_ignore_ascii_case(expected_hex) {
                TypeIdCheck::Match
            } else {
                TypeIdCheck::Mismatch {
                    received,
                    expected: expected_hex.to_string(),
                }
            };
        }
    }
    TypeIdCheck::Absent
}

/// Spec §8.3 — Application-Properties-Map aus Sample-Header bauen.
///
/// Liefert eine `AmqpExtValue::Map` mit den Standard-Keys, die
/// normativ aus `SampleHeader` ableitbar sind. Der Caller darf
/// weitere applikations-eigene Keys ergaenzen — diese duerfen
/// per Spec den Praefix `dds:` nicht verwenden.
#[must_use]
pub fn produce_application_properties(hdr: &SampleHeader) -> AmqpExtValue {
    let mut map: Vec<(AmqpExtValue, AmqpExtValue)> = Vec::new();

    // dds:nsec — nur wenn Sub-Millisekunden-Anteil > 0.
    if hdr.source_nsec_remainder != 0 {
        map.push((
            AmqpExtValue::Str(app_keys::NSEC.to_string()),
            AmqpExtValue::Uint(hdr.source_nsec_remainder),
        ));
    }

    // dds:partition — sequence<string> oder einzelner string.
    match hdr.partitions.len() {
        0 => {} // Default-Partition: weglassen.
        1 => {
            map.push((
                AmqpExtValue::Str(app_keys::PARTITION.to_string()),
                AmqpExtValue::Str(hdr.partitions[0].clone()),
            ));
        }
        _ => {
            let list: Vec<AmqpExtValue> = hdr
                .partitions
                .iter()
                .map(|p| AmqpExtValue::Str(p.clone()))
                .collect();
            map.push((
                AmqpExtValue::Str(app_keys::PARTITION.to_string()),
                AmqpExtValue::List(list),
            ));
        }
    }

    // dds:domain-id.
    map.push((
        AmqpExtValue::Str(app_keys::DOMAIN_ID.to_string()),
        AmqpExtValue::Uint(hdr.domain_id),
    ));

    // dds:type-id — bei TRUNCATED Pflicht (Spec §7.2.1.3); der
    // Caller muss `type_id_hex` setzen wenn descriptor_form =
    // DESC_TRUNCATED.
    if let Some(hex) = &hdr.type_id_hex {
        map.push((
            AmqpExtValue::Str(app_keys::TYPE_ID.to_string()),
            AmqpExtValue::Str(hex.clone()),
        ));
    }

    // dds:lifespan-ms — wenn LIFESPAN-Rest verfuegbar.
    if let Some(rem) = hdr.lifespan_remaining_ms {
        map.push((
            AmqpExtValue::Str(app_keys::LIFESPAN_MS.to_string()),
            AmqpExtValue::Long(rem),
        ));
    }

    // dds:operation — Default `write` weglassen (Spec §7.7.1).
    if hdr.operation != DdsOperation::Write {
        map.push((
            AmqpExtValue::Str(app_keys::OPERATION.to_string()),
            AmqpExtValue::Str(hdr.operation.as_str().to_string()),
        ));
    }

    // dds:instance-handle — immer (16 Byte binary).
    map.push((
        AmqpExtValue::Str(app_keys::INSTANCE_HANDLE.to_string()),
        AmqpExtValue::Binary(hdr.instance_handle.to_vec()),
    ));

    AmqpExtValue::Map(map)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    fn header() -> SampleHeader {
        SampleHeader {
            writer_guid: [
                0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e,
                0x0f, 0x10,
            ],
            seqnum: 0x0000_0000_0000_002a, // 42
            source_timestamp_ms: 1_700_000_000_000,
            source_nsec_remainder: 0,
            keyhash: None,
            instance_handle: [0u8; 16],
            lifespan_remaining_ms: None,
            operation: DdsOperation::Write,
            type_id_hex: None,
            domain_id: 0,
            partitions: Vec::new(),
        }
    }

    #[test]
    fn message_id_is_24_bytes_guid_then_seqnum() {
        let mid = message_id([0u8; 16], 1);
        assert_eq!(mid.len(), 24);
        assert_eq!(&mid[16..24], &[0u8, 0, 0, 0, 0, 0, 0, 1]);
    }

    #[test]
    fn message_id_distinguishes_consecutive_samples_same_instance() {
        // Spec-Rationale: ueber zwei Samples desselben Keys soll
        // message-id verschieden sein, sonst feuert Service-Bus-
        // Dedup faelschlich (Round-12-P0.3).
        let g = [0xAAu8; 16];
        let m1 = message_id(g, 1);
        let m2 = message_id(g, 2);
        assert_ne!(m1, m2);
    }

    #[test]
    fn dds_operation_round_trips_via_str() {
        for op in [
            DdsOperation::Write,
            DdsOperation::Register,
            DdsOperation::Unregister,
            DdsOperation::Dispose,
        ] {
            let s = op.as_str();
            let back = DdsOperation::parse(s).unwrap();
            assert_eq!(op, back);
        }
    }

    #[test]
    fn dds_operation_unknown_yields_err() {
        assert!(DdsOperation::parse("bogus").is_err());
    }

    #[test]
    fn produce_properties_keyhash_yields_group_id() {
        let mut hdr = header();
        hdr.keyhash = Some(b"\x00\x00\x00\x07".to_vec());
        let p = produce_properties(&hdr);
        assert_eq!(p.message_id.len(), 24);
        assert!(p.group_id.is_some());
        let g = p.group_id.unwrap();
        assert_eq!(g.len(), 64);
    }

    #[test]
    fn produce_properties_unkeyed_omits_group_id() {
        let p = produce_properties(&header());
        assert!(p.group_id.is_none());
    }

    #[test]
    fn produce_properties_lifespan_sets_expiry() {
        let mut hdr = header();
        hdr.source_timestamp_ms = 1000;
        hdr.lifespan_remaining_ms = Some(5000);
        let p = produce_properties(&hdr);
        assert_eq!(p.absolute_expiry_time_ms, Some(6000));
    }

    #[test]
    fn application_properties_default_minimal_set() {
        // Default-Header: nur dds:domain-id und dds:instance-handle.
        let m = produce_application_properties(&header());
        let entries = match m {
            AmqpExtValue::Map(v) => v,
            _ => panic!("expected Map"),
        };
        let keys: Vec<&str> = entries
            .iter()
            .map(|(k, _)| match k {
                AmqpExtValue::Str(s) => s.as_str(),
                _ => "",
            })
            .collect();
        assert!(keys.contains(&"dds:domain-id"));
        assert!(keys.contains(&"dds:instance-handle"));
        // Default-write wird weggelassen.
        assert!(!keys.contains(&"dds:operation"));
        // Sub-millisekunden = 0 wird weggelassen.
        assert!(!keys.contains(&"dds:nsec"));
    }

    #[test]
    fn application_properties_register_sets_operation() {
        let mut hdr = header();
        hdr.operation = DdsOperation::Register;
        let m = produce_application_properties(&hdr);
        let entries = match m {
            AmqpExtValue::Map(v) => v,
            _ => panic!(),
        };
        let op = entries
            .iter()
            .find(|(k, _)| matches!(k, AmqpExtValue::Str(s) if s == "dds:operation"))
            .map(|(_, v)| v.clone())
            .unwrap();
        assert_eq!(op, AmqpExtValue::Str("register".to_string()));
    }

    #[test]
    fn application_properties_truncated_type_id_present() {
        let mut hdr = header();
        hdr.type_id_hex = Some("deadbeefcafebabe1234567890ab".to_string());
        let m = produce_application_properties(&hdr);
        let entries = match m {
            AmqpExtValue::Map(v) => v,
            _ => panic!(),
        };
        let tid = entries
            .iter()
            .any(|(k, _)| matches!(k, AmqpExtValue::Str(s) if s == "dds:type-id"));
        assert!(tid);
    }

    #[test]
    fn application_properties_multi_partition_uses_list() {
        let mut hdr = header();
        hdr.partitions = alloc::vec!["alpha".into(), "beta".into()];
        let m = produce_application_properties(&hdr);
        let entries = match m {
            AmqpExtValue::Map(v) => v,
            _ => panic!(),
        };
        let part = entries
            .iter()
            .find(|(k, _)| matches!(k, AmqpExtValue::Str(s) if s == "dds:partition"))
            .map(|(_, v)| v.clone())
            .unwrap();
        match part {
            AmqpExtValue::List(items) => assert_eq!(items.len(), 2),
            other => panic!("expected list, got {other:?}"),
        }
    }

    #[test]
    fn application_properties_single_partition_uses_string() {
        let mut hdr = header();
        hdr.partitions = alloc::vec!["solo".into()];
        let m = produce_application_properties(&hdr);
        let entries = match m {
            AmqpExtValue::Map(v) => v,
            _ => panic!(),
        };
        let part = entries
            .iter()
            .find(|(k, _)| matches!(k, AmqpExtValue::Str(s) if s == "dds:partition"))
            .map(|(_, v)| v.clone())
            .unwrap();
        assert_eq!(part, AmqpExtValue::Str("solo".to_string()));
    }

    #[test]
    fn application_properties_nsec_set_when_nonzero() {
        let mut hdr = header();
        hdr.source_nsec_remainder = 123_456;
        let m = produce_application_properties(&hdr);
        let entries = match m {
            AmqpExtValue::Map(v) => v,
            _ => panic!(),
        };
        let nsec = entries
            .iter()
            .find(|(k, _)| matches!(k, AmqpExtValue::Str(s) if s == "dds:nsec"))
            .map(|(_, v)| v.clone())
            .unwrap();
        assert_eq!(nsec, AmqpExtValue::Uint(123_456));
    }

    #[test]
    fn hex_lower_24_byte() {
        let bytes = [0xAB, 0xCD];
        assert_eq!(hex_lower(&bytes), "abcd");
    }

    // ---- §7.2.1.3 Type-ID Inspector ----

    fn props_with(entries: Vec<(&str, AmqpExtValue)>) -> AmqpExtValue {
        AmqpExtValue::Map(
            entries
                .into_iter()
                .map(|(k, v)| (AmqpExtValue::Str(k.to_string()), v))
                .collect(),
        )
    }

    #[test]
    fn type_id_inspector_match_returns_match() {
        let p = props_with(alloc::vec![(
            app_keys::TYPE_ID,
            AmqpExtValue::Str("deadbeefcafebabe1234567890ab".to_string()),
        )]);
        let r = inspect_dds_type_id(&p, "deadbeefcafebabe1234567890ab");
        assert_eq!(r, TypeIdCheck::Match);
    }

    #[test]
    fn type_id_inspector_case_insensitive_match() {
        let p = props_with(alloc::vec![(
            app_keys::TYPE_ID,
            AmqpExtValue::Str("DEADBEEFCAFEBABE".to_string()),
        )]);
        let r = inspect_dds_type_id(&p, "deadbeefcafebabe");
        assert_eq!(r, TypeIdCheck::Match);
    }

    #[test]
    fn type_id_inspector_absent_returns_absent() {
        let p = props_with(alloc::vec![(app_keys::DOMAIN_ID, AmqpExtValue::Uint(42),)]);
        let r = inspect_dds_type_id(&p, "deadbeefcafebabe");
        assert_eq!(r, TypeIdCheck::Absent);
    }

    #[test]
    fn type_id_inspector_mismatch_detects_collision() {
        // Spec §7.2.1.3: descriptor matched (8-Byte ulong), aber
        // dds:type-id differs → Hash-Truncation-Kollision.
        let p = props_with(alloc::vec![(
            app_keys::TYPE_ID,
            AmqpExtValue::Str("deadbeefcafebabe1111111111ff".to_string()),
        )]);
        let r = inspect_dds_type_id(&p, "deadbeefcafebabe2222222222ee");
        match r {
            TypeIdCheck::Mismatch { received, expected } => {
                assert!(received.contains("1111"));
                assert!(expected.contains("2222"));
            }
            other => panic!("expected mismatch, got {other:?}"),
        }
    }

    #[test]
    fn type_id_inspector_accepts_symbol_form() {
        let p = props_with(alloc::vec![(
            app_keys::TYPE_ID,
            AmqpExtValue::Symbol("dds:type:abc".to_string()),
        )]);
        let r = inspect_dds_type_id(&p, "dds:type:abc");
        assert_eq!(r, TypeIdCheck::Match);
    }

    #[test]
    fn type_id_inspector_non_map_yields_absent() {
        let r = inspect_dds_type_id(&AmqpExtValue::Null, "x");
        assert_eq!(r, TypeIdCheck::Absent);
    }
}
