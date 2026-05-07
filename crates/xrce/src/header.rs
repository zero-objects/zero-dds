// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! XRCE Message-Header (Spec §8.3.2).
//!
//! ```text
//!   0                   1                   2                   3
//!   0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
//!  +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
//!  |  session_id   |  stream_id    |        sequence_nr            |
//!  +---------------+---------------+---------------+---------------+
//!  |                                                               |
//!  |              client_key (4 bytes, optional)                   |
//!  +---------------+---------------+---------------+---------------+
//! ```
//!
//! - `sequence_nr` ist Little-Endian (Spec §8.3.2.3).
//! - `client_key` ist nur present, wenn `session_id <= 127`
//!   (§8.3.2.1).
//!
//! Reservierte Werte:
//! - `SESSION_ID_NONE_WITH_CLIENT_KEY = 0x00`
//! - `SESSION_ID_NONE_WITHOUT_CLIENT_KEY = 0x80`

use crate::error::XrceError;
use crate::serial_number::SerialNumber16;

/// Laenge des `ClientKey`-Feldes in Bytes.
pub const CLIENT_KEY_LEN: usize = 4;

/// Reservierte session_id ohne ProxyClient-Bindung, mit ClientKey
/// im Header (Spec §8.3.2.1).
pub const SESSION_ID_NONE_WITH_CLIENT_KEY: u8 = 0x00;

/// Reservierte session_id ohne ProxyClient-Bindung, ohne ClientKey
/// im Header — Authentifizierung via Source-Address (Spec §8.3.2.1).
pub const SESSION_ID_NONE_WITHOUT_CLIENT_KEY: u8 = 0x80;

/// `SessionId`. Werte 0..=127 → ClientKey im Header; 128..=255 →
/// ohne ClientKey.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct SessionId(pub u8);

impl SessionId {
    /// `true`, wenn der MessageHeader das `client_key`-Feld traegt.
    #[must_use]
    pub fn carries_client_key(self) -> bool {
        self.0 <= 127
    }
}

/// `StreamId`. Werte:
/// - `0` = `STREAMID_NONE` (Spec §8.3.5.11/12: ACKNACK/HEARTBEAT/
///   TIMESTAMP nutzen den NONE-Stream).
/// - `1..=127` = best-effort.
/// - `128..=255` = reliable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct StreamId(pub u8);

impl StreamId {
    /// `STREAMID_NONE`.
    pub const NONE: Self = Self(0);
    /// `STREAMID_BUILTIN_BEST_EFFORTS` (Spec §8.3.2.2).
    pub const BUILTIN_BEST_EFFORT: Self = Self(0x01);
    /// `STREAMID_BUILTIN_RELIABLE` (Spec §8.3.2.2).
    pub const BUILTIN_RELIABLE: Self = Self(0x80);

    /// `true` fuer reliable Streams (id >= 128).
    #[must_use]
    pub fn is_reliable(self) -> bool {
        self.0 >= 128
    }

    /// `true` fuer best-effort Streams (1..=127).
    #[must_use]
    pub fn is_best_effort(self) -> bool {
        self.0 >= 1 && self.0 <= 127
    }

    /// `true` fuer `STREAMID_NONE`.
    #[must_use]
    pub fn is_none(self) -> bool {
        self.0 == 0
    }
}

/// 4-Byte ClientKey (Spec §7.7 ClientKey).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ClientKey(pub [u8; CLIENT_KEY_LEN]);

impl ClientKey {
    /// `CLIENTKEY_INVALID` per §7.8.2.2 — alle Bytes 0.
    pub const INVALID: Self = Self([0; CLIENT_KEY_LEN]);

    /// Roher Slice.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8; CLIENT_KEY_LEN] {
        &self.0
    }
}

/// XRCE Message-Header.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct MessageHeader {
    /// Session-Identifier; entscheidet ob `client_key` present ist.
    pub session_id: SessionId,
    /// Stream-Identifier (best-effort/reliable/none).
    pub stream_id: StreamId,
    /// RFC-1982 16-bit Serial Number (Little-Endian on wire).
    pub sequence_nr: SerialNumber16,
    /// Optionaler ClientKey — nur bei `session_id <= 127`.
    pub client_key: Option<ClientKey>,
}

impl MessageHeader {
    /// Wire-Size **ohne** ClientKey: 4 Bytes.
    pub const WIRE_SIZE_NO_KEY: usize = 4;
    /// Wire-Size **mit** ClientKey: 8 Bytes.
    pub const WIRE_SIZE_WITH_KEY: usize = 4 + CLIENT_KEY_LEN;

    /// Konstruktor ohne ClientKey (session_id muss >=128 sein).
    ///
    /// # Errors
    /// `ValueOutOfRange`, wenn `session_id <= 127` waere — der Header
    /// muesste dann einen ClientKey tragen.
    pub fn without_client_key(
        session_id: SessionId,
        stream_id: StreamId,
        sequence_nr: SerialNumber16,
    ) -> Result<Self, XrceError> {
        if session_id.carries_client_key() {
            return Err(XrceError::ValueOutOfRange {
                message: "session_id <=127 requires client_key",
            });
        }
        Ok(Self {
            session_id,
            stream_id,
            sequence_nr,
            client_key: None,
        })
    }

    /// Konstruktor mit ClientKey (session_id muss <=127 sein).
    ///
    /// # Errors
    /// `ValueOutOfRange`, wenn `session_id >= 128` waere.
    pub fn with_client_key(
        session_id: SessionId,
        stream_id: StreamId,
        sequence_nr: SerialNumber16,
        client_key: ClientKey,
    ) -> Result<Self, XrceError> {
        if !session_id.carries_client_key() {
            return Err(XrceError::ValueOutOfRange {
                message: "session_id >=128 must not carry client_key",
            });
        }
        Ok(Self {
            session_id,
            stream_id,
            sequence_nr,
            client_key: Some(client_key),
        })
    }

    /// Wire-Size in Bytes (4 oder 8).
    #[must_use]
    pub fn wire_size(&self) -> usize {
        if self.client_key.is_some() {
            Self::WIRE_SIZE_WITH_KEY
        } else {
            Self::WIRE_SIZE_NO_KEY
        }
    }

    /// Encodiert den Header in den gegebenen Buffer und liefert die
    /// geschriebene Anzahl Bytes.
    ///
    /// # Errors
    /// `WriteOverflow`, wenn `out.len() < wire_size()`.
    pub fn write_to(&self, out: &mut [u8]) -> Result<usize, XrceError> {
        let needed = self.wire_size();
        if out.len() < needed {
            return Err(XrceError::WriteOverflow {
                needed,
                available: out.len(),
            });
        }
        out[0] = self.session_id.0;
        out[1] = self.stream_id.0;
        let seq = self.sequence_nr.raw().to_le_bytes();
        out[2] = seq[0];
        out[3] = seq[1];
        if let Some(ck) = self.client_key {
            out[4..8].copy_from_slice(&ck.0);
        }
        Ok(needed)
    }

    /// Decodiert einen Header aus einem Slice. Liefert
    /// `(header, bytes_consumed)`.
    ///
    /// # Errors
    /// `UnexpectedEof`, wenn `bytes` zu kurz fuer die je nach
    /// `session_id` gewaehlte Variante ist.
    pub fn read_from(bytes: &[u8]) -> Result<(Self, usize), XrceError> {
        if bytes.len() < Self::WIRE_SIZE_NO_KEY {
            return Err(XrceError::UnexpectedEof {
                needed: Self::WIRE_SIZE_NO_KEY,
                offset: 0,
            });
        }
        let session_id = SessionId(bytes[0]);
        let stream_id = StreamId(bytes[1]);
        let mut seq_buf = [0u8; 2];
        seq_buf.copy_from_slice(&bytes[2..4]);
        let sequence_nr = SerialNumber16::new(u16::from_le_bytes(seq_buf));

        if session_id.carries_client_key() {
            if bytes.len() < Self::WIRE_SIZE_WITH_KEY {
                return Err(XrceError::UnexpectedEof {
                    needed: Self::WIRE_SIZE_WITH_KEY,
                    offset: bytes.len(),
                });
            }
            let mut ck = [0u8; CLIENT_KEY_LEN];
            ck.copy_from_slice(&bytes[4..8]);
            Ok((
                Self {
                    session_id,
                    stream_id,
                    sequence_nr,
                    client_key: Some(ClientKey(ck)),
                },
                Self::WIRE_SIZE_WITH_KEY,
            ))
        } else {
            Ok((
                Self {
                    session_id,
                    stream_id,
                    sequence_nr,
                    client_key: None,
                },
                Self::WIRE_SIZE_NO_KEY,
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]
    use super::*;

    #[test]
    fn session_id_127_carries_client_key() {
        assert!(SessionId(0).carries_client_key());
        assert!(SessionId(127).carries_client_key());
        assert!(!SessionId(128).carries_client_key());
        assert!(!SessionId(255).carries_client_key());
    }

    #[test]
    fn stream_id_classification() {
        assert!(StreamId::NONE.is_none());
        assert!(StreamId::BUILTIN_BEST_EFFORT.is_best_effort());
        assert!(StreamId::BUILTIN_RELIABLE.is_reliable());
        assert!(StreamId(127).is_best_effort());
        assert!(StreamId(128).is_reliable());
        assert!(StreamId(255).is_reliable());
    }

    #[test]
    fn header_no_client_key_wire_size_4() {
        let h = MessageHeader::without_client_key(
            SessionId(SESSION_ID_NONE_WITHOUT_CLIENT_KEY),
            StreamId::NONE,
            SerialNumber16::new(0),
        )
        .unwrap();
        assert_eq!(h.wire_size(), 4);
    }

    #[test]
    fn header_with_client_key_wire_size_8() {
        let h = MessageHeader::with_client_key(
            SessionId(0),
            StreamId::BUILTIN_RELIABLE,
            SerialNumber16::new(0),
            ClientKey([1, 2, 3, 4]),
        )
        .unwrap();
        assert_eq!(h.wire_size(), 8);
    }

    #[test]
    fn header_constructor_rejects_inconsistent_no_key() {
        // session_id=10 (<=127) waere mit client_key — without_client_key muss fail
        let res = MessageHeader::without_client_key(
            SessionId(10),
            StreamId::NONE,
            SerialNumber16::new(0),
        );
        assert!(res.is_err());
    }

    #[test]
    fn header_constructor_rejects_inconsistent_with_key() {
        // session_id=200 (>=128) waere ohne client_key — with_client_key muss fail
        let res = MessageHeader::with_client_key(
            SessionId(200),
            StreamId::NONE,
            SerialNumber16::new(0),
            ClientKey::INVALID,
        );
        assert!(res.is_err());
    }

    #[test]
    fn header_layout_bytes_no_key() {
        let h = MessageHeader::without_client_key(
            SessionId(0x80),
            StreamId(0x42),
            SerialNumber16::new(0x1234),
        )
        .unwrap();
        let mut buf = [0u8; 4];
        let n = h.write_to(&mut buf).unwrap();
        assert_eq!(n, 4);
        assert_eq!(buf[0], 0x80);
        assert_eq!(buf[1], 0x42);
        // sequence_nr LE
        assert_eq!(buf[2], 0x34);
        assert_eq!(buf[3], 0x12);
    }

    #[test]
    fn header_layout_bytes_with_key() {
        let h = MessageHeader::with_client_key(
            SessionId(0x10),
            StreamId(0x80),
            SerialNumber16::new(0xABCD),
            ClientKey([0xAA, 0xBB, 0xCC, 0xDD]),
        )
        .unwrap();
        let mut buf = [0u8; 8];
        h.write_to(&mut buf).unwrap();
        assert_eq!(buf[0], 0x10);
        assert_eq!(buf[1], 0x80);
        assert_eq!(buf[2], 0xCD);
        assert_eq!(buf[3], 0xAB);
        assert_eq!(&buf[4..8], &[0xAA, 0xBB, 0xCC, 0xDD]);
    }

    #[test]
    fn header_roundtrip_no_key() {
        let h = MessageHeader::without_client_key(
            SessionId(SESSION_ID_NONE_WITHOUT_CLIENT_KEY),
            StreamId::BUILTIN_BEST_EFFORT,
            SerialNumber16::new(7),
        )
        .unwrap();
        let mut buf = [0u8; 4];
        h.write_to(&mut buf).unwrap();
        let (decoded, n) = MessageHeader::read_from(&buf).unwrap();
        assert_eq!(n, 4);
        assert_eq!(decoded, h);
    }

    #[test]
    fn header_roundtrip_with_key() {
        let h = MessageHeader::with_client_key(
            SessionId(0x55),
            StreamId::BUILTIN_RELIABLE,
            SerialNumber16::new(0xFFFE),
            ClientKey([9, 8, 7, 6]),
        )
        .unwrap();
        let mut buf = [0u8; 8];
        h.write_to(&mut buf).unwrap();
        let (decoded, n) = MessageHeader::read_from(&buf).unwrap();
        assert_eq!(n, 8);
        assert_eq!(decoded, h);
    }

    #[test]
    fn header_decode_truncated_no_key() {
        let buf = [0x80u8, 0x01, 0x00]; // 3 Byte
        let res = MessageHeader::read_from(&buf);
        assert!(matches!(
            res,
            Err(XrceError::UnexpectedEof { needed: 4, .. })
        ));
    }

    #[test]
    fn header_decode_truncated_with_key() {
        // session_id=0 (<=127) → braucht 8 Byte
        let buf = [0x00u8, 0x01, 0x00, 0x00, 0xAA]; // 5 Byte
        let res = MessageHeader::read_from(&buf);
        assert!(matches!(
            res,
            Err(XrceError::UnexpectedEof { needed: 8, .. })
        ));
    }

    #[test]
    fn header_write_overflow_when_buffer_too_small() {
        let h = MessageHeader::with_client_key(
            SessionId(0),
            StreamId::NONE,
            SerialNumber16::new(0),
            ClientKey::INVALID,
        )
        .unwrap();
        let mut buf = [0u8; 4]; // braucht 8
        let res = h.write_to(&mut buf);
        assert!(matches!(
            res,
            Err(XrceError::WriteOverflow {
                needed: 8,
                available: 4
            })
        ));
    }

    #[test]
    fn header_extra_trailing_bytes_are_ignored() {
        let h = MessageHeader::without_client_key(
            SessionId(0xFF),
            StreamId(2),
            SerialNumber16::new(42),
        )
        .unwrap();
        let mut buf = [0u8; 16];
        h.write_to(&mut buf).unwrap();
        let (decoded, n) = MessageHeader::read_from(&buf).unwrap();
        assert_eq!(decoded, h);
        assert_eq!(n, 4);
    }
}
