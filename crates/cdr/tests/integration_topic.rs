//! W4 integration: realistic DDS topic encoders/decoders without macros.
//!
//! These tests simulate how a future code-gen backend would use the
//! `zerodds-cdr` helpers. They validate the end-to-end pipeline from
//! W1-W3 for realistic DDS topic structures.

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

use zerodds_cdr::Endianness;
use zerodds_cdr::buffer::{BufferReader, BufferWriter};
use zerodds_cdr::encode::{CdrDecode, CdrEncode};
use zerodds_cdr::error::{DecodeError, EncodeError};
use zerodds_cdr::struct_enc::{
    decode_appendable, decode_final, encode_appendable, encode_final, encode_mutable_member,
    read_all_mutable_members,
};

// ============================================================================
// @final SensorReading — a typical DDS topic without extensibility
// ============================================================================

#[derive(Debug, PartialEq)]
struct SensorReading {
    sensor_id: u32,
    value: f64,
    timestamp_ns: u64,
}

fn encode_sensor_final(s: &SensorReading, w: &mut BufferWriter) -> Result<(), EncodeError> {
    encode_final(w, |w| {
        s.sensor_id.encode(w)?;
        s.value.encode(w)?;
        s.timestamp_ns.encode(w)?;
        Ok(())
    })
}

fn decode_sensor_final(r: &mut BufferReader<'_>) -> Result<SensorReading, DecodeError> {
    decode_final(r, |r| {
        Ok(SensorReading {
            sensor_id: u32::decode(r)?,
            value: f64::decode(r)?,
            timestamp_ns: u64::decode(r)?,
        })
    })
}

#[test]
fn sensor_reading_final_roundtrip() {
    let s = SensorReading {
        sensor_id: 42,
        value: core::f64::consts::PI,
        timestamp_ns: 1_700_000_000_000_000_000,
    };
    let mut w = BufferWriter::new(Endianness::Little);
    encode_sensor_final(&s, &mut w).unwrap();
    let bytes = w.into_bytes();

    let mut r = BufferReader::new(&bytes, Endianness::Little);
    let decoded = decode_sensor_final(&mut r).unwrap();
    assert_eq!(decoded, s);
    assert_eq!(r.remaining(), 0);
}

#[test]
fn sensor_reading_be_le_independent() {
    let s = SensorReading {
        sensor_id: 0xDEAD_BEEF,
        value: -1.5e-10,
        timestamp_ns: 0xCAFE_BABE_1234_5678,
    };
    for end in [Endianness::Little, Endianness::Big] {
        let mut w = BufferWriter::new(end);
        encode_sensor_final(&s, &mut w).unwrap();
        let bytes = w.into_bytes();
        let mut r = BufferReader::new(&bytes, end);
        let decoded = decode_sensor_final(&mut r).unwrap();
        assert_eq!(decoded, s);
    }
}

// ============================================================================
// @appendable AlarmEvent — forward-compatible
// ============================================================================

#[derive(Debug, PartialEq)]
struct AlarmEventV1 {
    event_id: u32,
    severity: u8,
}

#[derive(Debug, PartialEq)]
struct AlarmEventV2 {
    event_id: u32,
    severity: u8,
    // Extension: timestamp new in V2
    timestamp_ns: u64,
}

fn encode_alarm_v2(s: &AlarmEventV2, w: &mut BufferWriter) -> Result<(), EncodeError> {
    encode_appendable(w, |w| {
        s.event_id.encode(w)?;
        s.severity.encode(w)?;
        s.timestamp_ns.encode(w)?;
        Ok(())
    })
}

#[test]
fn alarm_event_v2_can_be_decoded_as_v1() {
    // Forward compat: V2 writer + V1 reader. The reader skips the extra
    // bytes via the DHEADER sub-reader.
    let v2 = AlarmEventV2 {
        event_id: 7,
        severity: 3,
        timestamp_ns: 1_700_000_000,
    };
    let mut w = BufferWriter::new(Endianness::Little);
    encode_alarm_v2(&v2, &mut w).unwrap();
    let bytes = w.into_bytes();

    let mut r = BufferReader::new(&bytes, Endianness::Little);
    let v1 = decode_appendable(&mut r, |r| {
        Ok::<_, DecodeError>(AlarmEventV1 {
            event_id: u32::decode(r)?,
            severity: u8::decode(r)?,
        })
    })
    .unwrap();
    assert_eq!(v1.event_id, 7);
    assert_eq!(v1.severity, 3);
    // The V1 reader consumed the entire frame even though it did not read
    // the timestamp.
    assert_eq!(r.remaining(), 0);
}

// ============================================================================
// @mutable Configuration — member-ID-based, reorderable
// ============================================================================

#[derive(Debug, PartialEq)]
struct Configuration {
    max_payload_size: u32,
    enable: bool,
    label: u32, // ID 10 — non-consecutive ID
}

const ID_MAX_PAYLOAD_SIZE: u32 = 1;
const ID_ENABLE: u32 = 2;
const ID_LABEL: u32 = 10;

fn encode_config_mutable(c: &Configuration, w: &mut BufferWriter) -> Result<(), EncodeError> {
    encode_mutable_member(w, ID_MAX_PAYLOAD_SIZE, false, |w| {
        c.max_payload_size.encode(w)
    })?;
    encode_mutable_member(w, ID_ENABLE, false, |w| c.enable.encode(w))?;
    encode_mutable_member(w, ID_LABEL, false, |w| c.label.encode(w))?;
    Ok(())
}

fn decode_config_mutable(r: &mut BufferReader<'_>) -> Result<Configuration, DecodeError> {
    let members = read_all_mutable_members(r)?;
    let mut max_payload_size: Option<u32> = None;
    let mut enable: Option<bool> = None;
    let mut label: Option<u32> = None;
    for m in &members {
        let mut sub = BufferReader::new(m.body, r.endianness());
        match m.member_id {
            ID_MAX_PAYLOAD_SIZE => max_payload_size = Some(u32::decode(&mut sub)?),
            ID_ENABLE => enable = Some(bool::decode(&mut sub)?),
            ID_LABEL => label = Some(u32::decode(&mut sub)?),
            _ => {} // ignore unknown members (forward compat)
        }
    }
    Ok(Configuration {
        max_payload_size: max_payload_size.unwrap_or(0),
        enable: enable.unwrap_or(false),
        label: label.unwrap_or(0),
    })
}

#[test]
fn configuration_mutable_roundtrip() {
    let c = Configuration {
        max_payload_size: 1024,
        enable: true,
        label: 42,
    };
    let mut w = BufferWriter::new(Endianness::Little);
    encode_config_mutable(&c, &mut w).unwrap();
    let bytes = w.into_bytes();

    let mut r = BufferReader::new(&bytes, Endianness::Little);
    let decoded = decode_config_mutable(&mut r).unwrap();
    assert_eq!(decoded, c);
}

#[test]
fn configuration_decoder_ignores_unknown_member_ids() {
    // Write 4 members; the decoder knows only 3.
    let mut w = BufferWriter::new(Endianness::Little);
    encode_mutable_member(&mut w, ID_MAX_PAYLOAD_SIZE, false, |w| 999u32.encode(w)).unwrap();
    encode_mutable_member(&mut w, 9999, false, |w| 0xCAFEu32.encode(w)).unwrap();
    encode_mutable_member(&mut w, ID_ENABLE, false, |w| true.encode(w)).unwrap();
    encode_mutable_member(&mut w, ID_LABEL, false, |w| 7u32.encode(w)).unwrap();
    let bytes = w.into_bytes();

    let mut r = BufferReader::new(&bytes, Endianness::Little);
    let decoded = decode_config_mutable(&mut r).unwrap();
    assert_eq!(decoded.max_payload_size, 999);
    assert!(decoded.enable);
    assert_eq!(decoded.label, 7);
}

// ============================================================================
// Composite Types: Sensor + Alarms (Sequence + Optional + String)
// ============================================================================

#[derive(Debug, PartialEq)]
struct DeviceSnapshot {
    device_name: alloc::string::String,
    readings: alloc::vec::Vec<SensorReading>,
    last_alarm: Option<u32>,
}

extern crate alloc;

fn encode_device(s: &DeviceSnapshot, w: &mut BufferWriter) -> Result<(), EncodeError> {
    encode_final(w, |w| {
        s.device_name.encode(w)?;
        // Sequence length + items
        let len = u32::try_from(s.readings.len()).map_err(|_| EncodeError::ValueOutOfRange {
            message: "readings length overflow",
        })?;
        len.encode(w)?;
        for r in &s.readings {
            encode_sensor_final(r, w)?;
        }
        s.last_alarm.encode(w)?;
        Ok(())
    })
}

fn decode_device(r: &mut BufferReader<'_>) -> Result<DeviceSnapshot, DecodeError> {
    decode_final(r, |r| {
        let device_name = alloc::string::String::decode(r)?;
        let len = u32::decode(r)? as usize;
        let mut readings = alloc::vec::Vec::with_capacity(len);
        for _ in 0..len {
            readings.push(decode_sensor_final(r)?);
        }
        let last_alarm = Option::<u32>::decode(r)?;
        Ok(DeviceSnapshot {
            device_name,
            readings,
            last_alarm,
        })
    })
}

#[test]
fn device_snapshot_complex_roundtrip() {
    let snapshot = DeviceSnapshot {
        device_name: "thermo-3".into(),
        readings: alloc::vec![
            SensorReading {
                sensor_id: 1,
                value: 21.5,
                timestamp_ns: 100,
            },
            SensorReading {
                sensor_id: 2,
                value: 22.0,
                timestamp_ns: 200,
            },
        ],
        last_alarm: Some(0xABCD),
    };
    let mut w = BufferWriter::new(Endianness::Little);
    encode_device(&snapshot, &mut w).unwrap();
    let bytes = w.into_bytes();

    let mut r = BufferReader::new(&bytes, Endianness::Little);
    let decoded = decode_device(&mut r).unwrap();
    assert_eq!(decoded, snapshot);
}

#[test]
fn device_snapshot_empty_readings_and_no_alarm() {
    let snapshot = DeviceSnapshot {
        device_name: "idle".into(),
        readings: alloc::vec![],
        last_alarm: None,
    };
    let mut w = BufferWriter::new(Endianness::Big);
    encode_device(&snapshot, &mut w).unwrap();
    let bytes = w.into_bytes();
    let mut r = BufferReader::new(&bytes, Endianness::Big);
    let decoded = decode_device(&mut r).unwrap();
    assert_eq!(decoded, snapshot);
}
