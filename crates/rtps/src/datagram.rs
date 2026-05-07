// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Datagram-Encoder/-Decoder: kombiniert RTPS-Header und Submessages
//! zu einem fertigen Wire-Datagram (W4).

extern crate alloc;
use alloc::vec::Vec;

use crate::error::WireError;
use crate::header::RtpsHeader;
use crate::header_extension::{HeaderExtension, SUBMESSAGE_ID_HEADER_EXTENSION};
use crate::submessage_header::{FLAG_E_LITTLE_ENDIAN, SubmessageHeader, SubmessageId};
use crate::submessages::{
    ACKNACK_FLAG_FINAL, AckNackSubmessage, DATA_FRAG_FLAG_HASH_KEY, DATA_FRAG_FLAG_INLINE_QOS,
    DATA_FRAG_FLAG_KEY, DATA_FRAG_FLAG_NON_STANDARD, DataFragSubmessage, DataSubmessage,
    GAP_FLAG_FILTERED_COUNT, GAP_FLAG_GROUP_INFO, GapSubmessage, HEARTBEAT_FLAG_FINAL,
    HEARTBEAT_FLAG_GROUP_INFO, HEARTBEAT_FLAG_LIVELINESS, HeartbeatFragSubmessage,
    HeartbeatSubmessage, INFO_REPLY_FLAG_MULTICAST, INFO_TIMESTAMP_FLAG_INVALIDATE,
    InfoReplySubmessage, InfoSourceSubmessage, InfoTimestampSubmessage, NackFragSubmessage,
};

/// Encoded ein RTPS-Datagram = `RtpsHeader` + Sequenz von `DATA`-
/// Submessages. Variante: alle Submessages sind LE; ein
/// Datagram traegt eine Liste DATA-Bodies.
pub fn encode_data_datagram(
    header: RtpsHeader,
    data_submessages: &[DataSubmessage],
) -> Result<Vec<u8>, WireError> {
    let mut out = Vec::new();
    out.extend_from_slice(&header.to_bytes());
    for d in data_submessages {
        let (body, flags) = d.write_body(true);
        let body_len = u16::try_from(body.len()).map_err(|_| WireError::ValueOutOfRange {
            message: "DATA submessage body exceeds u16::MAX",
        })?;
        let sh = SubmessageHeader {
            submessage_id: SubmessageId::Data,
            flags,
            octets_to_next_header: body_len,
        };
        out.extend_from_slice(&sh.to_bytes());
        out.extend_from_slice(&body);
    }
    Ok(out)
}

/// Geparstes Datagram: Header + alle erkannten Submessages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedDatagram {
    /// RTPS-Header.
    pub header: RtpsHeader,
    /// Alle erkannten Submessages in Reihenfolge.
    pub submessages: Vec<ParsedSubmessage>,
}

/// Eine erkannte Submessage. unterstützt DATA/HEARTBEAT/ACKNACK/GAP/DATA_FRAG/HEARTBEAT_FRAG/NACK_FRAG/INFO_*; andere werden via `octets_to_next_header`
/// uebersprungen und als [`ParsedSubmessage::Unknown`] gemerkt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParsedSubmessage {
    /// DATA-Submessage.
    Data(DataSubmessage),
    /// DATA_FRAG-Submessage (Fragmentation).
    DataFrag(DataFragSubmessage),
    /// HEARTBEAT-Submessage.
    Heartbeat(HeartbeatSubmessage),
    /// HEARTBEAT_FRAG-Submessage.
    HeartbeatFrag(HeartbeatFragSubmessage),
    /// ACKNACK-Submessage.
    AckNack(AckNackSubmessage),
    /// NACK_FRAG-Submessage.
    NackFrag(NackFragSubmessage),
    /// GAP-Submessage.
    Gap(GapSubmessage),
    /// HeaderExtension-Submessage (DDSI-RTPS 2.5 §8.3.3.2).
    HeaderExtension(HeaderExtension),
    /// InfoSource-Submessage (§8.3.8.9.4).
    InfoSource(InfoSourceSubmessage),
    /// InfoReply-Submessage (§8.3.8.10.4).
    InfoReply(InfoReplySubmessage),
    /// InfoTimestamp-Submessage (§8.3.8.5 / §8.3.7.5).
    InfoTimestamp(InfoTimestampSubmessage),
    /// Andere Submessage-Klasse (geskippt). Traegt id + flags fuer
    /// Diagnostik.
    Unknown {
        /// Submessage-ID-Byte.
        id: u8,
        /// Flag-Byte.
        flags: u8,
    },
}

/// Submessage-Header Must-Understand-Bit (Bit 7 des Flag-Bytes,
/// DDSI-RTPS 2.5 §8.3.3.2). Bei unbekannter Submessage-ID + gesetztem
/// Bit MUSS die ganze RTPS-Message verworfen werden.
pub const SUBMESSAGE_FLAG_MUST_UNDERSTAND: u8 = 0x80;

/// Decoded ein RTPS-Datagram in Header + Submessage-Liste.
///
/// `octets_to_next_header == 0` (Last-Submessage-Marker, Spec §8.3.4.2)
/// wird so behandelt: die Submessage erstreckt sich bis zum Ende des
/// Datagrams.
///
/// # Errors
/// `InvalidMagic`, `UnexpectedEof`, oder Sub-Decoder-Fehler. Unbekannte
/// Submessage-IDs werden als `Unknown` markiert (kein Fehler).
pub fn decode_datagram(bytes: &[u8]) -> Result<ParsedDatagram, WireError> {
    let header = RtpsHeader::from_bytes(bytes)?;
    let mut pos = RtpsHeader::WIRE_SIZE;
    let mut submessages = Vec::new();

    while pos < bytes.len() {
        if bytes.len() < pos + SubmessageHeader::WIRE_SIZE {
            return Err(WireError::UnexpectedEof {
                needed: SubmessageHeader::WIRE_SIZE,
                offset: pos,
            });
        }
        // Wir lesen den Submessage-Header zuerst; bei Unknown-ID
        // resilient skippen.
        let id_byte = bytes[pos];
        let flags = bytes[pos + 1];
        let mut len_bytes = [0u8; 2];
        len_bytes.copy_from_slice(&bytes[pos + 2..pos + 4]);
        let little_endian = (flags & FLAG_E_LITTLE_ENDIAN) != 0;
        let octets = if little_endian {
            u16::from_le_bytes(len_bytes)
        } else {
            u16::from_be_bytes(len_bytes)
        };
        let body_start = pos + SubmessageHeader::WIRE_SIZE;
        let body_end = if octets == 0 {
            // Last-submessage marker: bis Ende des Datagrams.
            bytes.len()
        } else {
            body_start + octets as usize
        };
        if body_end > bytes.len() {
            return Err(WireError::UnexpectedEof {
                needed: body_end - bytes.len(),
                offset: body_start,
            });
        }
        let body = &bytes[body_start..body_end];
        let sub = match SubmessageId::from_u8(id_byte) {
            Ok(SubmessageId::Data) => {
                let d = DataSubmessage::read_body_with_flags(body, little_endian, flags)?;
                if let Some(pl) = &d.inline_qos {
                    pl.validate_must_understand_in_data_pipeline()?;
                }
                ParsedSubmessage::Data(d)
            }
            Ok(SubmessageId::Heartbeat) => {
                let final_flag = (flags & HEARTBEAT_FLAG_FINAL) != 0;
                let liveliness_flag = (flags & HEARTBEAT_FLAG_LIVELINESS) != 0;
                let group_info_flag = (flags & HEARTBEAT_FLAG_GROUP_INFO) != 0;
                ParsedSubmessage::Heartbeat(HeartbeatSubmessage::read_body(
                    body,
                    little_endian,
                    final_flag,
                    liveliness_flag,
                    group_info_flag,
                )?)
            }
            Ok(SubmessageId::AckNack) => {
                let final_flag = (flags & ACKNACK_FLAG_FINAL) != 0;
                ParsedSubmessage::AckNack(AckNackSubmessage::read_body(
                    body,
                    little_endian,
                    final_flag,
                )?)
            }
            Ok(SubmessageId::Gap) => {
                let group_info_flag = (flags & GAP_FLAG_GROUP_INFO) != 0;
                let filtered_count_flag = (flags & GAP_FLAG_FILTERED_COUNT) != 0;
                ParsedSubmessage::Gap(GapSubmessage::read_body(
                    body,
                    little_endian,
                    group_info_flag,
                    filtered_count_flag,
                )?)
            }
            Ok(SubmessageId::DataFrag) => {
                let inline_qos = (flags & DATA_FRAG_FLAG_INLINE_QOS) != 0;
                let hash_key = (flags & DATA_FRAG_FLAG_HASH_KEY) != 0;
                let key = (flags & DATA_FRAG_FLAG_KEY) != 0;
                let non_standard = (flags & DATA_FRAG_FLAG_NON_STANDARD) != 0;
                ParsedSubmessage::DataFrag(DataFragSubmessage::read_body(
                    body,
                    little_endian,
                    inline_qos,
                    hash_key,
                    key,
                    non_standard,
                )?)
            }
            Ok(SubmessageId::HeartbeatFrag) => ParsedSubmessage::HeartbeatFrag(
                HeartbeatFragSubmessage::read_body(body, little_endian)?,
            ),
            Ok(SubmessageId::NackFrag) => {
                ParsedSubmessage::NackFrag(NackFragSubmessage::read_body(body, little_endian)?)
            }
            Ok(SubmessageId::InfoSrc) => {
                ParsedSubmessage::InfoSource(InfoSourceSubmessage::read_body(body, little_endian)?)
            }
            Ok(SubmessageId::InfoTs) => {
                let invalidate = (flags & INFO_TIMESTAMP_FLAG_INVALIDATE) != 0;
                ParsedSubmessage::InfoTimestamp(InfoTimestampSubmessage::read_body(
                    body,
                    little_endian,
                    invalidate,
                )?)
            }
            Ok(SubmessageId::InfoReply) => {
                let multicast_flag = (flags & INFO_REPLY_FLAG_MULTICAST) != 0;
                ParsedSubmessage::InfoReply(InfoReplySubmessage::read_body(
                    body,
                    little_endian,
                    multicast_flag,
                )?)
            }
            // HeaderExtension (SubmessageId 0x80, ausserhalb der
            // Enum-Range — wir matchen explizit ueber das
            // ID-Byte). Spec §8.3.7.3: HE MUSS direkt nach dem Header
            // stehen (also als erste Submessage). Anderenfalls reject.
            //
            // Vendor-Compat: nur RTPS >= 2.5 hat 0x80 als HE definiert.
            // Bei aelteren Vendoren (z.B. Cyclone-2.1, FastDDS-2.x mit
            // protocol_version=2.1) faellt 0x80 in den Vendor-Specific-
            // Range [0x80, 0xFF] (Spec §8.3.3.2). Solche Submessages
            // werden — sofern Must-Understand-Flag nicht gesetzt —
            // einfach als `Unknown` markiert und uebersprungen.
            Ok(_) | Err(WireError::UnknownSubmessageId { .. })
                if id_byte == SUBMESSAGE_ID_HEADER_EXTENSION
                    && header.protocol_version
                        >= crate::wire_types::ProtocolVersion { major: 2, minor: 5 } =>
            {
                if !submessages.is_empty() {
                    return Err(WireError::ValueOutOfRange {
                        message: "HeaderExtension must appear directly after the RTPS header",
                    });
                }
                let he = HeaderExtension::decode_body(body, flags)?;
                if let Some(pl) = &he.parameters {
                    pl.validate_must_understand_in_data_pipeline()?;
                }
                ParsedSubmessage::HeaderExtension(he)
            }
            // Unbekannte Submessage-ID + Must-Understand-Bit:
            // SPEC §8.3.3.2 / §9.4.5.1 — ganze RTPS-Message
            // verwerfen.
            Ok(_) | Err(WireError::UnknownSubmessageId { .. })
                if (flags & SUBMESSAGE_FLAG_MUST_UNDERSTAND) != 0 =>
            {
                return Err(WireError::ValueOutOfRange {
                    message: "Unknown submessage id with must-understand flag",
                });
            }
            // Andere bekannte Submessage-IDs ohne Decoder
            // (PAD, InfoTs, …): skippen und als Unknown markieren.
            Ok(_) | Err(WireError::UnknownSubmessageId { .. }) => {
                ParsedSubmessage::Unknown { id: id_byte, flags }
            }
            Err(other) => return Err(other),
        };
        submessages.push(sub);
        pos = body_end;
    }

    Ok(ParsedDatagram {
        header,
        submessages,
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
    use super::*;
    use crate::wire_types::{EntityId, GuidPrefix, SequenceNumber, VendorId};
    use alloc::vec;

    fn header() -> RtpsHeader {
        RtpsHeader::new(VendorId::ZERODDS, GuidPrefix::from_bytes([1; 12]))
    }

    fn data_msg(sn: i64, payload: &[u8]) -> DataSubmessage {
        DataSubmessage {
            extra_flags: 0,
            reader_id: EntityId::user_reader_with_key([0xA, 0xB, 0xC]),
            writer_id: EntityId::user_writer_with_key([0x1, 0x2, 0x3]),
            writer_sn: SequenceNumber(sn),
            inline_qos: None,
            key_flag: false,
            non_standard_flag: false,
            serialized_payload: alloc::sync::Arc::from(payload),
        }
    }

    #[test]
    fn encode_decode_single_data_datagram() {
        let h = header();
        let d = data_msg(1, b"hello");
        let bytes = encode_data_datagram(h, &[d.clone()]).unwrap();
        let parsed = decode_datagram(&bytes).unwrap();
        assert_eq!(parsed.header, h);
        assert_eq!(parsed.submessages.len(), 1);
        match &parsed.submessages[0] {
            ParsedSubmessage::Data(decoded) => assert_eq!(decoded, &d),
            other => panic!("expected Data, got {other:?}"),
        }
    }

    #[test]
    fn encode_decode_two_data_submessages() {
        let h = header();
        let d1 = data_msg(1, b"first");
        let d2 = data_msg(2, b"second-payload");
        let bytes = encode_data_datagram(h, &[d1.clone(), d2.clone()]).unwrap();
        let parsed = decode_datagram(&bytes).unwrap();
        assert_eq!(parsed.submessages.len(), 2);
        match (&parsed.submessages[0], &parsed.submessages[1]) {
            (ParsedSubmessage::Data(a), ParsedSubmessage::Data(b)) => {
                assert_eq!(a, &d1);
                assert_eq!(b, &d2);
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn encode_decode_empty_payload() {
        let h = header();
        let d = data_msg(42, b"");
        let bytes = encode_data_datagram(h, &[d.clone()]).unwrap();
        let parsed = decode_datagram(&bytes).unwrap();
        assert_eq!(parsed.submessages.len(), 1);
        match &parsed.submessages[0] {
            ParsedSubmessage::Data(decoded) => {
                assert!(decoded.serialized_payload.is_empty());
                assert_eq!(decoded.writer_sn, SequenceNumber(42));
            }
            other => panic!("expected Data, got {other:?}"),
        }
    }

    #[test]
    fn decode_rejects_invalid_magic() {
        let mut bytes = vec![0u8; 32];
        bytes[..4].copy_from_slice(b"XXXX");
        let res = decode_datagram(&bytes);
        assert!(matches!(res, Err(WireError::InvalidMagic { .. })));
    }

    #[test]
    fn decode_handles_last_submessage_zero_length() {
        // Manuell konstruieren: Header + DATA-SH mit octets=0
        // Body ist DATA-Format (mind. 20 byte + payload).
        let h = header();
        let mut bytes = h.to_bytes().to_vec();
        let d = data_msg(7, b"X");
        let (body, flags) = d.write_body(true);
        // Submessage-Header mit octets=0 (last-marker)
        let sh = SubmessageHeader {
            submessage_id: SubmessageId::Data,
            flags,
            octets_to_next_header: 0,
        };
        bytes.extend_from_slice(&sh.to_bytes());
        bytes.extend_from_slice(&body);
        let parsed = decode_datagram(&bytes).unwrap();
        match &parsed.submessages[0] {
            ParsedSubmessage::Data(decoded) => {
                assert_eq!(decoded, &d);
            }
            other => panic!("expected Data, got {other:?}"),
        }
    }

    #[test]
    fn decode_marks_unknown_submessage_id_without_failing() {
        // Header + Sub-Header mit ID 0x01 (PAD-Submessage). Wir nutzen
        // PAD als Stand-In, weil InfoTs jetzt typed dekodiert wird (R3).
        let h = header();
        let mut bytes = h.to_bytes().to_vec();
        let body = [0u8; 0]; // PAD hat keinen Body
        let sh = SubmessageHeader {
            submessage_id: SubmessageId::Pad,
            flags: FLAG_E_LITTLE_ENDIAN,
            octets_to_next_header: body.len() as u16,
        };
        bytes.extend_from_slice(&sh.to_bytes());
        bytes.extend_from_slice(&body);
        let parsed = decode_datagram(&bytes).unwrap();
        assert_eq!(parsed.submessages.len(), 1);
        match &parsed.submessages[0] {
            ParsedSubmessage::Unknown { id, flags } => {
                assert_eq!(*id, 0x01);
                assert_eq!(*flags, FLAG_E_LITTLE_ENDIAN);
            }
            other => panic!("expected Unknown, got {other:?}"),
        }
    }

    #[test]
    fn decode_heartbeat_preserves_final_and_liveliness_flags() {
        // Regression fuer WP-1.1-Finding: F/L-Flag aus Submessage-Header
        // muss in HeartbeatSubmessage verfuegbar sein.
        let h = header();
        let hb = HeartbeatSubmessage {
            reader_id: crate::wire_types::EntityId::user_reader_with_key([1, 2, 3]),
            writer_id: crate::wire_types::EntityId::user_writer_with_key([4, 5, 6]),
            first_sn: SequenceNumber(1),
            last_sn: SequenceNumber(7),
            count: 42,
            final_flag: true,
            liveliness_flag: true,
            group_info: None,
        };
        let (body, flags) = hb.write_body(true);
        let mut bytes = h.to_bytes().to_vec();
        let sh = SubmessageHeader {
            submessage_id: SubmessageId::Heartbeat,
            flags,
            octets_to_next_header: body.len() as u16,
        };
        bytes.extend_from_slice(&sh.to_bytes());
        bytes.extend_from_slice(&body);
        let parsed = decode_datagram(&bytes).unwrap();
        match &parsed.submessages[0] {
            ParsedSubmessage::Heartbeat(decoded) => {
                assert_eq!(decoded, &hb);
                assert!(decoded.final_flag);
                assert!(decoded.liveliness_flag);
            }
            other => panic!("expected Heartbeat, got {other:?}"),
        }
    }

    #[test]
    fn decode_acknack_preserves_final_flag() {
        let h = header();
        let ack = AckNackSubmessage {
            reader_id: crate::wire_types::EntityId::user_reader_with_key([1, 2, 3]),
            writer_id: crate::wire_types::EntityId::user_writer_with_key([4, 5, 6]),
            reader_sn_state: crate::submessages::SequenceNumberSet {
                bitmap_base: SequenceNumber(1),
                num_bits: 0,
                bitmap: vec![],
            },
            count: 3,
            final_flag: true,
        };
        let (body, flags) = ack.write_body(true);
        let mut bytes = h.to_bytes().to_vec();
        let sh = SubmessageHeader {
            submessage_id: SubmessageId::AckNack,
            flags,
            octets_to_next_header: body.len() as u16,
        };
        bytes.extend_from_slice(&sh.to_bytes());
        bytes.extend_from_slice(&body);
        let parsed = decode_datagram(&bytes).unwrap();
        match &parsed.submessages[0] {
            ParsedSubmessage::AckNack(decoded) => {
                assert_eq!(decoded, &ack);
                assert!(decoded.final_flag);
            }
            other => panic!("expected AckNack, got {other:?}"),
        }
    }

    #[test]
    fn decode_data_frag_preserves_flags_and_payload() {
        let h = header();
        let df = DataFragSubmessage {
            extra_flags: 0,
            reader_id: crate::wire_types::EntityId::user_reader_with_key([1, 2, 3]),
            writer_id: crate::wire_types::EntityId::user_writer_with_key([4, 5, 6]),
            writer_sn: SequenceNumber(7),
            fragment_starting_num: crate::wire_types::FragmentNumber(1),
            fragments_in_submessage: 1,
            fragment_size: 4,
            sample_size: 12,
            serialized_payload: alloc::sync::Arc::<[u8]>::from([0xAA, 0xBB, 0xCC, 0xDD].as_slice()),
            inline_qos_flag: false,
            hash_key_flag: true,
            key_flag: false,
            non_standard_flag: false,
        };
        let (body, flags) = df.write_body(true);
        let mut bytes = h.to_bytes().to_vec();
        let sh = SubmessageHeader {
            submessage_id: SubmessageId::DataFrag,
            flags,
            octets_to_next_header: body.len() as u16,
        };
        bytes.extend_from_slice(&sh.to_bytes());
        bytes.extend_from_slice(&body);
        let parsed = decode_datagram(&bytes).unwrap();
        match &parsed.submessages[0] {
            ParsedSubmessage::DataFrag(decoded) => {
                assert_eq!(decoded, &df);
                assert!(decoded.hash_key_flag);
                assert!(!decoded.inline_qos_flag);
            }
            other => panic!("expected DataFrag, got {other:?}"),
        }
    }

    #[test]
    fn decode_heartbeat_frag_roundtrip() {
        let h = header();
        let hf = HeartbeatFragSubmessage {
            reader_id: crate::wire_types::EntityId::user_reader_with_key([1, 2, 3]),
            writer_id: crate::wire_types::EntityId::user_writer_with_key([4, 5, 6]),
            writer_sn: SequenceNumber(42),
            last_fragment_num: crate::wire_types::FragmentNumber(8),
            count: 3,
        };
        let (body, flags) = hf.write_body(true);
        let mut bytes = h.to_bytes().to_vec();
        let sh = SubmessageHeader {
            submessage_id: SubmessageId::HeartbeatFrag,
            flags,
            octets_to_next_header: body.len() as u16,
        };
        bytes.extend_from_slice(&sh.to_bytes());
        bytes.extend_from_slice(&body);
        let parsed = decode_datagram(&bytes).unwrap();
        match &parsed.submessages[0] {
            ParsedSubmessage::HeartbeatFrag(decoded) => assert_eq!(decoded, &hf),
            other => panic!("expected HeartbeatFrag, got {other:?}"),
        }
    }

    #[test]
    fn decode_nack_frag_roundtrip() {
        let h = header();
        let nf = NackFragSubmessage {
            reader_id: crate::wire_types::EntityId::user_reader_with_key([1, 2, 3]),
            writer_id: crate::wire_types::EntityId::user_writer_with_key([4, 5, 6]),
            writer_sn: SequenceNumber(5),
            fragment_number_state: crate::submessages::FragmentNumberSet {
                bitmap_base: crate::wire_types::FragmentNumber(1),
                num_bits: 4,
                bitmap: vec![0b1010_0000_0000_0000_0000_0000_0000_0000],
            },
            count: 1,
        };
        let (body, flags) = nf.write_body(true);
        let mut bytes = h.to_bytes().to_vec();
        let sh = SubmessageHeader {
            submessage_id: SubmessageId::NackFrag,
            flags,
            octets_to_next_header: body.len() as u16,
        };
        bytes.extend_from_slice(&sh.to_bytes());
        bytes.extend_from_slice(&body);
        let parsed = decode_datagram(&bytes).unwrap();
        match &parsed.submessages[0] {
            ParsedSubmessage::NackFrag(decoded) => assert_eq!(decoded, &nf),
            other => panic!("expected NackFrag, got {other:?}"),
        }
    }

    // ---- WP 1.E Stufe-E/F: InfoSource + InfoReply via Datagram ----

    #[test]
    fn decode_info_source_via_datagram() {
        use crate::wire_types::{GuidPrefix, ProtocolVersion as PV, VendorId};
        let h = header();
        let info = InfoSourceSubmessage {
            unused: 0,
            protocol_version: PV::V2_5,
            vendor_id: VendorId([0xAB, 0xCD]),
            guid_prefix: GuidPrefix::from_bytes([3; 12]),
        };
        let (body, flags) = info.write_body(true);
        let mut bytes = h.to_bytes().to_vec();
        let sh = SubmessageHeader {
            submessage_id: SubmessageId::InfoSrc,
            flags,
            octets_to_next_header: body.len() as u16,
        };
        bytes.extend_from_slice(&sh.to_bytes());
        bytes.extend_from_slice(&body);
        let parsed = decode_datagram(&bytes).unwrap();
        match &parsed.submessages[0] {
            ParsedSubmessage::InfoSource(decoded) => assert_eq!(decoded, &info),
            other => panic!("expected InfoSource, got {other:?}"),
        }
    }

    #[test]
    fn decode_info_reply_with_multicast_via_datagram() {
        use crate::wire_types::Locator;
        let h = header();
        let info = InfoReplySubmessage {
            unicast_locators: alloc::vec![Locator::udp_v4([10, 1, 2, 3], 7411)],
            multicast_locators: Some(alloc::vec![Locator::udp_v4([239, 255, 0, 1], 7400)]),
        };
        let (body, flags) = info.write_body(true);
        let mut bytes = h.to_bytes().to_vec();
        let sh = SubmessageHeader {
            submessage_id: SubmessageId::InfoReply,
            flags,
            octets_to_next_header: body.len() as u16,
        };
        bytes.extend_from_slice(&sh.to_bytes());
        bytes.extend_from_slice(&body);
        let parsed = decode_datagram(&bytes).unwrap();
        match &parsed.submessages[0] {
            ParsedSubmessage::InfoReply(decoded) => assert_eq!(decoded, &info),
            other => panic!("expected InfoReply, got {other:?}"),
        }
    }

    #[test]
    fn decode_rejects_truncated_after_header() {
        let h = header();
        let mut bytes = h.to_bytes().to_vec();
        bytes.extend_from_slice(&[0u8, 0, 0]); // nur 3 Byte Sub-Header statt 4
        let res = decode_datagram(&bytes);
        assert!(matches!(res, Err(WireError::UnexpectedEof { .. })));
    }

    #[test]
    fn decode_header_extension_in_datagram() {
        let h = header();
        let he = crate::header_extension::HeaderExtension {
            little_endian: true,
            message_length: Some(123),
            timestamp: Some(crate::header_extension::HeTimestamp {
                seconds: 1,
                fraction: 2,
            }),
            checksum: crate::header_extension::ChecksumValue::Crc32c(0xDEAD_BEEF),
            ..crate::header_extension::HeaderExtension::default()
        };
        let mut bytes = h.to_bytes().to_vec();
        bytes.extend_from_slice(&he.encode().unwrap());
        let parsed = decode_datagram(&bytes).unwrap();
        assert_eq!(parsed.submessages.len(), 1);
        match &parsed.submessages[0] {
            ParsedSubmessage::HeaderExtension(decoded) => assert_eq!(decoded, &he),
            other => panic!("expected HE, got {other:?}"),
        }
    }

    #[test]
    fn decode_rejects_unknown_submessage_with_must_understand() {
        // Submessage-ID 0x7E mit Must-Understand-Bit (0x80 im flag-byte)
        // → ganze Message verwerfen.
        let h = header();
        let mut bytes = h.to_bytes().to_vec();
        let body = [0u8; 4];
        let sh = SubmessageHeader {
            submessage_id: SubmessageId::Pad, // ID-Wert wird gleich ueberschrieben
            flags: FLAG_E_LITTLE_ENDIAN | SUBMESSAGE_FLAG_MUST_UNDERSTAND,
            octets_to_next_header: body.len() as u16,
        };
        let mut sh_bytes = sh.to_bytes();
        sh_bytes[0] = 0x7E; // unbekannte ID, ausserhalb 0x80-Range
        bytes.extend_from_slice(&sh_bytes);
        bytes.extend_from_slice(&body);
        let res = decode_datagram(&bytes);
        assert!(matches!(
            res,
            Err(WireError::ValueOutOfRange { message: msg }) if msg.contains("must-understand")
        ));
    }

    #[test]
    fn decode_skips_unknown_submessage_without_must_understand() {
        // Ohne Must-Understand-Bit: skippen + als Unknown markieren.
        let h = header();
        let mut bytes = h.to_bytes().to_vec();
        let body = [0u8; 4];
        let sh = SubmessageHeader {
            submessage_id: SubmessageId::Pad,
            flags: FLAG_E_LITTLE_ENDIAN,
            octets_to_next_header: body.len() as u16,
        };
        let mut sh_bytes = sh.to_bytes();
        sh_bytes[0] = 0x7E;
        bytes.extend_from_slice(&sh_bytes);
        bytes.extend_from_slice(&body);
        let parsed = decode_datagram(&bytes).unwrap();
        assert_eq!(parsed.submessages.len(), 1);
        match &parsed.submessages[0] {
            ParsedSubmessage::Unknown { id, .. } => assert_eq!(*id, 0x7E),
            other => panic!("expected Unknown, got {other:?}"),
        }
    }

    #[test]
    fn decode_data_after_header_extension() {
        // Wire-Layout: RtpsHeader || HE || DATA.
        let h = header();
        let he = crate::header_extension::HeaderExtension {
            little_endian: true,
            message_length: Some(0),
            ..crate::header_extension::HeaderExtension::default()
        };
        let d = data_msg(7, b"after-he");
        let mut bytes = h.to_bytes().to_vec();
        bytes.extend_from_slice(&he.encode().unwrap());
        let (body, flags) = d.write_body(true);
        let sh = SubmessageHeader {
            submessage_id: SubmessageId::Data,
            flags,
            octets_to_next_header: body.len() as u16,
        };
        bytes.extend_from_slice(&sh.to_bytes());
        bytes.extend_from_slice(&body);
        let parsed = decode_datagram(&bytes).unwrap();
        assert_eq!(parsed.submessages.len(), 2);
        assert!(matches!(
            &parsed.submessages[0],
            ParsedSubmessage::HeaderExtension(_)
        ));
        assert!(matches!(&parsed.submessages[1], ParsedSubmessage::Data(_)));
    }

    #[test]
    fn decode_rejects_header_extension_after_data_submessage() {
        // Spec §8.3.7.3: HE MUSS direkt nach dem RTPS-Header stehen.
        // Wenn vorher eine andere Submessage parst wurde, reject.
        // Wire-Layout: RtpsHeader || DATA || HE.
        let h = header();
        let d = data_msg(7, b"first");
        let he = crate::header_extension::HeaderExtension {
            little_endian: true,
            message_length: Some(0),
            ..crate::header_extension::HeaderExtension::default()
        };
        let mut bytes = h.to_bytes().to_vec();
        let (dbody, dflags) = d.write_body(true);
        let dsh = SubmessageHeader {
            submessage_id: SubmessageId::Data,
            flags: dflags,
            octets_to_next_header: dbody.len() as u16,
        };
        bytes.extend_from_slice(&dsh.to_bytes());
        bytes.extend_from_slice(&dbody);
        bytes.extend_from_slice(&he.encode().unwrap());
        let res = decode_datagram(&bytes);
        assert!(matches!(res, Err(WireError::ValueOutOfRange { .. })));
    }

    // ---- §9.4.2.11.2 Must-Understand-Bit Reject-Pfad ----

    #[test]
    fn decode_rejects_data_with_unknown_must_understand_pid_in_inline_qos() {
        use crate::parameter_list::{MUST_UNDERSTAND_BIT, Parameter, ParameterList};
        let h = header();
        // Inline-QoS mit unbekannter MU-PID 0x3500 (kein Standard-PID).
        let mut pl = ParameterList::new();
        pl.push(Parameter::new(
            MUST_UNDERSTAND_BIT | 0x3500,
            vec![1, 2, 3, 4],
        ));
        let d = DataSubmessage {
            extra_flags: 0,
            reader_id: EntityId::user_reader_with_key([0xA, 0xB, 0xC]),
            writer_id: EntityId::user_writer_with_key([0x1, 0x2, 0x3]),
            writer_sn: SequenceNumber(1),
            inline_qos: Some(pl),
            key_flag: false,
            non_standard_flag: false,
            serialized_payload: alloc::sync::Arc::from([] as [u8; 0]),
        };
        let mut bytes = h.to_bytes().to_vec();
        let (body, flags) = d.write_body(true);
        let sh = SubmessageHeader {
            submessage_id: SubmessageId::Data,
            flags,
            octets_to_next_header: body.len() as u16,
        };
        bytes.extend_from_slice(&sh.to_bytes());
        bytes.extend_from_slice(&body);
        let res = decode_datagram(&bytes);
        assert!(matches!(res, Err(WireError::ValueOutOfRange { .. })));
    }

    #[test]
    fn decode_accepts_data_with_known_must_understand_pid_in_inline_qos() {
        use crate::parameter_list::{MUST_UNDERSTAND_BIT, Parameter, ParameterList, pid};
        let h = header();
        let mut pl = ParameterList::new();
        // KEY_HASH ist Standard-PID, MU-Bit erlaubt.
        pl.push(Parameter::new(
            MUST_UNDERSTAND_BIT | pid::KEY_HASH,
            vec![0; 16],
        ));
        let d = DataSubmessage {
            extra_flags: 0,
            reader_id: EntityId::user_reader_with_key([0xA, 0xB, 0xC]),
            writer_id: EntityId::user_writer_with_key([0x1, 0x2, 0x3]),
            writer_sn: SequenceNumber(2),
            inline_qos: Some(pl),
            key_flag: false,
            non_standard_flag: false,
            serialized_payload: alloc::sync::Arc::from([] as [u8; 0]),
        };
        let mut bytes = h.to_bytes().to_vec();
        let (body, flags) = d.write_body(true);
        let sh = SubmessageHeader {
            submessage_id: SubmessageId::Data,
            flags,
            octets_to_next_header: body.len() as u16,
        };
        bytes.extend_from_slice(&sh.to_bytes());
        bytes.extend_from_slice(&body);
        decode_datagram(&bytes).expect("known MU PID should pass");
    }

    #[test]
    fn decode_accepts_vendor_specific_must_understand_pid() {
        use crate::parameter_list::{
            MUST_UNDERSTAND_BIT, Parameter, ParameterList, VENDOR_SPECIFIC_BIT,
        };
        let h = header();
        let mut pl = ParameterList::new();
        // Vendor-spezifische MU-PID — Spec §9.6.2 erlaubt ignorieren.
        pl.push(Parameter::new(
            MUST_UNDERSTAND_BIT | VENDOR_SPECIFIC_BIT | 0x0050,
            vec![0xCA, 0xFE, 0xBA, 0xBE],
        ));
        let d = DataSubmessage {
            extra_flags: 0,
            reader_id: EntityId::user_reader_with_key([0xA, 0xB, 0xC]),
            writer_id: EntityId::user_writer_with_key([0x1, 0x2, 0x3]),
            writer_sn: SequenceNumber(3),
            inline_qos: Some(pl),
            key_flag: false,
            non_standard_flag: false,
            serialized_payload: alloc::sync::Arc::from([] as [u8; 0]),
        };
        let mut bytes = h.to_bytes().to_vec();
        let (body, flags) = d.write_body(true);
        let sh = SubmessageHeader {
            submessage_id: SubmessageId::Data,
            flags,
            octets_to_next_header: body.len() as u16,
        };
        bytes.extend_from_slice(&sh.to_bytes());
        bytes.extend_from_slice(&body);
        decode_datagram(&bytes).expect("vendor-specific MU PID should pass");
    }

    #[test]
    fn rtps_2_1_treats_0x80_as_vendor_specific_not_header_extension() {
        use crate::wire_types::ProtocolVersion;
        // Spec §8.3.7.3: HeaderExtension (0x80) ist ein 2.5-Submessage.
        // Aeltere Vendoren (z.B. Cyclone-2.1) duerfen 0x80 als Vendor-
        // Specific verwenden. Wir verwerfen NICHT, sondern skippen.
        let mut h = header();
        h.protocol_version = ProtocolVersion::V2_1;
        let mut bytes = h.to_bytes().to_vec();
        // Erst eine echte DATA-Submessage (damit `submessages` non-empty
        // wird), dann 0x80 als Vendor-Submessage hinten dran.
        let d = data_msg(1, b"x");
        let (body, flags) = d.write_body(true);
        let sh = SubmessageHeader {
            submessage_id: SubmessageId::Data,
            flags,
            octets_to_next_header: body.len() as u16,
        };
        bytes.extend_from_slice(&sh.to_bytes());
        bytes.extend_from_slice(&body);
        // Vendor-Submessage 0x80 mit 4 Byte Body, kein MU-Bit.
        bytes.extend_from_slice(&[0x80, FLAG_E_LITTLE_ENDIAN, 4, 0]);
        bytes.extend_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF]);
        let parsed = decode_datagram(&bytes).expect("0x80 unter RTPS-2.1 muss skippen");
        assert!(matches!(
            parsed.submessages.last(),
            Some(ParsedSubmessage::Unknown { id: 0x80, .. })
        ));
    }
}
