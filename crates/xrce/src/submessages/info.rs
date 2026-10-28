// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `INFO` Submessage (id=6, Spec §8.3.5.7).
//!
//! Direction: Agent → Client. Reply to `GET_INFO`. Payload =
//! `INFO_Payload : BaseObjectReply { ObjectInfo }`.

extern crate alloc;
use alloc::vec::Vec;

use crate::encoding::Endianness;
use crate::error::XrceError;
use crate::object_info::{BaseObjectReply, ObjectInfo};
use crate::submessages::{FLAG_E_LITTLE_ENDIAN, Submessage, SubmessageId};

/// Opaque body for `INFO`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InfoPayload {
    /// XCDR2 `BaseObjectReply { ObjectInfo }`.
    pub representation: Vec<u8>,
}

impl InfoPayload {
    /// The typed payload (§8.3.5.7.2): `struct INFO_Payload : BaseObjectReply
    /// { ObjectInfo object_info; }`, in the byte order `e`. The `ObjectInfo`
    /// is aligned counted from the start of the payload, behind the six octets
    /// of the `BaseObjectReply`.
    ///
    /// # Errors
    /// As [`ObjectInfo::encode_into`].
    pub fn new(
        reply: BaseObjectReply,
        object_info: &ObjectInfo,
        e: Endianness,
    ) -> Result<Self, XrceError> {
        let mut representation = Vec::new();
        reply.encode_into(&mut representation, e);
        object_info.encode_into(&mut representation, e)?;
        Ok(Self { representation })
    }

    /// Reads the `BaseObjectReply` and the `ObjectInfo` out of the payload, in
    /// the byte order `e` of the submessage's E flag.
    ///
    /// # Errors
    /// `UnexpectedEof`, `ValueOutOfRange`.
    pub fn parse(&self, e: Endianness) -> Result<(BaseObjectReply, ObjectInfo), XrceError> {
        let (reply, n) = BaseObjectReply::decode_with(&self.representation, e)?;
        let (info, _) = ObjectInfo::decode_at(&self.representation, n, e)?;
        Ok((reply, info))
    }

    /// Packs into a `Submessage`, little-endian.
    ///
    /// # Errors
    /// `PayloadTooLarge`.
    pub fn into_submessage(self) -> Result<Submessage, XrceError> {
        self.into_submessage_with(Endianness::Little)
    }

    /// Packs into a `Submessage` whose E flag states the byte order `e` the
    /// payload was written in (§8.3.4.2).
    ///
    /// # Errors
    /// `PayloadTooLarge`.
    pub fn into_submessage_with(self, e: Endianness) -> Result<Submessage, XrceError> {
        let flags = if e.is_little() {
            FLAG_E_LITTLE_ENDIAN
        } else {
            0
        };
        Submessage::new(SubmessageId::Info, flags, self.representation)
    }

    /// Extracts from a `Submessage`.
    ///
    /// # Errors
    /// `ValueOutOfRange`.
    pub fn try_from_submessage(sm: &Submessage) -> Result<Self, XrceError> {
        if sm.header.submessage_id != SubmessageId::Info {
            return Err(XrceError::ValueOutOfRange {
                message: "submessage is not INFO",
            });
        }
        Ok(Self {
            representation: sm.body.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]
    use super::*;

    /// §8.3.5.7.2: `INFO_Payload : BaseObjectReply { ObjectInfo }`. The reply
    /// echoes the request and carries the status; with the six octets of the
    /// reply in front the `ObjectInfo` is aligned from the start of the
    /// payload.
    #[test]
    fn info_payload_is_the_reply_and_the_object_info() {
        use crate::object_id::OBJECTID_AGENT;
        use crate::object_info::{
            ActivityInfoVariant, BaseObjectRequest, ResultStatus, ResultStatusCode,
        };
        let reply = BaseObjectReply {
            related_request: BaseObjectRequest {
                request_id: [0, 9],
                object_id: OBJECTID_AGENT,
            },
            result: ResultStatus {
                status_code: ResultStatusCode::UnknownReference,
                implementation_status: 0,
            },
        };
        let info = ObjectInfo {
            activity: Some(ActivityInfoVariant::DataReader {
                highest_acked_num: 1,
            }),
            config: None,
        };
        for e in [Endianness::Little, Endianness::Big] {
            let sm = InfoPayload::new(reply, &info, e)
                .unwrap()
                .into_submessage_with(e)
                .unwrap();
            let p = InfoPayload::try_from_submessage(&sm).unwrap();
            let encoded = reply.encode_with(e);
            assert_eq!(&p.representation[..encoded.len()], encoded.as_slice());
            assert_eq!(
                p.parse(sm.header.body_endianness()).unwrap(),
                (reply, info.clone())
            );
        }
    }

    #[test]
    fn info_roundtrip() {
        let p = InfoPayload {
            representation: alloc::vec![1, 2, 3, 4, 5, 6, 7, 8],
        };
        let sm = p.clone().into_submessage().unwrap();
        assert_eq!(sm.header.submessage_id, SubmessageId::Info);
        let p2 = InfoPayload::try_from_submessage(&sm).unwrap();
        assert_eq!(p2, p);
    }
}
