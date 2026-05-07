//! Property-Tests fuer AMQP-Wire-Roundtrips.
//!
//! Invariante: `decode_value(encode_*(x))` decodiert byte-fuer-byte
//! korrekt zurueck. OASIS amqp-1.0-types §1.3-§1.6.

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

use proptest::prelude::*;
use zerodds_amqp_bridge::frame::{
    FrameHeader, FrameType, decode_frame_header, encode_frame_header,
};
use zerodds_amqp_bridge::types::{
    AmqpValue, decode_value, encode_binary, encode_boolean, encode_long, encode_null,
    encode_string, encode_symbol, encode_ulong,
};

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn boolean_roundtrip(v in any::<bool>()) {
        let bytes = encode_boolean(v);
        let (decoded, consumed) = decode_value(&bytes).unwrap();
        prop_assert_eq!(consumed, bytes.len());
        match decoded {
            AmqpValue::Boolean(b) => prop_assert_eq!(b, v),
            other => prop_assert!(false, "expected Boolean, got {other:?}"),
        }
    }

    #[test]
    fn ulong_roundtrip(v in any::<u64>()) {
        let bytes = encode_ulong(v);
        let (decoded, _) = decode_value(&bytes).unwrap();
        match decoded {
            AmqpValue::Ulong(u) => prop_assert_eq!(u, v),
            other => prop_assert!(false, "expected ULong, got {other:?}"),
        }
    }

    #[test]
    fn long_roundtrip(v in any::<i64>()) {
        let bytes = encode_long(v);
        let (decoded, _) = decode_value(&bytes).unwrap();
        match decoded {
            AmqpValue::Long(i) => prop_assert_eq!(i, v),
            other => prop_assert!(false, "expected Long, got {other:?}"),
        }
    }

    #[test]
    fn binary_roundtrip(data in prop::collection::vec(any::<u8>(), 0..1024)) {
        let bytes = encode_binary(&data).unwrap();
        let (decoded, _) = decode_value(&bytes).unwrap();
        match decoded {
            AmqpValue::Binary(b) => prop_assert_eq!(b, data),
            other => prop_assert!(false, "expected Binary, got {other:?}"),
        }
    }

    #[test]
    fn string_roundtrip(s in "[a-zA-Z0-9 _\\-]{0,256}") {
        let bytes = encode_string(&s).unwrap();
        let (decoded, _) = decode_value(&bytes).unwrap();
        match decoded {
            AmqpValue::String(d) => prop_assert_eq!(d, s),
            other => prop_assert!(false, "expected String, got {other:?}"),
        }
    }

    #[test]
    fn symbol_roundtrip(s in "[a-zA-Z0-9_\\-:.]{0,128}") {
        let bytes = encode_symbol(&s).unwrap();
        let (decoded, _) = decode_value(&bytes).unwrap();
        match decoded {
            AmqpValue::Symbol(d) => prop_assert_eq!(d, s),
            other => prop_assert!(false, "expected Symbol, got {other:?}"),
        }
    }
}

#[test]
fn null_roundtrip() {
    let bytes = encode_null();
    let (decoded, consumed) = decode_value(&bytes).unwrap();
    assert_eq!(consumed, bytes.len());
    assert!(matches!(decoded, AmqpValue::Null));
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn frame_header_roundtrip_amqp(
        size in 8u32..1_000_000,
        doff in 2u8..=63,
        channel in any::<u16>(),
    ) {
        // AMQP-1.0 §2.3.1: body offset (doff*4) muss <= size sein,
        // sonst ist der Frame strukturell invalid und decode_frame_header
        // gibt korrekt BodyOffsetExceedsSize zurueck. Filter den Fall.
        prop_assume!(size >= u32::from(doff) * 4);
        let h = FrameHeader::new_amqp(size, doff, channel);
        let bytes = encode_frame_header(h);
        let back = decode_frame_header(&bytes).unwrap();
        prop_assert_eq!(back.size, h.size);
        prop_assert_eq!(back.doff, h.doff);
        prop_assert_eq!(back.frame_type, FrameType::Amqp);
        prop_assert_eq!(back.channel, h.channel);
    }
}
