// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! MQTT v5.0 PUBLISH-Packet Codec — Spec §3.3.

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use crate::data_types::{
    DataTypeError, decode_two_byte_int, decode_utf8_string, encode_two_byte_int, encode_utf8_string,
};
use crate::packet::{ControlPacketType, FixedHeader};
use crate::vbi::{VbiError, decode_vbi, encode_vbi};

/// Codec-Fehler fuer PUBLISH-Encode/Decode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodecError {
    /// VBI-Encode/Decode-Fehler.
    Vbi(VbiError),
    /// Data-Type-Fehler.
    DataType(DataTypeError),
    /// Header-Byte fehlt.
    HeaderTooShort,
    /// Wrong packet type fuer den Decoder (z.B. CONNECT-Bytes an
    /// `decode_publish` uebergeben).
    WrongPacketType(u8),
    /// Spec §3.3.2.2 — Packet Identifier muss bei QoS > 0 vorhanden
    /// sein.
    MissingPacketIdentifier,
    /// Spec §3.3.1.2 — QoS-Wert 3 ist Malformed Packet.
    InvalidQoS(u8),
    /// Spec §2.1.4 — Remaining-Length groesser als verfuegbare Bytes.
    RemainingLengthMismatch,
}

impl From<VbiError> for CodecError {
    fn from(e: VbiError) -> Self {
        Self::Vbi(e)
    }
}

impl From<DataTypeError> for CodecError {
    fn from(e: DataTypeError) -> Self {
        Self::DataType(e)
    }
}

impl fmt::Display for CodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Vbi(e) => write!(f, "VBI: {e}"),
            Self::DataType(e) => write!(f, "data type: {e}"),
            Self::HeaderTooShort => f.write_str("packet header too short"),
            Self::WrongPacketType(t) => write!(f, "wrong packet type {t}"),
            Self::MissingPacketIdentifier => f.write_str("missing packet identifier"),
            Self::InvalidQoS(q) => write!(f, "invalid QoS {q}"),
            Self::RemainingLengthMismatch => f.write_str("remaining length exceeds bytes"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for CodecError {}

/// PUBLISH-Packet (Spec §3.3) — vereinfachte Form ohne Properties
/// (Properties werden als opaque Bytes durchgereicht).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishPacket {
    /// Spec §3.3.1.1 — DUP flag.
    pub dup: bool,
    /// Spec §3.3.1.2 — QoS level (0..=2).
    pub qos: u8,
    /// Spec §3.3.1.3 — RETAIN flag.
    pub retain: bool,
    /// Spec §3.3.2.1 — Topic Name.
    pub topic: String,
    /// Spec §3.3.2.2 — Packet Identifier (`Some` nur wenn `qos > 0`).
    pub packet_id: Option<u16>,
    /// Spec §3.3.2.3 — Properties (raw VBI-prefixed property block).
    /// Caller kann Properties separat parsen via [`crate::properties`].
    pub properties: Vec<u8>,
    /// Spec §3.3.3 — Application Message Payload.
    pub payload: Vec<u8>,
}

/// Encodiert ein PUBLISH-Packet zum Wire-Format.
///
/// # Errors
/// * `InvalidQoS(q)` wenn `qos > 2`.
/// * `MissingPacketIdentifier` wenn `qos > 0 && packet_id.is_none()`.
/// * VBI-/DataType-Fehler bei Topic/Payload-Length-Limits.
pub fn encode_publish(p: &PublishPacket) -> Result<Vec<u8>, CodecError> {
    if p.qos > 2 {
        return Err(CodecError::InvalidQoS(p.qos));
    }
    if p.qos > 0 && p.packet_id.is_none() {
        return Err(CodecError::MissingPacketIdentifier);
    }

    // Variable Header.
    let mut var_header = encode_utf8_string(&p.topic)?;
    if p.qos > 0 {
        let id = p.packet_id.ok_or(CodecError::MissingPacketIdentifier)?;
        var_header.extend_from_slice(&encode_two_byte_int(id));
    }
    // Properties (Spec §2.2.2.1) — wir nehmen das raw-Bytes-Feld als
    // Property-Block-Body; der Encoder schreibt VBI(len) + body. Caller
    // muss kein VBI-Prefix mitliefern, sondern nur die rohen Property-
    // Bytes nach der VBI-Length.
    let prop_len_u32 =
        u32::try_from(p.properties.len()).map_err(|_| CodecError::Vbi(VbiError::Malformed))?;
    let prop_len_vbi = encode_vbi(prop_len_u32).ok_or(CodecError::Vbi(VbiError::Malformed))?;
    var_header.extend_from_slice(&prop_len_vbi);
    var_header.extend_from_slice(&p.properties);

    // Payload.
    let mut body = var_header;
    body.extend_from_slice(&p.payload);

    // Fixed Header.
    let mut flags = 0u8;
    if p.dup {
        flags |= 0b1000;
    }
    flags |= (p.qos & 0b11) << 1;
    if p.retain {
        flags |= 0b0001;
    }
    let byte0 = (ControlPacketType::Publish.to_bits() << 4) | (flags & 0x0F);
    let mut out = Vec::with_capacity(1 + 4 + body.len());
    out.push(byte0);
    #[allow(clippy::cast_possible_truncation)]
    let remaining_length =
        u32::try_from(body.len()).map_err(|_| CodecError::Vbi(VbiError::Malformed))?;
    let vbi_bytes = encode_vbi(remaining_length).ok_or(CodecError::Vbi(VbiError::Malformed))?;
    out.extend_from_slice(&vbi_bytes);
    out.extend_from_slice(&body);
    Ok(out)
}

/// Decodiert ein PUBLISH-Packet aus dem Wire-Format.
///
/// # Errors
/// Siehe [`CodecError`].
pub fn decode_publish(bytes: &[u8]) -> Result<(FixedHeader, PublishPacket), CodecError> {
    if bytes.is_empty() {
        return Err(CodecError::HeaderTooShort);
    }
    let byte0 = bytes[0];
    let packet_type_bits = (byte0 >> 4) & 0x0F;
    if packet_type_bits != ControlPacketType::Publish.to_bits() {
        return Err(CodecError::WrongPacketType(packet_type_bits));
    }
    let flags = byte0 & 0x0F;
    let qos = (flags >> 1) & 0b11;
    if qos > 2 {
        return Err(CodecError::InvalidQoS(qos));
    }
    let dup = flags & 0b1000 != 0;
    let retain = flags & 0b0001 != 0;

    let (remaining_length, vbi_used) = decode_vbi(&bytes[1..])?;
    let header_total = 1 + vbi_used;
    let body_end = header_total + remaining_length as usize;
    if bytes.len() < body_end {
        return Err(CodecError::RemainingLengthMismatch);
    }
    let body = &bytes[header_total..body_end];

    // Variable Header.
    let mut cursor = 0usize;
    let (topic, used) = decode_utf8_string(&body[cursor..])?;
    cursor += used;
    let packet_id = if qos > 0 {
        let (id, used) = decode_two_byte_int(&body[cursor..])?;
        cursor += used;
        Some(id)
    } else {
        None
    };
    // Property-Length VBI. Wir normalisieren leere Property-Blocks
    // (VBI=0) zu `Vec::new()` damit Round-Trip mit empty-properties-
    // Input identisch ist; Caller, der raw-Properties haben moechte,
    // kann die VBI manuell parsen.
    let (prop_len, prop_vbi_used) = decode_vbi(&body[cursor..])?;
    cursor += prop_vbi_used;
    let prop_data_end = cursor + prop_len as usize;
    if body.len() < prop_data_end {
        return Err(CodecError::RemainingLengthMismatch);
    }
    let properties = if prop_len == 0 {
        Vec::new()
    } else {
        body[cursor..prop_data_end].to_vec()
    };
    cursor = prop_data_end;

    // Payload.
    let payload = body[cursor..].to_vec();

    let header = FixedHeader {
        packet_type: ControlPacketType::Publish,
        flags,
        remaining_length,
    };
    Ok((
        header,
        PublishPacket {
            dup,
            qos,
            retain,
            topic,
            packet_id,
            properties,
            payload,
        },
    ))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn publish_qos0_no_packet_id_round_trip() {
        // Spec §3.3.2.2 — QoS=0 → kein Packet Identifier.
        let p = PublishPacket {
            dup: false,
            qos: 0,
            retain: false,
            topic: String::from("sensors/temp"),
            packet_id: None,
            properties: Vec::new(),
            payload: alloc::vec![0xDE, 0xAD],
        };
        let bytes = encode_publish(&p).expect("encode");
        let (hdr, parsed) = decode_publish(&bytes).expect("decode");
        assert_eq!(parsed, p);
        assert_eq!(hdr.packet_type, ControlPacketType::Publish);
        assert!(!hdr.dup_flag());
        assert_eq!(hdr.qos(), 0);
    }

    #[test]
    fn publish_qos1_includes_packet_id_round_trip() {
        // Spec §3.3.2.2.
        let p = PublishPacket {
            dup: true,
            qos: 1,
            retain: true,
            topic: String::from("foo"),
            packet_id: Some(0x1234),
            properties: Vec::new(),
            payload: b"hello".to_vec(),
        };
        let bytes = encode_publish(&p).expect("encode");
        let (_, parsed) = decode_publish(&bytes).expect("decode");
        assert_eq!(parsed, p);
    }

    #[test]
    fn publish_qos2_round_trip() {
        let p = PublishPacket {
            dup: false,
            qos: 2,
            retain: false,
            topic: String::from("a/b/c"),
            packet_id: Some(42),
            properties: Vec::new(),
            payload: alloc::vec![1, 2, 3, 4, 5],
        };
        let bytes = encode_publish(&p).expect("encode");
        let (_, parsed) = decode_publish(&bytes).expect("decode");
        assert_eq!(parsed.packet_id, Some(42));
        assert_eq!(parsed.qos, 2);
    }

    #[test]
    fn invalid_qos_3_rejected_on_encode() {
        // Spec §3.3.1.2 — QoS=3 ist Malformed Packet.
        let mut p = PublishPacket {
            dup: false,
            qos: 3,
            retain: false,
            topic: String::from("x"),
            packet_id: None,
            properties: Vec::new(),
            payload: Vec::new(),
        };
        assert_eq!(encode_publish(&p), Err(CodecError::InvalidQoS(3)));
        p.qos = 2;
        p.packet_id = Some(1);
        assert!(encode_publish(&p).is_ok());
    }

    #[test]
    fn missing_packet_id_at_qos1_rejected() {
        // Spec §3.3.2.2.
        let p = PublishPacket {
            dup: false,
            qos: 1,
            retain: false,
            topic: String::from("x"),
            packet_id: None,
            properties: Vec::new(),
            payload: Vec::new(),
        };
        assert_eq!(encode_publish(&p), Err(CodecError::MissingPacketIdentifier));
    }

    #[test]
    fn wrong_packet_type_rejected_on_decode() {
        // Byte 0 = CONNECT (1) → decode_publish lehnt ab.
        let bytes = [0x10u8, 0x02, 0, 0];
        match decode_publish(&bytes) {
            Err(CodecError::WrongPacketType(1)) => {}
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn fixed_header_first_byte_layout_for_publish() {
        // Spec §2.1 — Type-Nibble 3 + Flags-Nibble.
        let p = PublishPacket {
            dup: true,
            qos: 2,
            retain: true,
            topic: String::from("t"),
            packet_id: Some(1),
            properties: Vec::new(),
            payload: Vec::new(),
        };
        let bytes = encode_publish(&p).expect("encode");
        // Type=3 (high nibble), Flags = DUP|QoS=2|RETAIN = 1101 = 0xD.
        assert_eq!(bytes[0], 0x3D);
    }

    #[test]
    fn empty_properties_round_trips_as_empty_vec() {
        // Decoder normalisiert empty-property-block → empty Vec.
        let p = PublishPacket {
            dup: false,
            qos: 0,
            retain: false,
            topic: String::from("t"),
            packet_id: None,
            properties: Vec::new(),
            payload: alloc::vec![1],
        };
        let bytes = encode_publish(&p).expect("encode");
        let (_, parsed) = decode_publish(&bytes).expect("decode");
        assert!(parsed.properties.is_empty());
    }

    #[test]
    fn non_empty_properties_round_trip_preserves_bytes() {
        // Caller liefert raw Property-Block-Body (kein VBI-Prefix).
        // Beispiel-Block: PayloadFormatIndicator(0x01)=0x01 +
        // ReceiveMaximum(0x21)=0x000A.
        let raw_props_payload = alloc::vec![0x01u8, 0x01, 0x21, 0x00, 0x0A];
        let p = PublishPacket {
            dup: false,
            qos: 0,
            retain: false,
            topic: String::from("t"),
            packet_id: None,
            properties: raw_props_payload.clone(),
            payload: alloc::vec![],
        };
        let bytes = encode_publish(&p).expect("encode");
        let (_, parsed) = decode_publish(&bytes).expect("decode");
        assert_eq!(parsed.properties, raw_props_payload);
    }

    #[test]
    fn truncated_remaining_length_decode_fails() {
        // Header sagt Remaining=10 aber nur 4 Body-Bytes.
        let bytes = [0x30u8, 0x0A, 0, 1, b'x'];
        assert_eq!(
            decode_publish(&bytes),
            Err(CodecError::RemainingLengthMismatch)
        );
    }

    #[test]
    fn empty_input_decode_fails() {
        assert_eq!(decode_publish(&[]), Err(CodecError::HeaderTooShort));
    }

    #[test]
    fn large_payload_encodes_multibyte_remaining_length() {
        // Spec §2.1.4 — VBI Remaining-Length, 200 → 1 byte (200 < 128
        // nein → 200 >= 128 → 2 bytes).
        let p = PublishPacket {
            dup: false,
            qos: 0,
            retain: false,
            topic: String::from("t"),
            packet_id: None,
            properties: Vec::new(),
            payload: alloc::vec![0xAB; 200],
        };
        let bytes = encode_publish(&p).expect("encode");
        // bytes[0] = 0x30 (publish), bytes[1..3] = VBI fuer ~204.
        assert_eq!(bytes[0], 0x30);
        // Wir kontrollieren nicht den exakten Wert (variable je nach
        // Topic-Length), aber dass Round-Trip funktioniert.
        let (_, parsed) = decode_publish(&bytes).expect("decode");
        assert_eq!(parsed.payload.len(), 200);
    }
}
