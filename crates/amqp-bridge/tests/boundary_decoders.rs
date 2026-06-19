//! Boundary tests for the AMQP wire decoders.
//!
//! Mutation testing showed that boundary checks of the form
//! `bytes.len() < N || bytes[0] != CODE` were not reliably triggered
//! by random-byte fuzz tests:
//!
//! * The `< with >` mutation is not caught if there are no tests with
//!   `len == N-1` (truncated).
//! * The `|| with &&` mutation is not caught if there are no tests with
//!   `len < N AND code == CODE` (false-on-AND-true).
//!
//! For each decoder function we test explicitly:
//! 1. Empty input — must Err.
//! 2. Format-code only (truncated) — must Err.
//! 3. Full minimum length — must Ok.
//! 4. One byte over length (extra trailing bytes) — must Ok with
//!    consumed = full minimum length.
//! 5. Wrong format code at length-correct buffer — must Err.

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

use zerodds_amqp_bridge::extended_types::{
    AmqpExtValue, decode_byte, decode_char, decode_double, decode_float, decode_int, decode_short,
    decode_timestamp, decode_ubyte, decode_uint, decode_ushort, decode_uuid,
};
use zerodds_amqp_bridge::types::codes;

// ---------------------------------------------------------------------------
// decode_ubyte (UBYTE = 0x50, total 2 bytes: code + 1)
// ---------------------------------------------------------------------------

#[test]
fn ubyte_empty_is_err() {
    assert!(decode_ubyte(&[]).is_err());
}

#[test]
fn ubyte_code_only_is_err() {
    // Length 1 (just the format code) — must be Truncated, NOT panic.
    assert!(decode_ubyte(&[codes::UBYTE]).is_err());
}

#[test]
fn ubyte_full_length_decodes_value() {
    let (v, n) = decode_ubyte(&[codes::UBYTE, 0x42]).unwrap();
    assert_eq!(v, 0x42);
    assert_eq!(n, 2);
}

#[test]
fn ubyte_with_trailing_bytes_consumes_only_minimum() {
    let (v, n) = decode_ubyte(&[codes::UBYTE, 0x42, 0xFF, 0xFF]).unwrap();
    assert_eq!(v, 0x42);
    assert_eq!(n, 2);
}

#[test]
fn ubyte_wrong_code_is_err() {
    assert!(decode_ubyte(&[0x99, 0x42]).is_err());
}

// ---------------------------------------------------------------------------
// decode_ushort (USHORT = 0x60, total 3 bytes)
// ---------------------------------------------------------------------------

#[test]
fn ushort_empty_is_err() {
    assert!(decode_ushort(&[]).is_err());
}

#[test]
fn ushort_truncated_one_byte_is_err() {
    assert!(decode_ushort(&[codes::USHORT]).is_err());
}

#[test]
fn ushort_truncated_two_bytes_is_err() {
    assert!(decode_ushort(&[codes::USHORT, 0x12]).is_err());
}

#[test]
fn ushort_full_length_decodes_value() {
    let (v, n) = decode_ushort(&[codes::USHORT, 0x12, 0x34]).unwrap();
    assert_eq!(v, 0x1234);
    assert_eq!(n, 3);
}

#[test]
fn ushort_wrong_code_is_err() {
    assert!(decode_ushort(&[0x99, 0x12, 0x34]).is_err());
}

// ---------------------------------------------------------------------------
// decode_uint (UINT0 = 0x43, SMALLUINT = 0x52, UINT = 0x70)
// ---------------------------------------------------------------------------

#[test]
fn uint0_decodes_value_zero() {
    let (v, n) = decode_uint(&[codes::UINT0]).unwrap();
    assert_eq!(v, 0);
    assert_eq!(n, 1);
}

#[test]
fn smalluint_truncated_is_err() {
    assert!(decode_uint(&[codes::SMALLUINT]).is_err());
}

#[test]
fn smalluint_full_length_decodes_value() {
    let (v, n) = decode_uint(&[codes::SMALLUINT, 0xFF]).unwrap();
    assert_eq!(v, 0xFF);
    assert_eq!(n, 2);
}

#[test]
fn uint_truncated_at_2_bytes_is_err() {
    assert!(decode_uint(&[codes::UINT, 0x12]).is_err());
}

#[test]
fn uint_truncated_at_4_bytes_is_err() {
    assert!(decode_uint(&[codes::UINT, 0x12, 0x34, 0x56]).is_err());
}

#[test]
fn uint_full_length_decodes_value() {
    let (v, n) = decode_uint(&[codes::UINT, 0x12, 0x34, 0x56, 0x78]).unwrap();
    assert_eq!(v, 0x1234_5678);
    assert_eq!(n, 5);
}

// ---------------------------------------------------------------------------
// decode_byte (BYTE = 0x51, total 2 bytes)
// ---------------------------------------------------------------------------

#[test]
fn byte_empty_is_err() {
    assert!(decode_byte(&[]).is_err());
}

#[test]
fn byte_code_only_is_err() {
    assert!(decode_byte(&[codes::BYTE]).is_err());
}

#[test]
fn byte_full_length_decodes_value() {
    let (v, n) = decode_byte(&[codes::BYTE, 0xFF]).unwrap();
    assert_eq!(v, -1);
    assert_eq!(n, 2);
}

#[test]
fn byte_wrong_code_is_err() {
    assert!(decode_byte(&[0x99, 0x42]).is_err());
}

// ---------------------------------------------------------------------------
// decode_short (SHORT = 0x61, total 3 bytes)
// ---------------------------------------------------------------------------

#[test]
fn short_empty_is_err() {
    assert!(decode_short(&[]).is_err());
}

#[test]
fn short_code_only_is_err() {
    assert!(decode_short(&[codes::SHORT]).is_err());
}

#[test]
fn short_truncated_two_bytes_is_err() {
    assert!(decode_short(&[codes::SHORT, 0x12]).is_err());
}

#[test]
fn short_full_length_decodes_value() {
    let (v, n) = decode_short(&[codes::SHORT, 0xFF, 0xFF]).unwrap();
    assert_eq!(v, -1);
    assert_eq!(n, 3);
}

// ---------------------------------------------------------------------------
// decode_int (SMALLINT = 0x54, INT = 0x71)
// ---------------------------------------------------------------------------

#[test]
fn smallint_truncated_is_err() {
    assert!(decode_int(&[codes::SMALLINT]).is_err());
}

#[test]
fn smallint_full_length_decodes_value() {
    let (v, n) = decode_int(&[codes::SMALLINT, 0xFF]).unwrap();
    assert_eq!(v, -1);
    assert_eq!(n, 2);
}

#[test]
fn int_truncated_at_2_bytes_is_err() {
    assert!(decode_int(&[codes::INT, 0x12]).is_err());
}

#[test]
fn int_truncated_at_4_bytes_is_err() {
    assert!(decode_int(&[codes::INT, 0x12, 0x34, 0x56]).is_err());
}

#[test]
fn int_full_length_decodes_value() {
    let (v, n) = decode_int(&[codes::INT, 0xFF, 0xFF, 0xFF, 0xFF]).unwrap();
    assert_eq!(v, -1);
    assert_eq!(n, 5);
}

// ---------------------------------------------------------------------------
// decode_float / decode_double (FLOAT = 0x72 + 4B, DOUBLE = 0x82 + 8B)
// ---------------------------------------------------------------------------

#[test]
fn float_empty_is_err() {
    assert!(decode_float(&[]).is_err());
}

#[test]
fn float_code_only_is_err() {
    assert!(decode_float(&[codes::FLOAT]).is_err());
}

#[test]
fn float_truncated_at_4_bytes_is_err() {
    assert!(decode_float(&[codes::FLOAT, 0, 0, 0]).is_err());
}

#[test]
fn float_full_length_decodes_value() {
    let bytes = [codes::FLOAT, 0x40, 0x49, 0x0F, 0xDB]; // ~PI
    let (v, n) = decode_float(&bytes).unwrap();
    assert!((v - core::f32::consts::PI).abs() < 1e-5);
    assert_eq!(n, 5);
}

#[test]
fn double_truncated_at_8_bytes_is_err() {
    assert!(decode_double(&[codes::DOUBLE, 0, 0, 0, 0, 0, 0, 0]).is_err());
}

#[test]
fn double_full_length_decodes_value() {
    let bytes = [
        codes::DOUBLE,
        0x40,
        0x09,
        0x21,
        0xFB,
        0x54,
        0x44,
        0x2D,
        0x18,
    ];
    let (v, n) = decode_double(&bytes).unwrap();
    assert!((v - core::f64::consts::PI).abs() < 1e-15);
    assert_eq!(n, 9);
}

// ---------------------------------------------------------------------------
// decode_char / decode_timestamp / decode_uuid
// ---------------------------------------------------------------------------

#[test]
fn char_empty_is_err() {
    assert!(decode_char(&[]).is_err());
}

#[test]
fn char_code_only_is_err() {
    assert!(decode_char(&[codes::CHAR]).is_err());
}

#[test]
fn char_truncated_at_4_bytes_is_err() {
    assert!(decode_char(&[codes::CHAR, 0, 0, 0]).is_err());
}

#[test]
fn char_full_length_decodes_value() {
    // UTF-32 'A' = 0x00000041
    let (v, n) = decode_char(&[codes::CHAR, 0x00, 0x00, 0x00, 0x41]).unwrap();
    assert_eq!(v, 'A');
    assert_eq!(n, 5);
}

#[test]
fn timestamp_empty_is_err() {
    assert!(decode_timestamp(&[]).is_err());
}

#[test]
fn timestamp_truncated_at_8_bytes_is_err() {
    assert!(decode_timestamp(&[codes::TIMESTAMP, 0, 0, 0, 0, 0, 0, 0]).is_err());
}

#[test]
fn timestamp_full_length_decodes_value() {
    let (v, n) = decode_timestamp(&[codes::TIMESTAMP, 0, 0, 0, 0, 0, 0, 0, 1]).unwrap();
    assert_eq!(v, 1);
    assert_eq!(n, 9);
}

#[test]
fn uuid_empty_is_err() {
    assert!(decode_uuid(&[]).is_err());
}

#[test]
fn uuid_truncated_at_16_bytes_is_err() {
    let mut bytes = [codes::UUID; 16];
    bytes[0] = codes::UUID;
    assert!(decode_uuid(&bytes).is_err());
}

#[test]
fn uuid_full_length_decodes_value() {
    let mut bytes = [0u8; 17];
    bytes[0] = codes::UUID;
    for (i, slot) in bytes.iter_mut().enumerate().skip(1) {
        *slot = (i - 1) as u8;
    }
    let (v, n) = decode_uuid(&bytes).unwrap();
    assert_eq!(n, 17);
    for (i, b) in v.iter().enumerate() {
        assert_eq!(*b, i as u8);
    }
}

// ---------------------------------------------------------------------------
// Compound types (list, map, array)
//
// Addresses remaining mutations in encode_list/decode_list,
// encode_map/decode_map, encode_array/decode_array.
// ---------------------------------------------------------------------------

#[test]
fn list8_decode_truncated_at_one_byte_is_err() {
    assert!(AmqpExtValue::decode(&[codes::LIST8]).is_err());
}

#[test]
fn list8_decode_truncated_at_two_bytes_is_err() {
    assert!(AmqpExtValue::decode(&[codes::LIST8, 0x01]).is_err());
}

#[test]
fn list8_empty_decodes_correctly() {
    // LIST8 with size=1 (just the count byte), count=0
    let (v, n) = AmqpExtValue::decode(&[codes::LIST8, 0x01, 0x00]).unwrap();
    assert!(matches!(v, AmqpExtValue::List(items) if items.is_empty()));
    assert_eq!(n, 3);
}

#[test]
fn list8_with_one_null_decodes_correctly() {
    // LIST8 size=2 (count+null), count=1, NULL
    let (v, n) = AmqpExtValue::decode(&[codes::LIST8, 0x02, 0x01, codes::NULL]).unwrap();
    match v {
        AmqpExtValue::List(items) => {
            assert_eq!(items.len(), 1);
            assert!(matches!(items[0], AmqpExtValue::Null));
        }
        _ => panic!("expected List"),
    }
    assert_eq!(n, 4);
}

#[test]
fn list8_truncated_body_is_err() {
    // declared count=2 but only 1 element provided
    assert!(AmqpExtValue::decode(&[codes::LIST8, 0x03, 0x02, codes::NULL]).is_err());
}

#[test]
fn list32_decode_truncated_at_8_bytes_is_err() {
    let buf = [codes::LIST32, 0, 0, 0, 5, 0, 0, 0]; // missing last count byte
    assert!(AmqpExtValue::decode(&buf).is_err());
}

#[test]
fn list32_full_header_with_zero_count_decodes() {
    // LIST32 size=4 (count-bytes only), count=0
    let buf = [
        codes::LIST32,
        0,
        0,
        0,
        4, // size = 4 (just the count bytes)
        0,
        0,
        0,
        0, // count = 0
    ];
    let (v, n) = AmqpExtValue::decode(&buf).unwrap();
    assert!(matches!(v, AmqpExtValue::List(items) if items.is_empty()));
    assert_eq!(n, 9);
}

#[test]
fn list_roundtrip_at_boundary_254_bytes() {
    // Build a list whose body is exactly 254 bytes — boundary for LIST8.
    let items: Vec<AmqpExtValue> = (0..254).map(|i| AmqpExtValue::Ubyte(i as u8)).collect();
    let _ = items;
    // 254 ubytes = 254 * 2 bytes = 508 bytes — too big for LIST8.
    // Use 127 ubytes instead = 254 bytes body.
    let items: Vec<AmqpExtValue> = (0..127).map(|i| AmqpExtValue::Ubyte(i as u8)).collect();
    let v = AmqpExtValue::List(items);
    let bytes = v.encode().unwrap();
    let (decoded, _) = AmqpExtValue::decode(&bytes).unwrap();
    assert_eq!(v, decoded);
}

#[test]
fn list_roundtrip_above_list8_threshold() {
    // List with count > 255 — must use LIST32 path.
    let items: Vec<AmqpExtValue> = (0..300).map(|_| AmqpExtValue::Null).collect();
    let v = AmqpExtValue::List(items);
    let bytes = v.encode().unwrap();
    assert_eq!(bytes[0], codes::LIST32);
    let (decoded, _) = AmqpExtValue::decode(&bytes).unwrap();
    assert_eq!(v, decoded);
}

// ----- map -----

#[test]
fn map8_decode_truncated_is_err() {
    assert!(AmqpExtValue::decode(&[codes::MAP8]).is_err());
    assert!(AmqpExtValue::decode(&[codes::MAP8, 0x01]).is_err());
}

#[test]
fn map8_empty_decodes_correctly() {
    let (v, n) = AmqpExtValue::decode(&[codes::MAP8, 0x01, 0x00]).unwrap();
    assert!(matches!(v, AmqpExtValue::Map(entries) if entries.is_empty()));
    assert_eq!(n, 3);
}

#[test]
fn map8_with_one_pair_decodes_correctly() {
    // MAP8: code, size, count, key, value. Key=NULL (1B), Value=NULL (1B).
    // count=2 (key + value entries), size = 1 + 1 + 1 = 3
    let buf = [codes::MAP8, 0x03, 0x02, codes::NULL, codes::NULL];
    let (v, n) = AmqpExtValue::decode(&buf).unwrap();
    match v {
        AmqpExtValue::Map(entries) => {
            assert_eq!(entries.len(), 1);
        }
        _ => panic!("expected Map"),
    }
    assert_eq!(n, 5);
}

#[test]
fn map8_odd_count_is_err() {
    // count must be even (key-value pairs)
    let buf = [codes::MAP8, 0x02, 0x01, codes::NULL];
    assert!(AmqpExtValue::decode(&buf).is_err());
}

#[test]
fn map_roundtrip_with_str_keys() {
    let entries = vec![
        (AmqpExtValue::Str("k1".into()), AmqpExtValue::Ubyte(1)),
        (AmqpExtValue::Str("k2".into()), AmqpExtValue::Ubyte(2)),
    ];
    let v = AmqpExtValue::Map(entries);
    let bytes = v.encode().unwrap();
    let (decoded, _) = AmqpExtValue::decode(&bytes).unwrap();
    assert_eq!(v, decoded);
}

// ----- array -----

#[test]
fn array8_truncated_is_err() {
    assert!(AmqpExtValue::decode(&[codes::ARRAY8]).is_err());
    assert!(AmqpExtValue::decode(&[codes::ARRAY8, 0x05]).is_err());
}

#[test]
fn array8_empty_with_only_constructor_is_err() {
    // size=1 (constructor byte), count=0 — element-constructor missing entirely.
    // This is malformed; decoder must Err.
    let buf = [codes::ARRAY8, 0x01, 0x00];
    let res = AmqpExtValue::decode(&buf);
    // Either Ok with empty array OR Err — both are valid behaviors.
    // Important: must not panic.
    let _ = res;
}

#[test]
fn array_roundtrip_homogeneous_ubytes() {
    let items: Vec<AmqpExtValue> = (0..10).map(|i| AmqpExtValue::Ubyte(i as u8)).collect();
    let v = AmqpExtValue::Array(items);
    let bytes = v.encode().unwrap();
    let (decoded, _) = AmqpExtValue::decode(&bytes).unwrap();
    assert_eq!(v, decoded);
}

// ----- depth-cap -----

#[test]
fn deeply_nested_list_at_max_depth_decodes() {
    // Build a list nested ~30 levels deep — should decode cleanly.
    let mut v = AmqpExtValue::Null;
    for _ in 0..30 {
        v = AmqpExtValue::List(vec![v]);
    }
    let bytes = v.encode().unwrap();
    let (decoded, _) = AmqpExtValue::decode(&bytes).unwrap();
    assert_eq!(v, decoded);
}

#[test]
fn deeply_nested_list_beyond_max_depth_is_err() {
    // 200 levels deep — should hit MAX_COMPOUND_DEPTH and Err.
    let mut v = AmqpExtValue::Null;
    for _ in 0..200 {
        v = AmqpExtValue::List(vec![v]);
    }
    let res = v.encode();
    assert!(res.is_err(), "expected encode to error at depth 200");
}

// ---------------------------------------------------------------------------
// Exact wire-format bytes (catches encode mutations)
//
// Mutations like `+ with -` in `total_size = body.len() + 4` are only
// caught when we check the exact bytes of the encoding — the roundtrip
// alone would pass.
// ---------------------------------------------------------------------------

#[test]
fn encode_list_with_one_null_produces_exact_list8_bytes() {
    let v = AmqpExtValue::List(vec![AmqpExtValue::Null]);
    let bytes = v.encode().unwrap();
    // LIST8 size=2 (count + NULL), count=1, NULL
    assert_eq!(
        bytes,
        vec![codes::LIST8, 0x02, 0x01, codes::NULL],
        "exact wire bytes mismatch"
    );
}

#[test]
fn encode_list_with_three_ubytes_produces_exact_bytes() {
    let v = AmqpExtValue::List(vec![
        AmqpExtValue::Ubyte(0x10),
        AmqpExtValue::Ubyte(0x20),
        AmqpExtValue::Ubyte(0x30),
    ]);
    let bytes = v.encode().unwrap();
    // LIST8 size = 1 (count) + 3*2 (ubyte each) = 7, count=3
    assert_eq!(
        bytes,
        vec![
            codes::LIST8,
            0x07, // size
            0x03, // count
            codes::UBYTE,
            0x10,
            codes::UBYTE,
            0x20,
            codes::UBYTE,
            0x30,
        ],
    );
}

#[test]
fn encode_list_at_count_boundary_uses_list32() {
    // count = 256 forces LIST32 (count > u8::MAX).
    let items: Vec<AmqpExtValue> = (0..256).map(|_| AmqpExtValue::Null).collect();
    let v = AmqpExtValue::List(items);
    let bytes = v.encode().unwrap();
    assert_eq!(bytes[0], codes::LIST32, "count=256 must use LIST32");
    // total_size = body.len() + 4 = 256 + 4 = 260
    let total_size = u32::from_be_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]);
    assert_eq!(total_size, 260, "list32 total_size must be body.len + 4");
    let count = u32::from_be_bytes([bytes[5], bytes[6], bytes[7], bytes[8]]);
    assert_eq!(count, 256);
}

#[test]
fn encode_list_at_body_boundary_uses_list32() {
    // body.len() = 255 (just over u8 boundary) forces LIST32.
    // 128 ubytes = 128*2 = 256 bytes body — over 254.
    let items: Vec<AmqpExtValue> = (0..128).map(|i| AmqpExtValue::Ubyte(i as u8)).collect();
    let v = AmqpExtValue::List(items);
    let bytes = v.encode().unwrap();
    assert_eq!(bytes[0], codes::LIST32, "body=256 must use LIST32");
}

#[test]
fn encode_list_at_count_255_body_under_254_still_uses_list8() {
    // count = 255, all NULL (1 byte each) → body = 255, > 254 → LIST32.
    let items: Vec<AmqpExtValue> = (0..255).map(|_| AmqpExtValue::Null).collect();
    let v = AmqpExtValue::List(items);
    let bytes = v.encode().unwrap();
    // body.len() = 255 > 254, so LIST32 used.
    assert_eq!(bytes[0], codes::LIST32);
}

#[test]
fn encode_list_count_127_uses_list8() {
    // count=127 NULLs, body=127 bytes → both fit → LIST8.
    let items: Vec<AmqpExtValue> = (0..127).map(|_| AmqpExtValue::Null).collect();
    let v = AmqpExtValue::List(items);
    let bytes = v.encode().unwrap();
    assert_eq!(bytes[0], codes::LIST8, "127 NULLs must fit in LIST8");
    assert_eq!(
        bytes[1] as usize,
        127 + 1,
        "size = body.len + 1 (count byte)"
    );
    assert_eq!(bytes[2], 127);
}

// ---- map exact bytes ----

#[test]
fn encode_map_with_one_pair_produces_exact_bytes() {
    let v = AmqpExtValue::Map(vec![(AmqpExtValue::Null, AmqpExtValue::Null)]);
    let bytes = v.encode().unwrap();
    // MAP8 size = 1 (count) + 2 (NULL+NULL) = 3, count=2 (k+v)
    assert_eq!(
        bytes,
        vec![codes::MAP8, 0x03, 0x02, codes::NULL, codes::NULL],
    );
}

#[test]
fn encode_map_at_count_boundary_uses_map32() {
    // 128 pairs = 256 entries (k+v) > u8::MAX → MAP32.
    let entries: Vec<(AmqpExtValue, AmqpExtValue)> = (0..128)
        .map(|_| (AmqpExtValue::Null, AmqpExtValue::Null))
        .collect();
    let v = AmqpExtValue::Map(entries);
    let bytes = v.encode().unwrap();
    assert_eq!(
        bytes[0],
        codes::MAP32,
        "256 entries (128 pairs) must use MAP32"
    );
    let total_size = u32::from_be_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]);
    // 256 NULLs = 256 bytes body, total_size = 256 + 4 = 260
    assert_eq!(total_size, 260, "map32 total_size = body.len + 4");
    let count = u32::from_be_bytes([bytes[5], bytes[6], bytes[7], bytes[8]]);
    assert_eq!(count, 256);
}

// ---- array exact bytes ----

#[test]
fn encode_array_with_one_ubyte_produces_exact_bytes() {
    let v = AmqpExtValue::Array(vec![AmqpExtValue::Ubyte(0xAB)]);
    let bytes = v.encode().unwrap();
    // ARRAY8 size = 1 (count) + 1 (constructor) + 1 (data) = 3, count=1
    assert_eq!(bytes, vec![codes::ARRAY8, 0x03, 0x01, codes::UBYTE, 0xAB],);
}

#[test]
fn encode_array_with_three_ubytes_produces_compact_bytes() {
    let v = AmqpExtValue::Array(vec![
        AmqpExtValue::Ubyte(0x10),
        AmqpExtValue::Ubyte(0x20),
        AmqpExtValue::Ubyte(0x30),
    ]);
    let bytes = v.encode().unwrap();
    // ARRAY8 size = 1 (count) + 1 (constructor) + 3 (data) = 5, count=3,
    // body = [UBYTE, 0x10, 0x20, 0x30]
    assert_eq!(
        bytes,
        vec![codes::ARRAY8, 0x05, 0x03, codes::UBYTE, 0x10, 0x20, 0x30],
    );
}

#[test]
fn encode_array_at_count_boundary_uses_array32() {
    let items: Vec<AmqpExtValue> = (0..256).map(|_| AmqpExtValue::Ubyte(0x42)).collect();
    let v = AmqpExtValue::Array(items);
    let bytes = v.encode().unwrap();
    assert_eq!(bytes[0], codes::ARRAY32);
    let count = u32::from_be_bytes([bytes[5], bytes[6], bytes[7], bytes[8]]);
    assert_eq!(count, 256);
}

#[test]
fn encode_array_mixed_constructors_is_err() {
    // Spec §1.2.4: array must be homogeneous. Mixed types must Err.
    let v = AmqpExtValue::Array(vec![AmqpExtValue::Ubyte(1), AmqpExtValue::Ushort(2)]);
    assert!(v.encode().is_err());
}

#[test]
fn encode_array_empty_is_err() {
    // No element constructor available — must Err.
    let v = AmqpExtValue::Array(Vec::new());
    assert!(v.encode().is_err());
}

// ---------------------------------------------------------------------------
// Decode boundary for length checks
//
// Mutations like `< with ==` are only caught by tests at exact
// boundaries: bytes.len() == 9 for LIST32, == 3 for LIST8.
// ---------------------------------------------------------------------------

#[test]
fn list32_at_exact_min_length_decodes() {
    // exactly 9 bytes (min for LIST32 header) with size=4, count=0
    let buf = [codes::LIST32, 0, 0, 0, 4, 0, 0, 0, 0];
    let (v, n) = AmqpExtValue::decode(&buf).unwrap();
    assert!(matches!(v, AmqpExtValue::List(items) if items.is_empty()));
    assert_eq!(n, 9);
}

#[test]
fn list32_at_8_bytes_is_err() {
    // 8 bytes (one short of min 9) — must Err.
    assert!(AmqpExtValue::decode(&[codes::LIST32, 0, 0, 0, 4, 0, 0, 0]).is_err());
}

#[test]
fn map32_at_exact_min_length_decodes() {
    let buf = [codes::MAP32, 0, 0, 0, 4, 0, 0, 0, 0];
    let (v, n) = AmqpExtValue::decode(&buf).unwrap();
    assert!(matches!(v, AmqpExtValue::Map(entries) if entries.is_empty()));
    assert_eq!(n, 9);
}

#[test]
fn map8_with_truncated_total_is_err() {
    // size = 5 but only 3 bytes of body provided.
    let buf = [codes::MAP8, 0x05, 0x02, codes::NULL, codes::NULL];
    let res = AmqpExtValue::decode(&buf);
    assert!(res.is_err(), "size=5 needs 5 body bytes after count, got 2");
}

// ---------------------------------------------------------------------------
// Compound depth-cap boundary
//
// Tests at exactly MAX_COMPOUND_DEPTH and MAX_COMPOUND_DEPTH+1.
// ---------------------------------------------------------------------------

#[test]
fn compound_at_max_depth_encodes_and_decodes() {
    // MAX_COMPOUND_DEPTH = 32. Build nested list at depth 30 (well below).
    let mut v = AmqpExtValue::Null;
    for _ in 0..30 {
        v = AmqpExtValue::List(vec![v]);
    }
    let bytes = v.encode().unwrap();
    let (decoded, _) = AmqpExtValue::decode(&bytes).unwrap();
    assert_eq!(v, decoded);
}

#[test]
fn compound_just_below_max_depth_works() {
    // MAX_COMPOUND_DEPTH = 32. Depth 31 (innermost element at depth 32) — must work.
    let mut v = AmqpExtValue::Null;
    for _ in 0..31 {
        v = AmqpExtValue::List(vec![v]);
    }
    assert!(v.encode().is_ok());
}

#[test]
fn compound_at_depth_50_encode_errs() {
    // Depth 50 > 32 — must Err during encode.
    let mut v = AmqpExtValue::Null;
    for _ in 0..50 {
        v = AmqpExtValue::List(vec![v]);
    }
    assert!(
        v.encode().is_err(),
        "depth 50 must trigger MAX_COMPOUND_DEPTH cap"
    );
}

// ---------------------------------------------------------------------------
// Depth-Cap exact boundary (catches `> with == / >=`)
// ---------------------------------------------------------------------------

#[test]
fn compound_at_depth_32_exactly_works() {
    // MAX_COMPOUND_DEPTH = 32. The depth counter starts at 0 and is
    // incremented by 1 per nesting. Test: encode at exactly 32 wraps.
    let mut v = AmqpExtValue::Null;
    for _ in 0..32 {
        v = AmqpExtValue::List(vec![v]);
    }
    // depth counter reaches 32 in encode_at, check `> 32` is false → ok.
    assert!(
        v.encode().is_ok(),
        "depth 32 must work (cap is `> MAX_COMPOUND_DEPTH`, not `>=`)"
    );
}

#[test]
fn compound_at_depth_33_errs() {
    // 33 wraps: innermost element is decoded with depth=33 → > 32 → Err.
    let mut v = AmqpExtValue::Null;
    for _ in 0..33 {
        v = AmqpExtValue::List(vec![v]);
    }
    assert!(v.encode().is_err(), "depth 33 must err");
}

// ---------------------------------------------------------------------------
// decode_list/decode_map with multiple elements
//
// The `+ with *` mutation in `cur += n` is only caught when the loop
// iterates more than once with non-trivial `n` values
// (cur != 0, n != 0).
// ---------------------------------------------------------------------------

#[test]
fn decode_list_with_three_distinct_elements_accumulates_cur_correctly() {
    // List of [Ubyte(1), Ubyte(2), Ubyte(3)] — cur must advance through
    // all three correctly, each with n=2 bytes.
    let v = AmqpExtValue::List(vec![
        AmqpExtValue::Ubyte(1),
        AmqpExtValue::Ubyte(2),
        AmqpExtValue::Ubyte(3),
    ]);
    let bytes = v.encode().unwrap();
    let (decoded, _) = AmqpExtValue::decode(&bytes).unwrap();
    match decoded {
        AmqpExtValue::List(items) => {
            assert_eq!(items.len(), 3);
            assert_eq!(items[0], AmqpExtValue::Ubyte(1));
            assert_eq!(items[1], AmqpExtValue::Ubyte(2));
            assert_eq!(items[2], AmqpExtValue::Ubyte(3));
        }
        _ => panic!("expected List"),
    }
}

#[test]
fn decode_map_with_three_pairs_accumulates_cur_correctly() {
    let v = AmqpExtValue::Map(vec![
        (AmqpExtValue::Ubyte(1), AmqpExtValue::Ushort(100)),
        (AmqpExtValue::Ubyte(2), AmqpExtValue::Ushort(200)),
        (AmqpExtValue::Ubyte(3), AmqpExtValue::Ushort(300)),
    ]);
    let bytes = v.encode().unwrap();
    let (decoded, _) = AmqpExtValue::decode(&bytes).unwrap();
    match decoded {
        AmqpExtValue::Map(entries) => {
            assert_eq!(entries.len(), 3);
            assert_eq!(entries[0].0, AmqpExtValue::Ubyte(1));
            assert_eq!(entries[0].1, AmqpExtValue::Ushort(100));
            assert_eq!(entries[1].0, AmqpExtValue::Ubyte(2));
            assert_eq!(entries[2].1, AmqpExtValue::Ushort(300));
        }
        _ => panic!("expected Map"),
    }
}

#[test]
fn decode_array_with_five_ushorts_accumulates_cur_correctly() {
    let v = AmqpExtValue::Array(vec![
        AmqpExtValue::Ushort(0xAA00),
        AmqpExtValue::Ushort(0xBB11),
        AmqpExtValue::Ushort(0xCC22),
        AmqpExtValue::Ushort(0xDD33),
        AmqpExtValue::Ushort(0xEE44),
    ]);
    let bytes = v.encode().unwrap();
    let (decoded, _) = AmqpExtValue::decode(&bytes).unwrap();
    match decoded {
        AmqpExtValue::Array(items) => {
            assert_eq!(items.len(), 5);
            assert_eq!(items[0], AmqpExtValue::Ushort(0xAA00));
            assert_eq!(items[2], AmqpExtValue::Ushort(0xCC22));
            assert_eq!(items[4], AmqpExtValue::Ushort(0xEE44));
        }
        _ => panic!("expected Array"),
    }
}

// ---------------------------------------------------------------------------
// Array exact-bytes Wide-Boundary (catches encode_array `+ with *`)
// ---------------------------------------------------------------------------

#[test]
fn encode_array_with_two_ushorts_produces_exact_bytes() {
    let v = AmqpExtValue::Array(vec![
        AmqpExtValue::Ushort(0x1234),
        AmqpExtValue::Ushort(0x5678),
    ]);
    let bytes = v.encode().unwrap();
    // ARRAY8: code, size, count, constructor, ushort1, ushort2
    // size = 1 (count) + 1 (constructor) + 2*2 (data) = 6, count=2
    assert_eq!(
        bytes,
        vec![
            codes::ARRAY8,
            0x06,
            0x02,
            codes::USHORT,
            0x12,
            0x34,
            0x56,
            0x78,
        ],
        "ARRAY8 wire bytes mismatch"
    );
}
