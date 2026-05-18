// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! RTPS-Wire-Basistypen (DDSI-RTPS 2.5 §8.3.5, §8.3.5.1).
//!
//! Diese Typen sind die Atome des RTPS-Wire-Formats: GUID-Komponenten,
//! Sequence-Numbers, Locators. Sie sind alle reine Byte-Strukturen mit
//! festem Layout (kein XCDR-Alignment, kein Endianness-Tagging am Typ —
//! die Endianness eines Submessage-Stream-Slices kommt aus dem
//! Submessage-Header E-Flag).
//!
//! # Konvention
//!
//! - `read_from_le` / `read_from_be`: Decoder mit expliziter Endianness.
//! - `write_to_le` / `write_to_be`: Encoder symmetrisch.
//! - `WIRE_SIZE`: Konstante mit der festen Bytezahl auf der Wire.

use crate::error::WireError;

// ============================================================================
// ProtocolVersion (§8.3.5.5)
// ============================================================================

/// `ProtocolVersion`: Major + Minor des RTPS-Protokolls. Aktuell 2.5.
///
/// `PartialOrd`/`Ord` vergleichen lexikographisch — `(major, minor)`-
/// Tupel-Reihenfolge — was der Spec-Versions-Ordnung entspricht
/// (2.4 < 2.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ProtocolVersion {
    /// Major version.
    pub major: u8,
    /// Minor version.
    pub minor: u8,
}

impl ProtocolVersion {
    /// Wire-Size: 2 Bytes.
    pub const WIRE_SIZE: usize = 2;

    /// RTPS 1.0 — historisch (Spec §8.3.5.5).
    pub const V1_0: Self = Self { major: 1, minor: 0 };
    /// RTPS 1.1 — historisch.
    pub const V1_1: Self = Self { major: 1, minor: 1 };
    /// RTPS 2.0 — historisch.
    pub const V2_0: Self = Self { major: 2, minor: 0 };
    /// RTPS 2.1 — historisch.
    pub const V2_1: Self = Self { major: 2, minor: 1 };
    /// RTPS 2.2 — historisch.
    pub const V2_2: Self = Self { major: 2, minor: 2 };
    /// RTPS 2.3 — historisch.
    pub const V2_3: Self = Self { major: 2, minor: 3 };
    /// RTPS 2.4 — Cyclone DDS Default vor Update auf 2.5.
    pub const V2_4: Self = Self { major: 2, minor: 4 };
    /// RTPS 2.5 (Default fuer ZeroDDS).
    pub const V2_5: Self = Self { major: 2, minor: 5 };

    /// `PROTOCOLVERSION` — Spec-Alias fuer den aktuellsten unterstuetzten
    /// Wert (RTPS 2.5).
    pub const CURRENT: Self = Self::V2_5;

    /// Bytes [major, minor].
    #[must_use]
    pub fn to_bytes(self) -> [u8; 2] {
        [self.major, self.minor]
    }

    /// Liest 2 Bytes.
    #[must_use]
    pub fn from_bytes(bytes: [u8; 2]) -> Self {
        Self {
            major: bytes[0],
            minor: bytes[1],
        }
    }
}

impl Default for ProtocolVersion {
    fn default() -> Self {
        Self::V2_5
    }
}

// ============================================================================
// VendorId (§8.3.5.6)
// ============================================================================

/// `VendorId`: 2-byte Vendor-Identifier. ZeroDDS nutzt `0x01F0` als
/// Interim-Wert aus dem OMG-Entwickler-Range, bis ein offizieller
/// VendorId beantragt wird.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VendorId(pub [u8; 2]);

impl VendorId {
    /// Wire-Size: 2 Bytes.
    pub const WIRE_SIZE: usize = 2;

    /// Sentinel "unknown" (0x00, 0x00) — nur fuer Tests/Stub.
    pub const UNKNOWN: Self = Self([0, 0]);

    /// ZeroDDS Interim-VendorId aus OMG-Entwickler-Range.
    pub const ZERODDS: Self = Self([0x01, 0xF0]);

    /// Bytes ungeaendert.
    #[must_use]
    pub fn to_bytes(self) -> [u8; 2] {
        self.0
    }

    /// Bytes ungeaendert.
    #[must_use]
    pub fn from_bytes(bytes: [u8; 2]) -> Self {
        Self(bytes)
    }
}

// ============================================================================
// GuidPrefix (§8.3.5.1)
// ============================================================================

/// `GuidPrefix`: 12-byte-Prefix einer GUID. Identifiziert einen
/// Participant; bleibt fuer alle Endpoints des Participants gleich.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct GuidPrefix(pub [u8; 12]);

impl GuidPrefix {
    /// Wire-Size: 12 Bytes.
    pub const WIRE_SIZE: usize = 12;

    /// Sentinel "unknown".
    pub const UNKNOWN: Self = Self([0; 12]);

    /// Bytes ungeaendert.
    #[must_use]
    pub fn to_bytes(self) -> [u8; 12] {
        self.0
    }

    /// Bytes ungeaendert.
    #[must_use]
    pub fn from_bytes(bytes: [u8; 12]) -> Self {
        Self(bytes)
    }

    /// ZeroDDS-Konvention (Spec `zerodds-zero-copy-1.0` §6 Welle 4):
    /// die ersten 4 Bytes des GuidPrefix tragen einen deterministischen
    /// Host-Identifier (Hash der `gethostname`-Ausgabe). Zwei
    /// Participants mit identischem Host-Id-Prefix laufen auf der
    /// gleichen Maschine und koennen einen Same-Host-Zero-Copy-Pfad
    /// aufbauen.
    ///
    /// Die RTPS-2.5-Spec §9.3.1.5 erlaubt vendor-spezifische
    /// Strukturierung der ersten 8 Bytes (Vendor-Vendor-Spezifisch);
    /// nur die Vergleichs-Semantik der gesamten 12 Bytes ist normativ.
    #[must_use]
    pub fn host_id(self) -> [u8; 4] {
        [self.0[0], self.0[1], self.0[2], self.0[3]]
    }

    /// Liefert `true`, wenn beide Participants denselben Host-Id-Prefix
    /// tragen. Siehe [`Self::host_id`].
    #[must_use]
    pub fn is_same_host(self, other: Self) -> bool {
        self.host_id() == other.host_id()
    }
}

// ============================================================================
// EntityId (§8.3.5.2 + Tabelle 9.1)
// ============================================================================

/// `EntityKind`: Klassifikation eines Endpunkts. Spec-Tabelle 9.1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u8)]
#[allow(missing_docs)]
pub enum EntityKind {
    Unknown = 0x00,
    UserWriterNoKey = 0x03,
    UserWriterWithKey = 0x02,
    UserReaderNoKey = 0x04,
    UserReaderWithKey = 0x07,
    BuiltinWriterNoKey = 0xC3,
    BuiltinWriterWithKey = 0xC2,
    BuiltinReaderNoKey = 0xC4,
    BuiltinReaderWithKey = 0xC7,
    Participant = 0xC1,
}

impl EntityKind {
    /// Konvertiert ein Byte in einen `EntityKind`. Unbekannte Bytes
    /// werden zu `Unknown` gemappt — das spiegelt Spec-Toleranz-Verhalten.
    #[must_use]
    pub fn from_byte(b: u8) -> Self {
        match b {
            0x03 => Self::UserWriterNoKey,
            0x02 => Self::UserWriterWithKey,
            0x04 => Self::UserReaderNoKey,
            0x07 => Self::UserReaderWithKey,
            0xC3 => Self::BuiltinWriterNoKey,
            0xC2 => Self::BuiltinWriterWithKey,
            0xC4 => Self::BuiltinReaderNoKey,
            0xC7 => Self::BuiltinReaderWithKey,
            0xC1 => Self::Participant,
            _ => Self::Unknown,
        }
    }
}

/// `EntityId`: 4-byte Endpoint-Identifier innerhalb eines Participants.
/// Layout: 3 Byte `entity_key` + 1 Byte `entity_kind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EntityId {
    /// Erste 3 Bytes (key).
    pub entity_key: [u8; 3],
    /// Letztes Byte (kind).
    pub entity_kind: EntityKind,
}

impl EntityId {
    /// Wire-Size: 4 Bytes.
    pub const WIRE_SIZE: usize = 4;

    /// Sentinel.
    pub const UNKNOWN: Self = Self {
        entity_key: [0; 3],
        entity_kind: EntityKind::Unknown,
    };

    /// Reservierter Participant-EntityId (Spec §9.3.1.2).
    pub const PARTICIPANT: Self = Self {
        entity_key: [0, 0, 1],
        entity_kind: EntityKind::Participant,
    };

    /// Konstruiert einen User-Writer-Endpoint mit Key.
    #[must_use]
    pub const fn user_writer_with_key(key: [u8; 3]) -> Self {
        Self {
            entity_key: key,
            entity_kind: EntityKind::UserWriterWithKey,
        }
    }

    /// Konstruiert einen User-Reader-Endpoint mit Key.
    #[must_use]
    pub const fn user_reader_with_key(key: [u8; 3]) -> Self {
        Self {
            entity_key: key,
            entity_kind: EntityKind::UserReaderWithKey,
        }
    }

    /// SPDP Builtin Participant Writer (Spec §9.3.1.5 Tabelle 9.4).
    /// Multicast-Beacon-Sender im Discovery-Pfad.
    pub const SPDP_BUILTIN_PARTICIPANT_WRITER: Self = Self {
        entity_key: [0, 0x01, 0x00],
        entity_kind: EntityKind::BuiltinWriterWithKey,
    };

    /// SPDP Builtin Participant Reader.
    pub const SPDP_BUILTIN_PARTICIPANT_READER: Self = Self {
        entity_key: [0, 0x01, 0x00],
        entity_kind: EntityKind::BuiltinReaderWithKey,
    };

    /// SEDP Subscriptions Writer .
    pub const SEDP_BUILTIN_SUBSCRIPTIONS_WRITER: Self = Self {
        entity_key: [0, 0x00, 0x04],
        entity_kind: EntityKind::BuiltinWriterWithKey,
    };

    /// SEDP Subscriptions Reader .
    pub const SEDP_BUILTIN_SUBSCRIPTIONS_READER: Self = Self {
        entity_key: [0, 0x00, 0x04],
        entity_kind: EntityKind::BuiltinReaderWithKey,
    };

    /// SEDP Publications Writer .
    pub const SEDP_BUILTIN_PUBLICATIONS_WRITER: Self = Self {
        entity_key: [0, 0x00, 0x03],
        entity_kind: EntityKind::BuiltinWriterWithKey,
    };

    /// SEDP Publications Reader .
    pub const SEDP_BUILTIN_PUBLICATIONS_READER: Self = Self {
        entity_key: [0, 0x00, 0x03],
        entity_kind: EntityKind::BuiltinReaderWithKey,
    };

    /// `BUILTIN_PARTICIPANT_MESSAGE_WRITER` — Writer-Liveliness-
    /// Protocol (WLP). Sendet `ParticipantMessageData` ueber das
    /// Topic `DCPSParticipantMessage` (DDSI-RTPS 2.5 §8.4.13,
    /// §9.3.1.5 Tab. 9.4 — EntityKey `[00, 02, 00]`,
    /// EntityKind `BuiltinWriterWithKey = 0xC2`).
    pub const BUILTIN_PARTICIPANT_MESSAGE_WRITER: Self = Self {
        entity_key: [0, 0x02, 0x00],
        entity_kind: EntityKind::BuiltinWriterWithKey,
    };

    /// `BUILTIN_PARTICIPANT_MESSAGE_READER` — Counterpart zum
    /// WLP-Writer (DDSI-RTPS 2.5 §8.4.13, §9.3.1.5 Tab. 9.4).
    pub const BUILTIN_PARTICIPANT_MESSAGE_READER: Self = Self {
        entity_key: [0, 0x02, 0x00],
        entity_kind: EntityKind::BuiltinReaderWithKey,
    };

    // TypeLookup Service (XTypes §7.6.3.3.4): RPC, kein Key.
    // ENTITYKIND_BUILTIN_WRITER_NO_KEY = 0xC3, _READER_NO_KEY = 0xC4.
    /// TypeLookup Service Request Writer.
    pub const TL_SVC_REQ_WRITER: Self = Self {
        entity_key: [0, 0x03, 0x00],
        entity_kind: EntityKind::BuiltinWriterNoKey,
    };
    /// TypeLookup Service Request Reader.
    pub const TL_SVC_REQ_READER: Self = Self {
        entity_key: [0, 0x03, 0x00],
        entity_kind: EntityKind::BuiltinReaderNoKey,
    };
    /// TypeLookup Service Reply Writer.
    pub const TL_SVC_REPLY_WRITER: Self = Self {
        entity_key: [0, 0x03, 0x01],
        entity_kind: EntityKind::BuiltinWriterNoKey,
    };
    /// TypeLookup Service Reply Reader.
    pub const TL_SVC_REPLY_READER: Self = Self {
        entity_key: [0, 0x03, 0x01],
        entity_kind: EntityKind::BuiltinReaderNoKey,
    };

    // ----------------------------------------------------------------
    // DDS-Security 1.2 §7.4.7.1 Tab.7 — 12 Secure-Builtin-EntityIds
    // (C3.8). EntityKey-Layout per Spec; Kind = WithKey ausser bei den
    // Stateless-Topics (NoKey, da diese Topics keyless sind).
    // ----------------------------------------------------------------

    /// `SEDP_BUILTIN_PUBLICATIONS_SECURE_WRITER` — Secure-SEDP
    /// Publications-Writer (Bit 16, §9.3.1.6 Tab.13).
    pub const SEDP_BUILTIN_PUBLICATIONS_SECURE_WRITER: Self = Self {
        entity_key: [0xff, 0x00, 0x03],
        entity_kind: EntityKind::BuiltinWriterWithKey,
    };
    /// `SEDP_BUILTIN_PUBLICATIONS_SECURE_READER` — Bit 17.
    pub const SEDP_BUILTIN_PUBLICATIONS_SECURE_READER: Self = Self {
        entity_key: [0xff, 0x00, 0x03],
        entity_kind: EntityKind::BuiltinReaderWithKey,
    };
    /// `SEDP_BUILTIN_SUBSCRIPTIONS_SECURE_WRITER` — Bit 18.
    pub const SEDP_BUILTIN_SUBSCRIPTIONS_SECURE_WRITER: Self = Self {
        entity_key: [0xff, 0x00, 0x04],
        entity_kind: EntityKind::BuiltinWriterWithKey,
    };
    /// `SEDP_BUILTIN_SUBSCRIPTIONS_SECURE_READER` — Bit 19.
    pub const SEDP_BUILTIN_SUBSCRIPTIONS_SECURE_READER: Self = Self {
        entity_key: [0xff, 0x00, 0x04],
        entity_kind: EntityKind::BuiltinReaderWithKey,
    };
    /// `BUILTIN_PARTICIPANT_MESSAGE_SECURE_WRITER` — Secure WLP-Writer
    /// (Bit 20, §7.4.7.1).
    pub const BUILTIN_PARTICIPANT_MESSAGE_SECURE_WRITER: Self = Self {
        entity_key: [0xff, 0x02, 0x00],
        entity_kind: EntityKind::BuiltinWriterWithKey,
    };
    /// `BUILTIN_PARTICIPANT_MESSAGE_SECURE_READER` — Bit 21.
    pub const BUILTIN_PARTICIPANT_MESSAGE_SECURE_READER: Self = Self {
        entity_key: [0xff, 0x02, 0x00],
        entity_kind: EntityKind::BuiltinReaderWithKey,
    };
    /// `BUILTIN_PARTICIPANT_STATELESS_MESSAGE_WRITER` — Auth-Handshake-
    /// Topic-Writer (Bit 22, §7.4.7.1, §10.3.4 Auth-Stateless-Wire).
    /// NoKey, da Stateless-Topic keyless ist.
    pub const BUILTIN_PARTICIPANT_STATELESS_MESSAGE_WRITER: Self = Self {
        entity_key: [0x00, 0x02, 0x01],
        entity_kind: EntityKind::BuiltinWriterNoKey,
    };
    /// `BUILTIN_PARTICIPANT_STATELESS_MESSAGE_READER` — Bit 23.
    pub const BUILTIN_PARTICIPANT_STATELESS_MESSAGE_READER: Self = Self {
        entity_key: [0x00, 0x02, 0x01],
        entity_kind: EntityKind::BuiltinReaderNoKey,
    };
    /// `BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_WRITER` — Crypto-
    /// KeyExchange-Topic-Writer (Bit 24, §7.4.7.1, §10.5.4
    /// VolatileMessageSecure-Wire).
    pub const BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_WRITER: Self = Self {
        entity_key: [0xff, 0x02, 0x02],
        entity_kind: EntityKind::BuiltinWriterWithKey,
    };
    /// `BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_READER` — Bit 25.
    pub const BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_READER: Self = Self {
        entity_key: [0xff, 0x02, 0x02],
        entity_kind: EntityKind::BuiltinReaderWithKey,
    };
    /// `SPDP_RELIABLE_BUILTIN_PARTICIPANTS_SECURE_WRITER` — Secure-
    /// SPDP-Writer fuer DCPSParticipantsSecure-Topic (Bit 26).
    pub const SPDP_RELIABLE_BUILTIN_PARTICIPANTS_SECURE_WRITER: Self = Self {
        entity_key: [0xff, 0x01, 0x01],
        entity_kind: EntityKind::BuiltinWriterWithKey,
    };
    /// `SPDP_RELIABLE_BUILTIN_PARTICIPANTS_SECURE_READER` — Bit 27.
    pub const SPDP_RELIABLE_BUILTIN_PARTICIPANTS_SECURE_READER: Self = Self {
        entity_key: [0xff, 0x01, 0x01],
        entity_kind: EntityKind::BuiltinReaderWithKey,
    };

    /// True wenn dies einer der 12 Secure-Builtin-EntityIds aus
    /// DDS-Security 1.2 §7.4.7.1 Tab.7 ist.
    #[must_use]
    pub const fn is_secure_builtin(self) -> bool {
        matches!(
            self,
            Self::SEDP_BUILTIN_PUBLICATIONS_SECURE_WRITER
                | Self::SEDP_BUILTIN_PUBLICATIONS_SECURE_READER
                | Self::SEDP_BUILTIN_SUBSCRIPTIONS_SECURE_WRITER
                | Self::SEDP_BUILTIN_SUBSCRIPTIONS_SECURE_READER
                | Self::BUILTIN_PARTICIPANT_MESSAGE_SECURE_WRITER
                | Self::BUILTIN_PARTICIPANT_MESSAGE_SECURE_READER
                | Self::BUILTIN_PARTICIPANT_STATELESS_MESSAGE_WRITER
                | Self::BUILTIN_PARTICIPANT_STATELESS_MESSAGE_READER
                | Self::BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_WRITER
                | Self::BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_READER
                | Self::SPDP_RELIABLE_BUILTIN_PARTICIPANTS_SECURE_WRITER
                | Self::SPDP_RELIABLE_BUILTIN_PARTICIPANTS_SECURE_READER
        )
    }

    /// Bytes [key0, key1, key2, kind].
    #[must_use]
    pub fn to_bytes(self) -> [u8; 4] {
        [
            self.entity_key[0],
            self.entity_key[1],
            self.entity_key[2],
            self.entity_kind as u8,
        ]
    }

    /// Liest 4 Bytes.
    #[must_use]
    pub fn from_bytes(bytes: [u8; 4]) -> Self {
        Self {
            entity_key: [bytes[0], bytes[1], bytes[2]],
            entity_kind: EntityKind::from_byte(bytes[3]),
        }
    }
}

// ============================================================================
// Guid (§8.3.5.3)
// ============================================================================

/// `Guid`: GuidPrefix + EntityId = 16 Bytes. Eindeutiger Identifier
/// eines Endpunkts global.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Guid {
    /// Participant-Prefix.
    pub prefix: GuidPrefix,
    /// Endpoint-Identifier innerhalb des Participants.
    pub entity_id: EntityId,
}

impl Guid {
    /// Wire-Size: 16 Bytes.
    pub const WIRE_SIZE: usize = 16;

    /// Sentinel.
    pub const UNKNOWN: Self = Self {
        prefix: GuidPrefix::UNKNOWN,
        entity_id: EntityId::UNKNOWN,
    };

    /// Konstruiert eine Guid aus Prefix + EntityId.
    #[must_use]
    pub const fn new(prefix: GuidPrefix, entity_id: EntityId) -> Self {
        Self { prefix, entity_id }
    }

    /// Bytes (Prefix + EntityId).
    #[must_use]
    pub fn to_bytes(self) -> [u8; 16] {
        let mut out = [0u8; 16];
        out[..12].copy_from_slice(&self.prefix.to_bytes());
        out[12..].copy_from_slice(&self.entity_id.to_bytes());
        out
    }

    /// Liest 16 Bytes.
    #[must_use]
    pub fn from_bytes(bytes: [u8; 16]) -> Self {
        let mut prefix_bytes = [0u8; 12];
        prefix_bytes.copy_from_slice(&bytes[..12]);
        let mut entity_bytes = [0u8; 4];
        entity_bytes.copy_from_slice(&bytes[12..]);
        Self {
            prefix: GuidPrefix::from_bytes(prefix_bytes),
            entity_id: EntityId::from_bytes(entity_bytes),
        }
    }
}

// ============================================================================
// SequenceNumber (§8.3.5.4)
// ============================================================================

/// `SequenceNumber`: 64-bit signed, encoded als (high: i32, low: u32).
/// Beide Felder werden mit der aktiven Submessage-Endianness geschrieben.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SequenceNumber(pub i64);

impl SequenceNumber {
    /// Wire-Size: 8 Bytes.
    pub const WIRE_SIZE: usize = 8;

    /// Sentinel "unknown" (high=-1, low=0) → -2^32.
    pub const UNKNOWN: Self = Self(-(1_i64 << 32));

    /// Splittet den i64 in (high, low) gemaess Spec.
    #[must_use]
    pub fn split(self) -> (i32, u32) {
        let value = self.0;
        let high = (value >> 32) as i32;
        let low = (value & 0xFFFF_FFFF) as u32;
        (high, low)
    }

    /// Setzt aus (high, low) zusammen.
    #[must_use]
    pub fn from_high_low(high: i32, low: u32) -> Self {
        let value = (i64::from(high) << 32) | i64::from(low);
        Self(value)
    }

    /// Schreibt in 8 Bytes mit gegebener Endianness (LE oder BE).
    #[must_use]
    pub fn to_bytes_le(self) -> [u8; 8] {
        let (high, low) = self.split();
        let mut out = [0u8; 8];
        out[..4].copy_from_slice(&high.to_le_bytes());
        out[4..].copy_from_slice(&low.to_le_bytes());
        out
    }

    /// BE-Variante.
    #[must_use]
    pub fn to_bytes_be(self) -> [u8; 8] {
        let (high, low) = self.split();
        let mut out = [0u8; 8];
        out[..4].copy_from_slice(&high.to_be_bytes());
        out[4..].copy_from_slice(&low.to_be_bytes());
        out
    }

    /// LE-Decoder.
    #[must_use]
    pub fn from_bytes_le(bytes: [u8; 8]) -> Self {
        let mut hi = [0u8; 4];
        hi.copy_from_slice(&bytes[..4]);
        let mut lo = [0u8; 4];
        lo.copy_from_slice(&bytes[4..]);
        Self::from_high_low(i32::from_le_bytes(hi), u32::from_le_bytes(lo))
    }

    /// BE-Decoder.
    #[must_use]
    pub fn from_bytes_be(bytes: [u8; 8]) -> Self {
        let mut hi = [0u8; 4];
        hi.copy_from_slice(&bytes[..4]);
        let mut lo = [0u8; 4];
        lo.copy_from_slice(&bytes[4..]);
        Self::from_high_low(i32::from_be_bytes(hi), u32::from_be_bytes(lo))
    }
}

// ============================================================================
// Vendor-Extension-Slots (§8.3.2 UExtension4_t / WExtension8_t)
// ============================================================================

/// `UExtension4_t` — 4-byte vendor-spezifischer Extension-Slot.
/// Spec §8.3.2: opaker 32-bit-Wert, Vendor entscheidet ueber Bedeutung;
/// Receiver propagiert den Wert per `extensions`-Feld in den
/// Receiver-State.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct UExtension4(pub [u8; 4]);

impl UExtension4 {
    /// Wire-Size: 4 Bytes.
    pub const WIRE_SIZE: usize = 4;

    /// Konstruktor aus u32 (Big-Endian).
    #[must_use]
    pub fn from_u32_be(v: u32) -> Self {
        Self(v.to_be_bytes())
    }

    /// Liefert den Wert als u32 (Big-Endian-Interpretation).
    #[must_use]
    pub fn to_u32_be(self) -> u32 {
        u32::from_be_bytes(self.0)
    }

    /// Roundtrip-Identitaet.
    #[must_use]
    pub fn to_bytes(self) -> [u8; 4] {
        self.0
    }

    /// Roundtrip-Identitaet.
    #[must_use]
    pub fn from_bytes(bytes: [u8; 4]) -> Self {
        Self(bytes)
    }
}

/// `WExtension8_t` — 8-byte vendor-spezifischer Extension-Slot.
/// Spec §8.3.2: opaker 64-bit-Wert (analog UExtension4_t fuer
/// Felder die 8 Byte brauchen, z.B. fuer 64-bit-Counter).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct WExtension8(pub [u8; 8]);

impl WExtension8 {
    /// Wire-Size: 8 Bytes.
    pub const WIRE_SIZE: usize = 8;

    /// Konstruktor aus u64 (Big-Endian).
    #[must_use]
    pub fn from_u64_be(v: u64) -> Self {
        Self(v.to_be_bytes())
    }

    /// Liefert den Wert als u64 (Big-Endian-Interpretation).
    #[must_use]
    pub fn to_u64_be(self) -> u64 {
        u64::from_be_bytes(self.0)
    }

    /// Roundtrip-Identitaet.
    #[must_use]
    pub fn to_bytes(self) -> [u8; 8] {
        self.0
    }

    /// Roundtrip-Identitaet.
    #[must_use]
    pub fn from_bytes(bytes: [u8; 8]) -> Self {
        Self(bytes)
    }
}

// ============================================================================
// FragmentNumber (§8.3.5.7)
// ============================================================================

/// `FragmentNumber`: 32-bit unsigned, 1-basiert (Fragment #1 ist das
/// erste Fragment eines Samples). `UNKNOWN` = 0 wird als Sentinel
/// verwendet, ist aber kein gueltiges Fragment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FragmentNumber(pub u32);

impl FragmentNumber {
    /// Wire-Size: 4 Bytes.
    pub const WIRE_SIZE: usize = 4;

    /// Sentinel "unknown" (= 0). Nie ein gueltiges Fragment.
    pub const UNKNOWN: Self = Self(0);

    /// LE-Bytes.
    #[must_use]
    pub fn to_bytes_le(self) -> [u8; 4] {
        self.0.to_le_bytes()
    }

    /// BE-Bytes.
    #[must_use]
    pub fn to_bytes_be(self) -> [u8; 4] {
        self.0.to_be_bytes()
    }

    /// LE-Decoder.
    #[must_use]
    pub fn from_bytes_le(bytes: [u8; 4]) -> Self {
        Self(u32::from_le_bytes(bytes))
    }

    /// BE-Decoder.
    #[must_use]
    pub fn from_bytes_be(bytes: [u8; 4]) -> Self {
        Self(u32::from_be_bytes(bytes))
    }
}

// ============================================================================
// Locator (§8.3.5.7)
// ============================================================================

/// `LocatorKind`: Adress-Familie eines Locators.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(i32)]
#[allow(missing_docs)]
pub enum LocatorKind {
    Invalid = -1,
    Reserved = 0,
    UdpV4 = 1,
    UdpV6 = 2,
    /// TCPv4 (DDS-TCP-PSM §4 `LOCATOR_KIND_TCPV4`).
    Tcpv4 = 4,
    /// TCPv6 (DDS-TCP-PSM §4 `LOCATOR_KIND_TCPV6`).
    Tcpv6 = 8,
    /// Shared-Memory (Vendor-Range, MSB=1 laut DDSI-RTPS §9.3.1.2 —
    /// negative `i32`-Werte sind vendor-spezifisch). `0x8100_0000` als
    /// ZeroDDS-Vendor-Token; Cyclone + Fast-DDS ignorieren unbekannte
    /// Kinds.
    Shm = -2_130_706_432, // 0x8100_0000 als i32
    /// Unix-Domain-Socket (Vendor-Range, ZeroDDS-Extension fuer
    /// Containerized-IPC wenn Multicast gesperrt oder POSIX-SHM
    /// cross-container nicht funktioniert). `0x8100_0001`. 16-byte
    /// Address-Feld traegt einen Identifier, der in einen Socket-Pfad
    /// unter einem konfigurierbaren Base-Directory aufgeloest wird
    /// (`/tmp/zerodds/uds/<hex>.sock`).
    Uds = -2_130_706_431, // 0x8100_0001 als i32
}

impl LocatorKind {
    /// Liefert den i32-Wire-Wert.
    #[must_use]
    pub fn as_i32(self) -> i32 {
        self as i32
    }

    /// Konvertiert i32 in LocatorKind.
    ///
    /// # Errors
    /// `WireError::InvalidLocatorKind` bei unbekanntem Wert.
    pub fn from_i32(v: i32) -> Result<Self, WireError> {
        match v {
            -1 => Ok(Self::Invalid),
            0 => Ok(Self::Reserved),
            1 => Ok(Self::UdpV4),
            2 => Ok(Self::UdpV6),
            4 => Ok(Self::Tcpv4),
            8 => Ok(Self::Tcpv6),
            -2_130_706_432 => Ok(Self::Shm),
            -2_130_706_431 => Ok(Self::Uds),
            other => Err(WireError::InvalidLocatorKind { kind: other }),
        }
    }
}

/// SPDP-Default-Multicast-Adresse (Spec §9.6.1.4.1): `239.255.0.1`.
pub const SPDP_DEFAULT_MULTICAST_ADDRESS: [u8; 4] = [239, 255, 0, 1];

/// SPDP-Discovery-Port-Base (Spec §9.6.1.4.1, PB).
pub const SPDP_PORT_BASE: u32 = 7400;

/// Domain-spezifischer Port-Offset (Spec §9.6.1.4.1, DG).
pub const SPDP_DOMAIN_GAIN: u32 = 250;

/// Multicast-Discovery-Port-Offset (Spec §9.6.1.4.1, d0).
pub const SPDP_DISCOVERY_MULTICAST_OFFSET: u32 = 0;

/// Berechnet den SPDP-Multicast-Discovery-Port fuer eine Domain.
/// Formel (Spec §9.6.1.4.1):
///   port = PB + DG * domain_id + d0
///        = 7400 + 250 * domain + 0
#[must_use]
pub fn spdp_multicast_port(domain_id: u32) -> u32 {
    SPDP_PORT_BASE + SPDP_DOMAIN_GAIN * domain_id + SPDP_DISCOVERY_MULTICAST_OFFSET
}

/// `Locator`: 24-byte Adresse (kind + port + 16-byte address).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Locator {
    /// Adress-Familie.
    pub kind: LocatorKind,
    /// UDP-Port.
    pub port: u32,
    /// 16-byte address. UDPv4 nutzt die letzten 4 Byte, davor 0.
    pub address: [u8; 16],
}

impl Locator {
    /// Wire-Size: 24 Bytes.
    pub const WIRE_SIZE: usize = 24;

    /// Spec §8.3.5.7 `LOCATOR_INVALID`.
    pub const INVALID: Self = Self {
        kind: LocatorKind::Invalid,
        port: 0,
        address: [0; 16],
    };

    /// Spec §8.3.5.7 `LOCATOR_KIND_RESERVED` Template (kind=0,
    /// port=0, address=0).
    pub const RESERVED: Self = Self {
        kind: LocatorKind::Reserved,
        port: 0,
        address: [0; 16],
    };

    /// Spec §8.3.5.7 — Default-UDPv4-Locator (kind=1, port=0, addr=0).
    pub const UDP_V4_ANY: Self = Self {
        kind: LocatorKind::UdpV4,
        port: 0,
        address: [0; 16],
    };

    /// Spec §8.3.5.7 — Default-UDPv6-Locator (kind=2, port=0, addr=0).
    pub const UDP_V6_ANY: Self = Self {
        kind: LocatorKind::UdpV6,
        port: 0,
        address: [0; 16],
    };

    /// ZeroDDS-Vendor-Extension SHM-Locator-Template.
    pub const SHM_ANY: Self = Self {
        kind: LocatorKind::Shm,
        port: 0,
        address: [0; 16],
    };

    /// Spec §8.3.5.7 — `LOCATOR_PORT_INVALID` Sentinel.
    pub const PORT_INVALID: u32 = 0;

    /// Spec §8.3.5.7 — `LOCATOR_ADDRESS_INVALID` Sentinel.
    pub const ADDRESS_INVALID: [u8; 16] = [0; 16];

    /// Konstruktor fuer UDPv6 (16-byte address + port).
    #[must_use]
    pub fn udp_v6(addr: [u8; 16], port: u32) -> Self {
        Self {
            kind: LocatorKind::UdpV6,
            port,
            address: addr,
        }
    }

    /// Konstruktor fuer UDPv4 (a.b.c.d:port).
    #[must_use]
    pub fn udp_v4(addr: [u8; 4], port: u32) -> Self {
        Self::with_address(LocatorKind::UdpV4, addr, port)
    }

    /// Konstruktor fuer TCPv4 (DDS-TCP-PSM §4).
    #[must_use]
    pub fn tcp_v4(addr: [u8; 4], port: u32) -> Self {
        Self::with_address(LocatorKind::Tcpv4, addr, port)
    }

    /// Legacy-Alias fuer [`Self::tcp_v4`]. **Deprecated**: Benenne
    /// Callsites auf `tcp_v4` um (konsistent mit `udp_v4`).
    #[must_use]
    #[deprecated(note = "use Locator::tcp_v4 instead (naming consistency)")]
    pub fn new_tcp_v4(addr: [u8; 4], port: u32) -> Self {
        Self::tcp_v4(addr, port)
    }

    /// Konstruktor fuer Unix-Domain-Socket-Endpoint mit einer 16-byte-
    /// ID. Port bleibt 0 (nicht sinnvoll fuer UDS). Der Transport
    /// resolved die ID zu `/<base_dir>/<hex>.sock`.
    #[must_use]
    pub fn uds(id: [u8; 16]) -> Self {
        Self {
            kind: LocatorKind::Uds,
            port: 0,
            address: id,
        }
    }

    /// Konstruktor fuer Shared-Memory-Segment mit einer ID (16-byte
    /// Token). Port-Feld bleibt 0 (nicht sinnvoll fuer SHM).
    #[must_use]
    pub fn shm(id: [u8; 16]) -> Self {
        Self {
            kind: LocatorKind::Shm,
            port: 0,
            address: id,
        }
    }

    /// Gemeinsamer IPv4-Konstruktor. Legt die IPv4-Bytes in die letzten
    /// 4 Byte des 16-byte-`address`-Felds (mapped IPv4-in-IPv6-Layout).
    #[must_use]
    fn with_address(kind: LocatorKind, addr: [u8; 4], port: u32) -> Self {
        let mut address = [0u8; 16];
        address[12..].copy_from_slice(&addr);
        Self {
            kind,
            port,
            address,
        }
    }

    /// IPv4-Adresse extrahieren (nur fuer Kind == UdpV4 sinnvoll).
    #[must_use]
    pub fn ipv4(self) -> [u8; 4] {
        let mut out = [0u8; 4];
        out.copy_from_slice(&self.address[12..]);
        out
    }

    /// LE-Encoder.
    #[must_use]
    pub fn to_bytes_le(self) -> [u8; 24] {
        let mut out = [0u8; 24];
        out[..4].copy_from_slice(&self.kind.as_i32().to_le_bytes());
        out[4..8].copy_from_slice(&self.port.to_le_bytes());
        out[8..].copy_from_slice(&self.address);
        out
    }

    /// LE-Decoder.
    ///
    /// # Errors
    /// `WireError::InvalidLocatorKind` bei unbekanntem Kind.
    pub fn from_bytes_le(bytes: [u8; 24]) -> Result<Self, WireError> {
        let mut kind_bytes = [0u8; 4];
        kind_bytes.copy_from_slice(&bytes[..4]);
        let kind = LocatorKind::from_i32(i32::from_le_bytes(kind_bytes))?;
        let mut port_bytes = [0u8; 4];
        port_bytes.copy_from_slice(&bytes[4..8]);
        let port = u32::from_le_bytes(port_bytes);
        let mut address = [0u8; 16];
        address.copy_from_slice(&bytes[8..]);
        Ok(Self {
            kind,
            port,
            address,
        })
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
    use super::*;

    // ---- ProtocolVersion ----
    #[test]
    fn protocol_version_default_is_2_5() {
        assert_eq!(ProtocolVersion::default(), ProtocolVersion::V2_5);
    }

    #[test]
    fn protocol_version_roundtrip() {
        let v = ProtocolVersion::V2_5;
        assert_eq!(ProtocolVersion::from_bytes(v.to_bytes()), v);
    }

    // ---- VendorId ----
    #[test]
    fn vendor_id_zerodds_constant() {
        assert_eq!(VendorId::ZERODDS.0, [0x01, 0xF0]);
    }

    #[test]
    fn vendor_id_roundtrip() {
        let v = VendorId([0xAB, 0xCD]);
        assert_eq!(VendorId::from_bytes(v.to_bytes()), v);
    }

    // ---- GuidPrefix ----
    #[test]
    fn guid_prefix_unknown_is_zero() {
        assert_eq!(GuidPrefix::UNKNOWN.0, [0u8; 12]);
    }

    #[test]
    fn guid_prefix_roundtrip() {
        let bytes = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
        let p = GuidPrefix::from_bytes(bytes);
        assert_eq!(p.to_bytes(), bytes);
    }

    #[test]
    fn guid_prefix_host_id_is_first_four_bytes() {
        let p = GuidPrefix::from_bytes([0xAB, 0xCD, 0x12, 0x34, 0, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(p.host_id(), [0xAB, 0xCD, 0x12, 0x34]);
    }

    #[test]
    fn guid_prefix_same_host_matches_on_first_four_bytes() {
        let a = GuidPrefix::from_bytes([1, 2, 3, 4, 9, 9, 9, 9, 9, 9, 9, 9]);
        let b = GuidPrefix::from_bytes([1, 2, 3, 4, 0, 0, 0, 0, 0, 0, 0, 0]);
        let c = GuidPrefix::from_bytes([1, 2, 3, 5, 9, 9, 9, 9, 9, 9, 9, 9]);
        assert!(a.is_same_host(b));
        assert!(!a.is_same_host(c));
    }

    // ---- EntityId ----
    #[test]
    fn entity_id_user_writer_with_key_layout() {
        let id = EntityId::user_writer_with_key([0xAA, 0xBB, 0xCC]);
        assert_eq!(id.to_bytes(), [0xAA, 0xBB, 0xCC, 0x02]);
    }

    #[test]
    fn entity_id_user_reader_with_key_layout() {
        let id = EntityId::user_reader_with_key([0x11, 0x22, 0x33]);
        assert_eq!(id.to_bytes(), [0x11, 0x22, 0x33, 0x07]);
    }

    #[test]
    fn entity_id_participant_constant() {
        assert_eq!(EntityId::PARTICIPANT.to_bytes(), [0, 0, 1, 0xC1]);
    }

    #[test]
    fn entity_id_unknown_kind_byte_maps_to_unknown() {
        let id = EntityId::from_bytes([1, 2, 3, 0xEE]);
        assert_eq!(id.entity_kind, EntityKind::Unknown);
    }

    #[test]
    fn entity_id_roundtrip() {
        let id = EntityId::user_writer_with_key([1, 2, 3]);
        assert_eq!(EntityId::from_bytes(id.to_bytes()), id);
    }

    // ---- Secure-Builtin-EntityIds (DDS-Security 1.2 §7.4.7.1, C3.8) ----

    #[test]
    fn secure_publications_writer_layout() {
        // Spec: key=[0xff, 0x00, 0x03], kind=BuiltinWriterWithKey=0xC2.
        let id = EntityId::SEDP_BUILTIN_PUBLICATIONS_SECURE_WRITER;
        assert_eq!(id.to_bytes(), [0xff, 0x00, 0x03, 0xC2]);
    }

    #[test]
    fn stateless_writer_is_no_key_kind() {
        // Stateless-Topic ist keyless → BuiltinWriterNoKey=0xC3.
        let id = EntityId::BUILTIN_PARTICIPANT_STATELESS_MESSAGE_WRITER;
        assert_eq!(id.entity_kind, EntityKind::BuiltinWriterNoKey);
        assert_eq!(id.to_bytes(), [0x00, 0x02, 0x01, 0xC3]);
    }

    #[test]
    fn volatile_secure_writer_layout() {
        let id = EntityId::BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_WRITER;
        assert_eq!(id.to_bytes(), [0xff, 0x02, 0x02, 0xC2]);
    }

    #[test]
    fn spdp_secure_reader_layout() {
        let id = EntityId::SPDP_RELIABLE_BUILTIN_PARTICIPANTS_SECURE_READER;
        assert_eq!(id.to_bytes(), [0xff, 0x01, 0x01, 0xC7]);
    }

    #[test]
    fn all_12_secure_entityids_roundtrip() {
        let ids = [
            EntityId::SEDP_BUILTIN_PUBLICATIONS_SECURE_WRITER,
            EntityId::SEDP_BUILTIN_PUBLICATIONS_SECURE_READER,
            EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_SECURE_WRITER,
            EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_SECURE_READER,
            EntityId::BUILTIN_PARTICIPANT_MESSAGE_SECURE_WRITER,
            EntityId::BUILTIN_PARTICIPANT_MESSAGE_SECURE_READER,
            EntityId::BUILTIN_PARTICIPANT_STATELESS_MESSAGE_WRITER,
            EntityId::BUILTIN_PARTICIPANT_STATELESS_MESSAGE_READER,
            EntityId::BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_WRITER,
            EntityId::BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_READER,
            EntityId::SPDP_RELIABLE_BUILTIN_PARTICIPANTS_SECURE_WRITER,
            EntityId::SPDP_RELIABLE_BUILTIN_PARTICIPANTS_SECURE_READER,
        ];
        assert_eq!(ids.len(), 12);
        for id in ids {
            assert!(id.is_secure_builtin(), "{id:?}");
            assert_eq!(EntityId::from_bytes(id.to_bytes()), id);
        }
    }

    #[test]
    fn standard_builtin_is_not_secure_builtin() {
        for id in [
            EntityId::SPDP_BUILTIN_PARTICIPANT_WRITER,
            EntityId::SEDP_BUILTIN_PUBLICATIONS_WRITER,
            EntityId::BUILTIN_PARTICIPANT_MESSAGE_WRITER,
            EntityId::PARTICIPANT,
            EntityId::user_writer_with_key([1, 2, 3]),
        ] {
            assert!(!id.is_secure_builtin(), "{id:?} should not be secure");
        }
    }

    // ---- Guid ----
    #[test]
    fn guid_layout_is_prefix_then_entity_id() {
        let g = Guid::new(
            GuidPrefix::from_bytes([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]),
            EntityId::user_writer_with_key([0xAA, 0xBB, 0xCC]),
        );
        let bytes = g.to_bytes();
        assert_eq!(&bytes[..12], &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);
        assert_eq!(&bytes[12..], &[0xAA, 0xBB, 0xCC, 0x02]);
    }

    #[test]
    fn guid_roundtrip() {
        let g = Guid::new(GuidPrefix::from_bytes([42; 12]), EntityId::PARTICIPANT);
        assert_eq!(Guid::from_bytes(g.to_bytes()), g);
    }

    // ---- SequenceNumber ----
    #[test]
    fn sequence_number_split_zero() {
        let (h, l) = SequenceNumber(0).split();
        assert_eq!((h, l), (0, 0));
    }

    #[test]
    fn sequence_number_split_one() {
        let (h, l) = SequenceNumber(1).split();
        assert_eq!((h, l), (0, 1));
    }

    #[test]
    fn sequence_number_split_high_low() {
        let sn = SequenceNumber::from_high_low(2, 5);
        assert_eq!(sn.0, (2_i64 << 32) | 5);
    }

    #[test]
    fn sequence_number_roundtrip_le() {
        let sn = SequenceNumber(0x0102_0304_0506_0708);
        assert_eq!(SequenceNumber::from_bytes_le(sn.to_bytes_le()), sn);
    }

    #[test]
    fn sequence_number_roundtrip_be() {
        let sn = SequenceNumber(0x0102_0304_0506_0708);
        assert_eq!(SequenceNumber::from_bytes_be(sn.to_bytes_be()), sn);
    }

    #[test]
    fn sequence_number_unknown_high_minus_one_low_zero() {
        let (h, l) = SequenceNumber::UNKNOWN.split();
        assert_eq!((h, l), (-1, 0));
    }

    // ---- FragmentNumber ----
    #[test]
    fn fragment_number_roundtrip_le_be() {
        let fns = [
            FragmentNumber(0),
            FragmentNumber(1),
            FragmentNumber(0x1234_5678),
            FragmentNumber(u32::MAX),
        ];
        for fnum in fns {
            assert_eq!(FragmentNumber::from_bytes_le(fnum.to_bytes_le()), fnum);
            assert_eq!(FragmentNumber::from_bytes_be(fnum.to_bytes_be()), fnum);
        }
    }

    #[test]
    fn fragment_number_unknown_is_zero() {
        assert_eq!(FragmentNumber::UNKNOWN.0, 0);
    }

    #[test]
    fn fragment_number_wire_size_is_four() {
        assert_eq!(FragmentNumber::WIRE_SIZE, 4);
    }

    // ---- Locator ----
    #[test]
    fn locator_kind_roundtrip() {
        for kind in [
            LocatorKind::Invalid,
            LocatorKind::Reserved,
            LocatorKind::UdpV4,
            LocatorKind::UdpV6,
            LocatorKind::Tcpv4,
            LocatorKind::Tcpv6,
            LocatorKind::Shm,
            LocatorKind::Uds,
        ] {
            assert_eq!(LocatorKind::from_i32(kind.as_i32()).unwrap(), kind);
        }
    }

    #[test]
    fn locator_uds_layout() {
        let id = [
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E,
            0x0F, 0x10,
        ];
        let l = Locator::uds(id);
        assert_eq!(l.kind, LocatorKind::Uds);
        assert_eq!(l.port, 0);
        assert_eq!(l.address, id);
    }

    #[test]
    fn locator_kind_rejects_unknown() {
        let res = LocatorKind::from_i32(99);
        assert!(matches!(
            res,
            Err(WireError::InvalidLocatorKind { kind: 99 })
        ));
    }

    #[test]
    fn locator_udp_v4_layout() {
        let l = Locator::udp_v4([192, 168, 1, 100], 7400);
        assert_eq!(l.kind, LocatorKind::UdpV4);
        assert_eq!(l.port, 7400);
        assert_eq!(&l.address[..12], &[0u8; 12]);
        assert_eq!(&l.address[12..], &[192, 168, 1, 100]);
        assert_eq!(l.ipv4(), [192, 168, 1, 100]);
    }

    #[test]
    fn locator_roundtrip_le() {
        let l = Locator::udp_v4([10, 0, 0, 1], 7400);
        let bytes = l.to_bytes_le();
        assert_eq!(Locator::from_bytes_le(bytes).unwrap(), l);
    }

    #[test]
    fn locator_invalid_kind_decoded() {
        let mut bytes = [0u8; 24];
        bytes[..4].copy_from_slice(&99_i32.to_le_bytes());
        let res = Locator::from_bytes_le(bytes);
        assert!(matches!(
            res,
            Err(WireError::InvalidLocatorKind { kind: 99 })
        ));
    }

    // ---- §8.3.5.7 Locator-Constants ----

    #[test]
    fn locator_invalid_constant_matches_spec() {
        assert_eq!(Locator::INVALID.kind, LocatorKind::Invalid);
        assert_eq!(Locator::INVALID.port, Locator::PORT_INVALID);
        assert_eq!(Locator::INVALID.address, Locator::ADDRESS_INVALID);
    }

    #[test]
    fn locator_reserved_constant_kind_is_zero() {
        assert_eq!(Locator::RESERVED.kind.as_i32(), 0);
    }

    #[test]
    fn locator_udp_v4_any_kind_is_one() {
        assert_eq!(Locator::UDP_V4_ANY.kind.as_i32(), 1);
        assert_eq!(Locator::UDP_V4_ANY.port, 0);
    }

    #[test]
    fn locator_udp_v6_any_kind_is_two() {
        assert_eq!(Locator::UDP_V6_ANY.kind.as_i32(), 2);
    }

    #[test]
    fn locator_shm_any_uses_vendor_kind() {
        assert!(Locator::SHM_ANY.kind.as_i32() < 0); // vendor-Range
    }

    #[test]
    fn locator_udp_v6_constructor_keeps_full_address() {
        let addr: [u8; 16] = [
            0x20, 0x01, 0x0d, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x01,
        ];
        let l = Locator::udp_v6(addr, 1234);
        assert_eq!(l.kind, LocatorKind::UdpV6);
        assert_eq!(l.address, addr);
        assert_eq!(l.port, 1234);
    }

    #[test]
    fn locator_udp_v6_roundtrip_le() {
        let l = Locator::udp_v6([1; 16], 9000);
        assert_eq!(Locator::from_bytes_le(l.to_bytes_le()).unwrap(), l);
    }

    // ---- §8.3.5.5 ProtocolVersion-Aliases ----

    #[test]
    fn protocol_version_aliases_match_spec() {
        assert_eq!(
            ProtocolVersion::V1_0,
            ProtocolVersion { major: 1, minor: 0 }
        );
        assert_eq!(
            ProtocolVersion::V1_1,
            ProtocolVersion { major: 1, minor: 1 }
        );
        assert_eq!(
            ProtocolVersion::V2_0,
            ProtocolVersion { major: 2, minor: 0 }
        );
        assert_eq!(
            ProtocolVersion::V2_1,
            ProtocolVersion { major: 2, minor: 1 }
        );
        assert_eq!(
            ProtocolVersion::V2_2,
            ProtocolVersion { major: 2, minor: 2 }
        );
        assert_eq!(
            ProtocolVersion::V2_3,
            ProtocolVersion { major: 2, minor: 3 }
        );
        assert_eq!(
            ProtocolVersion::V2_4,
            ProtocolVersion { major: 2, minor: 4 }
        );
        assert_eq!(
            ProtocolVersion::V2_5,
            ProtocolVersion { major: 2, minor: 5 }
        );
        assert_eq!(ProtocolVersion::CURRENT, ProtocolVersion::V2_5);
    }

    #[test]
    fn protocol_version_all_aliases_roundtrip() {
        for v in [
            ProtocolVersion::V1_0,
            ProtocolVersion::V1_1,
            ProtocolVersion::V2_0,
            ProtocolVersion::V2_1,
            ProtocolVersion::V2_2,
            ProtocolVersion::V2_3,
            ProtocolVersion::V2_4,
            ProtocolVersion::V2_5,
        ] {
            assert_eq!(ProtocolVersion::from_bytes(v.to_bytes()), v);
        }
    }

    // ---- §8.3.2 UExtension4_t ----

    #[test]
    fn uextension4_roundtrip() {
        let u = UExtension4([0xDE, 0xAD, 0xBE, 0xEF]);
        assert_eq!(UExtension4::from_bytes(u.to_bytes()), u);
    }

    #[test]
    fn uextension4_u32_be_roundtrip() {
        let u = UExtension4::from_u32_be(0x1122_3344);
        assert_eq!(u.to_u32_be(), 0x1122_3344);
        assert_eq!(u.0, [0x11, 0x22, 0x33, 0x44]);
    }

    #[test]
    fn uextension4_default_is_zero() {
        assert_eq!(UExtension4::default().0, [0u8; 4]);
        assert_eq!(UExtension4::WIRE_SIZE, 4);
    }

    // ---- §8.3.2 WExtension8_t ----

    #[test]
    fn wextension8_roundtrip() {
        let w = WExtension8([1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(WExtension8::from_bytes(w.to_bytes()), w);
    }

    #[test]
    fn wextension8_u64_be_roundtrip() {
        let w = WExtension8::from_u64_be(0x1122_3344_5566_7788);
        assert_eq!(w.to_u64_be(), 0x1122_3344_5566_7788);
        assert_eq!(w.0, [0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88]);
    }

    #[test]
    fn wextension8_default_is_zero() {
        assert_eq!(WExtension8::default().0, [0u8; 8]);
        assert_eq!(WExtension8::WIRE_SIZE, 8);
    }

    // ---- SPDP Builtin EntityIds (Spec §9.3.1.5 Tabelle 9.4) ----

    #[test]
    fn spdp_builtin_participant_writer_layout() {
        // Spec: 0x000100C2
        let bytes = EntityId::SPDP_BUILTIN_PARTICIPANT_WRITER.to_bytes();
        assert_eq!(bytes, [0x00, 0x01, 0x00, 0xC2]);
    }

    #[test]
    fn spdp_builtin_participant_reader_layout() {
        // Spec: 0x000100C7
        let bytes = EntityId::SPDP_BUILTIN_PARTICIPANT_READER.to_bytes();
        assert_eq!(bytes, [0x00, 0x01, 0x00, 0xC7]);
    }

    #[test]
    fn sedp_publications_writer_layout() {
        // Spec: 0x000003C2
        let bytes = EntityId::SEDP_BUILTIN_PUBLICATIONS_WRITER.to_bytes();
        assert_eq!(bytes, [0x00, 0x00, 0x03, 0xC2]);
    }

    #[test]
    fn sedp_subscriptions_reader_layout() {
        // Spec: 0x000004C7
        let bytes = EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_READER.to_bytes();
        assert_eq!(bytes, [0x00, 0x00, 0x04, 0xC7]);
    }

    // ---- SPDP-Multicast-Adresse + Port-Berechnung ----

    #[test]
    fn spdp_default_multicast_is_239_255_0_1() {
        assert_eq!(SPDP_DEFAULT_MULTICAST_ADDRESS, [239, 255, 0, 1]);
    }

    #[test]
    fn spdp_multicast_port_domain_0_is_7400() {
        assert_eq!(spdp_multicast_port(0), 7400);
    }

    #[test]
    fn spdp_multicast_port_domain_1_is_7650() {
        assert_eq!(spdp_multicast_port(1), 7650);
    }

    #[test]
    fn spdp_multicast_port_domain_5_is_8650() {
        assert_eq!(spdp_multicast_port(5), 8650);
    }
}
