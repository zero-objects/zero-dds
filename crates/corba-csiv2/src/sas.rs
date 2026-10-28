// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! SAS protocol — Spec §24.2, module `CSI` (`CSI.idl`).
//!
//! Four message types, the discriminator of `SASContextBody` being
//! `typedef short MsgType`:
//! * `EstablishContext` (`MTEstablishContext = 0`) — client starts a
//!   security context.
//! * `CompleteEstablishContext` (`MTCompleteEstablishContext = 1`) — server
//!   response.
//! * `ContextError` (`MTContextError = 4`) — error during context
//!   establishment.
//! * `MessageInContext` (`MTMessageInContext = 5`) — follow-up request with
//!   reused context.
//!
//! Wire form: a SAS message is transported as an `IOP::ServiceContext`
//! with tag `TAG_SECURITY_ATTRIBUTE_SERVICE = 15`.

use alloc::string::String;
use alloc::vec::Vec;

use zerodds_wire::{Endianness, Reader, Writer};

/// Error while encoding/decoding the `SASContextBody` wire format.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SasWireError(pub String);

/// SAS message discriminator (Spec §24.2.6.1): `typedef short MsgType`, so
/// two octets on the wire, aligned to two.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i16)]
pub enum SasMsgType {
    /// `MTEstablishContext = 0`.
    EstablishContext = 0,
    /// `MTCompleteEstablishContext = 1`.
    CompleteEstablishContext = 1,
    /// `MTContextError = 4` (the spec assigns non-consecutive IDs).
    ContextError = 4,
    /// `MTMessageInContext = 5`.
    MessageInContext = 5,
}

/// One element of an `AuthorizationToken` (`CSI::AuthorizationElement`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorizationElement {
    /// `AuthorizationElementType the_type` (`unsigned long`).
    pub the_type: u32,
    /// `AuthorizationElementContents the_element` (`sequence<octet>`).
    pub the_element: Vec<u8>,
}

/// `IdentityToken`-Discriminator (Spec §24.2.5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdentityToken {
    /// `ITTAbsent = 0` — no token.
    Absent,
    /// `ITTAnonymous = 1` — anonymous caller.
    Anonymous,
    /// `ITTPrincipalName = 2` — DER-encoded GSS name.
    PrincipalName(Vec<u8>),
    /// `ITTX509CertChain = 4`.
    X509CertChain(Vec<u8>),
    /// `ITTDistinguishedName = 8` — RFC-1779 DN.
    DistinguishedName(Vec<u8>),
    /// Any other `IdentityTokenType`: the union's `default` branch,
    /// `IdentityExtension id` (`sequence<octet>`).
    Extension {
        /// The `IdentityTokenType` the sender used.
        kind: u32,
        /// The extension's octets.
        id: Vec<u8>,
    },
}

impl IdentityToken {
    /// Discriminator value (Spec §24.2.5).
    #[must_use]
    pub const fn discriminator(&self) -> u32 {
        match self {
            Self::Absent => 0,
            Self::Anonymous => 1,
            Self::PrincipalName(_) => 2,
            Self::X509CertChain(_) => 4,
            Self::DistinguishedName(_) => 8,
            Self::Extension { kind, .. } => *kind,
        }
    }
}

/// `EstablishContext` (Spec §24.2.6.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EstablishContext {
    /// `ContextId` (`unsigned long long`). 0 = stateless.
    pub client_context_id: u64,
    /// `AuthorizationToken` (`sequence<AuthorizationElement>`, typically
    /// empty).
    pub authorization_token: Vec<AuthorizationElement>,
    /// `IdentityToken`.
    pub identity_token: IdentityToken,
    /// `ClientAuthenticationToken` (typically GSSUP-encapsulated).
    pub client_authentication_token: Vec<u8>,
}

/// `CompleteEstablishContext` (Spec §24.2.6.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompleteEstablishContext {
    /// Correlating ContextId.
    pub client_context_id: u64,
    /// `context_stateful` — `true` if the server keeps the context.
    pub context_stateful: bool,
    /// `final_context_token` — typically empty for GSSUP.
    pub final_context_token: Vec<u8>,
}

/// `MessageInContext` (Spec §24.2.6.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageInContext {
    /// Correlating ContextId.
    pub client_context_id: u64,
    /// `discard_context` — `true` means the server should discard the context.
    pub discard_context: bool,
}

/// `ContextError` (Spec §24.2.6.5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextError {
    /// Correlating ContextId.
    pub client_context_id: u64,
    /// Major status code (GSS-API-Style).
    pub major_status: u32,
    /// Minor status code.
    pub minor_status: u32,
    /// `error_token` (DER-encoded GSS error).
    pub error_token: Vec<u8>,
}

/// The SAS message union.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SasMessage {
    /// EstablishContext.
    EstablishContext(EstablishContext),
    /// CompleteEstablishContext.
    CompleteEstablishContext(CompleteEstablishContext),
    /// ContextError.
    ContextError(ContextError),
    /// MessageInContext.
    MessageInContext(MessageInContext),
}

fn cdr<E: core::fmt::Debug>(e: E) -> SasWireError {
    SasWireError(alloc::format!("{e:?}"))
}

fn write_seq_octet(w: &mut Writer, v: &[u8]) -> Result<(), SasWireError> {
    let n = u32::try_from(v.len()).map_err(|_| SasWireError("seq too long".into()))?;
    w.write_u32(n);
    w.write_bytes(v);
    Ok(())
}

fn read_seq_octet(r: &mut Reader<'_>) -> Result<Vec<u8>, SasWireError> {
    let n = r.read_u32().map_err(cdr)? as usize;
    Ok(r.read_bytes(n).map_err(cdr)?.to_vec())
}

fn write_authorization_token(
    w: &mut Writer,
    token: &[AuthorizationElement],
) -> Result<(), SasWireError> {
    let n = u32::try_from(token.len()).map_err(|_| SasWireError("seq too long".into()))?;
    w.write_u32(n);
    for e in token {
        w.write_u32(e.the_type);
        write_seq_octet(w, &e.the_element)?;
    }
    Ok(())
}

fn read_authorization_token(r: &mut Reader<'_>) -> Result<Vec<AuthorizationElement>, SasWireError> {
    let n = r.read_u32().map_err(cdr)? as usize;
    // Each element takes at least eight octets; a count the buffer cannot
    // hold is refused before anything is allocated for it.
    if n > r.remaining() / 8 {
        return Err(SasWireError(alloc::format!(
            "AuthorizationToken count {n} exceeds the {} octets left",
            r.remaining()
        )));
    }
    let mut token = Vec::with_capacity(n);
    for _ in 0..n {
        token.push(AuthorizationElement {
            the_type: r.read_u32().map_err(cdr)?,
            the_element: read_seq_octet(r)?,
        });
    }
    Ok(token)
}

impl IdentityToken {
    fn encode(&self, w: &mut Writer) -> Result<(), SasWireError> {
        w.write_u32(self.discriminator());
        match self {
            // ITTAbsent/ITTAnonymous: `boolean` (octet) — we set true.
            Self::Absent | Self::Anonymous => {
                w.write_u8(1);
                Ok(())
            }
            Self::PrincipalName(v)
            | Self::X509CertChain(v)
            | Self::DistinguishedName(v)
            | Self::Extension { id: v, .. } => write_seq_octet(w, v),
        }
    }
    fn decode(r: &mut Reader<'_>) -> Result<Self, SasWireError> {
        let disc = r.read_u32().map_err(cdr)?;
        Ok(match disc {
            0 => {
                r.read_u8().map_err(cdr)?;
                Self::Absent
            }
            1 => {
                r.read_u8().map_err(cdr)?;
                Self::Anonymous
            }
            2 => Self::PrincipalName(read_seq_octet(r)?),
            4 => Self::X509CertChain(read_seq_octet(r)?),
            8 => Self::DistinguishedName(read_seq_octet(r)?),
            kind => Self::Extension {
                kind,
                id: read_seq_octet(r)?,
            },
        })
    }
}

impl SasMessage {
    /// Discriminator.
    #[must_use]
    pub fn msg_type(&self) -> SasMsgType {
        match self {
            Self::EstablishContext(_) => SasMsgType::EstablishContext,
            Self::CompleteEstablishContext(_) => SasMsgType::CompleteEstablishContext,
            Self::ContextError(_) => SasMsgType::ContextError,
            Self::MessageInContext(_) => SasMsgType::MessageInContext,
        }
    }

    /// Encodes the `SASContextBody` union (Spec §24.2.6.1) as a CDR
    /// encapsulation for `IOP::ServiceContext` tag 15. **Wire-correct**:
    /// byte-order octet, then discriminator + members with alignment
    /// relative to the encapsulation start (a single `Writer`,
    /// auto-align — NOT the self-consistent separate-writer style that
    /// misaligns the u32).
    ///
    /// # Errors
    /// CDR encode error / overflow.
    pub fn encode_encapsulation(&self, endianness: Endianness) -> Result<Vec<u8>, SasWireError> {
        let mut w = Writer::new(endianness);
        w.write_u8(match endianness {
            Endianness::Big => 0,
            Endianness::Little => 1,
        });
        w.write_i16(self.msg_type() as i16);
        match self {
            Self::EstablishContext(ec) => {
                w.write_u64(ec.client_context_id);
                write_authorization_token(&mut w, &ec.authorization_token)?;
                ec.identity_token.encode(&mut w)?;
                write_seq_octet(&mut w, &ec.client_authentication_token)?;
            }
            Self::CompleteEstablishContext(c) => {
                w.write_u64(c.client_context_id);
                w.write_u8(u8::from(c.context_stateful));
                write_seq_octet(&mut w, &c.final_context_token)?;
            }
            Self::ContextError(e) => {
                w.write_u64(e.client_context_id);
                w.write_u32(e.major_status);
                w.write_u32(e.minor_status);
                write_seq_octet(&mut w, &e.error_token)?;
            }
            Self::MessageInContext(m) => {
                w.write_u64(m.client_context_id);
                w.write_u8(u8::from(m.discard_context));
            }
        }
        Ok(w.finish())
    }

    /// Decodes a `SASContextBody` encapsulation (counterpart to
    /// [`Self::encode_encapsulation`]). Reads over the ENTIRE buffer
    /// (including the byte-order octet at offset 0) so that alignment
    /// works out correctly relative to the encapsulation origin.
    ///
    /// # Errors
    /// Empty buffer, invalid endianness octet, CDR decode error.
    pub fn decode_encapsulation(bytes: &[u8]) -> Result<Self, SasWireError> {
        let endianness = match bytes.first() {
            Some(0) => Endianness::Big,
            Some(1) => Endianness::Little,
            _ => return Err(SasWireError("invalid/empty SAS encapsulation".into())),
        };
        let mut r = Reader::new(bytes, endianness);
        r.read_u8().map_err(cdr)?; // byte-order octet
        let disc = r.read_i16().map_err(cdr)?;
        Ok(match disc {
            0 => Self::EstablishContext(EstablishContext {
                client_context_id: r.read_u64().map_err(cdr)?,
                authorization_token: read_authorization_token(&mut r)?,
                identity_token: IdentityToken::decode(&mut r)?,
                client_authentication_token: read_seq_octet(&mut r)?,
            }),
            1 => Self::CompleteEstablishContext(CompleteEstablishContext {
                client_context_id: r.read_u64().map_err(cdr)?,
                context_stateful: r.read_u8().map_err(cdr)? != 0,
                final_context_token: read_seq_octet(&mut r)?,
            }),
            4 => Self::ContextError(ContextError {
                client_context_id: r.read_u64().map_err(cdr)?,
                major_status: r.read_u32().map_err(cdr)?,
                minor_status: r.read_u32().map_err(cdr)?,
                error_token: read_seq_octet(&mut r)?,
            }),
            5 => Self::MessageInContext(MessageInContext {
                client_context_id: r.read_u64().map_err(cdr)?,
                discard_context: r.read_u8().map_err(cdr)? != 0,
            }),
            other => return Err(SasWireError(alloc::format!("bad SAS msg disc {other}"))),
        })
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn sas_msg_type_values_match_spec() {
        // Spec §24.2.6.1.
        assert_eq!(SasMsgType::EstablishContext as i16, 0);
        assert_eq!(SasMsgType::CompleteEstablishContext as i16, 1);
        assert_eq!(SasMsgType::ContextError as i16, 4);
        assert_eq!(SasMsgType::MessageInContext as i16, 5);
    }

    /// JacORB 3.9's `EstablishContext` for `alice`/`secret`, taken from a
    /// capture of its SAS client interceptor (service context 15 of a GIOP
    /// 1.2 big-endian request): the discriminator is a `short` at offset 2,
    /// `client_context_id` 1, no authorization elements, `ITTAbsent`, and a
    /// 42-octet GSS-wrapped GSSUP token.
    const JACORB_ESTABLISH_CONTEXT: [u8; 74] = [
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x2a, 0x60, 0x28, 0x06, 0x06, 0x67, 0x81, 0x02, 0x01, 0x01, 0x01, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x05, 0x61, 0x6c, 0x69, 0x63, 0x65, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x06, 0x73, 0x65, 0x63, 0x72, 0x65, 0x74, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];

    #[test]
    fn a_jacorb_establish_context_decodes_to_its_credentials() {
        let SasMessage::EstablishContext(ec) =
            SasMessage::decode_encapsulation(&JACORB_ESTABLISH_CONTEXT).unwrap()
        else {
            panic!("expected EstablishContext");
        };
        assert_eq!(ec.client_context_id, 1);
        assert!(ec.authorization_token.is_empty());
        assert_eq!(ec.identity_token, IdentityToken::Absent);
        let token =
            crate::GssupCredentialToken::from_gss_token(&ec.client_authentication_token).unwrap();
        assert_eq!(token.username, "alice");
        assert_eq!(token.password, "secret");
        // Encoding it again gives JacORB's octets back.
        assert_eq!(
            SasMessage::EstablishContext(ec)
                .encode_encapsulation(Endianness::Big)
                .unwrap(),
            JACORB_ESTABLISH_CONTEXT
        );
    }

    /// The replies the bridge sends, octet by octet: `MsgType` is a `short`
    /// at offset 2, `ContextId` an `unsigned long long` aligned to 8 from the
    /// encapsulation start. Written out from `CSI.idl` and the CDR alignment
    /// rules; the live check is the JacORB client in
    /// `corba-dds-bridge/tests/jacorb_e2e.rs`.
    #[test]
    fn the_discriminator_is_a_short_at_offset_two() {
        let complete = SasMessage::CompleteEstablishContext(CompleteEstablishContext {
            client_context_id: 1,
            context_stateful: false,
            final_context_token: alloc::vec![],
        })
        .encode_encapsulation(Endianness::Little)
        .unwrap();
        assert_eq!(
            complete,
            [
                0x01, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, // order, pad, MsgType 1, pad
                0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // client_context_id 1
                0x00, 0x00, 0x00, 0x00, // context_stateful FALSE, pad
                0x00, 0x00, 0x00, 0x00, // final_context_token: 0 octets
            ]
        );
        let error = SasMessage::ContextError(ContextError {
            client_context_id: 1,
            major_status: 1,
            minor_status: 1,
            error_token: alloc::vec![],
        })
        .encode_encapsulation(Endianness::Big)
        .unwrap();
        assert_eq!(
            error,
            [
                0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00, 0x00, // order, pad, MsgType 4, pad
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, // client_context_id 1
                0x00, 0x00, 0x00, 0x01, // major_status 1
                0x00, 0x00, 0x00, 0x01, // minor_status 1
                0x00, 0x00, 0x00, 0x00, // error_token: 0 octets
            ]
        );
    }

    #[test]
    fn authorization_elements_and_an_identity_extension_round_trip() {
        let msg = SasMessage::EstablishContext(EstablishContext {
            client_context_id: 3,
            authorization_token: alloc::vec![
                AuthorizationElement {
                    the_type: 0x4F4D_0001,
                    the_element: alloc::vec![1, 2, 3],
                },
                AuthorizationElement {
                    the_type: 7,
                    the_element: alloc::vec![],
                },
            ],
            identity_token: IdentityToken::Extension {
                kind: 16,
                id: alloc::vec![9, 9],
            },
            client_authentication_token: alloc::vec![],
        });
        sas_roundtrip(&msg, Endianness::Big);
        sas_roundtrip(&msg, Endianness::Little);
    }

    #[test]
    fn an_authorization_token_count_the_buffer_cannot_hold_is_refused() {
        let mut bytes = JACORB_ESTABLISH_CONTEXT;
        // AuthorizationToken count at offset 16: 0x00ffffff elements.
        bytes[17] = 0xff;
        bytes[18] = 0xff;
        bytes[19] = 0xff;
        let err = SasMessage::decode_encapsulation(&bytes).unwrap_err();
        assert!(err.0.contains("AuthorizationToken count"), "{err:?}");
    }

    fn sas_roundtrip(msg: &SasMessage, e: Endianness) {
        let bytes = msg.encode_encapsulation(e).unwrap();
        // Byte-order octet correct.
        assert_eq!(bytes[0], if e == Endianness::Big { 0 } else { 1 });
        let decoded = SasMessage::decode_encapsulation(&bytes).unwrap();
        assert_eq!(&decoded, msg);
    }

    #[test]
    fn sas_context_body_roundtrip_all_messages_be_le() {
        let msgs = [
            SasMessage::EstablishContext(EstablishContext {
                client_context_id: 0x0123_4567_89ab_cdef,
                authorization_token: alloc::vec![],
                identity_token: IdentityToken::PrincipalName(alloc::vec![1, 2, 3]),
                client_authentication_token: alloc::vec![0xaa, 0xbb, 0xcc],
            }),
            SasMessage::CompleteEstablishContext(CompleteEstablishContext {
                client_context_id: 7,
                context_stateful: true,
                final_context_token: alloc::vec![],
            }),
            SasMessage::ContextError(ContextError {
                client_context_id: 7,
                major_status: 1,
                minor_status: 2,
                error_token: alloc::vec![0xff],
            }),
            SasMessage::MessageInContext(MessageInContext {
                client_context_id: 7,
                discard_context: false,
            }),
        ];
        for m in &msgs {
            sas_roundtrip(m, Endianness::Big);
            sas_roundtrip(m, Endianness::Little);
        }
    }

    #[test]
    fn server_eval_decodes_foreign_gss_wrapped_establish_context() {
        // Reverse SAS direction — ZeroDDS as the **SAS target** (#2 cross-ORB
        // reverse): a foreign ORB client (JacORB format) sends an
        // EstablishContext whose `client_authentication_token` is a
        // GSS-InitialContextToken-wrapped GSSUP, byte-identical to JacORB
        // (proven by `gssup::gssup_byte_identical_to_jacorb` +
        // `gss_token_structure_and_round_trip`). The server's exact decode
        // chain — `decode_encapsulation` → match `EstablishContext` →
        // `from_gss_token` — must recover the credentials so the validator can
        // accept/reject them. This is the in-repo half of the reverse cross-ORB
        // proof; the live JacORB-client→ZeroDDS-server rig is gated on the Linux test host.
        use crate::GssupCredentialToken;
        for e in [Endianness::Big, Endianness::Little] {
            let cred = GssupCredentialToken::new("alice".into(), "secret".into(), Vec::new());
            // GSS-wrapped GSSUP exactly as a foreign client puts it on the wire.
            let gss = cred.to_gss_token(e).unwrap();
            let msg = SasMessage::EstablishContext(EstablishContext {
                client_context_id: 0,
                authorization_token: alloc::vec![],
                identity_token: IdentityToken::Absent,
                client_authentication_token: gss,
            });
            let wire = msg.encode_encapsulation(e).unwrap();

            // ---- server side: the same decode chain `csiv2_authenticate` runs ----
            let decoded = SasMessage::decode_encapsulation(&wire).unwrap();
            let SasMessage::EstablishContext(ec) = decoded else {
                panic!("expected EstablishContext");
            };
            let token = GssupCredentialToken::from_gss_token(&ec.client_authentication_token)
                .expect("server must unwrap the foreign GSS-InitialContextToken");
            assert_eq!(token.username, "alice");
            assert_eq!(token.password, "secret");
        }
    }

    #[test]
    fn sas_identity_token_absent_anonymous_roundtrip() {
        for it in [IdentityToken::Absent, IdentityToken::Anonymous] {
            let m = SasMessage::EstablishContext(EstablishContext {
                client_context_id: 1,
                authorization_token: alloc::vec![],
                identity_token: it,
                client_authentication_token: alloc::vec![9],
            });
            sas_roundtrip(&m, Endianness::Big);
        }
    }

    #[test]
    fn identity_token_discriminator_matches_spec() {
        // Spec §24.2.5: ITT* = power-of-2.
        assert_eq!(IdentityToken::Absent.discriminator(), 0);
        assert_eq!(IdentityToken::Anonymous.discriminator(), 1);
        assert_eq!(
            IdentityToken::PrincipalName(alloc::vec![]).discriminator(),
            2
        );
        assert_eq!(
            IdentityToken::X509CertChain(alloc::vec![]).discriminator(),
            4
        );
        assert_eq!(
            IdentityToken::DistinguishedName(alloc::vec![]).discriminator(),
            8
        );
    }

    #[test]
    fn establish_context_holds_full_payload() {
        let ec = EstablishContext {
            client_context_id: 42,
            authorization_token: alloc::vec![],
            identity_token: IdentityToken::PrincipalName(b"alice@REALM".to_vec()),
            client_authentication_token: alloc::vec![0xab, 0xcd],
        };
        let msg = SasMessage::EstablishContext(ec);
        assert_eq!(msg.msg_type(), SasMsgType::EstablishContext);
    }

    #[test]
    fn complete_context_indicates_stateful() {
        let cc = CompleteEstablishContext {
            client_context_id: 42,
            context_stateful: true,
            final_context_token: alloc::vec![],
        };
        let msg = SasMessage::CompleteEstablishContext(cc);
        match msg {
            SasMessage::CompleteEstablishContext(c) => assert!(c.context_stateful),
            _ => panic!(),
        }
    }

    #[test]
    fn message_in_context_can_request_discard() {
        let m = MessageInContext {
            client_context_id: 42,
            discard_context: true,
        };
        let msg = SasMessage::MessageInContext(m);
        assert_eq!(msg.msg_type(), SasMsgType::MessageInContext);
    }

    #[test]
    fn context_error_carries_gss_status() {
        let e = ContextError {
            client_context_id: 42,
            major_status: 0x0007_0000, // GSS_S_DEFECTIVE_TOKEN
            minor_status: 0,
            error_token: alloc::vec![],
        };
        let msg = SasMessage::ContextError(e);
        assert_eq!(msg.msg_type(), SasMsgType::ContextError);
    }
}
