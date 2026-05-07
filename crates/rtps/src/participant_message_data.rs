// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `ParticipantMessageData` Wire-Encoding (DDSI-RTPS 2.5 §9.6.3.1).
//!
//! Payload-Struktur des Writer-Liveliness-Protocols (WLP, §8.4.13).
//! Wird vom `BUILTIN_PARTICIPANT_MESSAGE_WRITER` als DATA-Submessage
//! im Topic `DCPSParticipantMessage` periodisch publiziert. Reader
//! nutzen den Empfang als implicit `assert_liveliness()` und treiben
//! damit das Lease-Tracking pro Peer-Participant.
//!
//! # Wire-Layout (§9.6.3.1)
//!
//! ```text
//! struct ParticipantMessageData {
//!     GUID_t   participantGuidPrefix; // 12 byte (GuidPrefix only!)
//!     octet    kind[4];               // 4 byte u32 (BE/LE per CDR)
//!     sequence<octet> data;           // 4 byte length + N byte
//! };
//! ```
//!
//! Spec-Pitfall: trotz Feldname `participantGuidPrefix` wird in der
//! Praxis von Cyclone DDS und Fast-DDS ein voller 16-Byte-Guid
//! geschickt (Prefix + ENTITYID_PARTICIPANT). Wir schreiben deshalb
//! 16 Byte und parsen tolerant: 16 Byte → Voll-Guid, 12 Byte →
//! Prefix-Only.
//!
//! Die `data`-Sequenz ist semantisch eine `vec<octet>` mit voran-
//! gestellter 32-Bit-Laenge. Spec §9.6.3.1 definiert sie als opaque
//! Token; ZeroDDS nutzt sie fuer MANUAL_BY_TOPIC, um den Topic-Token
//! zu transportieren (Topic-Hash-Fingerprint).
//!
//! # CDR-Encoding
//!
//! Encoded als XCDR1 Plain (Encapsulation 0x0000 BE / 0x0001 LE)
//! bzw. XCDR2 Plain (0x0006 BE / 0x0007 LE). Cyclone schickt das
//! Topic per Default als XCDR1 Plain. Wir akzeptieren alle vier
//! Encapsulation-Kinds und schreiben per Default LE.
//!
//! # DoS-Caps
//!
//! `data.len()` ist auf [`MAX_DATA_LEN`] = 4096 Byte gedeckelt.
//! Encoder kuerzt nicht — Caller muss vor Aufruf cappen — Decoder
//! lehnt ueberlange Daten mit [`WireError::ValueOutOfRange`] ab.

extern crate alloc;
use alloc::vec::Vec;

use crate::error::WireError;
use crate::wire_types::GuidPrefix;

/// CDR-Encapsulation-Header: XCDR1 Plain Big-Endian (`0x0000`).
pub const ENCAPSULATION_CDR_BE: [u8; 2] = [0x00, 0x00];
/// CDR-Encapsulation-Header: XCDR1 Plain Little-Endian (`0x0001`).
pub const ENCAPSULATION_CDR_LE: [u8; 2] = [0x00, 0x01];
/// CDR-Encapsulation-Header: XCDR2 Plain Big-Endian (`0x0006`).
pub const ENCAPSULATION_CDR2_BE: [u8; 2] = [0x00, 0x06];
/// CDR-Encapsulation-Header: XCDR2 Plain Little-Endian (`0x0007`).
pub const ENCAPSULATION_CDR2_LE: [u8; 2] = [0x00, 0x07];

/// DoS-Cap fuer `data`-Sequenz (Topic-Token / vendor opaque).
/// Bewusst klein gewaehlt — WLP-Heartbeats sollen leicht sein, ein
/// Peer der mehr schickt ist entweder buggy oder bösartig.
pub const MAX_DATA_LEN: usize = 4096;

/// Spec-defined `kind` Code: AUTOMATIC_LIVELINESS_UPDATE (§9.6.3.1).
/// Wird vom Builtin-WLP-Writer geschickt, wenn LIVELINESS=AUTOMATIC.
pub const PARTICIPANT_MESSAGE_DATA_KIND_AUTOMATIC_LIVELINESS_UPDATE: u32 = 0x0000_0000;

/// Spec-defined `kind` Code: MANUAL_BY_PARTICIPANT_LIVELINESS_UPDATE
/// (§9.6.3.1). Getrigger durch `assert_liveliness()` auf
/// `DomainParticipant`.
pub const PARTICIPANT_MESSAGE_DATA_KIND_MANUAL_BY_PARTICIPANT_LIVELINESS_UPDATE: u32 = 0x0000_0001;

/// Vendor-Specific-Kind-Range: oberstes Bit gesetzt
/// (`0x80000000..=0xFFFFFFFF`). Spec §9.6.3.1 reserviert dies fuer
/// Vendor-eigene Kinds (z.B. ZeroDDS-MANUAL_BY_TOPIC mit Topic-Token
/// in der `data`-Sequenz).
pub const PARTICIPANT_MESSAGE_DATA_KIND_VENDOR_BASE: u32 = 0x8000_0000;

/// ZeroDDS-Vendor-Kind: MANUAL_BY_TOPIC. Wird durch
/// `DataWriter::assert_liveliness()` getriggert; die `data`-Sequenz
/// traegt einen Topic-Token (typisch 4-Byte-Hash). Cyclone-Peers
/// ignorieren das Vendor-Kind (Spec §9.6.3.1: "If kind has its
/// MSB set, implementations not understanding the kind shall ignore
/// the message").
pub const PARTICIPANT_MESSAGE_DATA_KIND_ZERODDS_MANUAL_BY_TOPIC: u32 = 0x8000_0001;

/// `ParticipantMessageData` (DDSI-RTPS 2.5 §9.6.3.1) — Payload des
/// `DCPSParticipantMessage`-Topics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParticipantMessageData {
    /// 16-Byte GUID des Senders (Prefix + EntityId::PARTICIPANT).
    /// Auch wenn die Spec "Prefix" sagt, schreiben Cyclone und Fast-
    /// DDS hier den vollen Guid; wir folgen.
    pub participant_guid: [u8; 16],
    /// Liveliness-Kind (siehe `PARTICIPANT_MESSAGE_DATA_KIND_*`).
    pub kind: u32,
    /// Opaque Token. Bei MANUAL_BY_TOPIC Topic-Hash, sonst leer.
    pub data: Vec<u8>,
}

impl ParticipantMessageData {
    /// Konstruktor fuer AUTOMATIC-Heartbeat (data leer).
    #[must_use]
    pub fn automatic(prefix: GuidPrefix) -> Self {
        Self {
            participant_guid: full_guid_bytes(prefix),
            kind: PARTICIPANT_MESSAGE_DATA_KIND_AUTOMATIC_LIVELINESS_UPDATE,
            data: Vec::new(),
        }
    }

    /// Konstruktor fuer MANUAL_BY_PARTICIPANT-Heartbeat (data leer).
    #[must_use]
    pub fn manual_by_participant(prefix: GuidPrefix) -> Self {
        Self {
            participant_guid: full_guid_bytes(prefix),
            kind: PARTICIPANT_MESSAGE_DATA_KIND_MANUAL_BY_PARTICIPANT_LIVELINESS_UPDATE,
            data: Vec::new(),
        }
    }

    /// Konstruktor fuer ZeroDDS-MANUAL_BY_TOPIC (data = Topic-Token).
    #[must_use]
    pub fn manual_by_topic(prefix: GuidPrefix, topic_token: Vec<u8>) -> Self {
        Self {
            participant_guid: full_guid_bytes(prefix),
            kind: PARTICIPANT_MESSAGE_DATA_KIND_ZERODDS_MANUAL_BY_TOPIC,
            data: topic_token,
        }
    }

    /// Encoded zu CDR-Bytes (mit 4-byte Encapsulation-Header).
    /// `little_endian = true` → `ENCAPSULATION_CDR_LE`, sonst BE.
    ///
    /// # Errors
    /// `WireError::ValueOutOfRange` wenn `data.len() > MAX_DATA_LEN`.
    pub fn to_cdr(&self, little_endian: bool) -> Result<Vec<u8>, WireError> {
        if self.data.len() > MAX_DATA_LEN {
            return Err(WireError::ValueOutOfRange {
                message: "ParticipantMessageData.data exceeds MAX_DATA_LEN",
            });
        }
        let data_len_u32 =
            u32::try_from(self.data.len()).map_err(|_| WireError::ValueOutOfRange {
                message: "ParticipantMessageData.data length exceeds u32",
            })?;
        let mut out = Vec::with_capacity(4 + 16 + 4 + 4 + self.data.len());
        // Encapsulation-Header
        if little_endian {
            out.extend_from_slice(&ENCAPSULATION_CDR_LE);
        } else {
            out.extend_from_slice(&ENCAPSULATION_CDR_BE);
        }
        out.extend_from_slice(&[0, 0]); // options
        // Body Start (CDR-Offset 0)
        // GUID — 16 byte raw, kein Endian-Swap (Bytes sind opaque).
        out.extend_from_slice(&self.participant_guid);
        // kind — 4 byte u32 BE/LE
        let kind_bytes = if little_endian {
            self.kind.to_le_bytes()
        } else {
            self.kind.to_be_bytes()
        };
        out.extend_from_slice(&kind_bytes);
        // data: u32 length + N byte
        let len_bytes = if little_endian {
            data_len_u32.to_le_bytes()
        } else {
            data_len_u32.to_be_bytes()
        };
        out.extend_from_slice(&len_bytes);
        out.extend_from_slice(&self.data);
        Ok(out)
    }

    /// Decoded aus CDR-Bytes (mit Encapsulation-Header).
    ///
    /// Akzeptiert `0x0000`/`0x0001` (XCDR1 Plain) und `0x0006`/`0x0007`
    /// (XCDR2 Plain). Andere Encapsulation-Kinds → Fehler.
    ///
    /// Tolerant gegenueber 12-Byte-Prefix-Only-Kodierung (fuellt mit
    /// 0 auf 16 Byte auf).
    ///
    /// # Errors
    /// - `UnsupportedEncapsulation` bei nicht-CDR-Encapsulation
    /// - `UnexpectedEof` wenn Bytes zu kurz fuer Header / Body
    /// - `ValueOutOfRange` wenn `data.len > MAX_DATA_LEN`
    pub fn from_cdr(bytes: &[u8]) -> Result<Self, WireError> {
        if bytes.len() < 4 {
            return Err(WireError::UnexpectedEof {
                needed: 4,
                offset: 0,
            });
        }
        let little_endian = match (bytes[0], bytes[1]) {
            (0x00, 0x00) | (0x00, 0x06) => false,
            (0x00, 0x01) | (0x00, 0x07) => true,
            (a, b) => {
                return Err(WireError::UnsupportedEncapsulation { kind: [a, b] });
            }
        };
        // Body startet bei Offset 4 (nach options).
        let body = &bytes[4..];
        // Wir akzeptieren 16 Byte Guid (Spec-de-facto) ODER 12 Byte
        // Prefix-Only (alternative Lesart der Spec-Schreibweise
        // "GUID_t guidPrefix" — strenge 12-Byte-Encoder existieren).
        // Heuristik: 16 Byte ist Default, 12 Byte ist nur dann gueltig,
        // wenn die Folge-Felder (kind + data-len) sich mit 12 Byte
        // korrekt parsen.
        let (guid_bytes, after_guid_offset) = parse_guid(body)?;
        if body.len() < after_guid_offset + 4 {
            return Err(WireError::UnexpectedEof {
                needed: after_guid_offset + 4,
                offset: 4,
            });
        }
        let kind_slice = &body[after_guid_offset..after_guid_offset + 4];
        let mut kind_arr = [0u8; 4];
        kind_arr.copy_from_slice(kind_slice);
        let kind = if little_endian {
            u32::from_le_bytes(kind_arr)
        } else {
            u32::from_be_bytes(kind_arr)
        };
        let len_offset = after_guid_offset + 4;
        if body.len() < len_offset + 4 {
            return Err(WireError::UnexpectedEof {
                needed: len_offset + 4,
                offset: 4,
            });
        }
        let mut len_arr = [0u8; 4];
        len_arr.copy_from_slice(&body[len_offset..len_offset + 4]);
        let data_len = if little_endian {
            u32::from_le_bytes(len_arr)
        } else {
            u32::from_be_bytes(len_arr)
        } as usize;
        if data_len > MAX_DATA_LEN {
            return Err(WireError::ValueOutOfRange {
                message: "ParticipantMessageData.data exceeds MAX_DATA_LEN",
            });
        }
        let data_offset = len_offset + 4;
        if body.len() < data_offset + data_len {
            return Err(WireError::UnexpectedEof {
                needed: data_offset + data_len,
                offset: 4,
            });
        }
        let data = body[data_offset..data_offset + data_len].to_vec();
        Ok(Self {
            participant_guid: guid_bytes,
            kind,
            data,
        })
    }

    /// Liefert den GuidPrefix (erste 12 Byte).
    #[must_use]
    pub fn prefix(&self) -> GuidPrefix {
        let mut p = [0u8; 12];
        p.copy_from_slice(&self.participant_guid[..12]);
        GuidPrefix::from_bytes(p)
    }

    /// `true` wenn der `kind`-Wert vendor-spezifisch ist (MSB gesetzt).
    #[must_use]
    pub fn is_vendor_kind(&self) -> bool {
        self.kind >= PARTICIPANT_MESSAGE_DATA_KIND_VENDOR_BASE
    }
}

fn full_guid_bytes(prefix: GuidPrefix) -> [u8; 16] {
    let mut g = [0u8; 16];
    g[..12].copy_from_slice(&prefix.to_bytes());
    // EntityId::PARTICIPANT = [0, 0, 1, 0xC1]
    g[12] = 0;
    g[13] = 0;
    g[14] = 1;
    g[15] = 0xC1;
    g
}

/// Parsed den GUID-Body. Versucht zuerst 16 Byte (Cyclone-/Fast-DDS-
/// Default), fallt zurueck auf 12 Byte (strenge Spec-Lesart).
fn parse_guid(body: &[u8]) -> Result<([u8; 16], usize), WireError> {
    // 16-Byte-Variante: braucht mindestens 16 + 4 (kind) + 4 (data-len).
    if body.len() >= 24 {
        let mut g = [0u8; 16];
        g.copy_from_slice(&body[..16]);
        return Ok((g, 16));
    }
    // 12-Byte-Variante: braucht mindestens 12 + 4 + 4.
    if body.len() >= 20 {
        let mut g = [0u8; 16];
        g[..12].copy_from_slice(&body[..12]);
        // EntityId-Default: PARTICIPANT
        g[14] = 1;
        g[15] = 0xC1;
        return Ok((g, 12));
    }
    Err(WireError::UnexpectedEof {
        needed: 24,
        offset: 4,
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
    use super::*;
    use alloc::vec;

    fn sample_prefix() -> GuidPrefix {
        GuidPrefix::from_bytes([0xA, 0xB, 0xC, 0xD, 1, 2, 3, 4, 5, 6, 7, 8])
    }

    #[test]
    fn participant_message_data_automatic_default_data_empty() {
        let m = ParticipantMessageData::automatic(sample_prefix());
        assert_eq!(
            m.kind,
            PARTICIPANT_MESSAGE_DATA_KIND_AUTOMATIC_LIVELINESS_UPDATE
        );
        assert!(m.data.is_empty());
        assert_eq!(m.prefix(), sample_prefix());
    }

    #[test]
    fn participant_message_data_kind_constants_match_spec() {
        // §9.6.3.1: AUTOMATIC = 0x00000000, MANUAL_BY_PARTICIPANT = 0x00000001.
        // Vendor-Range: MSB gesetzt, also >= 0x80000000.
        assert_eq!(
            PARTICIPANT_MESSAGE_DATA_KIND_AUTOMATIC_LIVELINESS_UPDATE,
            0x0000_0000
        );
        assert_eq!(
            PARTICIPANT_MESSAGE_DATA_KIND_MANUAL_BY_PARTICIPANT_LIVELINESS_UPDATE,
            0x0000_0001
        );
        assert_eq!(PARTICIPANT_MESSAGE_DATA_KIND_VENDOR_BASE, 0x8000_0000);
        // ZeroDDS-Vendor-Kind muss im Vendor-Range liegen (MSB gesetzt).
        // `assert_eq!` statt `assert!` weil clippy `assertions_on_constants`
        // sonst meckert; hier vergleichen wir mit konstantem Erwartwert.
        assert_eq!(
            PARTICIPANT_MESSAGE_DATA_KIND_ZERODDS_MANUAL_BY_TOPIC
                & PARTICIPANT_MESSAGE_DATA_KIND_VENDOR_BASE,
            PARTICIPANT_MESSAGE_DATA_KIND_VENDOR_BASE
        );
    }

    #[test]
    fn participant_message_data_roundtrip_le() {
        let m = ParticipantMessageData::manual_by_participant(sample_prefix());
        let bytes = m.to_cdr(true).unwrap();
        // Encapsulation-Header LE
        assert_eq!(&bytes[..4], &[0x00, 0x01, 0x00, 0x00]);
        let decoded = ParticipantMessageData::from_cdr(&bytes).unwrap();
        assert_eq!(decoded, m);
    }

    #[test]
    fn participant_message_data_roundtrip_be() {
        let m = ParticipantMessageData::automatic(sample_prefix());
        let bytes = m.to_cdr(false).unwrap();
        assert_eq!(&bytes[..4], &[0x00, 0x00, 0x00, 0x00]);
        let decoded = ParticipantMessageData::from_cdr(&bytes).unwrap();
        assert_eq!(decoded, m);
    }

    #[test]
    fn participant_message_data_roundtrip_with_topic_token() {
        let m =
            ParticipantMessageData::manual_by_topic(sample_prefix(), vec![0xDE, 0xAD, 0xBE, 0xEF]);
        assert!(m.is_vendor_kind());
        let bytes = m.to_cdr(true).unwrap();
        let decoded = ParticipantMessageData::from_cdr(&bytes).unwrap();
        assert_eq!(decoded.data, vec![0xDE, 0xAD, 0xBE, 0xEF]);
        assert_eq!(
            decoded.kind,
            PARTICIPANT_MESSAGE_DATA_KIND_ZERODDS_MANUAL_BY_TOPIC
        );
    }

    #[test]
    fn participant_message_data_accepts_xcdr2_le_encapsulation() {
        // ZeroDDS-Default fuer User-Topics ist XCDR2-LE (0x0007). Wenn
        // ein Peer das WLP-Topic mit XCDR2 schickt, muessen wir das
        // dekodieren koennen (Cyclone tut das mit Spec-2.5-default-rep).
        let m = ParticipantMessageData::automatic(sample_prefix());
        let mut bytes = m.to_cdr(true).unwrap();
        bytes[0] = 0x00;
        bytes[1] = 0x07; // XCDR2 LE
        let decoded = ParticipantMessageData::from_cdr(&bytes).unwrap();
        assert_eq!(decoded, m);
    }

    #[test]
    fn participant_message_data_accepts_xcdr2_be_encapsulation() {
        let m = ParticipantMessageData::automatic(sample_prefix());
        let mut bytes = m.to_cdr(false).unwrap();
        bytes[0] = 0x00;
        bytes[1] = 0x06; // XCDR2 BE
        let decoded = ParticipantMessageData::from_cdr(&bytes).unwrap();
        assert_eq!(decoded, m);
    }

    #[test]
    fn participant_message_data_rejects_unknown_encapsulation() {
        let mut bytes = vec![0x99, 0x99, 0, 0];
        bytes.extend_from_slice(&[0u8; 24]);
        let res = ParticipantMessageData::from_cdr(&bytes);
        assert!(matches!(
            res,
            Err(WireError::UnsupportedEncapsulation { kind: [0x99, 0x99] })
        ));
    }

    #[test]
    fn participant_message_data_rejects_overlong_data() {
        // Build manually: encapsulation + 16 byte guid + 4 byte kind +
        // 4 byte length=MAX+1.
        let mut bytes = vec![0x00, 0x01, 0x00, 0x00];
        bytes.extend_from_slice(&[0u8; 16]);
        bytes.extend_from_slice(&0u32.to_le_bytes()); // kind
        let too_big = (MAX_DATA_LEN as u32) + 1;
        bytes.extend_from_slice(&too_big.to_le_bytes());
        // data missing — that's fine, the cap check fires first.
        let res = ParticipantMessageData::from_cdr(&bytes);
        assert!(matches!(res, Err(WireError::ValueOutOfRange { .. })));
    }

    #[test]
    fn participant_message_data_encoder_caps_data_length() {
        let mut m = ParticipantMessageData::automatic(sample_prefix());
        m.data = vec![0u8; MAX_DATA_LEN + 1];
        let res = m.to_cdr(true);
        assert!(matches!(res, Err(WireError::ValueOutOfRange { .. })));
    }

    #[test]
    fn participant_message_data_too_short_encapsulation() {
        let bytes = [0x00];
        let res = ParticipantMessageData::from_cdr(&bytes);
        assert!(matches!(res, Err(WireError::UnexpectedEof { .. })));
    }

    #[test]
    fn participant_message_data_too_short_body() {
        // Encapsulation valid, body only 8 byte (less than 12-byte prefix variant).
        let bytes = vec![0x00, 0x01, 0x00, 0x00, 0, 0, 0, 0, 0, 0, 0, 0];
        let res = ParticipantMessageData::from_cdr(&bytes);
        assert!(matches!(res, Err(WireError::UnexpectedEof { .. })));
    }

    #[test]
    fn participant_message_data_truncated_data_section() {
        // length=8 but only 4 byte follow.
        let mut bytes = vec![0x00, 0x01, 0x00, 0x00];
        bytes.extend_from_slice(&[0u8; 16]);
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&8u32.to_le_bytes());
        bytes.extend_from_slice(&[1, 2, 3, 4]);
        let res = ParticipantMessageData::from_cdr(&bytes);
        assert!(matches!(res, Err(WireError::UnexpectedEof { .. })));
    }

    #[test]
    fn participant_message_data_le_be_bytes_differ_for_kind() {
        // Sanity: BE und LE Encoding unterscheiden sich ueber kind.
        let mut m = ParticipantMessageData::automatic(sample_prefix());
        m.kind = 0x0102_0304;
        let le = m.to_cdr(true).unwrap();
        let be = m.to_cdr(false).unwrap();
        assert_ne!(le, be);
        // Beide muessen wieder denselben Wert ergeben.
        assert_eq!(ParticipantMessageData::from_cdr(&le).unwrap(), m);
        assert_eq!(ParticipantMessageData::from_cdr(&be).unwrap(), m);
    }

    #[test]
    fn participant_message_data_accepts_12_byte_prefix_only_encoding() {
        // Legacy/strict Encoder schreibt nur 12 Byte (Prefix). Wir
        // muessen das dekodieren koennen + die EntityId-Default
        // (PARTICIPANT = 0xC1) auffuellen.
        let mut bytes = vec![0x00, 0x01, 0x00, 0x00];
        let prefix = sample_prefix().to_bytes();
        bytes.extend_from_slice(&prefix); // 12 byte
        bytes.extend_from_slice(&0u32.to_le_bytes()); // kind
        bytes.extend_from_slice(&0u32.to_le_bytes()); // data len = 0
        let decoded = ParticipantMessageData::from_cdr(&bytes).unwrap();
        assert_eq!(decoded.prefix(), sample_prefix());
        // EntityId-Default-Auffuellung
        assert_eq!(&decoded.participant_guid[12..], &[0, 0, 1, 0xC1]);
    }
}
