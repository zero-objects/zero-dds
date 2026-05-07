// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Codegen-Helpers fuer per-Language-PSM-Templates.
//!
//! Zentrale Stelle, an der Spec-Regeln fuer den Codegen erzwungen
//! werden — die idl-cpp/idl-java/idl-ts/idl-csharp-Templates
//! emittieren Calls hierhin, statt die Regeln einzeln zu
//! implementieren. Damit ist die Spec-Conformance an einer Stelle
//! testbar, nicht ueber 4 Codegen-Backends verstreut.
//!
//! Spec-Quellen:
//! * §7.1.2 — long double (16 Byte) → AMQP double (8 Byte) Narrowing.
//! * §7.1.4.1 — IDL char ASCII-Subset Validierung.
//! * §7.1.6.1 — 16-Byte-Identifier ohne RFC-4122-Konformitaet:
//!   `binary` (0xA0/0xB0), nicht `uuid` (0x98).
//! * §7.2.1.1/.2 — Composite-Descriptor: TRUNCATED→ulong(8B),
//!   FULL→symbol(`dds:type:<hex>`).
//! * §7.2.3 — Union als AMQP-list mit {discriminator, value}.

use alloc::string::String;
use alloc::vec::Vec;

use zerodds_amqp_bridge::extended_types::AmqpExtValue;

use crate::annex_a::DescriptorForm;

// ============================================================
// §7.1.2 — long double Narrowing
// ============================================================

/// Spec §7.1.2 — IDL `long double` (16 Byte IEEE 754-2008
/// binary128) wird auf AMQP `double` (8 Byte binary64) verengt.
///
/// Rust hat keinen nativen binary128-Typ; Codegen-Templates
/// liefern daher schon einen `f64` und dieses Funktion
/// dokumentiert die Narrowing-Stelle. Bei Werten ausserhalb des
/// binary64-Bereichs liefern wir Inf — Spec laesst die exakte
/// Behandlung dem Implementer.
#[must_use]
pub fn narrow_long_double_to_double(value_binary128_as_f64: f64) -> f64 {
    // Identitaet auf f64 (Rust hat keinen binary128). Die Stelle
    // existiert als Audit-Anker fuer §7.1.2.
    value_binary128_as_f64
}

// ============================================================
// §7.1.4.1 — char ASCII-Subset
// ============================================================

/// Spec §7.1.4.1 — IDL `char` ist auf UTF-8-ASCII beschraenkt.
/// Codepoints > 0x7F → `decode-error`.
///
/// # Errors
/// `Err(byte)` fuer Bytes > 0x7F; Codegen-Caller emittiert daraufhin
/// `amqp:decode-error`.
pub fn validate_char_ascii(byte: u8) -> Result<u8, u8> {
    if byte <= 0x7F { Ok(byte) } else { Err(byte) }
}

// ============================================================
// §7.1.6.1 — Identifier-Type-Routing
// ============================================================

/// Spec §7.1.6.1 — 16-Byte-Identifier-Form-Wahl.
///
/// `is_rfc4122_uuid`: hat das 16-Byte-Pattern eine valide
/// RFC-4122-Version+Variant-Codierung?
///
/// * `true` → AMQP `uuid` (0x98).
/// * `false` → AMQP `binary` (0xA0/0xB0). Spec: "InstanceHandle_t,
///   GUID_t, X-Types TypeIdentifier sind in der Regel keine
///   RFC-4122-UUIDs und MUESSEN als binary codiert werden."
#[must_use]
pub fn encode_16byte_identifier(bytes: [u8; 16], is_rfc4122_uuid: bool) -> AmqpExtValue {
    if is_rfc4122_uuid {
        AmqpExtValue::Uuid(bytes)
    } else {
        AmqpExtValue::Binary(bytes.to_vec())
    }
}

/// Spec §7.1.6.1 — RFC-4122-Konformitaet pruefen.
///
/// Pruefung: Variant-Bits 8-9 von Byte 8 = `10` (RFC-4122-Variant)
/// und Version-Nibble in Byte 6 high-nibble in {1..=5}. Streng
/// genommen ist die Spec hier konservativ — andere UUID-Varianten
/// (Microsoft-GUID, RFC-9562 Versions 6-8) werden hier nicht als
/// RFC-4122 anerkannt und gehen den binary-Pfad.
#[must_use]
pub fn is_rfc4122_uuid(bytes: &[u8; 16]) -> bool {
    // Variant-Check: byte[8] high-bits = 0b10xxxxxx.
    let variant_ok = (bytes[8] & 0xC0) == 0x80;
    // Version-Check: byte[6] high-nibble in 1..=5.
    let version = (bytes[6] >> 4) & 0x0F;
    let version_ok = (1..=5).contains(&version);
    variant_ok && version_ok
}

// ============================================================
// §7.2.1 — Composite-Descriptor
// ============================================================

/// Spec §7.2.1.1 — TRUNCATED-Descriptor: erste 8 Octets der
/// equivalence-hash als big-endian unsigned 64-bit-Integer fuer
/// AMQP `ulong` (0x80).
///
/// Eingabe ist die XTypes-equivalence-hash (mind. 8 Byte; typisch
/// 14B); zusaetzliche Bytes werden ignoriert.
///
/// # Errors
/// `Err` wenn `hash_bytes.len() < 8`.
pub fn compute_truncated_descriptor(hash_bytes: &[u8]) -> Result<u64, &'static str> {
    if hash_bytes.len() < 8 {
        return Err("hash_bytes shorter than 8 octets");
    }
    let mut buf = [0u8; 8];
    buf.copy_from_slice(&hash_bytes[..8]);
    Ok(u64::from_be_bytes(buf))
}

/// Spec §7.2.1.2 — FULL-Descriptor: `dds:type:<hex>` symbol-string
/// aus der vollen 14-Byte-TypeIdentifier-Form.
#[must_use]
pub fn make_full_descriptor_symbol(type_identifier_bytes: &[u8]) -> String {
    let mut s = String::with_capacity(9 + type_identifier_bytes.len() * 2);
    s.push_str("dds:type:");
    for b in type_identifier_bytes {
        let _ = core::fmt::Write::write_fmt(&mut s, core::format_args!("{b:02x}"));
    }
    s
}

/// Spec §7.2.1 — High-Level Descriptor-Routing per `descriptor_form`.
///
/// Liefert das Tupel `(numeric, symbolic)`:
/// * Bei `DESC_TRUNCATED` ist `numeric = Some(8B BE ulong)`,
///   `symbolic = None`.
/// * Bei `DESC_FULL` ist `numeric = None`,
///   `symbolic = Some("dds:type:<hex>")`.
///
/// Codegen-Caller benutzt das, das zu seinem Spec-§7.2.1-Pfad
/// gehoert.
///
/// # Errors
/// `Err` wenn TRUNCATED und `hash_bytes.len() < 8`.
pub fn route_descriptor(
    form: DescriptorForm,
    hash_bytes: &[u8],
) -> Result<(Option<u64>, Option<String>), &'static str> {
    match form {
        DescriptorForm::DescTruncated => {
            let n = compute_truncated_descriptor(hash_bytes)?;
            Ok((Some(n), None))
        }
        DescriptorForm::DescFull => Ok((None, Some(make_full_descriptor_symbol(hash_bytes)))),
    }
}

// ============================================================
// §7.2.3 — Union
// ============================================================

/// Spec §7.2.3 — DDS-IDL `union` ↔ AMQP `list` mit
/// `[discriminator, active-branch-value]`. Bei leerem aktivem
/// Branch (Spec laesst das offen) wird `value` ausgelassen.
#[must_use]
pub fn make_union_body(
    discriminator: AmqpExtValue,
    active_value: Option<AmqpExtValue>,
) -> AmqpExtValue {
    let mut items: Vec<AmqpExtValue> = Vec::with_capacity(2);
    items.push(discriminator);
    if let Some(v) = active_value {
        items.push(v);
    }
    AmqpExtValue::List(items)
}

// ============================================================
// §7.1.7 — Empty Sequence/Array (list0 helper)
// ============================================================

/// Spec §7.1.7 — leere Sequenz/Array als `list0` (0x45).
///
/// Codegen-Helper: liefert `AmqpExtValue::List(Vec::new())`, das
/// vom Wire-Encoder im `amqp-bridge` als `list0` codiert wird (vgl.
/// `extended_types::AmqpExtValue::encode` mit `LIST0`-Code).
#[must_use]
pub fn empty_sequence() -> AmqpExtValue {
    AmqpExtValue::List(Vec::new())
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    // --- §7.1.4.1 char ---

    #[test]
    fn ascii_validates() {
        for b in 0x00..=0x7F {
            assert_eq!(validate_char_ascii(b), Ok(b));
        }
    }

    #[test]
    fn non_ascii_rejected() {
        for b in 0x80..=0xFFu8 {
            assert_eq!(validate_char_ascii(b), Err(b));
        }
    }

    // --- §7.1.6.1 Identifier ---

    #[test]
    fn rfc4122_v4_uuid_recognised() {
        // Standard v4-UUID: byte[6] = 0x40..=0x4F, byte[8] = 0x80..=0xBF.
        let bytes = [
            0xAA, 0xBB, 0xCC, 0xDD, 0x11, 0x22, 0x4A, 0xBC, 0x82, 0x34, 0x56, 0x78, 0x9A, 0xBC,
            0xDE, 0xF0,
        ];
        assert!(is_rfc4122_uuid(&bytes));
    }

    #[test]
    fn arbitrary_16_bytes_not_recognised_as_uuid() {
        // InstanceHandle / GUID / TypeIdentifier: zufaellige Bytes,
        // typisch keine valide UUID-Version.
        let bytes = [0x00; 16];
        assert!(!is_rfc4122_uuid(&bytes));
    }

    #[test]
    fn encode_16byte_routes_binary_for_non_uuid() {
        let bytes = [0xDE, 0xAD, 0xBE, 0xEF, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let v = encode_16byte_identifier(bytes, false);
        match v {
            AmqpExtValue::Binary(b) => assert_eq!(b.len(), 16),
            _ => panic!("expected binary"),
        }
    }

    #[test]
    fn encode_16byte_routes_uuid_when_marked() {
        let bytes = [0u8; 16];
        let v = encode_16byte_identifier(bytes, true);
        assert!(matches!(v, AmqpExtValue::Uuid(_)));
    }

    // --- §7.2.1 Descriptor ---

    #[test]
    fn truncated_descriptor_first_8_bytes_be() {
        // 14-Byte-Hash; die ersten 8 Bytes als BE-u64.
        let hash = [
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E,
        ];
        let n = compute_truncated_descriptor(&hash).unwrap();
        assert_eq!(n, 0x0102_0304_0506_0708);
    }

    #[test]
    fn truncated_descriptor_too_short_errors() {
        let hash = [0u8; 7];
        assert!(compute_truncated_descriptor(&hash).is_err());
    }

    #[test]
    fn full_descriptor_symbol_format() {
        let bytes = [0xDE, 0xAD, 0xBE, 0xEF];
        let s = make_full_descriptor_symbol(&bytes);
        assert_eq!(s, "dds:type:deadbeef");
    }

    #[test]
    fn route_descriptor_truncated() {
        let hash = [
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E,
        ];
        let (num, sym) = route_descriptor(DescriptorForm::DescTruncated, &hash).unwrap();
        assert!(num.is_some());
        assert!(sym.is_none());
    }

    #[test]
    fn route_descriptor_full() {
        let hash = [
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E,
        ];
        let (num, sym) = route_descriptor(DescriptorForm::DescFull, &hash).unwrap();
        assert!(num.is_none());
        assert!(sym.is_some());
        assert!(sym.unwrap().starts_with("dds:type:"));
    }

    // --- §7.2.3 Union ---

    #[test]
    fn union_with_branch_has_two_elements() {
        let u = make_union_body(
            AmqpExtValue::Int(1),
            Some(AmqpExtValue::Str("hello".into())),
        );
        match u {
            AmqpExtValue::List(items) => {
                assert_eq!(items.len(), 2);
                assert_eq!(items[0], AmqpExtValue::Int(1));
            }
            _ => panic!("expected list"),
        }
    }

    #[test]
    fn union_empty_branch_omits_value() {
        let u = make_union_body(AmqpExtValue::Int(99), None);
        match u {
            AmqpExtValue::List(items) => {
                assert_eq!(items.len(), 1);
                assert_eq!(items[0], AmqpExtValue::Int(99));
            }
            _ => panic!(),
        }
    }

    // --- §7.1.7 Empty Sequence ---

    #[test]
    fn empty_sequence_yields_empty_list() {
        match empty_sequence() {
            AmqpExtValue::List(items) => assert!(items.is_empty()),
            _ => panic!(),
        }
    }

    // --- §7.1.2 Long Double ---

    #[test]
    fn long_double_narrowing_is_identity_on_f64() {
        let v = 1.234_567_890_123_456_7;
        assert_eq!(narrow_long_double_to_double(v), v);
    }
}
