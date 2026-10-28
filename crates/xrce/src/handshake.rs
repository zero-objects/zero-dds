// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `CLIENT_Representation` / `AGENT_Representation` (DDS-XRCE 1.0 Annex A
//! IDL Types, §7.8.2/§8.4.5).
//!
//! `CREATE_CLIENT` (§8.3.5.1) and `STATUS_AGENT` (§8.3.5.5) carry these two
//! structures as their whole body. `crate::submessages::create_client`
//! carries the typed [`ClientRepresentation`] directly; `STATUS_AGENT`'s
//! `crate::submessages::status_agent` still keeps its body opaque
//! (`representation: Vec<u8>`). This module is the shared codec
//! `crate::daemon::session::SessionEngine` (`zerodds-xrce-agent`) and
//! `zerodds-xrce-client`'s `XrceClient` use to run the handshake — moved
//! here from `zerodds-xrce-agent` (where it was originally written to avoid
//! colliding with concurrent, unrelated work in this crate's `submessages`
//! module) so every crate builds its own `CREATE_CLIENT` from the same
//! codec instead of duplicating it.
//!
//! ```text
//! @extensibility(FINAL)
//! struct CLIENT_Representation {
//!      XrceCookie      xrce_cookie;    // XRCE_COOKIE
//!      XrceVersion     xrce_version;
//!      XrceVendorId    xrce_vendor_id;
//!      ClientKey       client_key;
//!      SessionId       session_id;
//!      @optional     PropertySeq properties;
//! };
//!
//! @extensibility(FINAL)
//! struct AGENT_Representation {
//!      XrceCookie      xrce_cookie;     // XRCE_COOKIE
//!      XrceVersion     xrce_version;
//!      XrceVendorId    xrce_vendor_id;
//!      @optional     PropertySeq     properties;
//! };
//! ```
//!
//! ## Byte-oracle check, and the resolved `requested_mtu` question
//!
//! The first datagram of `tools/wire-capture/xrce-sessions/client-to-agent.hex`
//! is a `CREATE_CLIENT` with a 16-byte body:
//! `58524345 0100 010f ccccdddd 81 00fc01`. The first 13 bytes decode
//! cleanly against the struct above: cookie `"XRCE"`, version `{1,0}`,
//! vendor `{0x01,0x0f}` (eProsima Fast DDS's own registered RTPS vendor ID,
//! reused here), `client_key` `CCCCDDDD`, `session_id` `0x81`, then a `00`
//! properties-absent octet. Two bytes remain: `fc 01`.
//!
//! Read as a little-endian `unsigned short`, `fc 01` is **508** — and 508
//! is also, independently, the largest datagram in the *entire* 1,772-
//! datagram capture in both directions (measured directly off
//! `client-to-agent.hex`/`agent-to-client.hex`, including every
//! `FRAGMENT`-carrying one): no datagram in the whole recorded session
//! exceeds it. That is not a coincidence a "properties tail padding"
//! theory can produce — a fixed value that also happens to be the
//! session's hard MTU ceiling only makes sense as a declared MTU. This is
//! not in the OMG DDS-XRCE 1.0 text (no `CLIENT_Representation` field
//! after `properties` is defined anywhere in it); it is eProsima's own,
//! non-normative extension. [`ClientRepresentation`] therefore models it
//! as `requested_mtu: Option<u16>` — present whenever a two-byte tail
//! follows the properties octet, `None` otherwise — and both
//! `zerodds-xrce-agent`'s `SessionEngine` and `zerodds-xrce-client`'s
//! `XrceClient` use it (falling back to [`DEFAULT_MTU`] = 508 when absent,
//! the same value, for a client that omits it) to decide when an outbound
//! `DATA` needs `FRAGMENT`ing. See
//! `client_representation_decodes_the_captured_create_client_prefix` for
//! the byte-exact check and `mtu_508_is_the_captures_hard_ceiling` for the
//! measurement backing the 508 conclusion.
//!
//! The first datagram of `agent-to-client.hex` is a `STATUS_AGENT` with an
//! 11-byte body: `0000 58524345 0100 010f 00`. The leading `0000` is a
//! `ResultStatus` (`STATUS_OK`, `implementation_status=0`) — the spec
//! text's `STATUS_AGENT_Payload { AGENT_Representation agent_info; }`
//! listing drops this field, but the measured `submessage_length=11`
//! only closes with it present: `ResultStatus`(2) + `AGENT_Representation`
//! (9, cookie 4 + version 2 + vendor 2 + a `00` properties-absent octet).
//! `zerodds-xrce-agent`'s `session.rs::agent_status_reply` builds
//! `StatusAgentPayload` with that leading `ResultStatus`; this module only
//! models `AGENT_Representation` itself, decoded from the trailing 9 bytes
//! (see `agent_representation_decodes_the_captured_status_agent_body`).

extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;

use crate::encoding::Endianness;
use crate::encoding::{RepresentationTimestamps, representation_timestamps};
use crate::error::XrceError;
use crate::header::{CLIENT_KEY_LEN, ClientKey};
use crate::submessages::timestamp::TimePoint;

/// `XrceCookie` (Annex A `typedef octet XrceCookie[4]`) — spells `'X' 'R'
/// 'C' 'E'`.
pub const XRCE_COOKIE: [u8; 4] = *b"XRCE";

/// `XrceVersion` (Annex A) — `{ XRCE_VERSION_MAJOR, XRCE_VERSION_MINOR }`.
pub const XRCE_VERSION: [u8; 2] = [0x01, 0x00];

/// The MTU to assume for a client that sent no `requested_mtu` (see the
/// module doc) — the same 508 bytes eProsima's own client declares, so a
/// client not carrying the extension still gets the fragmentation
/// threshold its own reference implementation would have asked for.
pub const DEFAULT_MTU: u16 = 508;

/// A `Property { string name; string value; }` (Annex A), one element of a
/// `PropertySeq`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Property {
    /// Property name.
    pub name: String,
    /// Property value.
    pub value: String,
}

fn pad_to(out: &mut Vec<u8>, n: usize) {
    while out.len() % n != 0 {
        out.push(0);
    }
}

fn align_pos(pos: usize, n: usize) -> usize {
    pos.div_ceil(n) * n
}

fn write_u32(v: u32, out: &mut Vec<u8>, e: Endianness) {
    pad_to(out, 4);
    out.extend_from_slice(&match e {
        Endianness::Little => v.to_le_bytes(),
        Endianness::Big => v.to_be_bytes(),
    });
}

/// Reads an `unsigned long` that starts at the first 4-aligned position at or
/// after `pos`; returns it and the position after it.
fn read_u32(bytes: &[u8], pos: usize, e: Endianness) -> Result<(u32, usize), XrceError> {
    let at = align_pos(pos, 4);
    let b = bytes.get(at..at + 4).ok_or(XrceError::UnexpectedEof {
        needed: at + 4,
        offset: bytes.len(),
    })?;
    let raw = [b[0], b[1], b[2], b[3]];
    Ok((
        match e {
            Endianness::Little => u32::from_le_bytes(raw),
            Endianness::Big => u32::from_be_bytes(raw),
        },
        at + 4,
    ))
}

/// An XCDR2 `string`: the length (an `unsigned long`, aligned to 4) counting
/// the NUL, the characters, the NUL.
fn encode_xcdr_string(s: &str, out: &mut Vec<u8>, e: Endianness) -> Result<(), XrceError> {
    let bytes = s.as_bytes();
    let len = u32::try_from(bytes.len() + 1).map_err(|_| XrceError::ValueOutOfRange {
        message: "Property string exceeds u32",
    })?;
    write_u32(len, out, e);
    out.extend_from_slice(bytes);
    out.push(0);
    Ok(())
}

fn decode_xcdr_string(
    bytes: &[u8],
    pos: usize,
    e: Endianness,
) -> Result<(String, usize), XrceError> {
    let (len, at) = read_u32(bytes, pos, e)?;
    let len = len as usize;
    if len == 0 || len > 65_536 {
        return Err(XrceError::ValueOutOfRange {
            message: "Property string length out of range",
        });
    }
    let raw = bytes.get(at..at + len).ok_or(XrceError::UnexpectedEof {
        needed: at + len,
        offset: bytes.len(),
    })?;
    let trimmed = raw.strip_suffix(&[0]).unwrap_or(raw);
    let s = core::str::from_utf8(trimmed)
        .map(alloc::string::ToString::to_string)
        .map_err(|_| XrceError::ValueOutOfRange {
            message: "Property string is not valid utf-8",
        })?;
    Ok((s, at + len))
}

/// Writes the `Time_t` member of a representation when the setting asks for
/// it (after the 8 octets of cookie, version and vendor id).
fn encode_timestamp(
    timestamp: Option<TimePoint>,
    form: RepresentationTimestamps,
    out: &mut Vec<u8>,
    e: Endianness,
) {
    if form == RepresentationTimestamps::Present {
        pad_to(out, 4);
        timestamp.unwrap_or_default().encode_into(out, e);
    }
}

/// Reads the `Time_t` member that starts at `pos` when the setting asks for
/// it; returns it and the position after it.
fn decode_timestamp(
    bytes: &[u8],
    pos: usize,
    form: RepresentationTimestamps,
    e: Endianness,
) -> Result<(Option<TimePoint>, usize), XrceError> {
    if form == RepresentationTimestamps::Present {
        let (t, next) = TimePoint::decode_at(bytes, align_pos(pos, 4), e)?;
        Ok((Some(t), next))
    } else {
        Ok((None, pos))
    }
}

/// DoS cap on the number of `Property` entries a `PropertySeq` may carry.
pub const MAX_PROPERTIES: usize = 64;

/// `@optional PropertySeq`: the presence octet, and when present the
/// `sequence<Property>` — its length as an `unsigned long` aligned to 4, then
/// each `Property` as two strings (Annex A). Padding is counted from the start
/// of `out`, which starts the payload.
fn encode_properties(
    properties: Option<&[Property]>,
    out: &mut Vec<u8>,
    e: Endianness,
) -> Result<(), XrceError> {
    match properties {
        None => out.push(0),
        Some(props) => {
            out.push(1);
            let count = u32::try_from(props.len()).map_err(|_| XrceError::ValueOutOfRange {
                message: "PropertySeq length exceeds u32",
            })?;
            write_u32(count, out, e);
            for p in props {
                // Property carries no @extensibility (Annex A): APPENDABLE,
                // a DHEADER in front, unless the setting is `Final`.
                let members = crate::encoding::begin_unannotated(out);
                encode_xcdr_string(&p.name, out, e)?;
                encode_xcdr_string(&p.value, out, e)?;
                crate::encoding::end_unannotated(out, members, e);
            }
        }
    }
    Ok(())
}

/// Decodes an `@optional PropertySeq` that starts at `pos` of `bytes`, which
/// starts the payload. Returns the properties (`None` if the presence octet is
/// not `1`, or there is none) and the position after it: `pos + 1` for an
/// absent sequence, `pos` when `bytes` end before the presence octet, the end
/// of the sequence when present and well-formed. The caller uses the position
/// to find whatever comes after `properties` in the struct
/// (`ClientRepresentation`'s `requested_mtu`).
///
/// # Errors
/// A present sequence that does not parse (element count above
/// [`MAX_PROPERTIES`], a truncated or non-UTF-8 string). DDS-XRCE 1.0
/// §7.8.2.1 has the agent check the client properties and fail the
/// operation when the check fails; a sequence it cannot read is such a
/// failure and must not read as "no properties".
fn decode_properties(
    bytes: &[u8],
    pos: usize,
    e: Endianness,
) -> Result<(Option<Vec<Property>>, usize), XrceError> {
    let Some(&presence) = bytes.get(pos) else {
        return Ok((None, pos));
    };
    if presence != 1 {
        return Ok((None, pos + 1));
    }
    let (count, mut at) = read_u32(bytes, pos + 1, e)?;
    let count = count as usize;
    if count > MAX_PROPERTIES {
        return Err(XrceError::ValueOutOfRange {
            message: "PropertySeq element count exceeds cap",
        });
    }
    let mut props = Vec::with_capacity(count.min(16));
    for _ in 0..count {
        let (property, next) = decode_property(bytes, at, e)?;
        at = next;
        props.push(property);
    }
    Ok((Some(props), at))
}

/// One `Property` that starts at `pos`: in the form the setting names first
/// (with a DHEADER whose length the two strings fill exactly, or without),
/// else in the other.
fn decode_property(
    bytes: &[u8],
    pos: usize,
    e: Endianness,
) -> Result<(Property, usize), XrceError> {
    let members = |at: usize| -> Result<(Property, usize), XrceError> {
        let (name, next) = decode_xcdr_string(bytes, at, e)?;
        let (value, next) = decode_xcdr_string(bytes, next, e)?;
        Ok((Property { name, value }, next))
    };
    let appendable = || -> Result<(Property, usize), XrceError> {
        let (start, end) =
            crate::encoding::read_dheader(bytes, pos, e).ok_or(XrceError::UnexpectedEof {
                needed: align_pos(pos, 4) + 4,
                offset: bytes.len(),
            })?;
        let (property, next) = members(start)?;
        if next != end {
            return Err(XrceError::ValueOutOfRange {
                message: "Property: the DHEADER does not match its two strings",
            });
        }
        Ok((property, end))
    };
    match crate::encoding::unannotated_types() {
        crate::encoding::UnannotatedTypes::Appendable => appendable().or_else(|_| members(pos)),
        crate::encoding::UnannotatedTypes::Final => members(pos).or_else(|_| appendable()),
    }
}

/// `CLIENT_Representation` (Annex A, §7.8.2.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientRepresentation {
    /// `xrce_version` — `{major, minor}`.
    pub version: [u8; 2],
    /// `xrce_vendor_id` — the sender's registered vendor id, or `{0,0}`.
    pub vendor_id: [u8; 2],
    /// `client_timestamp` (§7.8.2.1 prose). Written and read only with
    /// [`RepresentationTimestamps::Present`](crate::encoding::RepresentationTimestamps);
    /// `None` when absent, written as zero when present and `None`.
    pub timestamp: Option<TimePoint>,
    /// The client's requested identity.
    pub client_key: ClientKey,
    /// The client's requested `session_id` (§8.3.2.1: `<=127` carries a
    /// `ClientKey` in the `MessageHeader`, `>=128` does not).
    pub session_id: u8,
    /// Optional authentication-protocol properties (§7.8.2.1: "may be used
    /// to implement an authentication protocol \[...\] the specific
    /// mechanism is outside the scope of this specification"). Not
    /// interpreted here.
    pub properties: Option<Vec<Property>>,
    /// eProsima's non-normative MTU extension after `properties` — see the
    /// module doc. `None` when the client sent no trailing bytes at all
    /// (a strictly spec-minimal `CLIENT_Representation`); use
    /// [`DEFAULT_MTU`] in that case, not `None` as "no limit".
    pub requested_mtu: Option<u16>,
}

/// `AGENT_Representation` (Annex A, §7.8.2.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentRepresentation {
    /// `xrce_version`.
    pub version: [u8; 2],
    /// `xrce_vendor_id` — this agent's registered vendor id.
    pub vendor_id: [u8; 2],
    /// `agent_timestamp` (§7.8.2.1 prose); see
    /// [`ClientRepresentation::timestamp`].
    pub timestamp: Option<TimePoint>,
    /// Optional authentication-protocol properties.
    pub properties: Option<Vec<Property>>,
}

impl ClientRepresentation {
    /// Encodes a representation that starts its payload (`CREATE_CLIENT`):
    /// the fixed prefix, the properties and `requested_mtu` (if set — see
    /// the module doc; a peer that does not understand this crate's own
    /// encoding of it should still read every byte up to it correctly,
    /// since it comes last), little-endian.
    ///
    /// # Errors
    /// `ValueOutOfRange` on a length overflow.
    pub fn encode(&self) -> Result<Vec<u8>, XrceError> {
        self.encode_with(Endianness::Little)
    }

    /// As [`Self::encode`], with the byte order of the submessage's
    /// E flag (DDS-XRCE 1.0 §8.3.4.2: "the endianness used to encode the
    /// submessage header and payload").
    ///
    /// # Errors
    /// `ValueOutOfRange` on a length overflow.
    pub fn encode_with(&self, e: Endianness) -> Result<Vec<u8>, XrceError> {
        let mut out = Vec::with_capacity(16);
        self.encode_into(&mut out, e)?;
        Ok(out)
    }

    /// Appends the representation to `out`, which starts the payload: the
    /// padding of the `PropertySeq` is counted from `out[0]`.
    ///
    /// # Errors
    /// `ValueOutOfRange` on a length overflow.
    pub fn encode_into(&self, out: &mut Vec<u8>, e: Endianness) -> Result<(), XrceError> {
        self.encode_form(out, e, representation_timestamps())
    }

    /// As [`Self::encode_into`], in the form `form` names instead of the one
    /// of the process (a client with its own setting).
    ///
    /// # Errors
    /// `ValueOutOfRange` on a length overflow.
    pub fn encode_form(
        &self,
        out: &mut Vec<u8>,
        e: Endianness,
        form: RepresentationTimestamps,
    ) -> Result<(), XrceError> {
        out.extend_from_slice(&XRCE_COOKIE);
        out.extend_from_slice(&self.version);
        out.extend_from_slice(&self.vendor_id);
        encode_timestamp(self.timestamp, form, out, e);
        out.extend_from_slice(self.client_key.as_bytes());
        out.push(self.session_id);
        encode_properties(self.properties.as_deref(), out, e)?;
        if let Some(mtu) = self.requested_mtu {
            pad_to(out, 2);
            out.extend_from_slice(&match e {
                Endianness::Little => mtu.to_le_bytes(),
                Endianness::Big => mtu.to_be_bytes(),
            });
        }
        Ok(())
    }

    /// Decodes a representation that starts its payload, little-endian.
    ///
    /// # Errors
    /// `UnexpectedEof` if shorter than the 13-byte fixed prefix;
    /// `ValueOutOfRange` if the cookie does not read `"XRCE"` or a present
    /// `PropertySeq` is malformed (DDS-XRCE 1.0 §7.8.2.1: the properties
    /// are checked and a failed check fails the operation).
    pub fn decode(bytes: &[u8]) -> Result<Self, XrceError> {
        Self::decode_with(bytes, Endianness::Little)
    }

    /// As [`Self::decode`], in the byte order of the submessage's E flag
    /// (§8.3.4.2).
    ///
    /// # Errors
    /// As [`Self::decode`].
    pub fn decode_with(bytes: &[u8], e: Endianness) -> Result<Self, XrceError> {
        Self::decode_at(bytes, 0, e).map(|(rep, _)| rep)
    }

    /// Decodes the representation that starts at `pos` of `bytes`, which
    /// starts the payload. Returns it and the position after it.
    ///
    /// # Errors
    /// As [`Self::decode`].
    pub fn decode_at(bytes: &[u8], pos: usize, e: Endianness) -> Result<(Self, usize), XrceError> {
        Self::decode_form(bytes, pos, e, representation_timestamps())
    }

    /// As [`Self::decode_at`], in the form `form` names.
    ///
    /// # Errors
    /// As [`Self::decode`].
    pub fn decode_form(
        bytes: &[u8],
        pos: usize,
        e: Endianness,
        form: RepresentationTimestamps,
    ) -> Result<(Self, usize), XrceError> {
        const PREFIX_LEN: usize = 4 + 2 + 2;
        let b = bytes.get(pos..).unwrap_or(&[]);
        if b.len() < PREFIX_LEN {
            return Err(XrceError::UnexpectedEof {
                needed: PREFIX_LEN,
                offset: b.len(),
            });
        }
        if b[0..4] != XRCE_COOKIE {
            return Err(XrceError::ValueOutOfRange {
                message: "CLIENT_Representation xrce_cookie is not \"XRCE\"",
            });
        }
        let version = [b[4], b[5]];
        let vendor_id = [b[6], b[7]];
        let (timestamp, key_at) = decode_timestamp(bytes, pos + PREFIX_LEN, form, e)?;
        let tail = bytes.get(key_at..).unwrap_or(&[]);
        if tail.len() < CLIENT_KEY_LEN + 1 {
            return Err(XrceError::UnexpectedEof {
                needed: key_at + CLIENT_KEY_LEN + 1,
                offset: bytes.len(),
            });
        }
        let mut key = [0u8; CLIENT_KEY_LEN];
        key.copy_from_slice(&tail[..CLIENT_KEY_LEN]);
        let session_id = tail[CLIENT_KEY_LEN];
        let (properties, mut at) = decode_properties(bytes, key_at + CLIENT_KEY_LEN + 1, e)?;
        let mtu_at = align_pos(at, 2);
        let requested_mtu = bytes.get(mtu_at..mtu_at + 2).map(|m| match e {
            Endianness::Little => u16::from_le_bytes([m[0], m[1]]),
            Endianness::Big => u16::from_be_bytes([m[0], m[1]]),
        });
        if requested_mtu.is_some() {
            at = mtu_at + 2;
        }
        Ok((
            Self {
                version,
                vendor_id,
                timestamp,
                client_key: ClientKey(key),
                session_id,
                properties,
                requested_mtu,
            },
            at,
        ))
    }
}

impl AgentRepresentation {
    /// Encodes a representation that starts its payload, little-endian.
    ///
    /// # Errors
    /// `ValueOutOfRange` on a length overflow.
    pub fn encode(&self) -> Result<Vec<u8>, XrceError> {
        self.encode_with(Endianness::Little)
    }

    /// As [`Self::encode`], in the byte order of the submessage's E flag
    /// (§8.3.4.2).
    ///
    /// # Errors
    /// `ValueOutOfRange` on a length overflow.
    pub fn encode_with(&self, e: Endianness) -> Result<Vec<u8>, XrceError> {
        let mut out = Vec::with_capacity(9);
        self.encode_into(&mut out, e)?;
        Ok(out)
    }

    /// Appends the representation to `out`, which starts the payload: the
    /// padding of the `PropertySeq` is counted from `out[0]` (a
    /// `STATUS_AGENT` has its two-octet `ResultStatus` in front, an `INFO`
    /// more).
    ///
    /// # Errors
    /// `ValueOutOfRange` on a length overflow.
    pub fn encode_into(&self, out: &mut Vec<u8>, e: Endianness) -> Result<(), XrceError> {
        self.encode_form(out, e, representation_timestamps())
    }

    /// As [`Self::encode_into`], in the form `form` names.
    ///
    /// # Errors
    /// `ValueOutOfRange` on a length overflow.
    pub fn encode_form(
        &self,
        out: &mut Vec<u8>,
        e: Endianness,
        form: RepresentationTimestamps,
    ) -> Result<(), XrceError> {
        out.extend_from_slice(&XRCE_COOKIE);
        out.extend_from_slice(&self.version);
        out.extend_from_slice(&self.vendor_id);
        encode_timestamp(self.timestamp, form, out, e);
        encode_properties(self.properties.as_deref(), out, e)
    }

    /// Decodes a representation that starts its payload, little-endian.
    ///
    /// # Errors
    /// `UnexpectedEof` if shorter than the 8-byte fixed prefix;
    /// `ValueOutOfRange` if the cookie does not read `"XRCE"` or a present
    /// `PropertySeq` is malformed.
    pub fn decode(bytes: &[u8]) -> Result<Self, XrceError> {
        Self::decode_with(bytes, Endianness::Little)
    }

    /// As [`Self::decode`], in the byte order of the submessage's E flag
    /// (§8.3.4.2).
    ///
    /// # Errors
    /// As [`Self::decode`].
    pub fn decode_with(bytes: &[u8], e: Endianness) -> Result<Self, XrceError> {
        Self::decode_at(bytes, 0, e).map(|(rep, _)| rep)
    }

    /// Decodes the representation that starts at `pos` of `bytes`, which
    /// starts the payload. Returns it and the position after it.
    ///
    /// # Errors
    /// As [`Self::decode`].
    pub fn decode_at(bytes: &[u8], pos: usize, e: Endianness) -> Result<(Self, usize), XrceError> {
        Self::decode_form(bytes, pos, e, representation_timestamps())
    }

    /// As [`Self::decode_at`], in the form `form` names.
    ///
    /// # Errors
    /// As [`Self::decode`].
    pub fn decode_form(
        bytes: &[u8],
        pos: usize,
        e: Endianness,
        form: RepresentationTimestamps,
    ) -> Result<(Self, usize), XrceError> {
        const FIXED_LEN: usize = 4 + 2 + 2;
        let b = bytes.get(pos..).unwrap_or(&[]);
        if b.len() < FIXED_LEN {
            return Err(XrceError::UnexpectedEof {
                needed: FIXED_LEN,
                offset: b.len(),
            });
        }
        if b[0..4] != XRCE_COOKIE {
            return Err(XrceError::ValueOutOfRange {
                message: "AGENT_Representation xrce_cookie is not \"XRCE\"",
            });
        }
        let (timestamp, props_at) = decode_timestamp(bytes, pos + FIXED_LEN, form, e)?;
        let (properties, at) = decode_properties(bytes, props_at, e)?;
        Ok((
            Self {
                version: [b[4], b[5]],
                vendor_id: [b[6], b[7]],
                timestamp,
                properties,
            },
            at,
        ))
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::encoding::{RepresentationTimestamps, representation_timestamps};

    fn key(b: u8) -> ClientKey {
        ClientKey([b; CLIENT_KEY_LEN])
    }

    #[test]
    fn client_representation_roundtrip_no_properties() {
        let c = ClientRepresentation {
            timestamp: None,
            version: XRCE_VERSION,
            vendor_id: [0x01, 0x0f],
            client_key: key(0xAB),
            session_id: 0x81,
            properties: None,
            requested_mtu: None,
        };
        let enc = c.encode().unwrap();
        assert_eq!(enc.len(), 14);
        let back = ClientRepresentation::decode(&enc).unwrap();
        assert_eq!(back, c);
    }

    #[test]
    fn client_representation_roundtrip_with_properties() {
        let c = ClientRepresentation {
            timestamp: None,
            version: XRCE_VERSION,
            vendor_id: [0, 0],
            client_key: key(0x01),
            session_id: 0x00,
            properties: Some(alloc::vec![Property {
                name: "auth".into(),
                value: "token123".into(),
            }]),
            requested_mtu: None,
        };
        let enc = c.encode().unwrap();
        let back = ClientRepresentation::decode(&enc).unwrap();
        assert_eq!(back, c);
    }

    #[test]
    fn client_representation_roundtrip_with_requested_mtu() {
        let c = ClientRepresentation {
            timestamp: None,
            version: XRCE_VERSION,
            vendor_id: [0x01, 0x0f],
            client_key: key(0xCC),
            session_id: 0x81,
            properties: None,
            requested_mtu: Some(DEFAULT_MTU),
        };
        let enc = c.encode().unwrap();
        assert_eq!(enc.len(), 16);
        let back = ClientRepresentation::decode(&enc).unwrap();
        assert_eq!(back, c);
    }

    #[test]
    fn client_representation_rejects_wrong_cookie() {
        let mut bad = alloc::vec![0u8; 14];
        bad[0..4].copy_from_slice(b"NOPE");
        let res = ClientRepresentation::decode(&bad);
        assert!(matches!(res, Err(XrceError::ValueOutOfRange { .. })));
    }

    #[test]
    fn client_representation_rejects_truncated_prefix() {
        let res = ClientRepresentation::decode(b"XRCE");
        assert!(matches!(res, Err(XrceError::UnexpectedEof { .. })));
    }

    /// Byte-oracle check: `client-to-agent.hex` line 1's 16-byte
    /// `CREATE_CLIENT` body, first 13 bytes — see the module doc.
    #[test]
    fn client_representation_decodes_the_captured_create_client_prefix() {
        let captured: [u8; 16] = [
            0x58, 0x52, 0x43, 0x45, 0x01, 0x00, 0x01, 0x0f, 0xcc, 0xcc, 0xdd, 0xdd, 0x81, 0x00,
            0xfc, 0x01,
        ];
        let c = ClientRepresentation::decode(&captured).expect("the fixed prefix must decode");
        assert_eq!(c.version, [0x01, 0x00]);
        assert_eq!(c.vendor_id, [0x01, 0x0f], "eProsima's registered vendor id");
        assert_eq!(c.client_key, ClientKey([0xcc, 0xcc, 0xdd, 0xdd]));
        assert_eq!(c.session_id, 0x81);
        assert_eq!(c.properties, None);
        assert_eq!(
            c.requested_mtu,
            Some(508),
            "the trailing 0xfc,0x01 is a little-endian 508 — see the module doc"
        );
    }

    /// Measured directly off both capture files: no datagram in the
    /// entire 1,772-datagram session (either direction, including every
    /// `FRAGMENT`-carrying one) exceeds 508 bytes — the same value as
    /// `client_representation_decodes_the_captured_create_client_prefix`'s
    /// `requested_mtu`. This is the evidence the module doc's `mtu`
    /// conclusion rests on, pinned here so a future capture update that
    /// moves it is not silent.
    #[test]
    fn mtu_508_is_the_captures_hard_ceiling() {
        fn max_datagram_len(path: &str) -> usize {
            std::fs::read_to_string(path)
                .unwrap()
                .lines()
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .map(|l| l.len() / 2)
                .max()
                .unwrap()
        }
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tools/wire-capture/xrce-sessions");
        let client_max = max_datagram_len(root.join("client-to-agent.hex").to_str().unwrap());
        let agent_max = max_datagram_len(root.join("agent-to-client.hex").to_str().unwrap());
        assert_eq!(client_max, 508);
        assert_eq!(agent_max, 508);
        assert_eq!(
            u16::try_from(client_max.max(agent_max)).unwrap(),
            DEFAULT_MTU
        );
    }

    #[test]
    fn agent_representation_roundtrip_no_properties() {
        let a = AgentRepresentation {
            timestamp: None,
            version: XRCE_VERSION,
            vendor_id: [0x01, 0x0f],
            properties: None,
        };
        let enc = a.encode().unwrap();
        assert_eq!(enc.len(), 9);
        let back = AgentRepresentation::decode(&enc).unwrap();
        assert_eq!(back, a);
    }

    #[test]
    fn agent_representation_rejects_wrong_cookie() {
        let bad = alloc::vec![0x00u8; 8];
        let res = AgentRepresentation::decode(&bad);
        assert!(matches!(res, Err(XrceError::ValueOutOfRange { .. })));
    }

    /// Byte-oracle check: `agent-to-client.hex` line 1's `STATUS_AGENT`
    /// body is `0000 58524345 0100 010f 00` (11 bytes: a leading
    /// `ResultStatus` this module does not model, then
    /// `AGENT_Representation`). The last 9 bytes decode as
    /// `AGENT_Representation` and match this module's own minimal
    /// encoding exactly.
    #[test]
    fn agent_representation_decodes_the_captured_status_agent_body() {
        let captured: [u8; 9] = [0x58, 0x52, 0x43, 0x45, 0x01, 0x00, 0x01, 0x0f, 0x00];
        let a = AgentRepresentation::decode(&captured).unwrap();
        assert_eq!(a.version, [0x01, 0x00]);
        assert_eq!(a.vendor_id, [0x01, 0x0f]);
        assert_eq!(a.properties, None);
        assert_eq!(a.encode().unwrap(), captured);
    }

    /// DDS-XRCE 1.0 §8.3.4.2: the payload is encoded in the byte order the
    /// E flag names. A big-endian CREATE_CLIENT carries its property count,
    /// string lengths and the MTU big-endian, and reads back. Annex A: the
    /// lengths are `unsigned long`s aligned to 4, counted from the start of
    /// the payload.
    #[test]
    fn a_big_endian_client_representation_with_properties_reads_back() {
        crate::encoding::with_unannotated_types(crate::encoding::UnannotatedTypes::Final, || {
            let c = ClientRepresentation {
                timestamp: None,
                version: XRCE_VERSION,
                vendor_id: [0, 0],
                client_key: key(0x01),
                session_id: 0x81,
                properties: Some(alloc::vec![Property {
                    name: "auth".into(),
                    value: "tok".into(),
                }]),
                requested_mtu: Some(0x01FC),
            };
            let be = c.encode_with(Endianness::Big).unwrap();
            // cookie, version, vendor, key, session id (13 octets), the presence
            // octet at 13, then padding to 16: the count 1 as a big-endian
            // u32, the length 5 of "auth\0" at 20 (aligned), the length 4 of
            // "tok\0" at 32 (29 + padding), and the MTU after the end at 40.
            assert_eq!(be[13], 1);
            assert_eq!(&be[14..16], &[0, 0]);
            assert_eq!(&be[16..20], &[0, 0, 0, 1]);
            assert_eq!(&be[20..24], &[0, 0, 0, 5]);
            assert_eq!(&be[24..29], b"auth\0");
            assert_eq!(&be[29..32], &[0, 0, 0]);
            assert_eq!(&be[32..36], &[0, 0, 0, 4]);
            assert_eq!(&be[36..40], b"tok\0");
            assert_eq!(&be[40..42], &[0x01, 0xFC]);
            assert_eq!(be.len(), 42);
            assert_eq!(
                ClientRepresentation::decode_with(&be, Endianness::Big).unwrap(),
                c
            );
            // Read as little-endian the same bytes are not the representation.
            assert_ne!(ClientRepresentation::decode(&be).ok(), Some(c));
        });
    }

    /// `Property` carries no @extensibility (Annex A): APPENDABLE, a DHEADER
    /// in front of its two strings (DDS-XTypes 1.3 §7.3.1.2.1.8).
    #[test]
    fn a_property_has_a_dheader_by_default() {
        let c = ClientRepresentation {
            timestamp: None,
            version: XRCE_VERSION,
            vendor_id: [0, 0],
            client_key: key(0x01),
            session_id: 0x81,
            properties: Some(alloc::vec![Property {
                name: "auth".into(),
                value: "tok".into(),
            }]),
            requested_mtu: Some(0x01FC),
        };
        let be = c.encode_with(Endianness::Big).unwrap();
        assert_eq!(&be[16..20], &[0, 0, 0, 1], "count");
        // 4 (length) + 5 ("auth\0") + 3 (padding) + 4 (length) + 4 ("tok\0")
        assert_eq!(&be[20..24], &[0, 0, 0, 20], "DHEADER of the Property");
        assert_eq!(&be[24..28], &[0, 0, 0, 5]);
        assert_eq!(&be[44..46], &[0x01, 0xFC]);
        assert_eq!(be.len(), 46);
        assert_eq!(
            ClientRepresentation::decode_with(&be, Endianness::Big).unwrap(),
            c
        );
        // The form without DHEADER is read as well.
        let direct = crate::encoding::with_unannotated_types(
            crate::encoding::UnannotatedTypes::Final,
            || c.encode_with(Endianness::Big).unwrap(),
        );
        assert_eq!(direct.len(), 42);
        assert_eq!(
            ClientRepresentation::decode_with(&direct, Endianness::Big).unwrap(),
            c
        );
    }

    #[test]
    fn a_big_endian_agent_representation_with_properties_reads_back() {
        let a = AgentRepresentation {
            timestamp: None,
            version: XRCE_VERSION,
            vendor_id: [0x01, 0x0f],
            properties: Some(alloc::vec![Property {
                name: "n".into(),
                value: "v".into(),
            }]),
        };
        let be = a.encode_with(Endianness::Big).unwrap();
        // presence octet at 8, count at 12.
        assert_eq!(be[8], 1);
        assert_eq!(&be[12..16], &[0, 0, 0, 1]);
        assert_eq!(
            AgentRepresentation::decode_with(&be, Endianness::Big).unwrap(),
            a
        );
    }

    /// The padding is counted from the start of the payload: with octets in
    /// front of the representation (the `ResultStatus` of a `STATUS_AGENT`,
    /// the `BaseObjectReply` of an `INFO`) the same properties are padded
    /// differently, and read back from where they start.
    #[test]
    fn padding_counts_from_the_start_of_the_payload() {
        let a = AgentRepresentation {
            timestamp: None,
            version: XRCE_VERSION,
            vendor_id: [0x01, 0x0f],
            properties: Some(alloc::vec![Property {
                name: "n".into(),
                value: "v".into(),
            }]),
        };
        let alone = a.encode().unwrap();
        assert_eq!(&alone[12..16], &1u32.to_le_bytes(), "count at 12");
        // Five octets in front: the presence octet is at 13, the count at 16.
        let mut behind = alloc::vec![0xEEu8; 5];
        a.encode_into(&mut behind, Endianness::Little).unwrap();
        assert_eq!(behind[13], 1);
        assert_eq!(&behind[16..20], &1u32.to_le_bytes(), "count at 16");
        let (back, end) = AgentRepresentation::decode_at(&behind, 5, Endianness::Little).unwrap();
        assert_eq!(back, a);
        assert_eq!(end, behind.len());
        // Without properties there is only the presence octet: no padding.
        let none = AgentRepresentation {
            timestamp: None,
            properties: None,
            ..a
        };
        assert_eq!(none.encode().unwrap().len(), 9);
    }

    #[test]
    fn max_properties_cap_enforced_on_decode() {
        // The presence octet, padding to 4, the count.
        let mut bad = alloc::vec![1u8, 0, 0, 0];
        let huge = (MAX_PROPERTIES as u32) + 1;
        bad.extend_from_slice(&huge.to_le_bytes());
        assert!(matches!(
            decode_properties(&bad, 0, Endianness::Little),
            Err(XrceError::ValueOutOfRange { .. })
        ));
    }

    /// DDS-XRCE 1.0 §7.8.2.1: the agent checks the client properties; a
    /// present `PropertySeq` it cannot read is an error of the
    /// representation, not "no properties".
    #[test]
    fn a_malformed_property_seq_is_an_error_not_absent_properties() {
        let mut bytes = alloc::vec![];
        bytes.extend_from_slice(b"XRCE");
        bytes.extend_from_slice(&[0x01, 0x00, 0x00, 0x00, 0xAA, 0xBB, 0xCC, 0xDD, 0x81]);
        // presence octet 1, padding to 16, two elements announced, no
        // element follows.
        bytes.push(1);
        bytes.extend_from_slice(&[0, 0]);
        bytes.extend_from_slice(&2u32.to_le_bytes());
        assert!(ClientRepresentation::decode(&bytes).is_err());
        let mut agent = alloc::vec![];
        agent.extend_from_slice(b"XRCE");
        agent.extend_from_slice(&[0x01, 0x00, 0x00, 0x00]);
        agent.push(1);
        agent.extend_from_slice(&[0, 0, 0]);
        agent.extend_from_slice(&2u32.to_le_bytes());
        assert!(AgentRepresentation::decode(&agent).is_err());
    }

    // ---- client_timestamp / agent_timestamp (§7.8.2.1) ----------------

    fn stamped_client() -> ClientRepresentation {
        ClientRepresentation {
            timestamp: Some(TimePoint {
                seconds: 0x5A88_AA8C,
                nanoseconds: 500_000_000,
            }),
            version: XRCE_VERSION,
            vendor_id: [0x0F, 0x0F],
            client_key: ClientKey([0x22, 0x33, 0x44, 0x55]),
            session_id: 0xDD,
            properties: None,
            requested_mtu: None,
        }
    }

    /// The form of Annex A and Annex B.1 (default): no timestamp member.
    #[test]
    fn by_default_the_representations_have_no_timestamp() {
        assert_eq!(
            representation_timestamps(),
            RepresentationTimestamps::Absent
        );
        let c = stamped_client();
        let bytes = c.encode().unwrap();
        assert_eq!(
            bytes,
            [
                b'X', b'R', b'C', b'E', 1, 0, 0x0F, 0x0F, 0x22, 0x33, 0x44, 0x55, 0xDD, 0
            ]
        );
        let back = ClientRepresentation::decode(&bytes).unwrap();
        assert_eq!(back.timestamp, None);
        assert_eq!(back.client_key, c.client_key);
    }

    /// The prose form: `Time_t client_timestamp` after `xrce_vendor_id`, with
    /// the values of the example of Annex B.1. `Time_t` carries no
    /// `@extensibility`: APPENDABLE by default (a DHEADER of 8), FINAL under
    /// the `Final` setting.
    #[test]
    fn the_prose_form_has_a_time_t_after_the_vendor_id() {
        use crate::encoding::{
            UnannotatedTypes, with_representation_timestamps, with_unannotated_types,
        };
        let c = stamped_client();
        with_representation_timestamps(RepresentationTimestamps::Present, || {
            with_unannotated_types(UnannotatedTypes::Final, || {
                let bytes = c.encode().unwrap();
                assert_eq!(
                    bytes,
                    [
                        b'X', b'R', b'C', b'E', 1, 0, 0x0F, 0x0F, // cookie, version, vendor
                        0x8C, 0xAA, 0x88, 0x5A, 0x00, 0x65, 0xCD, 0x1D, // client_timestamp
                        0x22, 0x33, 0x44, 0x55, 0xDD, 0
                    ]
                );
                assert_eq!(ClientRepresentation::decode(&bytes).unwrap(), c);
            });
            let bytes = c.encode().unwrap();
            assert_eq!(&bytes[8..12], &[8, 0, 0, 0], "DHEADER of the Time_t");
            assert_eq!(
                &bytes[12..20],
                &[0x8C, 0xAA, 0x88, 0x5A, 0x00, 0x65, 0xCD, 0x1D]
            );
            assert_eq!(&bytes[20..25], &[0x22, 0x33, 0x44, 0x55, 0xDD]);
            assert_eq!(ClientRepresentation::decode(&bytes).unwrap(), c);
            // Big-endian, the number in the byte order of the message.
            let be = c.encode_with(Endianness::Big).unwrap();
            assert_eq!(&be[8..12], &[0, 0, 0, 8]);
            assert_eq!(&be[12..16], &[0x5A, 0x88, 0xAA, 0x8C]);
            assert_eq!(
                ClientRepresentation::decode_with(&be, Endianness::Big).unwrap(),
                c
            );
        });
        // The form is not guessed: the form with a timestamp is not the form
        // without one, and read in the other setting it is not the same value.
        let with = with_representation_timestamps(RepresentationTimestamps::Present, || {
            c.encode().unwrap()
        });
        assert_ne!(ClientRepresentation::decode(&with).ok(), Some(c));
    }

    #[test]
    fn the_agent_representation_follows_the_setting_the_same_way() {
        use crate::encoding::{
            UnannotatedTypes, with_representation_timestamps, with_unannotated_types,
        };
        let a = AgentRepresentation {
            timestamp: Some(TimePoint {
                seconds: 1,
                nanoseconds: 2,
            }),
            version: XRCE_VERSION,
            vendor_id: [0, 0],
            properties: None,
        };
        assert_eq!(a.encode().unwrap(), [b'X', b'R', b'C', b'E', 1, 0, 0, 0, 0]);
        with_representation_timestamps(RepresentationTimestamps::Present, || {
            with_unannotated_types(UnannotatedTypes::Final, || {
                let bytes = a.encode().unwrap();
                assert_eq!(
                    bytes,
                    [
                        b'X', b'R', b'C', b'E', 1, 0, 0, 0, 1, 0, 0, 0, 2, 0, 0, 0, 0
                    ]
                );
                assert_eq!(AgentRepresentation::decode(&bytes).unwrap(), a);
            });
            let bytes = a.encode().unwrap();
            assert_eq!(bytes.len(), 8 + 4 + 8 + 1);
            assert_eq!(AgentRepresentation::decode(&bytes).unwrap(), a);
        });
    }
}
