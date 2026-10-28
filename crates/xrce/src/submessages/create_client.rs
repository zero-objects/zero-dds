// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `CREATE_CLIENT` Submessage (id=0, Spec §8.3.5.1).
//!
//! Direction: Client → Agent. The payload is the XCDR2 representation of
//! the Annex A `CREATE_CLIENT_Payload { CLIENT_Representation
//! client_representation; }` (§8.3.5.1.2), the client's requested
//! identity. The typed codec is [`crate::handshake::ClientRepresentation`]
//! (Annex A, §7.8.2.1); this submessage carries that value instead of an
//! opaque byte blob.

extern crate alloc;

use crate::encoding::Endianness;
use crate::error::XrceError;
use crate::handshake::ClientRepresentation;
use crate::submessages::{FLAG_E_LITTLE_ENDIAN, Submessage, SubmessageId};

/// Typed body for `CREATE_CLIENT` (§7.8.2.1/§8.3.5.1.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateClientPayload {
    /// The `client_representation` member of `CREATE_CLIENT_Payload`.
    pub representation: ClientRepresentation,
}

impl CreateClientPayload {
    /// Encodes the `CLIENT_Representation` and packs it into a
    /// `Submessage` with the LE body flag.
    ///
    /// # Errors
    /// `ValueOutOfRange` if the representation cannot be encoded
    /// (e.g. a property string too long for the XCDR2 length field);
    /// `PayloadTooLarge` if the encoded body is > `u16::MAX`.
    pub fn into_submessage(self) -> Result<Submessage, XrceError> {
        self.into_submessage_with(Endianness::Little)
    }

    /// As [`Self::into_submessage`], encoded in the byte order `e`, which
    /// the E flag of the submessage then states (§8.3.4.2).
    ///
    /// # Errors
    /// As [`Self::into_submessage`].
    pub fn into_submessage_with(self, e: Endianness) -> Result<Submessage, XrceError> {
        self.into_submessage_form(e, crate::encoding::representation_timestamps())
    }

    /// As [`Self::into_submessage_with`], with the form of the representation
    /// (with or without `client_timestamp`) named instead of taken from the
    /// process setting.
    ///
    /// # Errors
    /// As [`Self::into_submessage`].
    pub fn into_submessage_form(
        self,
        e: Endianness,
        form: crate::encoding::RepresentationTimestamps,
    ) -> Result<Submessage, XrceError> {
        let mut body = alloc::vec::Vec::new();
        self.representation.encode_form(&mut body, e, form)?;
        let flags = if e.is_little() {
            FLAG_E_LITTLE_ENDIAN
        } else {
            0
        };
        Submessage::new(SubmessageId::CreateClient, flags, body)
    }

    /// Decodes the `CLIENT_Representation` from the submessage body.
    ///
    /// # Errors
    /// `ValueOutOfRange` if the submessage ID is not `CreateClient`;
    /// `UnexpectedEof` if the body is shorter than the fixed prefix;
    /// `ValueOutOfRange` if `xrce_cookie` is not `"XRCE"`.
    pub fn try_from_submessage(sm: &Submessage) -> Result<Self, XrceError> {
        if sm.header.submessage_id != SubmessageId::CreateClient {
            return Err(XrceError::ValueOutOfRange {
                message: "submessage is not CREATE_CLIENT",
            });
        }
        Ok(Self {
            representation: ClientRepresentation::decode_with(
                &sm.body,
                sm.header.body_endianness(),
            )?,
        })
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
    use super::*;
    use crate::handshake::{DEFAULT_MTU, XRCE_VERSION};
    use crate::header::ClientKey;

    fn representation() -> ClientRepresentation {
        ClientRepresentation {
            timestamp: None,
            version: XRCE_VERSION,
            vendor_id: [0x01, 0x0f],
            client_key: ClientKey([0xCC, 0xCC, 0xDD, 0xDD]),
            session_id: 0x81,
            properties: None,
            requested_mtu: Some(DEFAULT_MTU),
        }
    }

    #[test]
    fn create_client_roundtrip_via_submessage() {
        let p = CreateClientPayload {
            representation: representation(),
        };
        let sm = p.clone().into_submessage().unwrap();
        assert_eq!(sm.header.submessage_id, SubmessageId::CreateClient);
        assert_eq!(sm.header.flags, FLAG_E_LITTLE_ENDIAN);
        let p2 = CreateClientPayload::try_from_submessage(&sm).unwrap();
        assert_eq!(p2, p);
    }

    /// DDS-XRCE 1.0 Annex B.1: the CREATE_CLIENT example. The Client is from
    /// vendor {0x0F, 0x0F}, uses xrce_version {0x01, 0x00}, has the client_key
    /// {0x22, 0x33, 0x44, 0x55} and asks for session_id 0xDD; the message
    /// header is {0x80, 0x00, 0x00, 0x00}; bytes 8 to 21 are the
    /// CLIENT_Representation: cookie, version, vendor, key, session_id and
    /// "properties? = FALSE". (The example's prose names a `request_id` and a
    /// `client_timestamp` its table does not have, flags 0x07 where
    /// CREATE_CLIENT has no flag bits, and a length 26 that its own bytes
    /// contradict; the bytes of the CLIENT_Representation are what it fixes.)
    #[test]
    fn annex_b_1_create_client_example_bytes() {
        use crate::header::{MessageHeader, SessionId, StreamId};
        use crate::serial_number::SerialNumber16;
        use crate::submessages::Message;
        let rep = ClientRepresentation {
            timestamp: None,
            version: XRCE_VERSION,
            vendor_id: [0x0F, 0x0F],
            client_key: ClientKey([0x22, 0x33, 0x44, 0x55]),
            session_id: 0xDD,
            properties: None,
            requested_mtu: None,
        };
        let sm = CreateClientPayload {
            representation: rep.clone(),
        }
        .into_submessage()
        .unwrap();
        let header = MessageHeader::without_client_key(
            SessionId(0x80),
            StreamId::NONE,
            SerialNumber16::new(0),
        )
        .unwrap();
        let bytes = Message::new(header, alloc::vec![sm])
            .unwrap()
            .encode()
            .unwrap();
        // Bytes 0 to 3: the message header of the example.
        assert_eq!(&bytes[0..4], &[0x80, 0x00, 0x00, 0x00]);
        // Byte 4 is the submessage id of CREATE_CLIENT (id 0, §8.3.5.1).
        assert_eq!(bytes[4], 0x00);
        // Bytes 8 to 21: the CLIENT_Representation.
        assert_eq!(
            &bytes[8..22],
            &[
                b'X', b'R', b'C', b'E', 0x01, 0x00, 0x0F, 0x0F, 0x22, 0x33, 0x44, 0x55, 0xDD, 0x00
            ]
        );
        assert_eq!(bytes.len(), 22);
        assert_eq!(ClientRepresentation::decode(&bytes[8..]).unwrap(), rep);
    }

    /// §8.3.4.2: the E flag decides how the payload is read.
    #[test]
    fn create_client_follows_the_e_flag() {
        let mut rep = representation();
        rep.properties = Some(alloc::vec![crate::handshake::Property {
            name: "auth".into(),
            value: "token".into(),
        }]);
        let p = CreateClientPayload {
            representation: rep,
        };
        let be = p.clone().into_submessage_with(Endianness::Big).unwrap();
        assert_eq!(be.header.flags & FLAG_E_LITTLE_ENDIAN, 0);
        assert_eq!(CreateClientPayload::try_from_submessage(&be).unwrap(), p);
        let le = p.clone().into_submessage().unwrap();
        assert_ne!(le.body, be.body);
        assert_eq!(CreateClientPayload::try_from_submessage(&le).unwrap(), p);
    }

    #[test]
    fn create_client_rejects_wrong_submessage_id() {
        let sm = Submessage::new(SubmessageId::Status, 0, alloc::vec![]).unwrap();
        let res = CreateClientPayload::try_from_submessage(&sm);
        assert!(matches!(res, Err(XrceError::ValueOutOfRange { .. })));
    }

    /// §8.3.5.1.2/Annex A: the committed `endpoints/testdata/xrce-client/
    /// xrce_client_create_client.bin` — produced by the real
    /// `zerodds_xrce_client::XrceClient::connect` — decodes member by
    /// member and writes back the exact same octets.
    #[test]
    fn decodes_the_committed_fixture_member_by_member() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../endpoints/testdata/xrce-client/xrce_client_create_client.bin");
        let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));

        let msg = crate::submessages::Message::decode(&bytes).expect("a real XRCE message");
        // §8.3.2.1: the CREATE_CLIENT goes in a header of the SESSION_ID_NONE
        // kind (0x00 with the ClientKey); the session it asks for (0x01) is in
        // the CLIENT_Representation below.
        assert_eq!(msg.header.session_id.0, 0x00);
        assert_eq!(
            msg.header.client_key,
            Some(ClientKey([0x01, 0x02, 0x03, 0x04]))
        );
        assert_eq!(msg.submessages.len(), 1);

        let sm = &msg.submessages[0];
        assert_eq!(sm.header.submessage_id, SubmessageId::CreateClient);
        let payload =
            CreateClientPayload::try_from_submessage(sm).expect("a real CLIENT_Representation");

        let rep = &payload.representation;
        assert_eq!(rep.version, XRCE_VERSION);
        assert_eq!(rep.vendor_id, [0x00, 0x00]);
        assert_eq!(rep.client_key, ClientKey([0x01, 0x02, 0x03, 0x04]));
        assert_eq!(rep.session_id, 0x01);
        assert_eq!(rep.properties, None);
        assert_eq!(rep.requested_mtu, Some(DEFAULT_MTU));

        // The representation re-encodes to the same body, and the whole
        // message to the same datagram.
        assert_eq!(rep.encode().unwrap(), sm.body);
        assert_eq!(
            CreateClientPayload {
                representation: payload.representation,
            }
            .into_submessage()
            .unwrap(),
            *sm
        );
        assert_eq!(msg.encode().unwrap(), bytes);
    }

    /// §7.8.2.1: "Check the xrce_cookie to ensure it matches XRCE_COOKIE."
    #[test]
    fn refuses_a_payload_whose_cookie_is_not_xrce() {
        let mut body = representation().encode().unwrap();
        body[0..4].copy_from_slice(b"NOPE");
        let sm = Submessage::new(SubmessageId::CreateClient, FLAG_E_LITTLE_ENDIAN, body).unwrap();
        let err = CreateClientPayload::try_from_submessage(&sm).unwrap_err();
        match err {
            XrceError::ValueOutOfRange { message } => {
                assert!(message.contains("cookie"), "unhelpful message: {message}");
            }
            other => panic!("expected a cookie error, got {other:?}"),
        }
    }

    /// A body shorter than the fixed prefix is an error, not a default.
    #[test]
    fn refuses_a_truncated_payload() {
        let sm = Submessage::new(
            SubmessageId::CreateClient,
            FLAG_E_LITTLE_ENDIAN,
            alloc::vec![b'X', b'R', b'C', b'E'],
        )
        .unwrap();
        let err = CreateClientPayload::try_from_submessage(&sm).unwrap_err();
        assert!(
            matches!(err, XrceError::UnexpectedEof { .. }),
            "expected a truncation error, got {err:?}"
        );
    }
}
