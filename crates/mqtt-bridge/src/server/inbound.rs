// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The packets a server receives, read strictly: every check of chapter 3
//! that a server makes on what a client sends, each failure with the
//! Reason Code §4.13 asks for — 0x81 Malformed Packet for data that
//! cannot be parsed, 0x82 Protocol Error for data that does not conform.
//! The decoders of [`crate::control_packets`] read leniently and leave
//! these checks to the caller; this is that caller.

use std::string::String;
use std::vec::Vec;

use crate::broker::{MessageProperties, QoS};
use crate::data_types::{decode_binary_data, decode_two_byte_int, decode_utf8_string};
use crate::packet::ControlPacketType;
use crate::property_set::{MALFORMED_PACKET, PROTOCOL_ERROR, PropertyContext, PropertySet};
use crate::topic_filter::validate_topic_name;
use crate::vbi::decode_vbi;
use crate::version::ProtocolVersion;

/// 0x84 Unsupported Protocol Version.
pub const UNSUPPORTED_PROTOCOL_VERSION: u8 = 0x84;
/// 0x85 Client Identifier not valid.
pub const CLIENT_IDENTIFIER_NOT_VALID: u8 = 0x85;
/// 0x90 Topic Name invalid.
pub const TOPIC_NAME_INVALID: u8 = 0x90;

/// The DISCONNECT Reason Codes of §3.14.2.1.
const DISCONNECT_CODES: &[u8] = &[
    0x00, 0x04, 0x80, 0x81, 0x82, 0x83, 0x87, 0x89, 0x8B, 0x8D, 0x8E, 0x8F, 0x90, 0x93, 0x94, 0x95,
    0x96, 0x97, 0x98, 0x99, 0x9A, 0x9B, 0x9C, 0x9D, 0x9E, 0x9F, 0xA0, 0xA1, 0xA2,
];
/// The PUBACK and PUBREC Reason Codes of §3.4.2.1 and §3.5.2.1.
const PUBACK_CODES: &[u8] = &[0x00, 0x10, 0x80, 0x83, 0x87, 0x90, 0x91, 0x97, 0x99];
/// The PUBREL and PUBCOMP Reason Codes of §3.6.2.1 and §3.7.2.1.
const PUBREL_CODES: &[u8] = &[0x00, 0x92];
/// The AUTH Reason Codes of §3.15.2.1.
const AUTH_CODES: &[u8] = &[0x00, 0x18, 0x19];

/// A cursor over a packet body; running past its end is a Malformed
/// Packet.
struct Cursor<'a> {
    b: &'a [u8],
    at: usize,
}

impl<'a> Cursor<'a> {
    fn new(b: &'a [u8]) -> Self {
        Self { b, at: 0 }
    }
    fn byte(&mut self) -> Result<u8, u8> {
        let v = *self.b.get(self.at).ok_or(MALFORMED_PACKET)?;
        self.at += 1;
        Ok(v)
    }
    fn two(&mut self) -> Result<u16, u8> {
        let (v, n) = decode_two_byte_int(&self.b[self.at..]).map_err(|_| MALFORMED_PACKET)?;
        self.at += n;
        Ok(v)
    }
    /// A UTF-8 Encoded String: ill-formed UTF-8 or U+0000 is a Malformed
    /// Packet ([MQTT-1.5.4-1], [MQTT-1.5.4-2]).
    fn string(&mut self) -> Result<String, u8> {
        let (v, n) = decode_utf8_string(&self.b[self.at..]).map_err(|_| MALFORMED_PACKET)?;
        self.at += n;
        Ok(v)
    }
    fn binary(&mut self) -> Result<Vec<u8>, u8> {
        let (v, n) = decode_binary_data(&self.b[self.at..]).map_err(|_| MALFORMED_PACKET)?;
        self.at += n;
        Ok(v)
    }
    /// A property block with its Variable Byte Integer length (§2.2.2.1),
    /// checked by [`PropertySet::parse`].
    fn properties(&mut self, ctx: PropertyContext) -> Result<PropertySet, u8> {
        let (len, n) = decode_vbi(&self.b[self.at..]).map_err(|_| MALFORMED_PACKET)?;
        self.at += n;
        let end = self
            .at
            .checked_add(len as usize)
            .filter(|e| *e <= self.b.len())
            .ok_or(MALFORMED_PACKET)?;
        let set = PropertySet::parse(&self.b[self.at..end], ctx)?;
        self.at = end;
        Ok(set)
    }
    fn rest(&mut self) -> &'a [u8] {
        let r = &self.b[self.at..];
        self.at = self.b.len();
        r
    }
    fn done(&self) -> bool {
        self.at == self.b.len()
    }
}

/// The Will of a CONNECT (§3.1.2.5, §3.1.3.2 to §3.1.3.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WillIn {
    /// Will Topic.
    pub topic: String,
    /// Will Payload.
    pub payload: Vec<u8>,
    /// Will QoS.
    pub qos: QoS,
    /// Will Retain.
    pub retain: bool,
    /// Will Properties.
    pub properties: PropertySet,
}

/// A CONNECT that passed the checks of §3.1.4 step 1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Connect {
    /// The Protocol Version.
    pub version: ProtocolVersion,
    /// Clean Start (Clean Session in 3.1.1).
    pub clean_start: bool,
    /// Keep Alive, seconds.
    pub keep_alive: u16,
    /// The CONNECT Properties.
    pub properties: PropertySet,
    /// The Client Identifier, possibly empty.
    pub client_id: String,
    /// The Will.
    pub will: Option<WillIn>,
    /// User Name.
    pub user_name: Option<String>,
    /// Password.
    pub password: Option<Vec<u8>>,
}

/// How a refused CONNECT is answered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectRefusal {
    /// Close the Network Connection without a CONNACK.
    Close,
    /// A CONNACK with this code in the format of this version, then close.
    Connack(ProtocolVersion, u8),
}

/// The answer to a refused CONNECT of `version` with the 5.0 Reason Code
/// `reason`: as it is for 5.0; for 3.1.1 the Return Code of its Table 3.1
/// that says the same, or no CONNACK where 3.1.1 has none (a Malformed
/// Packet or Protocol Error closes the connection, §4.8 of 3.1.1).
#[must_use]
pub fn refusal(version: ProtocolVersion, reason: u8) -> ConnectRefusal {
    match version {
        ProtocolVersion::V5 => ConnectRefusal::Connack(version, reason),
        ProtocolVersion::V311 => match reason {
            0x84 => ConnectRefusal::Connack(version, 0x01),
            0x85 => ConnectRefusal::Connack(version, 0x02),
            0x88 | 0x89 | 0x9C | 0x9D => ConnectRefusal::Connack(version, 0x03),
            0x86 => ConnectRefusal::Connack(version, 0x04),
            0x87 | 0x8C => ConnectRefusal::Connack(version, 0x05),
            _ => ConnectRefusal::Close,
        },
    }
}

/// Reads a CONNECT (§3.1) with the checks of §3.1.4: Protocol Name and
/// Level, the reserved flag ([MQTT-3.1.2-3]), the Will flags
/// ([MQTT-3.1.2-11], [MQTT-3.1.2-13]), the User Name and Password flags
/// ([MQTT-3.1.2-16] to [MQTT-3.1.2-19]), the property blocks, the field
/// order and that nothing follows the last field ([MQTT-3.1.4-1]).
///
/// # Errors
/// How the CONNECT is refused.
pub fn parse_connect(flags: u8, body: &[u8]) -> Result<Connect, ConnectRefusal> {
    let mut c = Cursor::new(body);
    let name = c.string().map_err(|_| ConnectRefusal::Close)?;
    let level = c.byte().map_err(|_| ConnectRefusal::Close)?;
    // §3.1.2.1: another Protocol Name is not this protocol; MQTT 3.1
    // (`MQIsdp`, level 3) is answered as unsupported.
    if name == "MQIsdp" && level == 3 {
        return Err(ConnectRefusal::Connack(ProtocolVersion::V311, 0x01));
    }
    if name != "MQTT" {
        return Err(ConnectRefusal::Close);
    }
    let version = match level {
        4 => ProtocolVersion::V311,
        5 => ProtocolVersion::V5,
        // [MQTT-3.1.2-2]: a CONNACK in the format the client can read.
        l if l > 5 => {
            return Err(ConnectRefusal::Connack(
                ProtocolVersion::V5,
                UNSUPPORTED_PROTOCOL_VERSION,
            ));
        }
        _ => return Err(ConnectRefusal::Connack(ProtocolVersion::V311, 0x01)),
    };
    parse_connect_rest(version, flags, &mut c).map_err(|code| refusal(version, code))
}

fn parse_connect_rest(
    version: ProtocolVersion,
    flags: u8,
    c: &mut Cursor<'_>,
) -> Result<Connect, u8> {
    use crate::control_packets::connect_flags as f;
    if flags != 0 {
        return Err(MALFORMED_PACKET);
    }
    let cf = c.byte()?;
    if cf & f::RESERVED != 0 {
        return Err(MALFORMED_PACKET);
    }
    let will_flag = cf & f::WILL != 0;
    let will_qos = (cf & f::WILL_QOS_MASK) >> 3;
    let will_retain = cf & f::WILL_RETAIN != 0;
    if will_qos == 3 || (!will_flag && (will_qos != 0 || will_retain)) {
        return Err(MALFORMED_PACKET);
    }
    let user_flag = cf & f::USER_NAME != 0;
    let password_flag = cf & f::PASSWORD != 0;
    // 3.1.1 [MQTT-3.1.2-22]: no Password without a User Name.
    if version == ProtocolVersion::V311 && password_flag && !user_flag {
        return Err(MALFORMED_PACKET);
    }
    let keep_alive = c.two()?;
    let v5 = version == ProtocolVersion::V5;
    let properties = if v5 {
        c.properties(PropertyContext::Packet(ControlPacketType::Connect))?
    } else {
        PropertySet::default()
    };
    // §3.1.2.11.10: Authentication Data needs an Authentication Method.
    if properties.authentication_data.is_some() && properties.authentication_method.is_none() {
        return Err(PROTOCOL_ERROR);
    }
    let client_id = c.string()?;
    let will = if will_flag {
        let wp = if v5 {
            c.properties(PropertyContext::Will)?
        } else {
            PropertySet::default()
        };
        let topic = c.string()?;
        let payload = c.binary()?;
        if validate_topic_name(&topic).is_err() {
            return Err(TOPIC_NAME_INVALID);
        }
        if response_topic_invalid(&wp) {
            return Err(PROTOCOL_ERROR);
        }
        Some(WillIn {
            topic,
            payload,
            qos: QoS::from_u8(will_qos).map_err(|()| MALFORMED_PACKET)?,
            retain: will_retain,
            properties: wp,
        })
    } else {
        None
    };
    let user_name = if user_flag { Some(c.string()?) } else { None };
    let password = if password_flag {
        Some(c.binary()?)
    } else {
        None
    };
    if !c.done() {
        return Err(MALFORMED_PACKET);
    }
    Ok(Connect {
        version,
        clean_start: cf & f::CLEAN_START != 0,
        keep_alive,
        properties,
        client_id,
        will,
        user_name,
        password,
    })
}

/// A Response Topic with a wildcard ([MQTT-3.3.2-14]).
#[must_use]
pub fn response_topic_invalid(p: &PropertySet) -> bool {
    p.response_topic
        .as_deref()
        .is_some_and(|t| validate_topic_name(t).is_err())
}

/// A PUBLISH as read (§3.3), before the Topic Alias is resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Publish {
    /// DUP.
    pub dup: bool,
    /// QoS.
    pub qos: QoS,
    /// RETAIN.
    pub retain: bool,
    /// Topic Name, empty when a Topic Alias stands for it.
    pub topic: String,
    /// Packet Identifier, for QoS 1 and 2.
    pub packet_id: Option<u16>,
    /// Properties.
    pub properties: PropertySet,
    /// Payload.
    pub payload: Vec<u8>,
}

/// Reads a PUBLISH: QoS 3 ([MQTT-3.3.1-4]) and DUP with QoS 0
/// ([MQTT-3.3.1-2]) are malformed, so is a Packet Identifier of 0
/// ([MQTT-2.2.1-3]).
///
/// # Errors
/// The Reason Code.
pub fn parse_publish(version: ProtocolVersion, flags: u8, body: &[u8]) -> Result<Publish, u8> {
    let qos = QoS::from_u8((flags >> 1) & 0b11).map_err(|()| MALFORMED_PACKET)?;
    let dup = flags & 0b1000 != 0;
    if dup && qos == QoS::AtMostOnce {
        return Err(MALFORMED_PACKET);
    }
    let mut c = Cursor::new(body);
    let topic = c.string()?;
    let packet_id = if qos == QoS::AtMostOnce {
        None
    } else {
        Some(packet_id(&mut c)?)
    };
    let properties = if version == ProtocolVersion::V5 {
        c.properties(PropertyContext::Packet(ControlPacketType::Publish))?
    } else {
        PropertySet::default()
    };
    Ok(Publish {
        dup,
        qos,
        retain: flags & 0b0001 != 0,
        topic,
        packet_id,
        properties,
        payload: c.rest().to_vec(),
    })
}

/// A SUBSCRIBE as read (§3.8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubscribeIn {
    /// Packet Identifier.
    pub packet_id: u16,
    /// Properties.
    pub properties: PropertySet,
    /// Topic Filter and Subscription Options, in their order.
    pub entries: Vec<(String, u8)>,
}

fn packet_id(c: &mut Cursor<'_>) -> Result<u16, u8> {
    match c.two()? {
        0 => Err(MALFORMED_PACKET),
        id => Ok(id),
    }
}

/// Reads a SUBSCRIBE: fixed header flags 0b0010 ([MQTT-3.8.1-1]), at
/// least one entry ([MQTT-3.8.3-2], a Protocol Error), reserved option
/// bits 0 ([MQTT-3.8.3-5]), no Maximum QoS or Retain Handling of 3
/// (§3.8.3.1, Protocol Errors). In 3.1.1 bits 2 to 7 of the options are
/// reserved (3.1.1 [MQTT-3-8.3-4]).
///
/// # Errors
/// The Reason Code.
pub fn parse_subscribe(
    version: ProtocolVersion,
    flags: u8,
    body: &[u8],
) -> Result<SubscribeIn, u8> {
    if flags != 0b0010 {
        return Err(MALFORMED_PACKET);
    }
    let mut c = Cursor::new(body);
    let packet_id = packet_id(&mut c)?;
    let v5 = version == ProtocolVersion::V5;
    let properties = if v5 {
        c.properties(PropertyContext::Packet(ControlPacketType::Subscribe))?
    } else {
        PropertySet::default()
    };
    let mut entries = Vec::new();
    while !c.done() {
        let filter = c.string()?;
        let options = c.byte()?;
        if v5 {
            if options & 0xC0 != 0 {
                return Err(MALFORMED_PACKET);
            }
            if options & 0x03 == 3 || (options >> 4) & 0x03 == 3 {
                return Err(PROTOCOL_ERROR);
            }
        } else if options & 0xFC != 0 || options & 0x03 == 3 {
            return Err(MALFORMED_PACKET);
        }
        entries.push((filter, options));
    }
    if entries.is_empty() {
        return Err(if v5 { PROTOCOL_ERROR } else { MALFORMED_PACKET });
    }
    Ok(SubscribeIn {
        packet_id,
        properties,
        entries,
    })
}

/// An UNSUBSCRIBE as read (§3.10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsubscribeIn {
    /// Packet Identifier.
    pub packet_id: u16,
    /// Topic Filters, in their order.
    pub filters: Vec<String>,
}

/// Reads an UNSUBSCRIBE: flags 0b0010 ([MQTT-3.10.1-1]), Topic Filters
/// as UTF-8 Encoded Strings ([MQTT-3.10.3-1]), at least one
/// ([MQTT-3.10.3-2]).
///
/// # Errors
/// The Reason Code.
pub fn parse_unsubscribe(
    version: ProtocolVersion,
    flags: u8,
    body: &[u8],
) -> Result<UnsubscribeIn, u8> {
    if flags != 0b0010 {
        return Err(MALFORMED_PACKET);
    }
    let mut c = Cursor::new(body);
    let packet_id = packet_id(&mut c)?;
    let v5 = version == ProtocolVersion::V5;
    if v5 {
        c.properties(PropertyContext::Packet(ControlPacketType::Unsubscribe))?;
    }
    let mut filters = Vec::new();
    while !c.done() {
        filters.push(c.string()?);
    }
    if filters.is_empty() {
        return Err(if v5 { PROTOCOL_ERROR } else { MALFORMED_PACKET });
    }
    Ok(UnsubscribeIn { packet_id, filters })
}

/// Reads a PUBACK, PUBREC, PUBREL or PUBCOMP (§3.4 to §3.7): the fixed
/// header flags (0b0010 for PUBREL, [MQTT-3.6.1-1]), the short forms of
/// Remaining Length 2 and 3, a Reason Code of the packet's table
/// ([MQTT-3.4.2-1], [MQTT-3.5.2-1], [MQTT-3.6.2-1], [MQTT-3.7.2-1]).
/// Returns the Packet Identifier, the Reason Code and the properties.
///
/// # Errors
/// The Reason Code of the violation.
pub fn parse_ack(
    version: ProtocolVersion,
    kind: ControlPacketType,
    flags: u8,
    body: &[u8],
) -> Result<(u16, u8, PropertySet), u8> {
    let want = if kind == ControlPacketType::PubRel {
        0b0010
    } else {
        0
    };
    if flags != want {
        return Err(MALFORMED_PACKET);
    }
    let mut c = Cursor::new(body);
    let id = packet_id(&mut c)?;
    if version == ProtocolVersion::V311 || c.done() {
        return if c.done() {
            Ok((id, 0x00, PropertySet::default()))
        } else {
            Err(MALFORMED_PACKET)
        };
    }
    let reason = c.byte()?;
    let allowed = match kind {
        ControlPacketType::PubAck | ControlPacketType::PubRec => PUBACK_CODES,
        _ => PUBREL_CODES,
    };
    if !allowed.contains(&reason) {
        return Err(PROTOCOL_ERROR);
    }
    let props = if c.done() {
        PropertySet::default()
    } else {
        c.properties(PropertyContext::Packet(kind))?
    };
    if !c.done() {
        return Err(MALFORMED_PACKET);
    }
    Ok((id, reason, props))
}

/// Reads a DISCONNECT (§3.14): flags 0 ([MQTT-3.14.1-1]), a Reason Code
/// of §3.14.2.1, nothing at all in 3.1.1.
///
/// # Errors
/// The Reason Code of the violation.
pub fn parse_disconnect(
    version: ProtocolVersion,
    flags: u8,
    body: &[u8],
) -> Result<(u8, PropertySet), u8> {
    parse_reason_body(
        version,
        flags,
        body,
        DISCONNECT_CODES,
        ControlPacketType::Disconnect,
    )
}

/// Reads an AUTH (§3.15): flags 0 ([MQTT-3.15.1-1]), Reason Code 0x00,
/// 0x18 or 0x19.
///
/// # Errors
/// The Reason Code of the violation.
pub fn parse_auth(flags: u8, body: &[u8]) -> Result<(u8, PropertySet), u8> {
    parse_reason_body(
        ProtocolVersion::V5,
        flags,
        body,
        AUTH_CODES,
        ControlPacketType::Auth,
    )
}

fn parse_reason_body(
    version: ProtocolVersion,
    flags: u8,
    body: &[u8],
    allowed: &[u8],
    kind: ControlPacketType,
) -> Result<(u8, PropertySet), u8> {
    if flags != 0 {
        return Err(MALFORMED_PACKET);
    }
    if body.is_empty() {
        return Ok((0x00, PropertySet::default()));
    }
    if version == ProtocolVersion::V311 {
        return Err(MALFORMED_PACKET);
    }
    let mut c = Cursor::new(body);
    let reason = c.byte()?;
    if !allowed.contains(&reason) {
        return Err(PROTOCOL_ERROR);
    }
    let props = if c.done() {
        PropertySet::default()
    } else {
        c.properties(PropertyContext::Packet(kind))?
    };
    if !c.done() {
        return Err(MALFORMED_PACKET);
    }
    Ok((reason, props))
}

/// The properties of a PUBLISH or Will a server forwards (§3.3.2.3).
#[must_use]
pub fn message_properties(p: &PropertySet) -> MessageProperties {
    MessageProperties {
        payload_format_indicator: p.payload_format_indicator,
        message_expiry_interval: p.message_expiry_interval,
        content_type: p.content_type.clone(),
        response_topic: p.response_topic.clone(),
        correlation_data: p.correlation_data.clone(),
        user_properties: p.user_properties.clone(),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use ProtocolVersion::{V5, V311};

    /// A CONNECT body: name, level, flags, keep alive, then `rest`.
    fn connect(level: u8, cflags: u8, rest: &[u8]) -> Vec<u8> {
        let mut b = std::vec![0, 4, b'M', b'Q', b'T', b'T', level, cflags, 0, 10];
        b.extend_from_slice(rest);
        b
    }

    #[test]
    fn mqtt5_3_1_2_the_protocol_name_level_and_reserved_flag_are_checked() {
        // Level 5, no properties, client id "c".
        let ok = connect(5, 0x02, &[0, 0, 1, b'c']);
        let c = parse_connect(0, &ok).unwrap();
        assert_eq!(
            (c.version, c.clean_start, c.client_id.as_str()),
            (V5, true, "c")
        );
        // [MQTT-3.1.2-2]: level 6 gets 0x84 in the 5.0 format, level 3
        // the 3.1.1 Return Code 0x01.
        assert_eq!(
            parse_connect(0, &connect(6, 0, &[0, 0, 1, b'c'])),
            Err(ConnectRefusal::Connack(V5, 0x84))
        );
        assert_eq!(
            parse_connect(0, &connect(3, 0, &[0, 1, b'c'])),
            Err(ConnectRefusal::Connack(V311, 0x01))
        );
        // Another protocol name: no CONNACK.
        let mut other = ok.clone();
        other[2] = b'X';
        assert_eq!(parse_connect(0, &other), Err(ConnectRefusal::Close));
        // [MQTT-3.1.2-3]: the reserved flag.
        assert_eq!(
            parse_connect(0, &connect(5, 0x03, &[0, 0, 1, b'c'])),
            Err(ConnectRefusal::Connack(V5, MALFORMED_PACKET))
        );
        // 3.1.1: a malformed CONNECT is closed without CONNACK.
        assert_eq!(
            parse_connect(0, &connect(4, 0x03, &[0, 1, b'c'])),
            Err(ConnectRefusal::Close)
        );
        // Fixed header flags other than 0.
        assert_eq!(
            parse_connect(1, &ok),
            Err(ConnectRefusal::Connack(V5, MALFORMED_PACKET))
        );
    }

    #[test]
    fn mqtt5_3_1_2_5_will_flags_without_a_will_and_trailing_octets_are_malformed() {
        // [MQTT-3.1.2-11]: Will QoS 1 without the Will Flag.
        let wq = connect(5, 0x08, &[0, 0, 1, b'c']);
        assert_eq!(
            parse_connect(0, &wq),
            Err(ConnectRefusal::Connack(V5, MALFORMED_PACKET))
        );
        // [MQTT-3.1.2-13]: Will Retain without the Will Flag.
        let wr = connect(5, 0x20, &[0, 0, 1, b'c']);
        assert_eq!(
            parse_connect(0, &wr),
            Err(ConnectRefusal::Connack(V5, MALFORMED_PACKET))
        );
        // [MQTT-3.1.4-1]: an octet after the last field.
        let extra = connect(5, 0, &[0, 0, 1, b'c', 0xAA]);
        assert_eq!(
            parse_connect(0, &extra),
            Err(ConnectRefusal::Connack(V5, MALFORMED_PACKET))
        );
        // [MQTT-3.1.2-9]: a Will with properties, topic and payload.
        let will = connect(
            5,
            0x04 | 0x08,
            &[
                0, 0, 1, b'c', 5, 0x18, 0, 0, 0, 9, 0, 1, b't', 0, 2, b'h', b'i',
            ],
        );
        let c = parse_connect(0, &will).unwrap();
        let w = c.will.unwrap();
        assert_eq!((w.topic.as_str(), w.qos), ("t", QoS::AtLeastOnce));
        assert_eq!(w.properties.will_delay_interval, Some(9));
        // A Will Topic with a wildcard: 0x90.
        let bad = connect(5, 0x04, &[0, 0, 1, b'c', 0, 0, 1, b'#', 0, 0]);
        assert_eq!(
            parse_connect(0, &bad),
            Err(ConnectRefusal::Connack(V5, TOPIC_NAME_INVALID))
        );
    }

    #[test]
    fn mqtt5_3_3_1_publish_flags_and_packet_identifier_are_checked() {
        let body = [0, 1, b't', 0, 1, 0, b'x'];
        let p = parse_publish(V5, 0b0010, &body).unwrap();
        assert_eq!(
            (p.qos, p.packet_id, p.payload.as_slice()),
            (QoS::AtLeastOnce, Some(1), &b"x"[..])
        );
        assert_eq!(
            parse_publish(V5, 0b0110, &body),
            Err(MALFORMED_PACKET),
            "QoS 3"
        );
        assert_eq!(
            parse_publish(V5, 0b1000, &[0, 1, b't', 0]),
            Err(MALFORMED_PACKET),
            "DUP at QoS 0"
        );
        assert_eq!(
            parse_publish(V5, 0b0010, &[0, 1, b't', 0, 0, 0]),
            Err(MALFORMED_PACKET),
            "id 0"
        );
        assert_eq!(
            parse_publish(V5, 0, &[0, 2, b't', 0, 0]),
            Err(MALFORMED_PACKET),
            "U+0000"
        );
    }

    #[test]
    fn mqtt5_3_8_3_subscribe_flags_options_and_payload_are_checked() {
        let one = [0, 1, 0, 0, 1, b't', 0x01];
        assert_eq!(
            parse_subscribe(V5, 0b0010, &one).unwrap().entries,
            [("t".into(), 1)]
        );
        assert_eq!(
            parse_subscribe(V5, 0, &one),
            Err(MALFORMED_PACKET),
            "[MQTT-3.8.1-1]"
        );
        assert_eq!(
            parse_subscribe(V5, 0b0010, &[0, 1, 0]),
            Err(PROTOCOL_ERROR),
            "no entry"
        );
        for (opt, code) in [
            (0x40, MALFORMED_PACKET),
            (0x03, PROTOCOL_ERROR),
            (0x30, PROTOCOL_ERROR),
        ] {
            let b = [0, 1, 0, 0, 1, b't', opt];
            assert_eq!(parse_subscribe(V5, 0b0010, &b), Err(code), "{opt:#x}");
        }
        // 3.1.1: No Local is a reserved bit there.
        assert_eq!(
            parse_subscribe(V311, 0b0010, &[0, 1, 0, 1, b't', 0x04]),
            Err(MALFORMED_PACKET)
        );
        assert_eq!(
            parse_unsubscribe(V5, 0b0010, &[0, 1, 0]),
            Err(PROTOCOL_ERROR)
        );
        assert_eq!(
            parse_unsubscribe(V5, 0, &[0, 1, 0, 0, 1, b't']),
            Err(MALFORMED_PACKET)
        );
    }

    #[test]
    fn mqtt5_3_4_2_acks_short_forms_flags_and_reason_codes() {
        use ControlPacketType::{PubAck, PubRel};
        assert_eq!(parse_ack(V5, PubAck, 0, &[0, 7]).unwrap().0, 7);
        assert_eq!(parse_ack(V5, PubAck, 0, &[0, 7, 0x10]).unwrap().1, 0x10);
        assert_eq!(parse_ack(V5, PubAck, 0, &[0, 7, 0x92]), Err(PROTOCOL_ERROR));
        assert_eq!(
            parse_ack(V5, PubRel, 0, &[0, 7]),
            Err(MALFORMED_PACKET),
            "[MQTT-3.6.1-1]"
        );
        assert_eq!(
            parse_ack(V5, PubRel, 0b0010, &[0, 7, 0x92, 0]).unwrap().1,
            0x92
        );
        assert_eq!(
            parse_ack(V311, PubAck, 0, &[0, 7, 0]),
            Err(MALFORMED_PACKET)
        );
        assert_eq!(parse_disconnect(V5, 0, &[]).unwrap().0, 0x00);
        assert_eq!(parse_disconnect(V5, 0, &[0x05]), Err(PROTOCOL_ERROR));
        assert_eq!(parse_disconnect(V5, 2, &[]), Err(MALFORMED_PACKET));
        assert_eq!(parse_auth(0, &[0x01, 0]), Err(PROTOCOL_ERROR));
    }

    #[test]
    fn mqtt5_3_1_3_the_3_1_1_return_codes_of_a_refusal() {
        assert_eq!(refusal(V311, 0x85), ConnectRefusal::Connack(V311, 0x02));
        assert_eq!(refusal(V311, 0x86), ConnectRefusal::Connack(V311, 0x04));
        assert_eq!(refusal(V311, 0x9C), ConnectRefusal::Connack(V311, 0x03));
        assert_eq!(refusal(V311, 0x81), ConnectRefusal::Close);
        assert_eq!(refusal(V5, 0x81), ConnectRefusal::Connack(V5, 0x81));
    }
}
