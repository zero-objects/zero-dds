// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! MQTT v5.0 Control Packet Body Codecs — Spec §3.1-§3.15.
//!
//! Per spec subsection an encoder + decoder + tests:
//! * §3.1 CONNECT
//! * §3.2 CONNACK
//! * §3.4 PUBACK, §3.5 PUBREC, §3.6 PUBREL, §3.7 PUBCOMP
//!   (all with the same body layout: PacketId + ReasonCode +
//!   Properties — shared [`AckBody`] helper)
//! * §3.8 SUBSCRIBE, §3.9 SUBACK
//! * §3.10 UNSUBSCRIBE, §3.11 UNSUBACK
//! * §3.14 DISCONNECT, §3.15 AUTH
//!
//! Properties are passed through as opaque VBI-prefixed bytes
//! (the caller layer can parse them more finely via [`crate::properties`]).

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use crate::codec::CodecError;
use crate::data_types::{
    decode_two_byte_int, decode_utf8_string, encode_two_byte_int, encode_utf8_string,
};
use crate::vbi::{decode_vbi, encode_vbi};
use crate::version::ProtocolVersion;

/// Writes a VBI-prefixed property block — but only for MQTT 5.0. MQTT 3.1.1
/// packets carry no properties, so nothing is emitted.
fn put_props(out: &mut Vec<u8>, props: &[u8], v: ProtocolVersion) -> Result<(), CodecError> {
    if v.has_properties() {
        out.extend_from_slice(
            &encode_vbi(u32::try_from(props.len()).unwrap_or(u32::MAX))
                .ok_or(CodecError::Vbi(crate::vbi::VbiError::Malformed))?,
        );
        out.extend_from_slice(props);
    }
    Ok(())
}

/// Reads a VBI-prefixed property block for MQTT 5.0; for 3.1.1 returns an empty
/// block consuming nothing.
fn get_props(bytes: &[u8], v: ProtocolVersion) -> Result<(Vec<u8>, usize), CodecError> {
    if v.has_properties() {
        consume_properties(bytes)
    } else {
        Ok((Vec::new(), 0))
    }
}

// ============================================================================
//  §3.4-§3.7 ACK body (shared for PUBACK/PUBREC/PUBREL/PUBCOMP).
// ============================================================================

/// Spec §3.4-§3.7 — ACK body for PUBACK/PUBREC/PUBREL/PUBCOMP.
///
/// Body layout (all 4 identical):
/// 1. Packet Identifier (2-byte BE)
/// 2. Reason Code (1 byte, optional from remaining length >= 4)
/// 3. Properties (VBI-prefixed, optional from remaining length >= 5)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AckBody {
    /// Spec §3.4.2 / §3.5.2 / §3.6.2 / §3.7.2 — Packet Identifier.
    pub packet_id: u16,
    /// Spec §3.4.2.1 / §3.5.2.1 / §3.6.2.1 / §3.7.2.1 — Reason Code.
    /// `0x00` = Success.
    pub reason_code: u8,
    /// Spec §3.4.2.2 — Properties (raw VBI-prefixed property block).
    pub properties: Vec<u8>,
}

/// Encodes an [`AckBody`] to the wire-format body (without the fixed
/// header).
///
/// # Errors
/// VBI error on property-length encoding.
pub fn encode_ack_body(a: &AckBody) -> Result<Vec<u8>, CodecError> {
    encode_ack_body_v(a, ProtocolVersion::V5)
}

/// Encodes an [`AckBody`] for a version. MQTT 3.1.1 PUBACK/PUBREC/
/// PUBREL/PUBCOMP are exactly the 2-byte packet identifier (no reason code,
/// no properties).
///
/// # Errors
/// VBI error on property-length encoding.
pub fn encode_ack_body_v(a: &AckBody, v: ProtocolVersion) -> Result<Vec<u8>, CodecError> {
    let mut out = Vec::with_capacity(4 + a.properties.len());
    out.extend_from_slice(&encode_two_byte_int(a.packet_id));
    if v.has_properties() {
        // Spec §3.4.2.1: "If the Remaining Length is less than 4 there is
        // no Reason Code"; we always emit the reason code (=Success
        // if 0x00). This is spec-conformant and eases decoder symmetry.
        out.push(a.reason_code);
        let prop_len = encode_vbi(u32::try_from(a.properties.len()).unwrap_or(u32::MAX))
            .ok_or(CodecError::Vbi(crate::vbi::VbiError::Malformed))?;
        out.extend_from_slice(&prop_len);
        out.extend_from_slice(&a.properties);
    }
    Ok(out)
}

/// Decodes an ACK body from the wire-format body.
///
/// # Errors
/// `HeaderTooShort` if fewer than 2 bytes; VBI error on
/// property length.
pub fn decode_ack_body(bytes: &[u8]) -> Result<AckBody, CodecError> {
    if bytes.len() < 2 {
        return Err(CodecError::HeaderTooShort);
    }
    let (packet_id, off) = decode_two_byte_int(bytes)?;
    if bytes.len() == off {
        // Spec §3.4.2.1 — short form: Remaining-Length=2 -> implicit
        // success without Reason-Code/Properties.
        return Ok(AckBody {
            packet_id,
            reason_code: 0,
            properties: Vec::new(),
        });
    }
    let reason_code = bytes[off];
    let mut cursor = off + 1;
    let properties = if cursor < bytes.len() {
        let (prop_len, vbi_consumed) = decode_vbi(&bytes[cursor..])?;
        cursor += vbi_consumed;
        let pl = prop_len as usize;
        if bytes.len() < cursor + pl {
            return Err(CodecError::RemainingLengthMismatch);
        }
        let p = bytes[cursor..cursor + pl].to_vec();
        cursor += pl;
        p
    } else {
        Vec::new()
    };
    let _ = cursor; // bytes after properties are out-of-scope per spec
    Ok(AckBody {
        packet_id,
        reason_code,
        properties,
    })
}

// ============================================================================
//  §3.1 CONNECT Body.
// ============================================================================

/// Spec §3.1 — CONNECT-Body (Variable Header + Payload).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectBody {
    /// Spec §3.1.2.1 — Protocol Name. MUST be "MQTT" for v5.0.
    pub protocol_name: String,
    /// Spec §3.1.2.2 — Protocol Version. MUST be 5 for v5.0.
    pub protocol_version: u8,
    /// Spec §3.1.2.3 — Connect Flags (Username/Password/WillRetain/
    /// WillQoS/WillFlag/CleanStart bits).
    pub connect_flags: u8,
    /// Spec §3.1.2.10 — Keep Alive in seconds.
    pub keep_alive: u16,
    /// Spec §3.1.2.11 — Properties (raw VBI-praefixed).
    pub properties: Vec<u8>,
    /// Spec §3.1.3.1 — Client Identifier.
    pub client_id: String,
    /// Spec §3.1.3.2 — Will Properties (only if Will Flag is set).
    pub will_properties: Vec<u8>,
    /// Spec §3.1.3.3 — Will Topic (only if Will Flag is set).
    pub will_topic: Option<String>,
    /// Spec §3.1.3.4 — Will Payload (only if Will Flag is set).
    pub will_payload: Vec<u8>,
    /// Spec §3.1.3.5 — User Name (only if User Name Flag is set).
    pub user_name: Option<String>,
    /// Spec §3.1.3.6 — Password (only if Password Flag is set).
    pub password: Vec<u8>,
}

/// Spec §3.1.2.3 — Connect-Flags bit positions.
pub mod connect_flags {
    /// Bit 0 — Reserved (SHALL be 0).
    pub const RESERVED: u8 = 0x01;
    /// Bit 1 — Clean Start.
    pub const CLEAN_START: u8 = 0x02;
    /// Bit 2 — Will Flag.
    pub const WILL: u8 = 0x04;
    /// Bit 3-4 — Will QoS (mask).
    pub const WILL_QOS_MASK: u8 = 0x18;
    /// Bit 5 — Will Retain.
    pub const WILL_RETAIN: u8 = 0x20;
    /// Bit 6 — Password Flag.
    pub const PASSWORD: u8 = 0x40;
    /// Bit 7 — User Name Flag.
    pub const USER_NAME: u8 = 0x80;
}

/// Encodes a [`ConnectBody`] to the wire-format body (MQTT 5.0).
///
/// # Errors
/// VBI/data-type error.
pub fn encode_connect_body(c: &ConnectBody) -> Result<Vec<u8>, CodecError> {
    encode_connect_body_v(c, ProtocolVersion::V5)
}

/// Encodes a [`ConnectBody`] for a concrete protocol version. For
/// 3.1.1 the property blocks (CONNECT + Will) are dropped.
///
/// # Errors
/// VBI/data-type error.
pub fn encode_connect_body_v(c: &ConnectBody, v: ProtocolVersion) -> Result<Vec<u8>, CodecError> {
    let mut out = Vec::with_capacity(64 + c.properties.len() + c.client_id.len());
    out.extend_from_slice(&encode_utf8_string(&c.protocol_name)?);
    out.push(c.protocol_version);
    out.push(c.connect_flags);
    out.extend_from_slice(&encode_two_byte_int(c.keep_alive));
    put_props(&mut out, &c.properties, v)?;
    // Payload
    out.extend_from_slice(&encode_utf8_string(&c.client_id)?);
    if c.connect_flags & connect_flags::WILL != 0 {
        put_props(&mut out, &c.will_properties, v)?;
        if let Some(t) = &c.will_topic {
            out.extend_from_slice(&encode_utf8_string(t)?);
        }
        out.extend_from_slice(&encode_two_byte_int(
            u16::try_from(c.will_payload.len()).unwrap_or(u16::MAX),
        ));
        out.extend_from_slice(&c.will_payload);
    }
    if c.connect_flags & connect_flags::USER_NAME != 0 {
        if let Some(u) = &c.user_name {
            out.extend_from_slice(&encode_utf8_string(u)?);
        }
    }
    if c.connect_flags & connect_flags::PASSWORD != 0 {
        out.extend_from_slice(&encode_two_byte_int(
            u16::try_from(c.password.len()).unwrap_or(u16::MAX),
        ));
        out.extend_from_slice(&c.password);
    }
    Ok(out)
}

/// Decodes a CONNECT body (MQTT 5.0).
///
/// # Errors
/// HeaderTooShort/VBI/DataType.
pub fn decode_connect_body(bytes: &[u8]) -> Result<ConnectBody, CodecError> {
    decode_connect_body_v(bytes, ProtocolVersion::V5)
}

/// Decodes a CONNECT body for a concrete version. For 3.1.1 no
/// property blocks are expected. (The version contained in the body is
/// **not** validated — the caller determines it from the protocol level.)
///
/// # Errors
/// HeaderTooShort/VBI/DataType.
pub fn decode_connect_body_v(bytes: &[u8], v: ProtocolVersion) -> Result<ConnectBody, CodecError> {
    let mut cur = 0;
    let (protocol_name, off) = decode_utf8_string(&bytes[cur..])?;
    cur += off;
    if bytes.len() < cur + 4 {
        return Err(CodecError::HeaderTooShort);
    }
    let protocol_version = bytes[cur];
    cur += 1;
    let connect_flags = bytes[cur];
    cur += 1;
    let (keep_alive, off) = decode_two_byte_int(&bytes[cur..])?;
    cur += off;
    let (props, n) = get_props(&bytes[cur..], v)?;
    cur += n;

    let (client_id, off) = decode_utf8_string(&bytes[cur..])?;
    cur += off;

    let mut will_properties = Vec::new();
    let mut will_topic = None;
    let mut will_payload = Vec::new();
    if connect_flags & connect_flags::WILL != 0 {
        let (wp, n) = get_props(&bytes[cur..], v)?;
        will_properties = wp;
        cur += n;
        let (t, off) = decode_utf8_string(&bytes[cur..])?;
        will_topic = Some(t);
        cur += off;
        if bytes.len() < cur + 2 {
            return Err(CodecError::HeaderTooShort);
        }
        let (wpl, off) = decode_two_byte_int(&bytes[cur..])?;
        cur += off;
        let pl = wpl as usize;
        if bytes.len() < cur + pl {
            return Err(CodecError::RemainingLengthMismatch);
        }
        will_payload = bytes[cur..cur + pl].to_vec();
        cur += pl;
    }

    let mut user_name = None;
    if connect_flags & connect_flags::USER_NAME != 0 {
        let (u, off) = decode_utf8_string(&bytes[cur..])?;
        user_name = Some(u);
        cur += off;
    }

    let mut password = Vec::new();
    if connect_flags & connect_flags::PASSWORD != 0 {
        if bytes.len() < cur + 2 {
            return Err(CodecError::HeaderTooShort);
        }
        let (pl, off) = decode_two_byte_int(&bytes[cur..])?;
        cur += off;
        let l = pl as usize;
        if bytes.len() < cur + l {
            return Err(CodecError::RemainingLengthMismatch);
        }
        password = bytes[cur..cur + l].to_vec();
    }

    Ok(ConnectBody {
        protocol_name,
        protocol_version,
        connect_flags,
        keep_alive,
        properties: props,
        client_id,
        will_properties,
        will_topic,
        will_payload,
        user_name,
        password,
    })
}

// ============================================================================
//  §3.2 CONNACK Body.
// ============================================================================

/// Spec §3.2 — CONNACK-Body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnackBody {
    /// Spec §3.2.2.1 — Connect Acknowledge Flags. Bit 0 = Session Present.
    pub session_present: bool,
    /// Spec §3.2.2.2 — Connect Reason Code. `0x00` = Success.
    pub reason_code: u8,
    /// Spec §3.2.2.3 — Properties (raw).
    pub properties: Vec<u8>,
}

/// Encode CONNACK body (MQTT 5.0).
///
/// # Errors
/// VBI errors.
pub fn encode_connack_body(c: &ConnackBody) -> Result<Vec<u8>, CodecError> {
    encode_connack_body_v(c, ProtocolVersion::V5)
}

/// Encode CONNACK body for a version. MQTT 3.1.1 CONNACK is exactly 2 bytes
/// (Acknowledge Flags + Return Code), with no property block (§3.2 v3.1.1).
///
/// # Errors
/// VBI errors.
pub fn encode_connack_body_v(c: &ConnackBody, v: ProtocolVersion) -> Result<Vec<u8>, CodecError> {
    let mut out = Vec::with_capacity(4 + c.properties.len());
    out.push(if c.session_present { 0x01 } else { 0x00 });
    out.push(c.reason_code);
    put_props(&mut out, &c.properties, v)?;
    Ok(out)
}

/// Decode CONNACK body (MQTT 5.0).
///
/// # Errors
/// HeaderTooShort / VBI / RemainingLengthMismatch.
pub fn decode_connack_body(bytes: &[u8]) -> Result<ConnackBody, CodecError> {
    decode_connack_body_v(bytes, ProtocolVersion::V5)
}

/// Decode CONNACK body for a version (3.1.1 has no property block).
///
/// # Errors
/// HeaderTooShort / VBI / RemainingLengthMismatch.
pub fn decode_connack_body_v(bytes: &[u8], v: ProtocolVersion) -> Result<ConnackBody, CodecError> {
    if bytes.len() < 2 {
        return Err(CodecError::HeaderTooShort);
    }
    let session_present = bytes[0] & 0x01 != 0;
    let reason_code = bytes[1];
    let (properties, _) = get_props(&bytes[2..], v)?;
    Ok(ConnackBody {
        session_present,
        reason_code,
        properties,
    })
}

// ============================================================================
//  §3.8-§3.11 SUBSCRIBE / SUBACK / UNSUBSCRIBE / UNSUBACK Bodies.
// ============================================================================

/// Spec §3.8 — subscription with filter + subscription options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subscription {
    /// Spec §3.8.3.1 — Topic Filter.
    pub topic_filter: String,
    /// Spec §3.8.3.1 — subscription-options byte (QoS + NL + RAP +
    /// Retain Handling + reserved bits).
    pub options: u8,
}

/// Spec §3.8 — SUBSCRIBE body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubscribeBody {
    /// Spec §3.8.2 — Packet Identifier.
    pub packet_id: u16,
    /// Spec §3.8.2.1 — Properties.
    pub properties: Vec<u8>,
    /// Spec §3.8.3 — list of subscriptions.
    pub subscriptions: Vec<Subscription>,
}

/// Encode SUBSCRIBE body.
///
/// # Errors
/// VBI / DataType.
pub fn encode_subscribe_body(s: &SubscribeBody) -> Result<Vec<u8>, CodecError> {
    encode_subscribe_body_v(s, ProtocolVersion::V5)
}

/// Encode SUBSCRIBE body for a version (3.1.1 has no property block).
///
/// # Errors
/// VBI / DataType.
pub fn encode_subscribe_body_v(
    s: &SubscribeBody,
    v: ProtocolVersion,
) -> Result<Vec<u8>, CodecError> {
    let mut out = Vec::with_capacity(8 + s.properties.len() + s.subscriptions.len() * 16);
    out.extend_from_slice(&encode_two_byte_int(s.packet_id));
    put_props(&mut out, &s.properties, v)?;
    for sub in &s.subscriptions {
        out.extend_from_slice(&encode_utf8_string(&sub.topic_filter)?);
        out.push(sub.options);
    }
    Ok(out)
}

/// Decode SUBSCRIBE body (MQTT 5.0).
///
/// # Errors
/// HeaderTooShort / VBI / DataType.
pub fn decode_subscribe_body(bytes: &[u8]) -> Result<SubscribeBody, CodecError> {
    decode_subscribe_body_v(bytes, ProtocolVersion::V5)
}

/// Decode SUBSCRIBE body for a version (3.1.1 has no property block).
///
/// # Errors
/// HeaderTooShort / VBI / DataType.
pub fn decode_subscribe_body_v(
    bytes: &[u8],
    v: ProtocolVersion,
) -> Result<SubscribeBody, CodecError> {
    if bytes.len() < 2 {
        return Err(CodecError::HeaderTooShort);
    }
    let (packet_id, off) = decode_two_byte_int(bytes)?;
    let mut cur = off;
    let (properties, n) = get_props(&bytes[cur..], v)?;
    cur += n;
    let mut subscriptions = Vec::new();
    while cur < bytes.len() {
        let (filter, off) = decode_utf8_string(&bytes[cur..])?;
        cur += off;
        if bytes.len() <= cur {
            return Err(CodecError::HeaderTooShort);
        }
        let options = bytes[cur];
        cur += 1;
        subscriptions.push(Subscription {
            topic_filter: filter,
            options,
        });
    }
    Ok(SubscribeBody {
        packet_id,
        properties,
        subscriptions,
    })
}

/// Spec §3.9 — SUBACK-Body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubackBody {
    /// Spec §3.9.2 — Packet Identifier.
    pub packet_id: u16,
    /// Spec §3.9.2.1 — Properties.
    pub properties: Vec<u8>,
    /// Spec §3.9.3 — Reason-Codes pro Subscription (1 Byte je
    /// filters, in the same order as SUBSCRIBE).
    pub reason_codes: Vec<u8>,
}

/// Encode SUBACK body.
///
/// # Errors
/// VBI.
pub fn encode_suback_body(s: &SubackBody) -> Result<Vec<u8>, CodecError> {
    encode_suback_body_v(s, ProtocolVersion::V5)
}

/// Encode SUBACK body for a version. MQTT 3.1.1 has no property block; the
/// per-filter return codes (granted QoS 0/1/2 or 0x80 = failure) follow the
/// Packet Identifier directly.
///
/// # Errors
/// VBI.
pub fn encode_suback_body_v(s: &SubackBody, v: ProtocolVersion) -> Result<Vec<u8>, CodecError> {
    let mut out = Vec::with_capacity(4 + s.properties.len() + s.reason_codes.len());
    out.extend_from_slice(&encode_two_byte_int(s.packet_id));
    put_props(&mut out, &s.properties, v)?;
    out.extend_from_slice(&s.reason_codes);
    Ok(out)
}

/// Decode SUBACK body (MQTT 5.0).
///
/// # Errors
/// HeaderTooShort / VBI.
pub fn decode_suback_body(bytes: &[u8]) -> Result<SubackBody, CodecError> {
    decode_suback_body_v(bytes, ProtocolVersion::V5)
}

/// Decode SUBACK body for a version (3.1.1 has no property block).
///
/// # Errors
/// HeaderTooShort / VBI.
pub fn decode_suback_body_v(bytes: &[u8], v: ProtocolVersion) -> Result<SubackBody, CodecError> {
    if bytes.len() < 2 {
        return Err(CodecError::HeaderTooShort);
    }
    let (packet_id, off) = decode_two_byte_int(bytes)?;
    let (properties, n) = get_props(&bytes[off..], v)?;
    let cur = off + n;
    let reason_codes = bytes[cur..].to_vec();
    Ok(SubackBody {
        packet_id,
        properties,
        reason_codes,
    })
}

/// Spec §3.10 — UNSUBSCRIBE-Body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsubscribeBody {
    /// Spec §3.10.2 — Packet Identifier.
    pub packet_id: u16,
    /// Spec §3.10.2.1 — Properties.
    pub properties: Vec<u8>,
    /// Spec §3.10.3 — list of topic filters.
    pub topic_filters: Vec<String>,
}

/// Encode UNSUBSCRIBE body.
///
/// # Errors
/// VBI / DataType.
pub fn encode_unsubscribe_body(u: &UnsubscribeBody) -> Result<Vec<u8>, CodecError> {
    encode_unsubscribe_body_v(u, ProtocolVersion::V5)
}

/// Encode UNSUBSCRIBE body for a version (3.1.1 has no property block).
///
/// # Errors
/// VBI / DataType.
pub fn encode_unsubscribe_body_v(
    u: &UnsubscribeBody,
    v: ProtocolVersion,
) -> Result<Vec<u8>, CodecError> {
    let mut out = Vec::with_capacity(4 + u.properties.len() + u.topic_filters.len() * 16);
    out.extend_from_slice(&encode_two_byte_int(u.packet_id));
    put_props(&mut out, &u.properties, v)?;
    for f in &u.topic_filters {
        out.extend_from_slice(&encode_utf8_string(f)?);
    }
    Ok(out)
}

/// Decode UNSUBSCRIBE body (MQTT 5.0).
///
/// # Errors
/// HeaderTooShort / VBI / DataType.
pub fn decode_unsubscribe_body(bytes: &[u8]) -> Result<UnsubscribeBody, CodecError> {
    decode_unsubscribe_body_v(bytes, ProtocolVersion::V5)
}

/// Decode UNSUBSCRIBE body for a version (3.1.1 has no property block).
///
/// # Errors
/// HeaderTooShort / VBI / DataType.
pub fn decode_unsubscribe_body_v(
    bytes: &[u8],
    v: ProtocolVersion,
) -> Result<UnsubscribeBody, CodecError> {
    if bytes.len() < 2 {
        return Err(CodecError::HeaderTooShort);
    }
    let (packet_id, off) = decode_two_byte_int(bytes)?;
    let mut cur = off;
    let (properties, n) = get_props(&bytes[cur..], v)?;
    cur += n;
    let mut topic_filters = Vec::new();
    while cur < bytes.len() {
        let (f, off) = decode_utf8_string(&bytes[cur..])?;
        cur += off;
        topic_filters.push(f);
    }
    Ok(UnsubscribeBody {
        packet_id,
        properties,
        topic_filters,
    })
}

/// Spec §3.11 — UNSUBACK-Body. Layout identisch zu SUBACK.
pub type UnsubackBody = SubackBody;

/// Encode UNSUBACK body (MQTT 5.0 — identisch zu SUBACK: Properties +
/// Reason-Codes).
///
/// # Errors
/// VBI.
pub fn encode_unsuback_body(u: &UnsubackBody) -> Result<Vec<u8>, CodecError> {
    encode_unsuback_body_v(u, ProtocolVersion::V5)
}

/// Encode UNSUBACK body for a version. **Important version difference:** MQTT
/// 3.1.1 UNSUBACK (§3.11 v3.1.1) carries **only** the Packet Identifier — no
/// property block and no per-filter reason codes (those were added in 5.0).
///
/// # Errors
/// VBI.
pub fn encode_unsuback_body_v(u: &UnsubackBody, v: ProtocolVersion) -> Result<Vec<u8>, CodecError> {
    match v {
        ProtocolVersion::V5 => encode_suback_body_v(u, v),
        ProtocolVersion::V311 => Ok(encode_two_byte_int(u.packet_id).to_vec()),
    }
}

/// Decode UNSUBACK body (MQTT 5.0 — identisch zu SUBACK).
///
/// # Errors
/// HeaderTooShort / VBI.
pub fn decode_unsuback_body(bytes: &[u8]) -> Result<UnsubackBody, CodecError> {
    decode_unsuback_body_v(bytes, ProtocolVersion::V5)
}

/// Decode UNSUBACK body for a version (3.1.1 = Packet Identifier only).
///
/// # Errors
/// HeaderTooShort / VBI.
pub fn decode_unsuback_body_v(
    bytes: &[u8],
    v: ProtocolVersion,
) -> Result<UnsubackBody, CodecError> {
    match v {
        ProtocolVersion::V5 => decode_suback_body_v(bytes, v),
        ProtocolVersion::V311 => {
            if bytes.len() < 2 {
                return Err(CodecError::HeaderTooShort);
            }
            let (packet_id, _) = decode_two_byte_int(bytes)?;
            Ok(UnsubackBody {
                packet_id,
                properties: Vec::new(),
                reason_codes: Vec::new(),
            })
        }
    }
}

// ============================================================================
//  §3.14 DISCONNECT + §3.15 AUTH Bodies (Reason-Code + Properties).
// ============================================================================

/// Spec §3.14 — DISCONNECT-Body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisconnectBody {
    /// Spec §3.14.2.1 — Reason-Code. `0x00` = Normal Disconnection.
    pub reason_code: u8,
    /// Spec §3.14.2.2 — Properties.
    pub properties: Vec<u8>,
}

/// Encode DISCONNECT body.
///
/// # Errors
/// VBI.
pub fn encode_disconnect_body(d: &DisconnectBody) -> Result<Vec<u8>, CodecError> {
    encode_disconnect_body_v(d, ProtocolVersion::V5)
}

/// Encode DISCONNECT body for a version. MQTT 3.1.1 DISCONNECT (§3.14 v3.1.1)
/// has an **empty** variable header/payload — no reason code, no properties
/// (the fixed header `0xE0 0x00` is the whole packet).
///
/// # Errors
/// VBI.
pub fn encode_disconnect_body_v(
    d: &DisconnectBody,
    v: ProtocolVersion,
) -> Result<Vec<u8>, CodecError> {
    if !v.has_properties() {
        return Ok(Vec::new());
    }
    let mut out = Vec::with_capacity(2 + d.properties.len());
    out.push(d.reason_code);
    out.extend_from_slice(
        &encode_vbi(u32::try_from(d.properties.len()).unwrap_or(u32::MAX))
            .ok_or(CodecError::Vbi(crate::vbi::VbiError::Malformed))?,
    );
    out.extend_from_slice(&d.properties);
    Ok(out)
}

/// Decode DISCONNECT body.
///
/// # Errors
/// HeaderTooShort / VBI.
pub fn decode_disconnect_body(bytes: &[u8]) -> Result<DisconnectBody, CodecError> {
    if bytes.is_empty() {
        // Spec §3.14.2.1 — short form: missing reason code -> implicit
        // 0x00 Normal Disconnection.
        return Ok(DisconnectBody {
            reason_code: 0,
            properties: Vec::new(),
        });
    }
    let reason_code = bytes[0];
    let (properties, _) = consume_properties(&bytes[1..])?;
    Ok(DisconnectBody {
        reason_code,
        properties,
    })
}

/// Spec §3.15 — AUTH body. Same layout as DISCONNECT.
pub type AuthBody = DisconnectBody;

/// Encode AUTH body (identisch zu encode_disconnect_body).
///
/// # Errors
/// VBI.
pub fn encode_auth_body(a: &AuthBody) -> Result<Vec<u8>, CodecError> {
    encode_disconnect_body(a)
}

/// Decode AUTH body (identisch zu decode_disconnect_body).
///
/// # Errors
/// HeaderTooShort / VBI.
pub fn decode_auth_body(bytes: &[u8]) -> Result<AuthBody, CodecError> {
    decode_disconnect_body(bytes)
}

// ============================================================================
//  §2.2.2 property-value decoding — per-identifier value-schema map.
// ============================================================================

/// Spec §2.2.2 Tab 2-4 — property data type per identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropertyDataType {
    /// One byte.
    Byte,
    /// Two byte big-endian integer.
    TwoByteInt,
    /// Four byte big-endian integer.
    FourByteInt,
    /// Variable Byte Integer.
    VariableByteInt,
    /// UTF-8-prefixed string.
    Utf8String,
    /// VBI-prefixed binary blob.
    BinaryData,
    /// UTF-8 String Pair.
    Utf8StringPair,
}

/// Spec §2.2.2.2 + Tab 2-4 — property value data type per identifier ID.
///
/// # Returns
/// `Some` if the ID is a known spec property identifier.
#[must_use]
pub fn property_data_type(id: u8) -> Option<PropertyDataType> {
    Some(match id {
        // Spec Tab 2-4 (S. 23-26):
        0x01 => PropertyDataType::Byte, // Payload Format Indicator
        0x02 => PropertyDataType::FourByteInt, // Message Expiry Interval
        0x03 => PropertyDataType::Utf8String, // Content Type
        0x08 => PropertyDataType::Utf8String, // Response Topic
        0x09 => PropertyDataType::BinaryData, // Correlation Data
        0x0B => PropertyDataType::VariableByteInt, // Subscription Identifier
        0x11 => PropertyDataType::FourByteInt, // Session Expiry Interval
        0x12 => PropertyDataType::Utf8String, // Assigned Client Identifier
        0x13 => PropertyDataType::TwoByteInt, // Server Keep Alive
        0x15 => PropertyDataType::Utf8String, // Authentication Method
        0x16 => PropertyDataType::BinaryData, // Authentication Data
        0x17 => PropertyDataType::Byte, // Request Problem Information
        0x18 => PropertyDataType::FourByteInt, // Will Delay Interval
        0x19 => PropertyDataType::Byte, // Request Response Information
        0x1A => PropertyDataType::Utf8String, // Response Information
        0x1C => PropertyDataType::Utf8String, // Server Reference
        0x1F => PropertyDataType::Utf8String, // Reason String
        0x21 => PropertyDataType::TwoByteInt, // Receive Maximum
        0x22 => PropertyDataType::TwoByteInt, // Topic Alias Maximum
        0x23 => PropertyDataType::TwoByteInt, // Topic Alias
        0x24 => PropertyDataType::Byte, // Maximum QoS
        0x25 => PropertyDataType::Byte, // Retain Available
        0x26 => PropertyDataType::Utf8StringPair, // User Property
        0x27 => PropertyDataType::FourByteInt, // Maximum Packet Size
        0x28 => PropertyDataType::Byte, // Wildcard Subscription Available
        0x29 => PropertyDataType::Byte, // Subscription Identifier Available
        0x2A => PropertyDataType::Byte, // Shared Subscription Available
        _ => return None,
    })
}

impl fmt::Display for PropertyDataType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Byte => "Byte",
            Self::TwoByteInt => "TwoByteInt",
            Self::FourByteInt => "FourByteInt",
            Self::VariableByteInt => "VariableByteInt",
            Self::Utf8String => "Utf8String",
            Self::BinaryData => "BinaryData",
            Self::Utf8StringPair => "Utf8StringPair",
        })
    }
}

// ============================================================================
//  Helper.
// ============================================================================

/// Reads a VBI-prefixed property block from `bytes` and returns
/// (raw_bytes, total_consumed_incl_vbi).
fn consume_properties(bytes: &[u8]) -> Result<(Vec<u8>, usize), CodecError> {
    if bytes.is_empty() {
        return Ok((Vec::new(), 0));
    }
    let (prop_len, vbi_consumed) = decode_vbi(bytes)?;
    let pl = prop_len as usize;
    let cur = vbi_consumed;
    if bytes.len() < cur + pl {
        return Err(CodecError::RemainingLengthMismatch);
    }
    Ok((bytes[cur..cur + pl].to_vec(), cur + pl))
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    fn round<T, E, D>(value: T, encode: E, decode: D)
    where
        T: PartialEq + core::fmt::Debug + Clone,
        E: Fn(&T) -> Result<Vec<u8>, CodecError>,
        D: Fn(&[u8]) -> Result<T, CodecError>,
    {
        let bytes = encode(&value).expect("encode");
        let parsed = decode(&bytes).expect("decode");
        assert_eq!(parsed, value);
    }

    #[test]
    fn ack_body_round_trip_with_properties() {
        let ack = AckBody {
            packet_id: 0x1234,
            reason_code: 0x10, // No matching subscribers
            properties: alloc::vec![0x1F, 0x00, 0x05, b'h', b'e', b'l', b'l', b'o'],
        };
        round(ack, encode_ack_body, decode_ack_body);
    }

    #[test]
    fn ack_body_short_form_no_reason_code_no_properties() {
        // Spec §3.4.2.1 short form: 2-byte body = packet_id only.
        let bytes = alloc::vec![0xAB, 0xCD];
        let parsed = decode_ack_body(&bytes).expect("decode");
        assert_eq!(parsed.packet_id, 0xABCD);
        assert_eq!(parsed.reason_code, 0);
        assert!(parsed.properties.is_empty());
    }

    #[test]
    fn connect_body_round_trip_minimal() {
        let c = ConnectBody {
            protocol_name: "MQTT".to_string(),
            protocol_version: 5,
            connect_flags: 0x02, // Clean Start
            keep_alive: 60,
            properties: Vec::new(),
            client_id: "test-client".to_string(),
            will_properties: Vec::new(),
            will_topic: None,
            will_payload: Vec::new(),
            user_name: None,
            password: Vec::new(),
        };
        round(c, encode_connect_body, decode_connect_body);
    }

    #[test]
    fn connect_body_round_trip_with_will_and_credentials() {
        let c = ConnectBody {
            protocol_name: "MQTT".to_string(),
            protocol_version: 5,
            connect_flags: connect_flags::CLEAN_START
                | connect_flags::WILL
                | connect_flags::USER_NAME
                | connect_flags::PASSWORD,
            keep_alive: 30,
            properties: Vec::new(),
            client_id: "edge-1".to_string(),
            will_properties: Vec::new(),
            will_topic: Some("status".to_string()),
            will_payload: alloc::vec![0xDE, 0xAD],
            user_name: Some("alice".to_string()),
            password: alloc::vec![1, 2, 3, 4],
        };
        round(c, encode_connect_body, decode_connect_body);
    }

    #[test]
    fn connack_body_round_trip() {
        let c = ConnackBody {
            session_present: true,
            reason_code: 0x00,
            properties: alloc::vec![0x21, 0x00, 0x10], // Receive Maximum = 16
        };
        round(c, encode_connack_body, decode_connack_body);
    }

    #[test]
    fn subscribe_body_round_trip_with_two_filters() {
        let s = SubscribeBody {
            packet_id: 1,
            properties: Vec::new(),
            subscriptions: alloc::vec![
                Subscription {
                    topic_filter: "sensors/+".to_string(),
                    options: 0x01, // QoS 1
                },
                Subscription {
                    topic_filter: "alerts/#".to_string(),
                    options: 0x02, // QoS 2
                }
            ],
        };
        round(s, encode_subscribe_body, decode_subscribe_body);
    }

    #[test]
    fn suback_body_round_trip_with_reason_codes() {
        let s = SubackBody {
            packet_id: 1,
            properties: Vec::new(),
            reason_codes: alloc::vec![0x00, 0x01, 0x80], // Granted-0, Granted-1, Failure
        };
        round(s, encode_suback_body, decode_suback_body);
    }

    #[test]
    fn unsubscribe_body_round_trip() {
        let u = UnsubscribeBody {
            packet_id: 5,
            properties: Vec::new(),
            topic_filters: alloc::vec!["a/b".to_string(), "c/d".to_string()],
        };
        round(u, encode_unsubscribe_body, decode_unsubscribe_body);
    }

    #[test]
    fn unsuback_body_round_trip_via_alias() {
        let u = UnsubackBody {
            packet_id: 5,
            properties: Vec::new(),
            reason_codes: alloc::vec![0x00, 0x11], // Success, No subscription existed
        };
        round(u, encode_unsuback_body, decode_unsuback_body);
    }

    #[test]
    fn disconnect_body_round_trip_with_reason_string_property() {
        let d = DisconnectBody {
            reason_code: 0x82, // Protocol Error
            properties: alloc::vec![0x1F, 0x00, 0x03, b'b', b'a', b'd'],
        };
        round(d, encode_disconnect_body, decode_disconnect_body);
    }

    #[test]
    fn disconnect_body_short_form_implicit_normal_disconnection() {
        // Spec §3.14.2.1 — 0-byte body = implicit Normal Disconnection.
        let parsed = decode_disconnect_body(&[]).expect("decode");
        assert_eq!(parsed.reason_code, 0);
        assert!(parsed.properties.is_empty());
    }

    #[test]
    fn auth_body_round_trip_via_alias() {
        let a = AuthBody {
            reason_code: 0x18, // Continue authentication
            properties: alloc::vec![0x15, 0x00, 0x06, b'S', b'C', b'R', b'A', b'M', b'!'],
        };
        round(a, encode_auth_body, decode_auth_body);
    }

    #[test]
    fn property_data_type_table_covers_all_spec_identifiers() {
        // Spec §2.2.2 Tab 2-4 — all 27 property identifiers must
        // return a data type.
        for id in [
            0x01_u8, 0x02, 0x03, 0x08, 0x09, 0x0B, 0x11, 0x12, 0x13, 0x15, 0x16, 0x17, 0x18, 0x19,
            0x1A, 0x1C, 0x1F, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28, 0x29, 0x2A,
        ] {
            assert!(
                property_data_type(id).is_some(),
                "missing data type for property id 0x{id:02X}"
            );
        }
    }

    #[test]
    fn property_data_type_returns_none_for_unknown_id() {
        assert!(property_data_type(0x00).is_none());
        assert!(property_data_type(0xFF).is_none());
    }

    #[test]
    fn property_data_type_user_property_is_utf8_pair() {
        assert_eq!(
            property_data_type(0x26),
            Some(PropertyDataType::Utf8StringPair)
        );
    }

    // ---- MQTT 3.1.1 version-aware codec --------------------------------

    #[test]
    fn v311_connect_omits_property_blocks() {
        // A 3.1.1 CONNECT must not carry the CONNECT or Will property VBI.
        let c = ConnectBody {
            protocol_name: "MQTT".to_string(),
            protocol_version: 4,
            connect_flags: connect_flags::CLEAN_START | connect_flags::WILL,
            keep_alive: 60,
            properties: Vec::new(),
            client_id: "c".to_string(),
            will_properties: Vec::new(),
            will_topic: Some("lwt".to_string()),
            will_payload: alloc::vec![1, 2],
            user_name: None,
            password: Vec::new(),
        };
        let v5 = encode_connect_body_v(&c, ProtocolVersion::V5).expect("v5");
        let v311 = encode_connect_body_v(&c, ProtocolVersion::V311).expect("v311");
        // v5 carries two extra property-length VBI bytes (CONNECT + Will = 0x00).
        assert_eq!(
            v5.len(),
            v311.len() + 2,
            "v311 drops 2 property-length bytes"
        );
        // Round-trips under its own version.
        let back = decode_connect_body_v(&v311, ProtocolVersion::V311).expect("decode v311");
        assert_eq!(back.client_id, "c");
        assert_eq!(back.will_topic.as_deref(), Some("lwt"));
        assert!(back.properties.is_empty());
    }

    #[test]
    fn v311_connack_is_exactly_two_bytes() {
        let c = ConnackBody {
            session_present: true,
            reason_code: 0x00,
            properties: Vec::new(),
        };
        let v311 = encode_connack_body_v(&c, ProtocolVersion::V311).expect("encode");
        assert_eq!(v311, alloc::vec![0x01, 0x00], "3.1.1 CONNACK = 2 bytes");
        let back = decode_connack_body_v(&v311, ProtocolVersion::V311).expect("decode");
        assert_eq!(back, c);
    }

    #[test]
    fn v311_ack_is_packet_id_only() {
        let a = AckBody {
            packet_id: 0x1234,
            reason_code: 0x10, // ignored in 3.1.1
            properties: alloc::vec![0xAA],
        };
        let v311 = encode_ack_body_v(&a, ProtocolVersion::V311).expect("encode");
        assert_eq!(
            v311,
            alloc::vec![0x12, 0x34],
            "3.1.1 PUBACK = packet id only"
        );
        // The shared decoder reads it back as the short form.
        let back = decode_ack_body(&v311).expect("decode");
        assert_eq!(back.packet_id, 0x1234);
        assert_eq!(back.reason_code, 0);
    }

    #[test]
    fn v311_suback_has_no_property_block() {
        let s = SubackBody {
            packet_id: 1,
            properties: Vec::new(),
            reason_codes: alloc::vec![0x00, 0x01, 0x80], // granted QoS / failure
        };
        let v311 = encode_suback_body_v(&s, ProtocolVersion::V311).expect("encode");
        // packet_id(2) + 3 return codes, NO property VBI.
        assert_eq!(v311.len(), 5);
        let back = decode_suback_body_v(&v311, ProtocolVersion::V311).expect("decode");
        assert_eq!(back, s);
    }

    #[test]
    fn v311_unsuback_is_packet_id_only_no_reason_codes() {
        let u = UnsubackBody {
            packet_id: 7,
            properties: Vec::new(),
            reason_codes: alloc::vec![0x00, 0x11], // present in 5.0, absent in 3.1.1
        };
        let v311 = encode_unsuback_body_v(&u, ProtocolVersion::V311).expect("encode");
        assert_eq!(
            v311,
            alloc::vec![0x00, 0x07],
            "3.1.1 UNSUBACK = packet id only"
        );
        let back = decode_unsuback_body_v(&v311, ProtocolVersion::V311).expect("decode");
        assert_eq!(back.packet_id, 7);
        assert!(back.reason_codes.is_empty());
    }

    #[test]
    fn v311_disconnect_body_is_empty() {
        let d = DisconnectBody {
            reason_code: 0x82,
            properties: alloc::vec![0x1F, 0x00, 0x03, b'b', b'a', b'd'],
        };
        let v311 = encode_disconnect_body_v(&d, ProtocolVersion::V311).expect("encode");
        assert!(v311.is_empty(), "3.1.1 DISCONNECT has an empty body");
    }
}
