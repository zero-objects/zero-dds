// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Codegen helpers for per-language PSM templates.
//!
//! Central place where spec rules for the codegen are enforced — the
//! idl-cpp/idl-java/idl-ts/idl-csharp templates emit calls here
//! instead of implementing the rules individually. This makes spec
//! conformance testable in one place rather than scattered across
//! 4 codegen backends.
//!
//! Spec sources:
//! * §7.1.2 — long double (16 byte) → AMQP double (8 byte) narrowing.
//! * §7.1.4.1 — IDL char ASCII-subset validation.
//! * §7.1.6.1 — 16-byte identifier without RFC-4122 conformance:
//!   `binary` (0xA0/0xB0), not `uuid` (0x98).
//! * §7.2.1.1/.2 — composite descriptor: TRUNCATED→ulong(8B),
//!   FULL→symbol(`dds:type:<hex>`).
//! * §7.2.3 — union as AMQP list with {discriminator, value}.

use alloc::string::String;
use alloc::vec::Vec;

use zerodds_amqp_bridge::extended_types::AmqpExtValue;

use crate::annex_a::DescriptorForm;

// ============================================================
// §7.1.2 — long double narrowing
// ============================================================

/// Spec §7.1.2 — IDL `long double` (16 byte IEEE 754-2008
/// binary128) is narrowed to AMQP `double` (8 byte binary64).
///
/// Rust has no native binary128 type; codegen templates therefore
/// already supply an `f64`, and this function documents the
/// narrowing point. For values outside the binary64 range we
/// return Inf — the spec leaves the exact handling to the
/// implementer.
#[must_use]
pub fn narrow_long_double_to_double(value_binary128_as_f64: f64) -> f64 {
    // Identity on f64 (Rust has no binary128). This point exists
    // as an audit anchor for §7.1.2.
    value_binary128_as_f64
}

// ============================================================
// §7.1.4.1 — char ASCII subset
// ============================================================

/// Spec §7.1.4.1 — IDL `char` is restricted to UTF-8 ASCII.
/// Codepoints > 0x7F → `decode-error`.
///
/// # Errors
/// `Err(byte)` for bytes > 0x7F; the codegen caller then emits
/// `amqp:decode-error`.
pub fn validate_char_ascii(byte: u8) -> Result<u8, u8> {
    if byte <= 0x7F { Ok(byte) } else { Err(byte) }
}

// ============================================================
// §7.1.6.1 — identifier type routing
// ============================================================

/// Spec §7.1.6.1 — 16-byte identifier form choice.
///
/// `is_rfc4122_uuid`: does the 16-byte pattern have a valid
/// RFC-4122 version+variant encoding?
///
/// * `true` → AMQP `uuid` (0x98).
/// * `false` → AMQP `binary` (0xA0/0xB0). Spec: "InstanceHandle_t,
///   GUID_t, X-Types TypeIdentifier are generally not
///   RFC-4122 UUIDs and MUST be encoded as binary."
#[must_use]
pub fn encode_16byte_identifier(bytes: [u8; 16], is_rfc4122_uuid: bool) -> AmqpExtValue {
    if is_rfc4122_uuid {
        AmqpExtValue::Uuid(bytes)
    } else {
        AmqpExtValue::Binary(bytes.to_vec())
    }
}

/// Spec §7.1.6.1 — check RFC-4122 conformance.
///
/// Check: variant bits 8-9 of byte 8 = `10` (RFC-4122 variant)
/// and version nibble in byte 6 high nibble in {1..=5}. Strictly
/// speaking the spec is conservative here — other UUID variants
/// (Microsoft GUID, RFC-9562 versions 6-8) are not recognized as
/// RFC-4122 and take the binary path.
#[must_use]
pub fn is_rfc4122_uuid(bytes: &[u8; 16]) -> bool {
    // Variant check: byte[8] high bits = 0b10xxxxxx.
    let variant_ok = (bytes[8] & 0xC0) == 0x80;
    // Version check: byte[6] high nibble in 1..=5.
    let version = (bytes[6] >> 4) & 0x0F;
    let version_ok = (1..=5).contains(&version);
    variant_ok && version_ok
}

// ============================================================
// §7.2.1 — composite descriptor
// ============================================================

/// Spec §7.2.1.1 — TRUNCATED descriptor: first 8 octets of the
/// equivalence hash as a big-endian unsigned 64-bit integer for
/// AMQP `ulong` (0x80).
///
/// Input is the XTypes equivalence hash (at least 8 bytes; typically
/// 14B); additional bytes are ignored.
///
/// # Errors
/// `Err` when `hash_bytes.len() < 8`.
pub fn compute_truncated_descriptor(hash_bytes: &[u8]) -> Result<u64, &'static str> {
    if hash_bytes.len() < 8 {
        return Err("hash_bytes shorter than 8 octets");
    }
    let mut buf = [0u8; 8];
    buf.copy_from_slice(&hash_bytes[..8]);
    Ok(u64::from_be_bytes(buf))
}

/// Spec §7.2.1.2 — FULL descriptor: `dds:type:<hex>` symbol string
/// from the full 14-byte TypeIdentifier form.
#[must_use]
pub fn make_full_descriptor_symbol(type_identifier_bytes: &[u8]) -> String {
    let mut s = String::with_capacity(9 + type_identifier_bytes.len() * 2);
    s.push_str("dds:type:");
    for b in type_identifier_bytes {
        let _ = core::fmt::Write::write_fmt(&mut s, core::format_args!("{b:02x}"));
    }
    s
}

/// Spec §7.2.1 — high-level descriptor routing by `descriptor_form`.
///
/// Returns the tuple `(numeric, symbolic)`:
/// * For `DESC_TRUNCATED`, `numeric = Some(8B BE ulong)`,
///   `symbolic = None`.
/// * For `DESC_FULL`, `numeric = None`,
///   `symbolic = Some("dds:type:<hex>")`.
///
/// The codegen caller uses whichever one matches its §7.2.1 path.
///
/// # Errors
/// `Err` when TRUNCATED and `hash_bytes.len() < 8`.
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

/// Spec §7.2.3 — DDS-IDL `union` ↔ AMQP `list` with
/// `[discriminator, active-branch-value]`. For an empty active
/// branch (the spec leaves this open) `value` is omitted.
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

/// Spec §7.1.7 — empty sequence/array as `list0` (0x45).
///
/// Codegen helper: returns `AmqpExtValue::List(Vec::new())`, which
/// the wire encoder in `amqp-bridge` encodes as `list0` (cf.
/// `extended_types::AmqpExtValue::encode` with the `LIST0` code).
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
        // Standard v4 UUID: byte[6] = 0x40..=0x4F, byte[8] = 0x80..=0xBF.
        let bytes = [
            0xAA, 0xBB, 0xCC, 0xDD, 0x11, 0x22, 0x4A, 0xBC, 0x82, 0x34, 0x56, 0x78, 0x9A, 0xBC,
            0xDE, 0xF0,
        ];
        assert!(is_rfc4122_uuid(&bytes));
    }

    #[test]
    fn arbitrary_16_bytes_not_recognised_as_uuid() {
        // InstanceHandle / GUID / TypeIdentifier: random bytes,
        // typically not a valid UUID version.
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
        // 14-byte hash; the first 8 bytes as BE u64.
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
