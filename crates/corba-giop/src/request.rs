// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Request-Message — Spec §15.4.2.
//!
//! GIOP 1.0 (`§15.4.2.1`):
//! ```text
//! struct RequestHeader_1_0 {
//!     IOP::ServiceContextList service_context;
//!     unsigned long           request_id;
//!     boolean                 response_expected;
//!     sequence<octet>         object_key;
//!     string                  operation;
//!     CSI::AuthorizationToken requesting_principal;  // sequence<octet>
//! };
//! ```
//!
//! GIOP 1.1 — wie 1.0 plus 3-Byte-`reserved` zwischen
//! `response_expected` und `object_key` (Spec §15.4.2.1).
//!
//! GIOP 1.2 (`§15.4.2.2`):
//! ```text
//! struct RequestHeader_1_2 {
//!     unsigned long           request_id;
//!     octet                   response_flags;
//!     octet                   reserved[3];
//!     TargetAddress           target;
//!     string                  operation;
//!     IOP::ServiceContextList service_context;
//!     // body follows after CDR-alignment to 8
//! };
//! ```

use alloc::string::String;
use alloc::vec::Vec;

use zerodds_cdr::{BufferReader, BufferWriter};

use crate::error::{GiopError, GiopResult};
use crate::service_context::ServiceContextList;
use crate::target_address::TargetAddress;
use crate::version::Version;

/// `response_flags`-Octet (Spec §15.4.2.2).
///
/// Bits encodieren das `Messaging::SyncScope`-Modell aus CORBA-
/// Messaging (Spec §22.2.5):
/// * `SYNC_NONE = 0x00` — fire-and-forget.
/// * `SYNC_WITH_TRANSPORT = 0x01` — Reply nach Transport-Buffer-Send.
/// * `SYNC_WITH_SERVER = 0x02` — Reply nach Server-Receive.
/// * `SYNC_WITH_TARGET = 0x03` — Reply nach Servant-Invocation
///   (klassisches synchrones Verhalten).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ResponseFlags(pub u8);

impl ResponseFlags {
    /// `SYNC_NONE` (Spec §22.2.5).
    pub const SYNC_NONE: Self = Self(0x00);
    /// `SYNC_WITH_TRANSPORT`.
    pub const SYNC_WITH_TRANSPORT: Self = Self(0x01);
    /// `SYNC_WITH_SERVER`.
    pub const SYNC_WITH_SERVER: Self = Self(0x02);
    /// `SYNC_WITH_TARGET` — Default fuer synchrone GIOP-1.0/1.1-
    /// Requests (`response_expected = true`).
    pub const SYNC_WITH_TARGET: Self = Self(0x03);

    /// `true` wenn der Caller einen Reply erwartet (`SyncScope >=
    /// SYNC_WITH_SERVER`, Spec §22.2.5).
    #[must_use]
    pub const fn response_expected(self) -> bool {
        self.0 >= Self::SYNC_WITH_SERVER.0
    }

    /// Konvertiert von GIOP-1.0/1.1-Boolean zu Flags-Octet.
    #[must_use]
    pub const fn from_response_expected(response_expected: bool) -> Self {
        if response_expected {
            Self::SYNC_WITH_TARGET
        } else {
            Self::SYNC_NONE
        }
    }
}

/// Request-Message-Body (versions-uniform Repraesentation).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    /// `request_id`.
    pub request_id: u32,
    /// `response_flags` (GIOP 1.2) bzw. `response_expected`-Aequivalent.
    pub response_flags: ResponseFlags,
    /// Object-Adressing — in GIOP 1.0/1.1 immer `Key`; in GIOP 1.2
    /// kann jede `TargetAddress`-Variante vorkommen.
    pub target: TargetAddress,
    /// Operation-Name (`string`, NUL-terminiert ueber CDR).
    pub operation: String,
    /// `requesting_principal` (`CSI::AuthorizationToken` =
    /// `sequence<octet>`). In GIOP 1.2 entfernt; wir halten das Feld
    /// als Option fuer Versions-Inter-Op.
    pub requesting_principal: Option<Vec<u8>>,
    /// `service_context_list`.
    pub service_context: ServiceContextList,
    /// Body-Bytes (CDR-encoded `in`/`inout`-Args). Caller liefert
    /// die fertig-kodierten Bytes; dieser Codec haengt sie unaligned
    /// an den Header — das `body`-Alignment-Anchor (8 Bytes ab
    /// Header-Start) ist in GIOP 1.2 spec-relevant und wird vom
    /// Encoder erzwungen (Spec §15.4.2.2 normativ).
    pub body: Vec<u8>,
}

impl Request {
    /// Konstruktor.
    #[must_use]
    pub fn new(
        request_id: u32,
        response_flags: ResponseFlags,
        target: TargetAddress,
        operation: String,
    ) -> Self {
        Self {
            request_id,
            response_flags,
            target,
            operation,
            requesting_principal: None,
            service_context: ServiceContextList::default(),
            body: Vec::new(),
        }
    }

    /// CDR-Encode in einen `BufferWriter`. Body-Alignment auf 8 Bytes
    /// ab Header-Start (= 8 Bytes ab Buffer-Start in GIOP 1.2,
    /// Spec §15.4 normativ).
    ///
    /// # Errors
    /// Buffer-Schreibfehler oder Length-Overflow.
    pub fn encode(&self, version: Version, w: &mut BufferWriter) -> GiopResult<()> {
        if version.uses_v1_2_request_layout() {
            // GIOP 1.2 — request_id zuerst.
            w.write_u32(self.request_id)?;
            w.write_u8(self.response_flags.0)?;
            // 3 Bytes reserved.
            w.write_u8(0)?;
            w.write_u8(0)?;
            w.write_u8(0)?;
            // TargetAddress union.
            self.target.encode(w)?;
            write_string(w, &self.operation)?;
            self.service_context.encode(w)?;
            // Body 8-Byte-aligned (Spec §15.4 normativ ab GIOP 1.2).
            w.align(8);
        } else {
            // GIOP 1.0 / 1.1 — service_context zuerst.
            self.service_context.encode(w)?;
            w.write_u32(self.request_id)?;
            // boolean response_expected.
            w.write_u8(u8::from(self.response_flags.response_expected()))?;
            if version >= Version::V1_1 {
                // GIOP 1.1: 3-Byte-reserved.
                w.write_u8(0)?;
                w.write_u8(0)?;
                w.write_u8(0)?;
            }
            // object_key sequence<octet> — TargetAddress muss
            // KeyAddr-Form sein.
            let key = match &self.target {
                TargetAddress::Key(k) => k.as_slice(),
                _ => {
                    return Err(GiopError::Malformed(
                        "GIOP 1.0/1.1 only supports TargetAddress::Key".into(),
                    ));
                }
            };
            let n = u32::try_from(key.len())
                .map_err(|_| GiopError::Malformed("object_key too long".into()))?;
            w.write_u32(n)?;
            w.write_bytes(key)?;
            write_string(w, &self.operation)?;
            // requesting_principal sequence<octet>.
            let p = self.requesting_principal.as_deref().unwrap_or(&[]);
            let pn = u32::try_from(p.len())
                .map_err(|_| GiopError::Malformed("principal too long".into()))?;
            w.write_u32(pn)?;
            w.write_bytes(p)?;
        }
        // Body-Bytes.
        w.write_bytes(&self.body)?;
        Ok(())
    }

    /// CDR-Decode aus `BufferReader`.
    ///
    /// # Errors
    /// Buffer-Lesefehler.
    pub fn decode(version: Version, r: &mut BufferReader<'_>) -> GiopResult<Self> {
        if version.uses_v1_2_request_layout() {
            let request_id = r.read_u32()?;
            let response_flags = ResponseFlags(r.read_u8()?);
            // Skip 3 reserved bytes.
            let _ = r.read_u8()?;
            let _ = r.read_u8()?;
            let _ = r.read_u8()?;
            let target = TargetAddress::decode(r)?;
            let operation = read_string(r)?;
            let service_context = ServiceContextList::decode(r)?;
            // Body-Alignment 8 ab Buffer-Start.
            r.align(8)?;
            let body = r.read_bytes(r.remaining())?.to_vec();
            Ok(Self {
                request_id,
                response_flags,
                target,
                operation,
                requesting_principal: None,
                service_context,
                body,
            })
        } else {
            let service_context = ServiceContextList::decode(r)?;
            let request_id = r.read_u32()?;
            let response_expected = r.read_u8()? != 0;
            let response_flags = ResponseFlags::from_response_expected(response_expected);
            if version >= Version::V1_1 {
                let _ = r.read_u8()?;
                let _ = r.read_u8()?;
                let _ = r.read_u8()?;
            }
            let key_len = r.read_u32()? as usize;
            let key_bytes = r.read_bytes(key_len)?;
            let target = TargetAddress::Key(key_bytes.to_vec());
            let operation = read_string(r)?;
            let pn = r.read_u32()? as usize;
            let principal = r.read_bytes(pn)?.to_vec();
            let body = r.read_bytes(r.remaining())?.to_vec();
            Ok(Self {
                request_id,
                response_flags,
                target,
                operation,
                requesting_principal: Some(principal),
                service_context,
                body,
            })
        }
    }
}

/// CDR-`string`-Encoder (length-prefix + bytes + NUL-terminator).
fn write_string(w: &mut BufferWriter, s: &str) -> GiopResult<()> {
    w.write_string(s)?;
    Ok(())
}

/// CDR-`string`-Decoder.
fn read_string(r: &mut BufferReader<'_>) -> GiopResult<String> {
    Ok(r.read_string()?)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use zerodds_cdr::Endianness;

    #[test]
    fn response_flags_sync_levels_match_spec() {
        // Spec §22.2.5.
        assert_eq!(ResponseFlags::SYNC_NONE.0, 0);
        assert_eq!(ResponseFlags::SYNC_WITH_TRANSPORT.0, 1);
        assert_eq!(ResponseFlags::SYNC_WITH_SERVER.0, 2);
        assert_eq!(ResponseFlags::SYNC_WITH_TARGET.0, 3);
    }

    #[test]
    fn response_expected_returns_true_at_with_server_or_higher() {
        assert!(!ResponseFlags::SYNC_NONE.response_expected());
        assert!(!ResponseFlags::SYNC_WITH_TRANSPORT.response_expected());
        assert!(ResponseFlags::SYNC_WITH_SERVER.response_expected());
        assert!(ResponseFlags::SYNC_WITH_TARGET.response_expected());
    }

    fn sample_request(target: TargetAddress) -> Request {
        Request {
            request_id: 7,
            response_flags: ResponseFlags::SYNC_WITH_TARGET,
            target,
            operation: "ping".into(),
            requesting_principal: Some(alloc::vec::Vec::new()),
            service_context: ServiceContextList::default(),
            body: alloc::vec![1, 2, 3, 4],
        }
    }

    #[test]
    fn round_trip_giop_1_0_request() {
        let req = sample_request(TargetAddress::Key(alloc::vec![0xab, 0xcd]));
        let mut w = BufferWriter::new(Endianness::Big);
        req.encode(Version::V1_0, &mut w).unwrap();
        let bytes = w.into_bytes();
        let mut r = BufferReader::new(&bytes, Endianness::Big);
        let decoded = Request::decode(Version::V1_0, &mut r).unwrap();
        assert_eq!(decoded, req);
    }

    #[test]
    fn round_trip_giop_1_1_request_with_reserved_bytes() {
        let req = sample_request(TargetAddress::Key(alloc::vec![0x10, 0x20, 0x30]));
        let mut w = BufferWriter::new(Endianness::Little);
        req.encode(Version::V1_1, &mut w).unwrap();
        let bytes = w.into_bytes();
        let mut r = BufferReader::new(&bytes, Endianness::Little);
        let decoded = Request::decode(Version::V1_1, &mut r).unwrap();
        assert_eq!(decoded, req);
    }

    #[test]
    fn round_trip_giop_1_2_request_with_target_address() {
        let mut req = sample_request(TargetAddress::Key(alloc::vec![0x11, 0x22]));
        // GIOP 1.2 — kein requesting_principal.
        req.requesting_principal = None;
        let mut w = BufferWriter::new(Endianness::Big);
        req.encode(Version::V1_2, &mut w).unwrap();
        let bytes = w.into_bytes();
        let mut r = BufferReader::new(&bytes, Endianness::Big);
        let decoded = Request::decode(Version::V1_2, &mut r).unwrap();
        assert_eq!(decoded, req);
    }

    #[test]
    fn giop_1_0_rejects_profile_target_address() {
        let req = sample_request(TargetAddress::Profile(alloc::vec![1, 2]));
        let mut w = BufferWriter::new(Endianness::Big);
        let err = req.encode(Version::V1_0, &mut w).unwrap_err();
        assert!(matches!(err, GiopError::Malformed(_)));
    }

    #[test]
    fn giop_1_2_request_body_is_8_aligned() {
        // Spec §15.4 normativ: in GIOP 1.2 ist Body 8-aligned ab
        // Header-Start. Wir testen indirekt ueber Buffer-Position.
        let req = Request {
            request_id: 1,
            response_flags: ResponseFlags::SYNC_WITH_TARGET,
            target: TargetAddress::Key(alloc::vec![0xa]),
            operation: "x".into(),
            requesting_principal: None,
            service_context: ServiceContextList::default(),
            body: alloc::vec![0xff],
        };
        let mut w = BufferWriter::new(Endianness::Big);
        req.encode(Version::V1_2, &mut w).unwrap();
        let bytes = w.into_bytes();
        // Letzten Body-Byte rueckwaerts suchen.
        let body_pos = bytes.iter().rposition(|b| *b == 0xff).unwrap();
        assert_eq!(
            body_pos % 8,
            0,
            "body must be 8-byte aligned, got pos {body_pos}"
        );
    }
}
