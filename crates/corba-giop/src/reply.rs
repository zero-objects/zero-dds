// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The Reply message — Spec §15.4.3.
//!
//! GIOP 1.0/1.1 (`§15.4.3.1`):
//! ```text
//! enum ReplyStatusType_1_0 {
//!     NO_EXCEPTION, USER_EXCEPTION, SYSTEM_EXCEPTION, LOCATION_FORWARD
//! };
//! struct ReplyHeader_1_0 {
//!     IOP::ServiceContextList service_context;
//!     unsigned long           request_id;
//!     ReplyStatusType_1_0     reply_status;
//! };
//! ```
//!
//! GIOP 1.2 (`§15.4.3.2`):
//! ```text
//! enum ReplyStatusType_1_2 {
//!     NO_EXCEPTION, USER_EXCEPTION, SYSTEM_EXCEPTION,
//!     LOCATION_FORWARD, LOCATION_FORWARD_PERM, NEEDS_ADDRESSING_MODE
//! };
//! struct ReplyHeader_1_2 {
//!     unsigned long           request_id;
//!     ReplyStatusType_1_2     reply_status;
//!     IOP::ServiceContextList service_context;
//!     // body 8-aligned
//! };
//! ```

use alloc::vec::Vec;

use zerodds_wire::{Reader, Writer};

use crate::error::{GiopError, GiopResult};
use crate::service_context::ServiceContextList;
use crate::version::Version;

/// The ZeroDDS vendor service context that moves a GIOP 1.0/1.1 reply
/// body onto an eight-aligned offset ([`Reply::place_body_at_eight`]):
/// four zero octets, ASCII `ZDPA` as its id — the vendor id space of
/// Part 2 §7.7.1 (an id whose high 20 bits are not zero), next to the
/// `ZDUD` component of the UIOP transport.
pub const PADDING_CONTEXT_ID: u32 = 0x5A44_5041;

/// Reply status — all 6 spec values (Spec §15.4.3.1 + §15.4.3.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum ReplyStatusType {
    /// `NO_EXCEPTION` — Operation success.
    NoException = 0,
    /// `USER_EXCEPTION` — IDL-declared user exception.
    UserException = 1,
    /// `SYSTEM_EXCEPTION` — system-level fault.
    SystemException = 2,
    /// `LOCATION_FORWARD` — the server asks the client to go to a
    /// different endpoint (transient).
    LocationForward = 3,
    /// `LOCATION_FORWARD_PERM` — as above, but persistent (GIOP 1.2+).
    LocationForwardPerm = 4,
    /// `NEEDS_ADDRESSING_MODE` — the server wants a different
    /// `TargetAddress` discriminator (GIOP 1.2+).
    NeedsAddressingMode = 5,
}

impl ReplyStatusType {
    /// Discriminant value.
    #[must_use]
    pub const fn as_u32(self) -> u32 {
        self as u32
    }

    /// Parse from `unsigned long`.
    ///
    /// # Errors
    /// `UnknownReplyStatus` outside 0..=5; `Malformed` when the
    /// value does not match the GIOP version (LOCATION_FORWARD_PERM /
    /// NEEDS_ADDRESSING_MODE not allowed in 1.0/1.1).
    pub fn from_u32(value: u32, version: Version) -> GiopResult<Self> {
        match value {
            0 => Ok(Self::NoException),
            1 => Ok(Self::UserException),
            2 => Ok(Self::SystemException),
            3 => Ok(Self::LocationForward),
            4 if version.uses_v1_2_request_layout() => Ok(Self::LocationForwardPerm),
            5 if version.uses_v1_2_request_layout() => Ok(Self::NeedsAddressingMode),
            4..=5 => Err(GiopError::Malformed(alloc::format!(
                "ReplyStatus {value} only valid in GIOP 1.2+, got {}.{}",
                version.major,
                version.minor
            ))),
            other => Err(GiopError::UnknownReplyStatus(other)),
        }
    }
}

/// The Reply body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reply {
    /// `request_id`.
    pub request_id: u32,
    /// Reply-status discriminator.
    pub reply_status: ReplyStatusType,
    /// `service_context_list`.
    pub service_context: ServiceContextList,
    /// Body bytes (CDR-encoded `out`/`return`/exception content).
    /// In GIOP 1.2, 8-byte aligned from the header start.
    pub body: Vec<u8>,
}

impl Reply {
    /// CDR encode.
    ///
    /// # Errors
    /// Buffer write error.
    pub fn encode(&self, version: Version, w: &mut Writer) -> GiopResult<()> {
        // Validate status against version.
        match self.reply_status {
            ReplyStatusType::LocationForwardPerm | ReplyStatusType::NeedsAddressingMode
                if !version.uses_v1_2_request_layout() =>
            {
                return Err(GiopError::Malformed(alloc::format!(
                    "ReplyStatus {:?} only valid in GIOP 1.2+",
                    self.reply_status
                )));
            }
            _ => {}
        }
        if version.uses_v1_2_request_layout() {
            w.write_u32(self.request_id);
            w.write_u32(self.reply_status.as_u32());
            self.service_context.encode(w)?;
            // Only when a body follows — see `Request::encode` for the
            // clause and for what omniORB says about the alternative.
            if !self.body.is_empty() {
                w.align_to(8);
            }
        } else {
            self.service_context.encode(w)?;
            w.write_u32(self.request_id);
            w.write_u32(self.reply_status.as_u32());
        }
        w.write_bytes(&self.body);
        Ok(())
    }

    /// The offset of the body in the GIOP message of `version` and
    /// `endianness` that carries this reply, counted from the first
    /// octet of the message header (CORBA 3.3 Part 2 §9.4); as
    /// [`crate::Request::body_offset`].
    ///
    /// # Errors
    /// As [`crate::encode_message`] for the reply without its body.
    pub fn body_offset(
        &self,
        version: Version,
        endianness: zerodds_wire::Endianness,
    ) -> GiopResult<usize> {
        let head = Self {
            request_id: self.request_id,
            reply_status: self.reply_status,
            service_context: self.service_context.clone(),
            body: Vec::new(),
        };
        let len = crate::codec::encode_message(
            version,
            endianness,
            false,
            &crate::codec::Message::Reply(head),
        )?
        .len();
        Ok(if version.uses_v1_2_request_layout() {
            len.next_multiple_of(8)
        } else {
            len
        })
    }

    /// Places a body written from an eight-aligned origin (offset 0, the
    /// way a reply body is written before its header is known) on an
    /// eight-aligned offset of the message (CORBA 3.3 Part 2 §9.4.3,
    /// §9.3.1). In GIOP 1.2 and later the body is aligned to eight
    /// already. In GIOP 1.0 and 1.1 it follows the header without padding,
    /// at a multiple of four that the service contexts decide; when it
    /// falls on four modulo eight, a [`PADDING_CONTEXT_ID`] context with
    /// four octets moves it by twelve. A receiver ignores the context
    /// (§7.7.2), and a reader of the body at its offset, or from its first
    /// octet, finds every 8-octet value where it was written.
    ///
    /// Call it after the last service context is added.
    ///
    /// # Errors
    /// As [`Self::body_offset`].
    pub fn place_body_at_eight(
        &mut self,
        version: Version,
        endianness: zerodds_wire::Endianness,
    ) -> GiopResult<()> {
        self.place_body_at(version, endianness, 0)
    }

    /// Places a body written for `origin`, its offset in the message it
    /// was written for, on an offset of this message with the same
    /// remainder modulo eight, where its 8-octet values stay aligned (CORBA
    /// 3.3 Part 2 §9.4.3, §9.3.1): a user exception body relayed from a
    /// GIOP 1.0/1.1 reply of another ORB keeps the offset it had there.
    /// [`Self::place_body_at_eight`] is `origin` 0. In GIOP 1.0 and 1.1
    /// the reply header ends on a multiple of four, and a
    /// [`PADDING_CONTEXT_ID`] context of four octets moves the body by
    /// twelve; in GIOP 1.2 and later the body is on eight.
    ///
    /// Call it after the last service context is added.
    ///
    /// # Errors
    /// As [`Self::body_offset`]; [`GiopError::Malformed`] for a body that
    /// no offset of this reply header can carry: one written for an
    /// offset that is not a multiple of eight in GIOP 1.2 and later, or
    /// not a multiple of four in GIOP 1.0 and 1.1.
    pub fn place_body_at(
        &mut self,
        version: Version,
        endianness: zerodds_wire::Endianness,
        origin: usize,
    ) -> GiopResult<()> {
        let want = origin % 8;
        if self.body.is_empty() {
            return Ok(());
        }
        if version.uses_v1_2_request_layout() {
            return if want == 0 {
                Ok(())
            } else {
                Err(GiopError::Malformed(alloc::format!(
                    "a reply body written for {want} modulo eight in a GIOP {}.{} reply, \
                     whose body is on eight",
                    version.major,
                    version.minor
                )))
            };
        }
        let at = self.body_offset(version, endianness)? % 8;
        if at == want {
            return Ok(());
        }
        if (at + 4) % 8 != want {
            return Err(GiopError::Malformed(alloc::format!(
                "a reply body written for {want} modulo eight after a GIOP {}.{} reply \
                 header, which ends on a multiple of four",
                version.major,
                version.minor
            )));
        }
        self.service_context
            .0
            .push(crate::service_context::ServiceContext::new(
                PADDING_CONTEXT_ID,
                alloc::vec![0; 4],
            ));
        debug_assert_eq!(self.body_offset(version, endianness)? % 8, want);
        Ok(())
    }

    /// CDR decode.
    ///
    /// The body comes without the octets before it; under GIOP 1.0 and
    /// 1.1 [`Self::body_offset`] says where it stood in the message.
    ///
    /// # Errors
    /// Buffer read error or unknown status.
    pub fn decode(version: Version, r: &mut Reader<'_>) -> GiopResult<Self> {
        if version.uses_v1_2_request_layout() {
            let request_id = r.read_u32()?;
            let status_raw = r.read_u32()?;
            let reply_status = ReplyStatusType::from_u32(status_raw, version)?;
            let service_context = ServiceContextList::decode(r)?;
            let body = if r.remaining() > 0 {
                r.align_to(8)?;
                r.read_bytes(r.remaining())?.to_vec()
            } else {
                Vec::new()
            };
            Ok(Self {
                request_id,
                reply_status,
                service_context,
                body,
            })
        } else {
            let service_context = ServiceContextList::decode(r)?;
            let request_id = r.read_u32()?;
            let status_raw = r.read_u32()?;
            let reply_status = ReplyStatusType::from_u32(status_raw, version)?;
            let body = r.read_bytes(r.remaining())?.to_vec();
            Ok(Self {
                request_id,
                reply_status,
                service_context,
                body,
            })
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    /// Part 2 §9.4.3, OPEN-ITEMS 133 (reply direction): a GIOP 1.0/1.1
    /// reply whose service contexts leave the body on four modulo eight
    /// gets a padding context; the body then starts on a multiple of
    /// eight. A 1.2 reply and a reply already on eight are left alone.
    #[test]
    fn corba_p2_9_4_3_a_giop_1_0_reply_body_is_placed_on_eight() {
        use crate::service_context::ServiceContext;
        use zerodds_wire::Endianness;
        for version in [Version::V1_0, Version::V1_1] {
            for ctx_len in 0..8usize {
                let mut r = Reply {
                    request_id: 9,
                    reply_status: ReplyStatusType::NoException,
                    service_context: ServiceContextList(alloc::vec![ServiceContext::new(
                        0x5A44_0002,
                        alloc::vec![1; ctx_len],
                    )]),
                    body: alloc::vec![1, 2, 3, 4, 5, 6, 7, 8],
                };
                let before = r.body_offset(version, Endianness::Big).unwrap();
                r.place_body_at_eight(version, Endianness::Big).unwrap();
                let after = r.body_offset(version, Endianness::Big).unwrap();
                assert_eq!(after % 8, 0, "{version:?} ctx {ctx_len}");
                let padded = r.service_context.0.len() == 2;
                assert_eq!(padded, !before.is_multiple_of(8));
                if padded {
                    assert_eq!(r.service_context.0[1].context_id, PADDING_CONTEXT_ID);
                }
                let bytes = crate::codec::encode_message(
                    version,
                    Endianness::Big,
                    false,
                    &crate::codec::Message::Reply(r.clone()),
                )
                .unwrap();
                assert_eq!(&bytes[after..], &[1, 2, 3, 4, 5, 6, 7, 8]);
            }
        }
        let mut r = Reply {
            request_id: 1,
            reply_status: ReplyStatusType::NoException,
            service_context: ServiceContextList(alloc::vec![ServiceContext::new(
                1,
                alloc::vec![1]
            )]),
            body: alloc::vec![1],
        };
        r.place_body_at_eight(Version::V1_2, Endianness::Big)
            .unwrap();
        assert_eq!(r.service_context.0.len(), 1);
    }

    /// Part 2 §9.4.3: a body written for four modulo eight (a user
    /// exception relayed from another ORB's GIOP 1.0/1.1 reply) goes on
    /// four modulo eight of a 1.0/1.1 reply, whatever the reply's own
    /// service contexts leave; a 1.2 reply cannot carry it, nor can any
    /// version a body written for an odd offset.
    #[test]
    fn corba_p2_9_4_3_a_body_written_for_four_modulo_eight_is_placed_there() {
        use crate::service_context::ServiceContext;
        use zerodds_wire::Endianness;
        for version in [Version::V1_0, Version::V1_1] {
            for ctx_len in 0..8usize {
                for origin in [4usize, 12, 0, 8] {
                    let mut r = Reply {
                        request_id: 9,
                        reply_status: ReplyStatusType::UserException,
                        service_context: ServiceContextList(alloc::vec![ServiceContext::new(
                            0x5A44_0002,
                            alloc::vec![1; ctx_len],
                        )]),
                        body: alloc::vec![1, 2, 3, 4, 5, 6, 7, 8],
                    };
                    r.place_body_at(version, Endianness::Big, origin).unwrap();
                    let at = r.body_offset(version, Endianness::Big).unwrap();
                    assert_eq!(
                        at % 8,
                        origin % 8,
                        "{version:?} ctx {ctx_len} origin {origin}"
                    );
                    let bytes = crate::codec::encode_message(
                        version,
                        Endianness::Big,
                        false,
                        &crate::codec::Message::Reply(r.clone()),
                    )
                    .unwrap();
                    assert_eq!(&bytes[at..], &[1, 2, 3, 4, 5, 6, 7, 8]);
                }
                let mut odd = Reply {
                    request_id: 9,
                    reply_status: ReplyStatusType::UserException,
                    service_context: ServiceContextList::default(),
                    body: alloc::vec![1],
                };
                assert!(odd.place_body_at(version, Endianness::Big, 6).is_err());
            }
        }
        let mut r = Reply {
            request_id: 1,
            reply_status: ReplyStatusType::UserException,
            service_context: ServiceContextList::default(),
            body: alloc::vec![1],
        };
        assert!(matches!(
            r.place_body_at(Version::V1_2, Endianness::Big, 4),
            Err(GiopError::Malformed(_))
        ));
        r.place_body_at(Version::V1_2, Endianness::Big, 16).unwrap();
        assert!(r.service_context.0.is_empty());
    }
    use zerodds_wire::Endianness;

    #[test]
    fn reply_status_values_match_spec() {
        // Spec §15.4.3.1 + §15.4.3.2.
        assert_eq!(ReplyStatusType::NoException.as_u32(), 0);
        assert_eq!(ReplyStatusType::UserException.as_u32(), 1);
        assert_eq!(ReplyStatusType::SystemException.as_u32(), 2);
        assert_eq!(ReplyStatusType::LocationForward.as_u32(), 3);
        assert_eq!(ReplyStatusType::LocationForwardPerm.as_u32(), 4);
        assert_eq!(ReplyStatusType::NeedsAddressingMode.as_u32(), 5);
    }

    #[test]
    fn round_trip_giop_1_0_no_exception() {
        let r = Reply {
            request_id: 42,
            reply_status: ReplyStatusType::NoException,
            service_context: ServiceContextList::default(),
            body: alloc::vec![0xde, 0xad],
        };
        let mut w = Writer::new(Endianness::Big);
        r.encode(Version::V1_0, &mut w).unwrap();
        let bytes = w.finish();
        let mut rd = Reader::new(&bytes, Endianness::Big);
        let decoded = Reply::decode(Version::V1_0, &mut rd).unwrap();
        assert_eq!(decoded, r);
    }

    #[test]
    fn round_trip_giop_1_2_user_exception() {
        let r = Reply {
            request_id: 1,
            reply_status: ReplyStatusType::UserException,
            service_context: ServiceContextList::default(),
            body: alloc::vec![1, 2, 3, 4, 5, 6, 7, 8],
        };
        let mut w = Writer::new(Endianness::Little);
        r.encode(Version::V1_2, &mut w).unwrap();
        let bytes = w.finish();
        let mut rd = Reader::new(&bytes, Endianness::Little);
        let decoded = Reply::decode(Version::V1_2, &mut rd).unwrap();
        assert_eq!(decoded, r);
    }

    #[test]
    fn round_trip_location_forward_perm_in_1_2() {
        let r = Reply {
            request_id: 5,
            reply_status: ReplyStatusType::LocationForwardPerm,
            service_context: ServiceContextList::default(),
            body: alloc::vec::Vec::new(),
        };
        let mut w = Writer::new(Endianness::Big);
        r.encode(Version::V1_2, &mut w).unwrap();
        let bytes = w.finish();
        let mut rd = Reader::new(&bytes, Endianness::Big);
        let decoded = Reply::decode(Version::V1_2, &mut rd).unwrap();
        assert_eq!(decoded, r);
    }

    #[test]
    fn location_forward_perm_in_giop_1_0_is_rejected() {
        let r = Reply {
            request_id: 1,
            reply_status: ReplyStatusType::LocationForwardPerm,
            service_context: ServiceContextList::default(),
            body: alloc::vec::Vec::new(),
        };
        let mut w = Writer::new(Endianness::Big);
        let err = r.encode(Version::V1_0, &mut w).unwrap_err();
        assert!(matches!(err, GiopError::Malformed(_)));
    }

    #[test]
    fn unknown_reply_status_is_diagnostic() {
        let mut w = Writer::new(Endianness::Big);
        // Service-context list (empty) + request_id + invalid status.
        ServiceContextList::default().encode(&mut w).unwrap();
        w.write_u32(1);
        w.write_u32(99);
        let bytes = w.finish();
        let mut rd = Reader::new(&bytes, Endianness::Big);
        let err = Reply::decode(Version::V1_0, &mut rd).unwrap_err();
        assert!(matches!(err, GiopError::UnknownReplyStatus(99)));
    }
}
