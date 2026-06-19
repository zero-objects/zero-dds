// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! RTPS wire base types (DDSI-RTPS 2.5 §8.3.5, §8.3.5.1).
//!
//! These types are the atoms of the RTPS wire format: GUID components,
//! sequence numbers, locators. They are all pure byte structures with
//! a fixed layout (no XCDR alignment, no endianness tagging on the type —
//! the endianness of a submessage stream slice comes from the
//! submessage-header E flag).
//!
//! # Convention
//!
//! - `read_from_le` / `read_from_be`: decoder with explicit endianness.
//! - `write_to_le` / `write_to_be`: encoder, symmetric.
//! - `WIRE_SIZE`: constant with the fixed byte count on the wire.

use crate::error::WireError;

// ============================================================================
// ProtocolVersion (§8.3.5.5)
// ============================================================================

/// `ProtocolVersion`: major + minor of the RTPS protocol. Currently 2.5.
///
/// `PartialOrd`/`Ord` compare lexicographically — `(major, minor)`
/// tuple order — which matches the spec version ordering
/// (2.4 < 2.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ProtocolVersion {
    /// Major version.
    pub major: u8,
    /// Minor version.
    pub minor: u8,
}

impl ProtocolVersion {
    /// Wire size: 2 bytes.
    pub const WIRE_SIZE: usize = 2;

    /// RTPS 1.0 — historical (Spec §8.3.5.5).
    pub const V1_0: Self = Self { major: 1, minor: 0 };
    /// RTPS 1.1 — historical.
    pub const V1_1: Self = Self { major: 1, minor: 1 };
    /// RTPS 2.0 — historical.
    pub const V2_0: Self = Self { major: 2, minor: 0 };
    /// RTPS 2.1 — historical.
    pub const V2_1: Self = Self { major: 2, minor: 1 };
    /// RTPS 2.2 — historical.
    pub const V2_2: Self = Self { major: 2, minor: 2 };
    /// RTPS 2.3 — historical.
    pub const V2_3: Self = Self { major: 2, minor: 3 };
    /// RTPS 2.4 — Cyclone DDS default before the update to 2.5.
    pub const V2_4: Self = Self { major: 2, minor: 4 };
    /// RTPS 2.5 (default for ZeroDDS).
    pub const V2_5: Self = Self { major: 2, minor: 5 };

    /// `PROTOCOLVERSION` — spec alias for the most recent supported
    /// value (RTPS 2.5).
    pub const CURRENT: Self = Self::V2_5;

    /// Bytes [major, minor].
    #[must_use]
    pub fn to_bytes(self) -> [u8; 2] {
        [self.major, self.minor]
    }

    /// Reads 2 bytes.
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

/// `VendorId`: 2-byte vendor identifier. ZeroDDS uses `0x01F0` as an
/// interim value from the OMG developer range, until an official
/// VendorId is requested.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VendorId(pub [u8; 2]);

impl VendorId {
    /// Wire size: 2 bytes.
    pub const WIRE_SIZE: usize = 2;

    /// Sentinel "unknown" (0x00, 0x00) — only for tests/stub.
    pub const UNKNOWN: Self = Self([0, 0]);

    /// ZeroDDS interim VendorId from the OMG developer range.
    pub const ZERODDS: Self = Self([0x01, 0xF0]);

    /// OCI OpenDDS (OMG-assigned VendorId).
    pub const OPENDDS: Self = Self([0x01, 0x03]);

    /// eProsima Fast DDS (OMG-assigned VendorId).
    pub const FASTDDS: Self = Self([0x01, 0x0F]);

    /// Eclipse Cyclone DDS (OMG-assigned VendorId).
    pub const CYCLONE: Self = Self([0x01, 0x10]);

    /// Bytes unchanged.
    #[must_use]
    pub fn to_bytes(self) -> [u8; 2] {
        self.0
    }

    /// Bytes unchanged.
    #[must_use]
    pub fn from_bytes(bytes: [u8; 2]) -> Self {
        Self(bytes)
    }
}

// ============================================================================
// GuidPrefix (§8.3.5.1)
// ============================================================================

/// `GuidPrefix`: 12-byte prefix of a GUID. Identifies a
/// participant; stays the same for all endpoints of the participant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct GuidPrefix(pub [u8; 12]);

impl GuidPrefix {
    /// Wire size: 12 bytes.
    pub const WIRE_SIZE: usize = 12;

    /// Sentinel "unknown".
    pub const UNKNOWN: Self = Self([0; 12]);

    /// Bytes unchanged.
    #[must_use]
    pub fn to_bytes(self) -> [u8; 12] {
        self.0
    }

    /// Bytes unchanged.
    #[must_use]
    pub fn from_bytes(bytes: [u8; 12]) -> Self {
        Self(bytes)
    }

    /// ZeroDDS convention (Spec `zerodds-zero-copy-1.0` §6 wave 4):
    /// the first 4 bytes of the GuidPrefix carry a deterministic
    /// host identifier (hash of the `gethostname` output). Two
    /// participants with an identical host-id prefix run on the
    /// same machine and can set up a same-host zero-copy path.
    ///
    /// The RTPS 2.5 spec §9.3.1.5 allows vendor-specific
    /// structuring of the first 8 bytes (vendor-specific);
    /// only the comparison semantics of the full 12 bytes is normative.
    #[must_use]
    pub fn host_id(self) -> [u8; 4] {
        [self.0[0], self.0[1], self.0[2], self.0[3]]
    }

    /// Returns `true` if both participants carry the same host-id
    /// prefix. See [`Self::host_id`].
    #[must_use]
    pub fn is_same_host(self, other: Self) -> bool {
        self.host_id() == other.host_id()
    }
}

// ============================================================================
// EntityId (§8.3.5.2 + Table 9.1)
// ============================================================================

/// `EntityKind`: classification of an endpoint. Spec Table 9.1.
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
    /// Converts a byte into an `EntityKind`. Unknown bytes
    /// are mapped to `Unknown` — this mirrors spec tolerance behaviour.
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

/// `EntityId`: 4-byte endpoint identifier within a participant.
/// Layout: 3 bytes `entity_key` + 1 byte `entity_kind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EntityId {
    /// First 3 bytes (key).
    pub entity_key: [u8; 3],
    /// Last byte (kind).
    pub entity_kind: EntityKind,
}

impl EntityId {
    /// Wire size: 4 bytes.
    pub const WIRE_SIZE: usize = 4;

    /// Sentinel.
    pub const UNKNOWN: Self = Self {
        entity_key: [0; 3],
        entity_kind: EntityKind::Unknown,
    };

    /// Reserved participant EntityId (Spec §9.3.1.2).
    pub const PARTICIPANT: Self = Self {
        entity_key: [0, 0, 1],
        entity_kind: EntityKind::Participant,
    };

    /// Constructs a user-writer endpoint with key.
    #[must_use]
    pub const fn user_writer_with_key(key: [u8; 3]) -> Self {
        Self {
            entity_key: key,
            entity_kind: EntityKind::UserWriterWithKey,
        }
    }

    /// Constructs a user-reader endpoint with key.
    #[must_use]
    pub const fn user_reader_with_key(key: [u8; 3]) -> Self {
        Self {
            entity_key: key,
            entity_kind: EntityKind::UserReaderWithKey,
        }
    }

    /// Constructs a user-writer endpoint without key (NoKey).
    /// Spec §9.3.1.2 Table 9.1: entity_kind=0x03 for NoKey writers.
    /// Important for cross-vendor interop: if the IDL type has no @key,
    /// the writer must be published as NoKey, otherwise a
    /// remote reader rejects the DATA submessage (writer/reader kind mismatch).
    #[must_use]
    pub const fn user_writer_no_key(key: [u8; 3]) -> Self {
        Self {
            entity_key: key,
            entity_kind: EntityKind::UserWriterNoKey,
        }
    }

    /// Constructs a user-reader endpoint without key (NoKey).
    /// Spec §9.3.1.2 Table 9.1: entity_kind=0x04 for NoKey readers.
    #[must_use]
    pub const fn user_reader_no_key(key: [u8; 3]) -> Self {
        Self {
            entity_key: key,
            entity_kind: EntityKind::UserReaderNoKey,
        }
    }

    /// SPDP Builtin Participant Writer (Spec §9.3.1.5 Table 9.4).
    /// Multicast beacon sender in the discovery path.
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

    /// `BUILTIN_PARTICIPANT_MESSAGE_WRITER` — Writer Liveliness
    /// Protocol (WLP). Sends `ParticipantMessageData` over the
    /// topic `DCPSParticipantMessage` (DDSI-RTPS 2.5 §8.4.13,
    /// §9.3.1.5 Tab. 9.4 — EntityKey `[00, 02, 00]`,
    /// EntityKind `BuiltinWriterWithKey = 0xC2`).
    pub const BUILTIN_PARTICIPANT_MESSAGE_WRITER: Self = Self {
        entity_key: [0, 0x02, 0x00],
        entity_kind: EntityKind::BuiltinWriterWithKey,
    };

    /// `BUILTIN_PARTICIPANT_MESSAGE_READER` — counterpart to the
    /// WLP writer (DDSI-RTPS 2.5 §8.4.13, §9.3.1.5 Tab. 9.4).
    pub const BUILTIN_PARTICIPANT_MESSAGE_READER: Self = Self {
        entity_key: [0, 0x02, 0x00],
        entity_kind: EntityKind::BuiltinReaderWithKey,
    };

    // TypeLookup Service (XTypes §7.6.3.3.4): RPC, no key.
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
    // DDS-Security 1.2 §7.4.7.1 Tab.7 — 12 secure builtin EntityIds
    // (C3.8). EntityKey layout per spec; kind = WithKey except for the
    // stateless topics (NoKey, since those topics are keyless).
    // ----------------------------------------------------------------

    /// `SEDP_BUILTIN_PUBLICATIONS_SECURE_WRITER` — secure SEDP
    /// publications writer (bit 16, §9.3.1.6 Tab.13).
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
    /// `BUILTIN_PARTICIPANT_MESSAGE_SECURE_WRITER` — secure WLP writer
    /// (bit 20, §7.4.7.1).
    pub const BUILTIN_PARTICIPANT_MESSAGE_SECURE_WRITER: Self = Self {
        entity_key: [0xff, 0x02, 0x00],
        entity_kind: EntityKind::BuiltinWriterWithKey,
    };
    /// `BUILTIN_PARTICIPANT_MESSAGE_SECURE_READER` — Bit 21.
    pub const BUILTIN_PARTICIPANT_MESSAGE_SECURE_READER: Self = Self {
        entity_key: [0xff, 0x02, 0x00],
        entity_kind: EntityKind::BuiltinReaderWithKey,
    };
    /// `BUILTIN_PARTICIPANT_STATELESS_MESSAGE_WRITER` — auth-handshake
    /// topic writer (bit 22, §7.4.7.1, §10.3.4 auth stateless wire).
    /// NoKey, since the stateless topic is keyless.
    pub const BUILTIN_PARTICIPANT_STATELESS_MESSAGE_WRITER: Self = Self {
        entity_key: [0x00, 0x02, 0x01],
        entity_kind: EntityKind::BuiltinWriterNoKey,
    };
    /// `BUILTIN_PARTICIPANT_STATELESS_MESSAGE_READER` — Bit 23.
    pub const BUILTIN_PARTICIPANT_STATELESS_MESSAGE_READER: Self = Self {
        entity_key: [0x00, 0x02, 0x01],
        entity_kind: EntityKind::BuiltinReaderNoKey,
    };
    /// `BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_WRITER` — crypto
    /// key-exchange topic writer (bit 24, §7.4.7.1, §10.5.4
    /// VolatileMessageSecure wire).
    pub const BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_WRITER: Self = Self {
        entity_key: [0xff, 0x02, 0x02],
        // Kind byte 0xc3 (NoKey value) — Spec §7.4.7.1 Tab.7 + cyclone wire
        // (ff0202c3); NOT WithKey/0xc2, otherwise the crypto channel does
        // not match cross-vendor.
        entity_kind: EntityKind::BuiltinWriterNoKey,
    };
    /// `BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_READER` — Bit 25.
    pub const BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_READER: Self = Self {
        entity_key: [0xff, 0x02, 0x02],
        // Kind byte 0xc4 (NoKey value) — cyclone wire (ff0202c4).
        entity_kind: EntityKind::BuiltinReaderNoKey,
    };
    /// `SPDP_RELIABLE_BUILTIN_PARTICIPANTS_SECURE_WRITER` — secure
    /// SPDP writer for the DCPSParticipantsSecure topic (bit 26).
    pub const SPDP_RELIABLE_BUILTIN_PARTICIPANTS_SECURE_WRITER: Self = Self {
        entity_key: [0xff, 0x01, 0x01],
        entity_kind: EntityKind::BuiltinWriterWithKey,
    };
    /// `SPDP_RELIABLE_BUILTIN_PARTICIPANTS_SECURE_READER` — Bit 27.
    pub const SPDP_RELIABLE_BUILTIN_PARTICIPANTS_SECURE_READER: Self = Self {
        entity_key: [0xff, 0x01, 0x01],
        entity_kind: EntityKind::BuiltinReaderWithKey,
    };

    /// True if this is one of the 12 secure builtin EntityIds from
    /// DDS-Security 1.2 §7.4.7.1 Tab.7.
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

    /// Reads 4 bytes.
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

/// `Guid`: GuidPrefix + EntityId = 16 bytes. Globally unique identifier
/// of an endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Guid {
    /// Participant prefix.
    pub prefix: GuidPrefix,
    /// Endpoint identifier within the participant.
    pub entity_id: EntityId,
}

impl Guid {
    /// Wire size: 16 bytes.
    pub const WIRE_SIZE: usize = 16;

    /// Sentinel.
    pub const UNKNOWN: Self = Self {
        prefix: GuidPrefix::UNKNOWN,
        entity_id: EntityId::UNKNOWN,
    };

    /// Constructs a Guid from prefix + EntityId.
    #[must_use]
    pub const fn new(prefix: GuidPrefix, entity_id: EntityId) -> Self {
        Self { prefix, entity_id }
    }

    /// Bytes (prefix + EntityId).
    #[must_use]
    pub fn to_bytes(self) -> [u8; 16] {
        let mut out = [0u8; 16];
        out[..12].copy_from_slice(&self.prefix.to_bytes());
        out[12..].copy_from_slice(&self.entity_id.to_bytes());
        out
    }

    /// Reads 16 bytes.
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

/// `SequenceNumber`: 64-bit signed, encoded as (high: i32, low: u32).
/// Both fields are written with the active submessage endianness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SequenceNumber(pub i64);

impl SequenceNumber {
    /// Wire size: 8 bytes.
    pub const WIRE_SIZE: usize = 8;

    /// Sentinel "unknown" (high=-1, low=0) → -2^32.
    pub const UNKNOWN: Self = Self(-(1_i64 << 32));

    /// Splits the i64 into (high, low) per spec.
    #[must_use]
    pub fn split(self) -> (i32, u32) {
        let value = self.0;
        let high = (value >> 32) as i32;
        let low = (value & 0xFFFF_FFFF) as u32;
        (high, low)
    }

    /// Assembles from (high, low).
    #[must_use]
    pub fn from_high_low(high: i32, low: u32) -> Self {
        let value = (i64::from(high) << 32) | i64::from(low);
        Self(value)
    }

    /// Writes into 8 bytes with the given endianness (LE or BE).
    #[must_use]
    pub fn to_bytes_le(self) -> [u8; 8] {
        let (high, low) = self.split();
        let mut out = [0u8; 8];
        out[..4].copy_from_slice(&high.to_le_bytes());
        out[4..].copy_from_slice(&low.to_le_bytes());
        out
    }

    /// BE variant.
    #[must_use]
    pub fn to_bytes_be(self) -> [u8; 8] {
        let (high, low) = self.split();
        let mut out = [0u8; 8];
        out[..4].copy_from_slice(&high.to_be_bytes());
        out[4..].copy_from_slice(&low.to_be_bytes());
        out
    }

    /// LE decoder.
    #[must_use]
    pub fn from_bytes_le(bytes: [u8; 8]) -> Self {
        let mut hi = [0u8; 4];
        hi.copy_from_slice(&bytes[..4]);
        let mut lo = [0u8; 4];
        lo.copy_from_slice(&bytes[4..]);
        Self::from_high_low(i32::from_le_bytes(hi), u32::from_le_bytes(lo))
    }

    /// BE decoder.
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
// Vendor extension slots (§8.3.2 UExtension4_t / WExtension8_t)
// ============================================================================

/// `UExtension4_t` — 4-byte vendor-specific extension slot.
/// Spec §8.3.2: opaque 32-bit value, the vendor decides its meaning;
/// the receiver propagates the value via the `extensions` field into the
/// receiver state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct UExtension4(pub [u8; 4]);

impl UExtension4 {
    /// Wire size: 4 bytes.
    pub const WIRE_SIZE: usize = 4;

    /// Constructor from u32 (big-endian).
    #[must_use]
    pub fn from_u32_be(v: u32) -> Self {
        Self(v.to_be_bytes())
    }

    /// Returns the value as u32 (big-endian interpretation).
    #[must_use]
    pub fn to_u32_be(self) -> u32 {
        u32::from_be_bytes(self.0)
    }

    /// Roundtrip identity.
    #[must_use]
    pub fn to_bytes(self) -> [u8; 4] {
        self.0
    }

    /// Roundtrip identity.
    #[must_use]
    pub fn from_bytes(bytes: [u8; 4]) -> Self {
        Self(bytes)
    }
}

/// `WExtension8_t` — 8-byte vendor-specific extension slot.
/// Spec §8.3.2: opaque 64-bit value (analogous to UExtension4_t for
/// fields that need 8 bytes, e.g. for 64-bit counters).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct WExtension8(pub [u8; 8]);

impl WExtension8 {
    /// Wire size: 8 bytes.
    pub const WIRE_SIZE: usize = 8;

    /// Constructor from u64 (big-endian).
    #[must_use]
    pub fn from_u64_be(v: u64) -> Self {
        Self(v.to_be_bytes())
    }

    /// Returns the value as u64 (big-endian interpretation).
    #[must_use]
    pub fn to_u64_be(self) -> u64 {
        u64::from_be_bytes(self.0)
    }

    /// Roundtrip identity.
    #[must_use]
    pub fn to_bytes(self) -> [u8; 8] {
        self.0
    }

    /// Roundtrip identity.
    #[must_use]
    pub fn from_bytes(bytes: [u8; 8]) -> Self {
        Self(bytes)
    }
}

// ============================================================================
// FragmentNumber (§8.3.5.7)
// ============================================================================

/// `FragmentNumber`: 32-bit unsigned, 1-based (fragment #1 is the
/// first fragment of a sample). `UNKNOWN` = 0 is used as a sentinel,
/// but is not a valid fragment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FragmentNumber(pub u32);

impl FragmentNumber {
    /// Wire size: 4 bytes.
    pub const WIRE_SIZE: usize = 4;

    /// Sentinel "unknown" (= 0). Never a valid fragment.
    pub const UNKNOWN: Self = Self(0);

    /// LE bytes.
    #[must_use]
    pub fn to_bytes_le(self) -> [u8; 4] {
        self.0.to_le_bytes()
    }

    /// BE bytes.
    #[must_use]
    pub fn to_bytes_be(self) -> [u8; 4] {
        self.0.to_be_bytes()
    }

    /// LE decoder.
    #[must_use]
    pub fn from_bytes_le(bytes: [u8; 4]) -> Self {
        Self(u32::from_le_bytes(bytes))
    }

    /// BE decoder.
    #[must_use]
    pub fn from_bytes_be(bytes: [u8; 4]) -> Self {
        Self(u32::from_be_bytes(bytes))
    }
}

// ============================================================================
// Locator (§8.3.5.7)
// ============================================================================

/// `LocatorKind`: address family of a locator.
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
    /// Shared memory (vendor range, MSB=1 per DDSI-RTPS §9.3.1.2 —
    /// negative `i32` values are vendor-specific). `0x8100_0000` as the
    /// ZeroDDS vendor token; Cyclone + Fast-DDS ignore unknown
    /// kinds.
    Shm = -2_130_706_432, // 0x8100_0000 as i32
    /// Unix domain socket (vendor range, ZeroDDS extension for
    /// containerized IPC when multicast is blocked or POSIX SHM
    /// does not work cross-container). `0x8100_0001`. The 16-byte
    /// address field carries an identifier that is resolved into a socket path
    /// under a configurable base directory
    /// (`/tmp/zerodds/uds/<hex>.sock`).
    Uds = -2_130_706_431, // 0x8100_0001 as i32
    /// Time-Sensitive-Networking L2 endpoint (vendor range, ZeroDDS
    /// extension for DDS-TSN 1.0 Annex A — RTPS-over-Ethernet with
    /// EtherType 0x88B5). `0x8100_0002`. The 16-byte address field
    /// carries the destination MAC (bytes 0..6) + optional VLAN VID
    /// (bytes 6..8, big-endian).
    Tsn = -2_130_706_430, // 0x8100_0002 as i32
}

impl LocatorKind {
    /// Returns the i32 wire value.
    #[must_use]
    pub fn as_i32(self) -> i32 {
        self as i32
    }

    /// Converts i32 into a LocatorKind.
    ///
    /// # Errors
    /// `WireError::InvalidLocatorKind` for an unknown value.
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
            -2_130_706_430 => Ok(Self::Tsn),
            other => Err(WireError::InvalidLocatorKind { kind: other }),
        }
    }
}

/// SPDP default multicast address (Spec §9.6.1.4.1): `239.255.0.1`.
pub const SPDP_DEFAULT_MULTICAST_ADDRESS: [u8; 4] = [239, 255, 0, 1];

/// SPDP discovery port base (Spec §9.6.1.4.1, PB).
pub const SPDP_PORT_BASE: u32 = 7400;

/// Domain-specific port offset (Spec §9.6.1.4.1, DG).
pub const SPDP_DOMAIN_GAIN: u32 = 250;

/// Multicast discovery port offset (Spec §9.6.1.4.1, d0).
pub const SPDP_DISCOVERY_MULTICAST_OFFSET: u32 = 0;

/// Computes the SPDP multicast discovery port for a domain.
/// Formula (Spec §9.6.1.4.1):
///   port = PB + DG * domain_id + d0
///        = 7400 + 250 * domain + 0
#[must_use]
pub fn spdp_multicast_port(domain_id: u32) -> u32 {
    SPDP_PORT_BASE + SPDP_DOMAIN_GAIN * domain_id + SPDP_DISCOVERY_MULTICAST_OFFSET
}

/// `Locator`: 24-byte address (kind + port + 16-byte address).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Locator {
    /// Address family.
    pub kind: LocatorKind,
    /// UDP port.
    pub port: u32,
    /// 16-byte address. UDPv4 uses the last 4 bytes, 0 before that.
    pub address: [u8; 16],
}

impl Locator {
    /// Wire size: 24 bytes.
    pub const WIRE_SIZE: usize = 24;

    /// Spec §8.3.5.7 `LOCATOR_INVALID`.
    pub const INVALID: Self = Self {
        kind: LocatorKind::Invalid,
        port: 0,
        address: [0; 16],
    };

    /// Spec §8.3.5.7 `LOCATOR_KIND_RESERVED` template (kind=0,
    /// port=0, address=0).
    pub const RESERVED: Self = Self {
        kind: LocatorKind::Reserved,
        port: 0,
        address: [0; 16],
    };

    /// Spec §8.3.5.7 — default UDPv4 locator (kind=1, port=0, addr=0).
    pub const UDP_V4_ANY: Self = Self {
        kind: LocatorKind::UdpV4,
        port: 0,
        address: [0; 16],
    };

    /// Spec §8.3.5.7 — default UDPv6 locator (kind=2, port=0, addr=0).
    pub const UDP_V6_ANY: Self = Self {
        kind: LocatorKind::UdpV6,
        port: 0,
        address: [0; 16],
    };

    /// ZeroDDS vendor-extension SHM locator template.
    pub const SHM_ANY: Self = Self {
        kind: LocatorKind::Shm,
        port: 0,
        address: [0; 16],
    };

    /// Spec §8.3.5.7 — `LOCATOR_PORT_INVALID` sentinel.
    pub const PORT_INVALID: u32 = 0;

    /// Spec §8.3.5.7 — `LOCATOR_ADDRESS_INVALID` sentinel.
    pub const ADDRESS_INVALID: [u8; 16] = [0; 16];

    /// Constructor for UDPv6 (16-byte address + port).
    #[must_use]
    pub fn udp_v6(addr: [u8; 16], port: u32) -> Self {
        Self {
            kind: LocatorKind::UdpV6,
            port,
            address: addr,
        }
    }

    /// Constructor for UDPv4 (a.b.c.d:port).
    #[must_use]
    pub fn udp_v4(addr: [u8; 4], port: u32) -> Self {
        Self::with_address(LocatorKind::UdpV4, addr, port)
    }

    /// Constructor for TCPv4 (DDS-TCP-PSM §4).
    #[must_use]
    pub fn tcp_v4(addr: [u8; 4], port: u32) -> Self {
        Self::with_address(LocatorKind::Tcpv4, addr, port)
    }

    /// Constructor for TCPv6 (DDSI-RTPS 2.5 §9.4, LocatorKind=8).
    /// 16-byte network-byte-order address.
    #[must_use]
    pub fn tcp_v6(addr: [u8; 16], port: u32) -> Self {
        Self {
            kind: LocatorKind::Tcpv6,
            port,
            address: addr,
        }
    }

    /// Legacy alias for [`Self::tcp_v4`]. **Deprecated**: rename
    /// call sites to `tcp_v4` (consistent with `udp_v4`).
    #[must_use]
    #[deprecated(note = "use Locator::tcp_v4 instead (naming consistency)")]
    pub fn new_tcp_v4(addr: [u8; 4], port: u32) -> Self {
        Self::tcp_v4(addr, port)
    }

    /// Constructor for a Unix domain socket endpoint with a 16-byte
    /// ID. The port stays 0 (not meaningful for UDS). The transport
    /// resolves the ID to `/<base_dir>/<hex>.sock`.
    #[must_use]
    pub fn uds(id: [u8; 16]) -> Self {
        Self {
            kind: LocatorKind::Uds,
            port: 0,
            address: id,
        }
    }

    /// Constructor for a shared-memory segment with an ID (16-byte
    /// token). The port field stays 0 (not meaningful for SHM).
    #[must_use]
    pub fn shm(id: [u8; 16]) -> Self {
        Self {
            kind: LocatorKind::Shm,
            port: 0,
            address: id,
        }
    }

    /// Constructor for a TSN L2 endpoint (DDS-TSN 1.0 Annex A).
    /// `mac` is the 6-byte destination MAC, `vlan_vid` the IEEE 802.1Q VID
    /// (0 = untagged). Layout in the 16-byte address field: bytes 0..6 MAC,
    /// bytes 6..8 VID (big-endian), rest 0. The port stays 0.
    #[must_use]
    pub fn tsn(mac: [u8; 6], vlan_vid: u16) -> Self {
        let mut address = [0u8; 16];
        address[..6].copy_from_slice(&mac);
        address[6..8].copy_from_slice(&vlan_vid.to_be_bytes());
        Self {
            kind: LocatorKind::Tsn,
            port: 0,
            address,
        }
    }

    /// Shared IPv4 constructor. Places the IPv4 bytes in the last
    /// 4 bytes of the 16-byte `address` field (mapped IPv4-in-IPv6 layout).
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

    /// Extracts the IPv4 address (only meaningful for kind == UdpV4).
    #[must_use]
    pub fn ipv4(self) -> [u8; 4] {
        let mut out = [0u8; 4];
        out.copy_from_slice(&self.address[12..]);
        out
    }

    /// LE encoder.
    #[must_use]
    pub fn to_bytes_le(self) -> [u8; 24] {
        let mut out = [0u8; 24];
        out[..4].copy_from_slice(&self.kind.as_i32().to_le_bytes());
        out[4..8].copy_from_slice(&self.port.to_le_bytes());
        out[8..].copy_from_slice(&self.address);
        out
    }

    /// BE encoder (for PL_CDR_BE, e.g. handshake `c.pdata`).
    #[must_use]
    pub fn to_bytes_be(self) -> [u8; 24] {
        let mut out = [0u8; 24];
        out[..4].copy_from_slice(&self.kind.as_i32().to_be_bytes());
        out[4..8].copy_from_slice(&self.port.to_be_bytes());
        out[8..].copy_from_slice(&self.address);
        out
    }

    /// LE decoder.
    ///
    /// # Errors
    /// `WireError::InvalidLocatorKind` on an unknown kind.
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

    // ---- Secure builtin EntityIds (DDS-Security 1.2 §7.4.7.1, C3.8) ----

    #[test]
    fn secure_publications_writer_layout() {
        // Spec: key=[0xff, 0x00, 0x03], kind=BuiltinWriterWithKey=0xC2.
        let id = EntityId::SEDP_BUILTIN_PUBLICATIONS_SECURE_WRITER;
        assert_eq!(id.to_bytes(), [0xff, 0x00, 0x03, 0xC2]);
    }

    #[test]
    fn stateless_writer_is_no_key_kind() {
        // Stateless topic is keyless → BuiltinWriterNoKey=0xC3.
        let id = EntityId::BUILTIN_PARTICIPANT_STATELESS_MESSAGE_WRITER;
        assert_eq!(id.entity_kind, EntityKind::BuiltinWriterNoKey);
        assert_eq!(id.to_bytes(), [0x00, 0x02, 0x01, 0xC3]);
    }

    #[test]
    fn volatile_secure_writer_layout() {
        // Spec §7.4.7.1 Tab.7 + cyclone wire: 0xff0202c3 (NoKey kind byte),
        // NOT 0xc2 — otherwise the crypto-token channel does not match cross-vendor.
        let id = EntityId::BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_WRITER;
        assert_eq!(id.to_bytes(), [0xff, 0x02, 0x02, 0xC3]);
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

    // ---- §8.3.5.7 Locator constants ----

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
        assert!(Locator::SHM_ANY.kind.as_i32() < 0); // vendor range
    }

    #[test]
    fn locator_tsn_carries_mac_and_vlan() {
        let mac = [0x02, 0x00, 0x00, 0x11, 0x22, 0x33];
        let loc = Locator::tsn(mac, 42);
        assert_eq!(loc.kind, LocatorKind::Tsn);
        assert_eq!(&loc.address[..6], &mac);
        assert_eq!(u16::from_be_bytes([loc.address[6], loc.address[7]]), 42);
        // Vendor range + i32 roundtrip.
        assert!(loc.kind.as_i32() < 0);
        assert_eq!(
            LocatorKind::from_i32(loc.kind.as_i32()).unwrap(),
            LocatorKind::Tsn
        );
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

    // ---- §8.3.5.5 ProtocolVersion aliases ----

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

    // ---- SPDP builtin EntityIds (Spec §9.3.1.5 Table 9.4) ----

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

    // ---- SPDP multicast address + port computation ----

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

    #[test]
    fn volatile_message_secure_entity_ids_match_spec_and_cyclone() {
        // DDS-Security §7.4.7.1 Tab.7 + cyclone-wire-verified
        // (trace: new_writer ...:ff0202c3 / new_reader ...:ff0202c4 for
        // DCPSParticipantVolatileMessageSecure). The kind bytes are c3/c4
        // (NoKey bytes), NOT c2/c7 — otherwise the crypto-token channel does
        // not match cross-vendor.
        assert_eq!(
            EntityId::BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_WRITER.to_bytes(),
            [0xff, 0x02, 0x02, 0xc3],
            "VolatileMessageSecure writer must be 0xff0202c3"
        );
        assert_eq!(
            EntityId::BUILTIN_PARTICIPANT_VOLATILE_MESSAGE_SECURE_READER.to_bytes(),
            [0xff, 0x02, 0x02, 0xc4],
            "VolatileMessageSecure reader must be 0xff0202c4"
        );
    }
}
