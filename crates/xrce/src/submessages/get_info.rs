// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `GET_INFO` Submessage (id=2, Spec §8.3.5.3).
//!
//! Direction: Client → Agent. Payload = `GET_INFO_Payload`
//! (`BaseObjectRequest` + `InfoMask`). The ObjectKind in the lower 4 bits of
//! the `object_id` routes to `Root::get_info` (OBJK_AGENT) or
//! `ProxyClient::get_info`.

extern crate alloc;
use alloc::vec::Vec;

use crate::encoding::Endianness;
use crate::error::XrceError;
use crate::object_info::{BaseObjectRequest, InfoMask, ObjectInfo};
use crate::submessages::{FLAG_E_LITTLE_ENDIAN, Submessage, SubmessageId};

/// Opaque body for `GET_INFO`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GetInfoPayload {
    /// XCDR2 body.
    pub representation: Vec<u8>,
}

impl GetInfoPayload {
    /// The typed payload (§8.3.5.3.2): `struct GET_INFO_Payload :
    /// BaseObjectRequest { InfoMask info_mask; }`, in the byte order `e`.
    #[must_use]
    pub fn new(base: BaseObjectRequest, info_mask: InfoMask, e: Endianness) -> Self {
        let mut representation = base.encode().to_vec();
        representation.extend_from_slice(&info_mask.encode(e));
        Self { representation }
    }

    /// The payload of a `get_info` that also carries `client_info`
    /// (§7.8.2.2: "client_info (ObjectInfo): a representation of the Client",
    /// whose `config` is a `CLIENT_Representation` with the `client_key` set to
    /// `CLIENTKEY_INVALID`, and Figure 10 of §8.4.4). Annex A's
    /// `GET_INFO_Payload` has no member for it; it follows the `InfoMask`, and
    /// a receiver that does not read it ignores the octets.
    ///
    /// # Errors
    /// `PayloadTooLarge` or `ValueOutOfRange` from encoding `client_info`.
    pub fn with_client_info(
        base: BaseObjectRequest,
        info_mask: InfoMask,
        client_info: &ObjectInfo,
        e: Endianness,
    ) -> Result<Self, XrceError> {
        let mut payload = Self::new(base, info_mask, e);
        client_info.encode_into(&mut payload.representation, e)?;
        Ok(payload)
    }

    /// The `client_info` behind the `InfoMask` of a payload built by
    /// [`Self::with_client_info`]; `None` if the payload ends with the mask.
    ///
    /// # Errors
    /// `UnexpectedEof` or `ValueOutOfRange` if the octets behind the mask are
    /// no `ObjectInfo`.
    pub fn client_info(&self, e: Endianness) -> Result<Option<ObjectInfo>, XrceError> {
        const MASK_END: usize = BaseObjectRequest::WIRE_SIZE + 4;
        if self.representation.len() <= MASK_END {
            return Ok(None);
        }
        let (info, _) = ObjectInfo::decode_at(&self.representation, MASK_END, e)?;
        Ok(Some(info))
    }

    /// Reads the `BaseObjectRequest` and the `InfoMask` out of the payload,
    /// in the byte order `e` of the submessage's E flag.
    ///
    /// # Errors
    /// `UnexpectedEof` if the payload is shorter than the 8 octets of
    /// `GET_INFO_Payload`, `ValueOutOfRange` for an invalid `ObjectId`.
    pub fn parse(&self, e: Endianness) -> Result<(BaseObjectRequest, InfoMask), XrceError> {
        let (base, n) = BaseObjectRequest::decode(&self.representation)?;
        let (mask, _) = InfoMask::decode(&self.representation[n..], e)?;
        Ok((base, mask))
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
        Submessage::new(SubmessageId::GetInfo, flags, self.representation)
    }

    /// Extracts from a `Submessage`.
    ///
    /// # Errors
    /// `ValueOutOfRange`.
    pub fn try_from_submessage(sm: &Submessage) -> Result<Self, XrceError> {
        if sm.header.submessage_id != SubmessageId::GetInfo {
            return Err(XrceError::ValueOutOfRange {
                message: "submessage is not GET_INFO",
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

    /// §8.3.5.3.2: `GET_INFO_Payload : BaseObjectRequest { InfoMask info_mask; }`
    /// — the request, then a 32-bit mask, in the byte order of the E flag.
    #[test]
    fn get_info_payload_is_the_request_and_the_info_mask() {
        let base = BaseObjectRequest {
            request_id: [0, 7],
            object_id: crate::object_id::OBJECTID_AGENT,
        };
        for (e, mask_bytes) in [
            (Endianness::Little, [3u8, 0, 0, 0]),
            (Endianness::Big, [0, 0, 0, 3]),
        ] {
            let sm = GetInfoPayload::new(base, InfoMask::ALL, e)
                .into_submessage_with(e)
                .unwrap();
            assert_eq!(sm.body.len(), 8);
            assert_eq!(&sm.body[4..], &mask_bytes);
            let p = GetInfoPayload::try_from_submessage(&sm).unwrap();
            assert_eq!(
                p.parse(sm.header.body_endianness()).unwrap(),
                (base, InfoMask::ALL)
            );
        }
        // A payload without the mask is short.
        let short = GetInfoPayload {
            representation: base.encode().to_vec(),
        };
        assert!(short.parse(Endianness::Little).is_err());
    }

    /// §7.8.2.2, §8.4.4 Figure 10: `client_info` follows the mask; the mask
    /// is read the same with or without it.
    #[test]
    fn client_info_follows_the_info_mask() {
        use crate::handshake::ClientRepresentation;
        use crate::header::ClientKey;
        use crate::object_info::ObjectConfig;
        let base = BaseObjectRequest {
            request_id: [0, 7],
            object_id: crate::object_id::OBJECTID_AGENT,
        };
        let info = ObjectInfo {
            activity: None,
            config: Some(ObjectConfig::Client(ClientRepresentation {
                timestamp: None,
                version: crate::handshake::XRCE_VERSION,
                vendor_id: [0, 0],
                client_key: ClientKey([0; 4]),
                session_id: 0x81,
                properties: None,
                requested_mtu: None,
            })),
        };
        for e in [Endianness::Little, Endianness::Big] {
            let p = GetInfoPayload::with_client_info(base, InfoMask::ALL, &info, e).unwrap();
            assert_eq!(p.parse(e).unwrap(), (base, InfoMask::ALL));
            assert_eq!(p.client_info(e).unwrap(), Some(info.clone()));
            let without = GetInfoPayload::new(base, InfoMask::ALL, e);
            assert_eq!(without.client_info(e).unwrap(), None);
        }
        // Octets behind the mask that are no ObjectInfo are an error.
        let mut broken = GetInfoPayload::new(base, InfoMask::ALL, Endianness::Little);
        broken.representation.push(7);
        assert!(broken.client_info(Endianness::Little).is_err());
    }

    #[test]
    fn get_info_roundtrip() {
        let p = GetInfoPayload {
            representation: alloc::vec![0xAA; 16],
        };
        let sm = p.clone().into_submessage().unwrap();
        assert_eq!(sm.header.submessage_id, SubmessageId::GetInfo);
        let p2 = GetInfoPayload::try_from_submessage(&sm).unwrap();
        assert_eq!(p2, p);
    }
}
