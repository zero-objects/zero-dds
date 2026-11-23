// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The DDS pump of the daemon: what a sample of a DataReader is to the
//! clients, and what a client's binary frame is to the DataWriter.
//!
//! Spec: `zerodds-ws-bridge-1.0.md` §4.3 (the binary frame `ZDB1` and the
//! JSON sample frames), §12.1. The threads that move the samples are in
//! [`super::server`]; this module holds the mapping between a
//! [`UserSample`] of the DCPS runtime and the frames of a connection.

use zerodds_dcps::runtime::UserSample;

use super::json_frames::{self, SampleFrame, SampleOp, TopicType};
use super::router::{DdsSample, SampleKind};

/// `ZDB1`, the magic of a binary frame (§4.3).
pub const BINARY_MAGIC: [u8; 4] = *b"ZDB1";
/// Flag bit 0 of a binary frame: a dispose (§4.3).
pub const FLAG_DISPOSE: u8 = 0x01;
/// Flag bit 1 of a binary frame: a sample of a coherent set (§4.3).
pub const FLAG_COHERENT_SET: u8 = 0x02;
/// Flag bit 2 of a binary frame: an unregister (§4.3).
pub const FLAG_UNREGISTER: u8 = 0x04;
/// The representation identifier of a key holder, PLAIN_CDR2 big endian
/// (DDS-XTypes 1.3 §7.6.8): the payload of a dispose or an unregister.
pub const KEY_HOLDER_IDENTIFIER: [u8; 2] = [0x00, 0x06];

/// What a sample of a DDS reader is to the clients (§4.3): a sample, or a
/// dispose or an unregister with the key holder where the marker carried
/// it; `None` for what carries no user data (the end-of-coherent-set
/// marker, DDSI-RTPS 2.5 §8.7.6, and a lifecycle change to alive).
#[must_use]
pub fn dds_sample_of(sample: UserSample) -> Option<DdsSample> {
    use zerodds_rtps::history_cache::ChangeKind;
    match sample {
        UserSample::Alive {
            payload,
            representation_identifier,
            key_hash,
            source_timestamp,
            coherent_set_sn,
            ..
        } => Some(DdsSample {
            payload: payload.to_vec(),
            representation_identifier,
            kind: SampleKind::Alive,
            key_hash,
            timestamp_ns: source_timestamp.map(|t| json_frames::nanos_of(t.seconds, t.fraction)),
            coherent_set: coherent_set_sn.is_some(),
        }),
        UserSample::Lifecycle {
            key_hash,
            kind,
            key_holder,
            source_timestamp,
            coherent_set_sn,
            ..
        } => {
            let kind = match kind {
                ChangeKind::NotAliveDisposed => SampleKind::Disposed,
                ChangeKind::NotAliveUnregistered => SampleKind::Unregistered,
                ChangeKind::NotAliveDisposedUnregistered => SampleKind::DisposedUnregistered,
                ChangeKind::Alive | ChangeKind::AliveFiltered => return None,
            };
            Some(DdsSample {
                payload: key_holder.unwrap_or_default(),
                representation_identifier: KEY_HOLDER_IDENTIFIER,
                kind,
                key_hash: Some(key_hash),
                timestamp_ns: source_timestamp
                    .map(|t| json_frames::nanos_of(t.seconds, t.fraction)),
                coherent_set: coherent_set_sn.is_some(),
            })
        }
        UserSample::CoherentSetEnd { .. } => None,
    }
}

/// A binary frame of §4.3: `ZDB1`, the flags octet (bit 0 a dispose, bit 1
/// a sample of a coherent set, bit 2 an unregister), the encapsulation
/// header the payload came under (DDSI-RTPS 2.5 §10.5: its representation
/// identifier, which names version, format and byte order, and options),
/// the CDR bytes as they travelled over RTPS; for a dispose or an
/// unregister the key holder.
#[must_use]
pub fn binary_sample_frame(sample: &DdsSample) -> Vec<u8> {
    let flags = match sample.kind {
        SampleKind::Alive => 0,
        SampleKind::Disposed => FLAG_DISPOSE,
        SampleKind::Unregistered => FLAG_UNREGISTER,
        SampleKind::DisposedUnregistered => FLAG_DISPOSE | FLAG_UNREGISTER,
    } | if sample.coherent_set {
        FLAG_COHERENT_SET
    } else {
        0
    };
    let identifier = sample.representation_identifier;
    let mut out = Vec::with_capacity(sample.payload.len() + 9);
    out.extend_from_slice(&BINARY_MAGIC);
    out.push(flags);
    out.extend_from_slice(&[identifier[0], identifier[1], 0, 0]);
    out.extend_from_slice(&sample.payload);
    out
}

/// The flags, the representation identifier and the CDR bytes of a binary
/// frame of §4.3.
///
/// # Errors
/// A frame shorter than magic, flags and header, or a header that names no
/// CDR representation (DDSI-RTPS 2.5 §10.5, DDS-XTypes 1.3 Table 60).
pub fn split_binary_sample(frame: &[u8]) -> Result<(u8, [u8; 2], &[u8]), String> {
    if frame.len() < 9 || frame[..4] != BINARY_MAGIC {
        return Err("a binary frame shorter than ZDB1, flags and encapsulation header".into());
    }
    let identifier = [frame[5], frame[6]];
    match u16::from_be_bytes(identifier) {
        0x0000..=0x0003 | 0x0006..=0x000b => Ok((frame[4], identifier, &frame[9..])),
        other => Err(format!(
            "encapsulation {other:#06x} is not a CDR representation"
        )),
    }
}

/// The flags of a client's binary frame (§4.3): a client sends a sample with
/// flags 0. It disposes and unregisters with a JSON sample frame; bit 1
/// marks a sample of a coherent set the daemon forwards, and a client opens
/// no coherent set; bits 3 to 7 are reserved.
///
/// # Errors
/// The message of the error frame 400 for any flag bit.
pub fn check_client_flags(topic: &str, flags: u8) -> Result<(), String> {
    if flags & (FLAG_DISPOSE | FLAG_UNREGISTER) != 0 {
        return Err(format!(
            "{topic}: a client disposes or unregisters with a JSON sample frame"
        ));
    }
    if flags != 0 {
        return Err(format!(
            "{topic}: flags {flags:#04x}: a client sends a sample with flags 0"
        ));
    }
    Ok(())
}

/// How the DDS writer frames a body that came under `identifier` (DDS-XTypes
/// 1.3 Table 60): its data representation (`CDR`, `PL_CDR` are XCDR1, the
/// others XCDR2), the extensibility the format belongs to (a parameter list
/// is `@mutable`, a delimited body `@appendable`, anything else `@final`) and
/// whether it is big-endian (the even identifiers).
#[must_use]
pub fn writer_form(identifier: [u8; 2]) -> (i16, zerodds_types::qos::ExtensibilityForRepr, bool) {
    use zerodds_rtps::publication_data::data_representation as dr;
    use zerodds_types::qos::ExtensibilityForRepr as Ext;
    let representation = if identifier[1] <= 0x03 {
        dr::XCDR
    } else {
        dr::XCDR2
    };
    let extensibility = match identifier[1] & !0x01 {
        0x02 | 0x0a => Ext::Mutable,
        0x08 => Ext::Appendable,
        _ => Ext::Final,
    };
    (representation, extensibility, identifier[1] & 0x01 == 0)
}

/// The error frame for a JSON-mode client of a topic without `idl_file`
/// (§3): its samples cannot be JSON without the type.
#[must_use]
pub fn no_type_error(topic: &str) -> String {
    json_frames::error(
        415,
        &format!("{topic} has no idl_file: the JSON text frames need the topic's type"),
    )
}

/// The text frames of a DDS sample for a JSON-mode client (§4.3): a
/// `publish` with the sample as JSON, a `dispose`, an `unregister` (both
/// for a marker that is both).
///
/// # Errors
/// The error frame (§4.4) for a topic without type or a sample that does
/// not decode as its type.
pub fn json_sample_frames(
    topic: &str,
    sample: &DdsSample,
    ty: Option<&TopicType>,
) -> Result<Vec<String>, String> {
    let frame = |op: SampleOp, data: Option<serde_json::Value>| SampleFrame {
        topic: topic.to_string(),
        op,
        key_hash: sample.key_hash,
        data,
        timestamp_ns: sample.timestamp_ns,
    };
    let frames = match sample.kind {
        SampleKind::Alive => {
            let ty = ty.ok_or_else(|| no_type_error(topic))?;
            let data = ty
                .decode(&sample.payload, sample.representation_identifier)
                .map_err(|e| {
                    json_frames::error(422, &format!("a sample of {topic} does not decode: {e}"))
                })?;
            vec![frame(SampleOp::Publish, Some(data))]
        }
        SampleKind::Disposed => vec![frame(SampleOp::Dispose, None)],
        SampleKind::Unregistered => vec![frame(SampleOp::Unregister, None)],
        SampleKind::DisposedUnregistered => vec![
            frame(SampleOp::Dispose, None),
            frame(SampleOp::Unregister, None),
        ],
    };
    Ok(frames.iter().map(SampleFrame::render).collect())
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use zerodds_dcps::sample_bytes::SampleBytes;
    use zerodds_rtps::history_cache::ChangeKind;

    fn alive(payload: Vec<u8>, coherent_set_sn: Option<i64>) -> UserSample {
        UserSample::Alive {
            key_hash: Some([9; 16]),
            payload: SampleBytes::from_vec(payload),
            writer_guid: [0; 16],
            writer_strength: 0,
            representation: 2,
            big_endian: false,
            representation_identifier: [0x00, 0x09],
            source_timestamp: None,
            reception_timestamp: None,
            source_sequence_number: 5,
            coherent_set_sn,
            lifespan_nanos: 0,
            related_sample_identity: None,
            group: None,
            crypto_content: false,
        }
    }

    fn marker(kind: ChangeKind, key_holder: Option<Vec<u8>>) -> UserSample {
        UserSample::Lifecycle {
            key_hash: [7; 16],
            key_hash_unavailable: false,
            kind,
            key_holder,
            source_timestamp: None,
            group: None,
            writer_guid: [0; 16],
            coherent_set_sn: None,
            source_sequence_number: 6,
        }
    }

    fn frame_of(sample: UserSample) -> Vec<u8> {
        binary_sample_frame(&dds_sample_of(sample).expect("a frame"))
    }

    /// zerodds-ws-bridge-1.0 §4.3 with DDS-XTypes 1.3 §7.6.3.1.2, Table 60:
    /// a binary frame carries the encapsulation header the sample came
    /// under. Before, it was built from the version and the byte order
    /// alone, so a `PL_CDR2` or `D_CDR2` sample went out as `CDR2`
    /// (OPEN-ITEMS 60).
    #[test]
    fn xtypes_7_6_3_1_2_a_binary_frame_carries_the_identifier_the_sample_came_under() {
        for identifier in [[0x00, 0x01], [0x00, 0x03], [0x00, 0x08], [0x00, 0x0b]] {
            let frame = binary_sample_frame(&DdsSample::alive(vec![1, 2, 3, 4], identifier));
            assert_eq!(
                frame[..9],
                [
                    b'Z',
                    b'D',
                    b'B',
                    b'1',
                    0,
                    identifier[0],
                    identifier[1],
                    0,
                    0
                ]
            );
            let (flags, back, body) = split_binary_sample(&frame).unwrap();
            assert_eq!((flags, back, body), (0, identifier, &[1u8, 2, 3, 4][..]));
        }
        // XML (`00 04`) is no CDR representation.
        assert!(split_binary_sample(b"ZDB1\x00\x00\x04\x00\x00").is_err());
        // Shorter than magic, flags and header, or another magic.
        assert!(split_binary_sample(b"ZDB1\x00\x00\x07\x00").is_err());
        assert!(split_binary_sample(b"ZDB2\x00\x00\x07\x00\x00").is_err());
    }

    /// zerodds-ws-bridge-1.0 §4.3: a sample a writer wrote inside a coherent
    /// set (`PID_COHERENT_SET`, DDSI-RTPS 2.5 §8.7.6) carries flag bit 1, a
    /// dispose of the set bits 0 and 1; a sample outside a set, and the end
    /// marker of the set, which carries no data, do not.
    #[test]
    fn wsbridge_4_3_a_sample_of_a_coherent_set_carries_bit_1() {
        assert_eq!(frame_of(alive(vec![1, 2, 3, 4], Some(3)))[4], 0x02);
        assert_eq!(frame_of(alive(vec![1, 2, 3, 4], None))[4], 0x00);
        let dispose = |coherent_set_sn: Option<i64>| UserSample::Lifecycle {
            key_hash: [7; 16],
            key_hash_unavailable: false,
            kind: ChangeKind::NotAliveDisposed,
            key_holder: None,
            source_timestamp: None,
            group: None,
            writer_guid: [0; 16],
            coherent_set_sn,
            source_sequence_number: 6,
        };
        assert_eq!(frame_of(dispose(Some(3)))[4], 0x03);
        assert_eq!(frame_of(dispose(None))[4], 0x01);
        assert!(
            dds_sample_of(UserSample::CoherentSetEnd {
                writer_guid: [0; 16],
                set_first_sn: Some(3),
                ecs_sn: Some(7),
                group: None,
                complete: Some(true),
            })
            .is_none()
        );
    }

    /// DDS-XTypes 1.3 Table 60: the identifier of a client's frame names the
    /// data representation, the extensibility and the byte order the DDS
    /// writer gives the body.
    #[test]
    fn xtypes_7_6_3_1_2_the_identifier_names_the_form_of_the_dds_writer() {
        use zerodds_types::qos::ExtensibilityForRepr as Ext;
        for (identifier, form) in [
            ([0x00, 0x00], (0, Ext::Final, true)),
            ([0x00, 0x01], (0, Ext::Final, false)),
            ([0x00, 0x02], (0, Ext::Mutable, true)),
            ([0x00, 0x03], (0, Ext::Mutable, false)),
            ([0x00, 0x06], (2, Ext::Final, true)),
            ([0x00, 0x07], (2, Ext::Final, false)),
            ([0x00, 0x08], (2, Ext::Appendable, true)),
            ([0x00, 0x09], (2, Ext::Appendable, false)),
            ([0x00, 0x0a], (2, Ext::Mutable, true)),
            ([0x00, 0x0b], (2, Ext::Mutable, false)),
        ] {
            assert_eq!(writer_form(identifier), form, "{identifier:02x?}");
        }
    }

    /// Spec §12.1, §4.3: a sample is the frame `ZDB1`, flags 0, the header
    /// it came under and its CDR bytes unchanged; key hash and source time
    /// are kept for the JSON frame.
    #[test]
    fn wsbridge_12_1_an_alive_sample_is_a_frame_with_flags_0() {
        let sample = dds_sample_of(alive(vec![0xde, 0xad, 0xbe, 0xef], None)).unwrap();
        assert_eq!(sample.kind, SampleKind::Alive);
        assert_eq!(sample.key_hash, Some([9; 16]));
        assert_eq!(
            binary_sample_frame(&sample),
            b"ZDB1\x00\x00\x09\x00\x00\xde\xad\xbe\xef".to_vec()
        );
    }

    /// Spec §12.1, §4.3: a dispose carries bit 0 and, as payload, the
    /// instance's key holder under PLAIN_CDR2 big endian; without a key
    /// holder (a marker with the key hash alone) the payload is empty.
    #[test]
    fn wsbridge_12_1_a_dispose_carries_bit_0_and_the_key_holder() {
        let frame = frame_of(marker(
            ChangeKind::NotAliveDisposed,
            Some(vec![0, 0, 0, 42]),
        ));
        assert_eq!(frame, b"ZDB1\x01\x00\x06\x00\x00\x00\x00\x00\x2a".to_vec());
        let frame = frame_of(marker(ChangeKind::NotAliveDisposed, None));
        assert_eq!(frame, b"ZDB1\x01\x00\x06\x00\x00".to_vec());
    }

    /// Spec §12.1, §4.3: an unregister carries bit 2 and not bit 0.
    #[test]
    fn wsbridge_12_1_an_unregister_carries_bit_2() {
        let frame = frame_of(marker(ChangeKind::NotAliveUnregistered, Some(vec![1])));
        assert_eq!(frame[4], FLAG_UNREGISTER);
        assert_eq!(&frame[5..7], &KEY_HOLDER_IDENTIFIER);
        assert_eq!(&frame[9..], &[1]);
    }

    /// Spec §12.1, §4.3: a marker that is a dispose and an unregister at
    /// once carries both bits in one binary frame, and is two JSON frames,
    /// `dispose` then `unregister`, each with the key hash and no `data`.
    #[test]
    fn wsbridge_12_1_a_marker_that_is_both_carries_both() {
        let sample = dds_sample_of(marker(ChangeKind::NotAliveDisposedUnregistered, None)).unwrap();
        assert_eq!(binary_sample_frame(&sample)[4], 0x05);
        let texts = json_sample_frames("T", &sample, None).expect("no type needed for markers");
        assert_eq!(texts.len(), 2);
        let parsed: Vec<serde_json::Value> = texts
            .iter()
            .map(|t| serde_json::from_str(t).expect("JSON"))
            .collect();
        assert_eq!(parsed[0]["op"], "dispose");
        assert_eq!(parsed[1]["op"], "unregister");
        for p in &parsed {
            assert_eq!(p["topic"], "T");
            assert_eq!(p["key_hash"], "07".repeat(16));
            assert!(p.get("data").is_none(), "{p}");
        }
    }

    /// Spec §12.1, §4.3: the end of a coherent set and a lifecycle change to
    /// alive carry no data and are no frame.
    #[test]
    fn wsbridge_12_1_the_coherent_set_end_is_no_frame() {
        assert!(
            dds_sample_of(UserSample::CoherentSetEnd {
                writer_guid: [0; 16],
                set_first_sn: None,
                ecs_sn: None,
                group: None,
                complete: None,
            })
            .is_none()
        );
        assert!(dds_sample_of(marker(ChangeKind::Alive, None)).is_none());
        assert!(dds_sample_of(marker(ChangeKind::AliveFiltered, None)).is_none());
    }

    /// Spec §12.1, §4.3: a client's binary frame with any flag bit (bit 0,
    /// bit 1, bit 2 or a reserved bit) is refused; flags 0 passes.
    #[test]
    fn wsbridge_12_1_a_client_frame_with_any_flag_bit_is_refused() {
        assert_eq!(check_client_flags("T", 0), Ok(()));
        for bit in 0..8 {
            let flags = 1u8 << bit;
            let message = check_client_flags("T", flags).expect_err("refused");
            assert!(message.starts_with("T: "), "{message}");
        }
        assert!(
            check_client_flags("T", FLAG_DISPOSE)
                .unwrap_err()
                .contains("JSON sample frame")
        );
    }

    /// Spec §12.1, §4.3: a sample of a JSON-mode client of a topic without
    /// type is the error 415, not a frame.
    #[test]
    fn wsbridge_12_1_a_sample_of_a_topic_without_type_is_the_error_415() {
        let sample = DdsSample::alive(vec![1, 2, 3, 4], [0x00, 0x07]);
        let error = json_sample_frames("Untyped", &sample, None).expect_err("415");
        let parsed: serde_json::Value = serde_json::from_str(&error).unwrap();
        assert_eq!(parsed["op"], "error");
        assert_eq!(parsed["code"], 415);
    }
}
