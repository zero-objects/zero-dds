// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-codec` with `zerodds-dcps`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-dcps`: the `DdsType` trait, which
//! `derive(DdsType)` implements through `__private` — `TYPE_NAME`,
//! `EXTENSIBILITY`, `HAS_KEY`, `IS_NESTED`, the eight encode and decode
//! methods, `encode_continuing`/`decode_continuing`,
//! `encode_key_holder_be`/`decode_key_holder_be`, `key_holder_max_size`,
//! `type_identifier`, `type_object`, `type_object_dependencies`,
//! `type_component`, `field_value` — and the trait's provided
//! `compute_key_hash`/`key_hash`; `Extensibility`; `EncodeError::Invalid`,
//! `DecodeError::Invalid` and `DecodeError::from(WireError)`; `FilterValue`;
//! `dds_type::PlainCdr2BeKeyHolder::write_bytes` and `as_bytes`; `DdsAny`.
//!
//! Oracle: DDSI-RTPS 2.5 §9.6.3.8 and DDS-XTypes 1.3 §7.6.8 (a key holder
//! of at most sixteen octets is the KeyHash, zero-padded), DDS 1.4
//! §2.2.2.3.2 (the type name a topic is registered under).

#![allow(clippy::unwrap_used, clippy::panic, missing_docs)]

use zerodds_codec::DdsType;
use zerodds_dcps::dds_type::DdsAny;
use zerodds_dcps::dds_type::PlainCdr2BeKeyHolder;
use zerodds_dcps::{DecodeError, Extensibility, FilterValue};

#[derive(DdsType, Debug, Clone, PartialEq)]
#[dds(name = "d::Sensor", extensibility = "mutable", nested)]
struct Sensor {
    #[dds(key)]
    id: u16,
    #[dds(key)]
    zone: u8,
    reading: f32,
}

#[test]
fn dds_1_4_2_2_2_3_2_the_constants_a_topic_is_registered_with() {
    assert_eq!(<Sensor as zerodds_dcps::DdsType>::TYPE_NAME, "d::Sensor");
    assert_eq!(
        <Sensor as zerodds_dcps::DdsType>::EXTENSIBILITY,
        Extensibility::Mutable
    );
    const { assert!(<Sensor as zerodds_dcps::DdsType>::HAS_KEY) };
    const { assert!(<Sensor as zerodds_dcps::DdsType>::IS_NESTED) };
    assert_eq!(
        <Sensor as zerodds_dcps::DdsType>::key_holder_max_size(),
        Some(16)
    );
}

#[test]
fn rtps_2_5_9_6_3_8_the_trait_computes_the_key_hash_from_the_holder() {
    let sensor = Sensor {
        id: 0x0102,
        zone: 3,
        reading: 1.0,
    };
    let mut holder = PlainCdr2BeKeyHolder::new();
    zerodds_dcps::DdsType::encode_key_holder_be(&sensor, &mut holder);
    assert_eq!(holder.as_bytes(), [0x01, 0x02, 0x03]);
    let mut expected = [0u8; 16];
    expected[..3].copy_from_slice(&[0x01, 0x02, 0x03]);
    assert_eq!(zerodds_dcps::DdsType::key_hash(&sensor), Some(expected));
    let back = <Sensor as zerodds_dcps::DdsType>::decode_key_holder_be(holder.as_bytes()).unwrap();
    assert_eq!((back.id, back.zone, back.reading), (0x0102, 3, 0.0));
}

#[test]
fn dds_1_4_2_2_2_3_2_a_derived_type_travels_in_dds_any() {
    let sensor = Sensor {
        id: 9,
        zone: 1,
        reading: -0.5,
    };
    let any = DdsAny::pack(&sensor).unwrap();
    assert_eq!(any.type_name, "d::Sensor");
    assert_eq!(any.unpack::<Sensor>().unwrap(), sensor);
}

#[test]
fn xtypes_7_4_a_refused_stream_is_a_decode_error_with_its_reason() {
    match <Sensor as zerodds_dcps::DdsType>::decode(&[1, 2]) {
        Err(DecodeError::Invalid { what }) => assert!(!what.is_empty()),
        other => panic!("a decode error, got {other:?}"),
    }
    assert_eq!(
        zerodds_dcps::DdsType::field_value(
            &Sensor {
                id: 4,
                zone: 0,
                reading: 0.0
            },
            "id"
        ),
        Some(FilterValue::Int(4))
    );
}
