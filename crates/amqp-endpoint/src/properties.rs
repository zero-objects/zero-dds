// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Properties + application-properties producer.
//!
//! Spec sources:
//! * dds-amqp-1.0 §8.2 — properties section (`message-id`,
//!   `group-id`, `creation-time`, `absolute-expiry-time`, ...).
//! * dds-amqp-1.0 §8.3 — application properties (`dds:*` keys).
//!
//! This module provides the normatively prescribed population
//! functions and ensures that the per-sample
//! `message-id` identifier (24 bytes: writer GUID || RTPS seqnum)
//! is used instead of the per-instance `InstanceHandle_t` (see
//! Spec §8.2 Rationale).

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use zerodds_amqp_bridge::extended_types::AmqpExtValue;

use crate::keyhash;

/// Spec §8.2 — DDS sample operation encoded in the app property
/// `dds:operation`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DdsOperation {
    /// Plain data sample (default when `dds:operation` is absent).
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
    /// String representation per Spec §7.7.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Write => "write",
            Self::Register => "register",
            Self::Unregister => "unregister",
            Self::Dispose => "dispose",
        }
    }

    /// Inverse decode from a `dds:operation` string.
    ///
    /// # Errors
    /// `Err(input)` for an unknown value (Spec §11.2 →
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

/// Input data for the properties producer, per sample.
#[derive(Debug, Clone)]
pub struct SampleHeader {
    /// RTPS writer GUID (16 bytes).
    pub writer_guid: [u8; 16],
    /// RTPS sequence number.
    pub seqnum: u64,
    /// DDS `Time_t.sec * 1000 + nanosec/1_000_000` (ms precision).
    pub source_timestamp_ms: i64,
    /// Sub-millisecond part from `Time_t.nanosec % 1_000_000`.
    pub source_nsec_remainder: u32,
    /// XCDR2 KeyHash bytes (source for §7.6.1 group-id);
    /// `None` for an unkeyed topic (group-id is omitted).
    pub keyhash: Option<Vec<u8>>,
    /// `InstanceHandle_t` (16 bytes) — goes to the
    /// `dds:instance-handle` app property instead of `message-id`.
    pub instance_handle: [u8; 16],
    /// Optional remaining LIFESPAN remainder in milliseconds;
    /// when `Some`, feeds `absolute-expiry-time`.
    pub lifespan_remaining_ms: Option<i64>,
    /// DDS sample operation (default `Write`).
    pub operation: DdsOperation,
    /// X-Types TypeIdentifier in full 14-byte hex form;
    /// MANDATORY when `descriptor_form = DESC_TRUNCATED` (Spec
    /// §7.2.1.3), otherwise `None`.
    pub type_id_hex: Option<String>,
    /// DDS domain id.
    pub domain_id: u32,
    /// DDS partition QoS (sequence; `vec![]` = default partition).
    pub partitions: Vec<String>,
}

/// Spec §8.2 — produce `message-id` as a 24-byte binary.
///
/// Format: `writer_guid (16B BE) || seqnum (8B BE)`. Unique
/// per sample (avoids the broker dedup trap such as Service Bus
/// `EnableDuplicateDetection`, cf. §8.2 Rationale).
#[must_use]
pub fn message_id(writer_guid: [u8; 16], seqnum: u64) -> Vec<u8> {
    let mut out = Vec::with_capacity(24);
    out.extend_from_slice(&writer_guid);
    out.extend_from_slice(&seqnum.to_be_bytes());
    out
}

/// Hex-encode 16 or 24 bytes for the JSON-mode surface (§8.1.2).
#[must_use]
pub fn hex_lower(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = core::fmt::Write::write_fmt(&mut out, core::format_args!("{b:02x}"));
    }
    out
}

/// Spec §8.2 — produced properties-section fields.
///
/// We return a sub-structure with the fields populated by this
/// spec; the caller (endpoint daemon) builds the
/// `MessageSection::Properties` AMQP list composite from it.
#[derive(Debug, Clone)]
pub struct ProducedProperties {
    /// `message-id` as binary(24).
    pub message_id: Vec<u8>,
    /// `creation-time` (8-byte BE timestamp, ms since epoch).
    pub creation_time_ms: i64,
    /// `absolute-expiry-time` (timestamp ms) — `None` when LIFESPAN
    /// is not configured.
    pub absolute_expiry_time_ms: Option<i64>,
    /// `group-id` (SHA-256 hex digest from KeyHash) — `None` for an
    /// unkeyed topic.
    pub group_id: Option<String>,
}

/// Spec §8.2 — build the properties section from a sample header.
///
/// Returns the normatively populated fields. `to`, `subject`,
/// `reply-to`, `correlation-id`, `content-type`,
/// `content-encoding`, `group-sequence`, `reply-to-group-id`,
/// `user-id` are optional and up to the caller (subject, e.g., an
/// application-specific routing key).
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

/// Spec §8.3 — standard `dds:*` app-property keys.
pub mod app_keys {
    /// `dds:nsec` — sub-millisecond part of `Time_t`.
    pub const NSEC: &str = "dds:nsec";
    /// `dds:partition` — DDS partition QoS (list-of-string or string).
    pub const PARTITION: &str = "dds:partition";
    /// `dds:domain-id` — DDS domain id (integer).
    pub const DOMAIN_ID: &str = "dds:domain-id";
    /// `dds:type-id` — XTypes TypeIdentifier hex (MANDATORY for TRUNCATED).
    pub const TYPE_ID: &str = "dds:type-id";
    /// `dds:source-guid` — originating-endpoint GUID hex.
    pub const SOURCE_GUID: &str = "dds:source-guid";
    /// `dds:lifespan-ms` — remaining LIFESPAN in milliseconds.
    pub const LIFESPAN_MS: &str = "dds:lifespan-ms";
    /// `dds:sample-state` — read / not-read.
    pub const SAMPLE_STATE: &str = "dds:sample-state";
    /// `dds:view-state` — new / not-new.
    pub const VIEW_STATE: &str = "dds:view-state";
    /// `dds:instance-state` — alive / not-alive-disposed / not-alive-no-writers.
    pub const INSTANCE_STATE: &str = "dds:instance-state";
    /// `dds:operation` — write / register / unregister / dispose.
    pub const OPERATION: &str = "dds:operation";
    /// `dds:bridge-id` — list of traversed bridge UUIDs (loop prevention).
    pub const BRIDGE_ID: &str = "dds:bridge-id";
    /// `dds:bridge-hop` — hop counter (loop prevention).
    pub const BRIDGE_HOP: &str = "dds:bridge-hop";
    /// `dds:instance-handle` — DDS InstanceHandle_t as binary(16).
    pub const INSTANCE_HANDLE: &str = "dds:instance-handle";
}

// ============================================================
// §7.2.1.3 — Receiver-Side Type-ID Collision Inspector
// ============================================================

/// Spec §7.2.1.3 — result of a type-id inspection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeIdCheck {
    /// `dds:type-id` matched the locally expected TypeIdentifier
    /// hex form. Decode may proceed.
    Match,
    /// `dds:type-id` is absent (the sender used
    /// `descriptor_form = DESC_FULL`). Decode may proceed.
    Absent,
    /// `dds:type-id` did NOT match the expected form — a hash-
    /// truncation collision was detected. The receiver must reject
    /// the transfer with `amqp:decode-error` (Spec §7.2.1.3).
    Mismatch {
        /// `dds:type-id` value from the application property.
        received: String,
        /// Locally expected TypeIdentifier hex form.
        expected: String,
    },
}

/// Spec §7.2.1.3 — receiver-side inspector for
/// hash-truncation collisions.
///
/// When the sender uses `descriptor_form = DESC_TRUNCATED`
/// (default), it MUST include the full 14-byte TypeIdentifier hex
/// form as the `dds:type-id` application property. The receiver
/// compares it against the locally known type form. On a
/// mismatch, a hash-truncation collision pair has been detected —
/// the sample MUST be rejected.
///
/// Returns [`TypeIdCheck::Match`] when the property is set and
/// matches; [`TypeIdCheck::Absent`] when not set
/// (DESC_FULL path); [`TypeIdCheck::Mismatch`] when set
/// and not matching.
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

/// Spec §8.3 — build the application-properties map from a sample header.
///
/// Returns an `AmqpExtValue::Map` with the standard keys that are
/// normatively derivable from `SampleHeader`. The caller may add
/// further application-specific keys — per the spec these must
/// not use the `dds:` prefix.
#[must_use]
pub fn produce_application_properties(hdr: &SampleHeader) -> AmqpExtValue {
    let mut map: Vec<(AmqpExtValue, AmqpExtValue)> = Vec::new();

    // dds:nsec — only when the sub-millisecond part is > 0.
    if hdr.source_nsec_remainder != 0 {
        map.push((
            AmqpExtValue::Str(app_keys::NSEC.to_string()),
            AmqpExtValue::Uint(hdr.source_nsec_remainder),
        ));
    }

    // dds:partition — sequence<string> or a single string.
    match hdr.partitions.len() {
        0 => {} // Default partition: omit.
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

    // dds:type-id — mandatory for TRUNCATED (Spec §7.2.1.3); the
    // caller must set `type_id_hex` when descriptor_form =
    // DESC_TRUNCATED.
    if let Some(hex) = &hdr.type_id_hex {
        map.push((
            AmqpExtValue::Str(app_keys::TYPE_ID.to_string()),
            AmqpExtValue::Str(hex.clone()),
        ));
    }

    // dds:lifespan-ms — when a LIFESPAN remainder is available.
    if let Some(rem) = hdr.lifespan_remaining_ms {
        map.push((
            AmqpExtValue::Str(app_keys::LIFESPAN_MS.to_string()),
            AmqpExtValue::Long(rem),
        ));
    }

    // dds:operation — omit the default `write` (Spec §7.7.1).
    if hdr.operation != DdsOperation::Write {
        map.push((
            AmqpExtValue::Str(app_keys::OPERATION.to_string()),
            AmqpExtValue::Str(hdr.operation.as_str().to_string()),
        ));
    }

    // dds:instance-handle — always (16-byte binary).
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
        // Spec rationale: across two samples of the same key the
        // message-id should differ, otherwise Service Bus dedup
        // fires incorrectly (Round-12-P0.3).
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
        // Default header: only dds:domain-id and dds:instance-handle.
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
        // The default write is omitted.
        assert!(!keys.contains(&"dds:operation"));
        // Sub-milliseconds = 0 is omitted.
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
        // Spec §7.2.1.3: descriptor matched (8-byte ulong), but
        // dds:type-id differs → hash-truncation collision.
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
