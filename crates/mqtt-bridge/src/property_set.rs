// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The typed property set of one packet — MQTT 5.0 §2.2.2.
//!
//! [`PropertySet::parse`] reads a property block (without its length
//! prefix, as the decoded bodies of [`crate::control_packets`] hold it) and
//! checks it against Table 2-4 and the rules of the packet it came in:
//!
//! * an identifier outside Table 2-4, or one the table does not list for
//!   the packet, is a Malformed Packet (§2.2.2.2);
//! * a property other than User Property and Subscription Identifier that
//!   appears twice is a Protocol Error (§3.1.2.11 and the property
//!   descriptions of chapter 3);
//! * a value that ends inside the block, a string that is not well-formed
//!   UTF-8 or contains U+0000 ([MQTT-1.5.4-1], [MQTT-1.5.4-2]) is a
//!   Malformed Packet;
//! * the values the descriptions forbid (a Receive Maximum, Maximum Packet
//!   Size or Subscription Identifier of 0; a Payload Format Indicator,
//!   Request Problem Information or Request Response Information other
//!   than 0 or 1) are a Protocol Error;
//! * a Topic Alias of 0 is a Protocol Error with the more specific Reason
//!   Code 0x94 Topic Alias invalid (§3.3.2.3.4, §4.13.1).
//!
//! [`PropertySet::encode`] writes the set back in the order of Table 2-4,
//! User Properties in the order they were added ([MQTT-3.3.2-18]).

use alloc::string::String;
use alloc::vec::Vec;

use crate::packet::ControlPacketType;
use crate::properties::{PropertyId, PropertyValueKind};
use crate::vbi::{decode_vbi, encode_vbi};

/// Reason Code 0x81 Malformed Packet (§2.4).
pub const MALFORMED_PACKET: u8 = 0x81;
/// Reason Code 0x82 Protocol Error (§2.4).
pub const PROTOCOL_ERROR: u8 = 0x82;
/// Reason Code 0x94 Topic Alias invalid (§2.4, §3.3.2.3.4).
pub const TOPIC_ALIAS_INVALID: u8 = 0x94;

/// Where a property block stands: in a packet, or the Will Properties of a
/// CONNECT (§3.1.3.2), whose identifiers Table 2-4 lists apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropertyContext {
    /// The property block of a packet of this type.
    Packet(ControlPacketType),
    /// The Will Properties of a CONNECT.
    Will,
}

/// Whether Table 2-4 allows `id` in `ctx` (its column "Packet / Will
/// Properties").
#[must_use]
pub fn allowed_in(id: PropertyId, ctx: PropertyContext) -> bool {
    use ControlPacketType as T;
    use PropertyContext::{Packet, Will};
    use PropertyId as P;
    match id {
        P::PayloadFormatIndicator
        | P::MessageExpiryInterval
        | P::ContentType
        | P::ResponseTopic
        | P::CorrelationData => matches!(ctx, Packet(T::Publish) | Will),
        P::SubscriptionIdentifier => matches!(ctx, Packet(T::Publish | T::Subscribe)),
        P::SessionExpiryInterval => {
            matches!(ctx, Packet(T::Connect | T::ConnAck | T::Disconnect))
        }
        P::AssignedClientIdentifier
        | P::ServerKeepAlive
        | P::ResponseInformation
        | P::MaximumQos
        | P::RetainAvailable
        | P::WildcardSubscriptionAvailable
        | P::SubscriptionIdentifierAvailable
        | P::SharedSubscriptionAvailable => matches!(ctx, Packet(T::ConnAck)),
        P::AuthenticationMethod | P::AuthenticationData => {
            matches!(ctx, Packet(T::Connect | T::ConnAck | T::Auth))
        }
        P::RequestProblemInformation | P::RequestResponseInformation => {
            matches!(ctx, Packet(T::Connect))
        }
        P::WillDelayInterval => matches!(ctx, Will),
        P::ServerReference => matches!(ctx, Packet(T::ConnAck | T::Disconnect)),
        P::ReasonString => matches!(
            ctx,
            Packet(
                T::ConnAck
                    | T::PubAck
                    | T::PubRec
                    | T::PubRel
                    | T::PubComp
                    | T::SubAck
                    | T::UnsubAck
                    | T::Disconnect
                    | T::Auth
            )
        ),
        P::ReceiveMaximum | P::TopicAliasMaximum | P::MaximumPacketSize => {
            matches!(ctx, Packet(T::Connect | T::ConnAck))
        }
        P::TopicAlias => matches!(ctx, Packet(T::Publish)),
        P::UserProperty => !matches!(ctx, Packet(T::PingReq | T::PingResp)),
        P::Other(_) => false,
    }
}

/// Whether `s` is a UTF-8 Encoded String of §1.5.4: well-formed (a Rust
/// `str` is) and without U+0000 ([MQTT-1.5.4-2]).
#[must_use]
pub fn is_mqtt_string(s: &str) -> bool {
    !s.contains('\0')
}

/// The properties of one packet, typed. `None` and empty mean absent.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PropertySet {
    /// 0x01 Payload Format Indicator.
    pub payload_format_indicator: Option<u8>,
    /// 0x02 Message Expiry Interval, seconds.
    pub message_expiry_interval: Option<u32>,
    /// 0x03 Content Type.
    pub content_type: Option<String>,
    /// 0x08 Response Topic.
    pub response_topic: Option<String>,
    /// 0x09 Correlation Data.
    pub correlation_data: Option<Vec<u8>>,
    /// 0x0B Subscription Identifier, each occurrence.
    pub subscription_identifiers: Vec<u32>,
    /// 0x11 Session Expiry Interval, seconds.
    pub session_expiry_interval: Option<u32>,
    /// 0x12 Assigned Client Identifier.
    pub assigned_client_identifier: Option<String>,
    /// 0x13 Server Keep Alive, seconds.
    pub server_keep_alive: Option<u16>,
    /// 0x15 Authentication Method.
    pub authentication_method: Option<String>,
    /// 0x16 Authentication Data.
    pub authentication_data: Option<Vec<u8>>,
    /// 0x17 Request Problem Information.
    pub request_problem_information: Option<u8>,
    /// 0x18 Will Delay Interval, seconds.
    pub will_delay_interval: Option<u32>,
    /// 0x19 Request Response Information.
    pub request_response_information: Option<u8>,
    /// 0x1A Response Information.
    pub response_information: Option<String>,
    /// 0x1C Server Reference.
    pub server_reference: Option<String>,
    /// 0x1F Reason String.
    pub reason_string: Option<String>,
    /// 0x21 Receive Maximum.
    pub receive_maximum: Option<u16>,
    /// 0x22 Topic Alias Maximum.
    pub topic_alias_maximum: Option<u16>,
    /// 0x23 Topic Alias.
    pub topic_alias: Option<u16>,
    /// 0x24 Maximum QoS.
    pub maximum_qos: Option<u8>,
    /// 0x25 Retain Available.
    pub retain_available: Option<u8>,
    /// 0x26 User Property, in wire order.
    pub user_properties: Vec<(String, String)>,
    /// 0x27 Maximum Packet Size.
    pub maximum_packet_size: Option<u32>,
    /// 0x28 Wildcard Subscription Available.
    pub wildcard_subscription_available: Option<u8>,
    /// 0x29 Subscription Identifier Available.
    pub subscription_identifier_available: Option<u8>,
    /// 0x2A Shared Subscription Available.
    pub shared_subscription_available: Option<u8>,
}

/// A cursor over the value bytes of a property block.
struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8], u8> {
        let end = self.at.checked_add(n).ok_or(MALFORMED_PACKET)?;
        let out = self.bytes.get(self.at..end).ok_or(MALFORMED_PACKET)?;
        self.at = end;
        Ok(out)
    }
    fn byte(&mut self) -> Result<u8, u8> {
        Ok(self.take(1)?[0])
    }
    fn two(&mut self) -> Result<u16, u8> {
        let b = self.take(2)?;
        Ok(u16::from_be_bytes([b[0], b[1]]))
    }
    fn four(&mut self) -> Result<u32, u8> {
        let b = self.take(4)?;
        Ok(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }
    fn vbi(&mut self) -> Result<u32, u8> {
        let (v, n) = decode_vbi(&self.bytes[self.at..]).map_err(|_| MALFORMED_PACKET)?;
        self.at += n;
        Ok(v)
    }
    fn binary(&mut self) -> Result<Vec<u8>, u8> {
        let n = usize::from(self.two()?);
        Ok(self.take(n)?.to_vec())
    }
    fn string(&mut self) -> Result<String, u8> {
        let raw = self.binary()?;
        let s = String::from_utf8(raw).map_err(|_| MALFORMED_PACKET)?;
        if !is_mqtt_string(&s) {
            return Err(MALFORMED_PACKET);
        }
        Ok(s)
    }
}

/// Sets `slot` once; a second occurrence is a Protocol Error.
fn once<T>(slot: &mut Option<T>, value: T) -> Result<(), u8> {
    if slot.is_some() {
        return Err(PROTOCOL_ERROR);
    }
    *slot = Some(value);
    Ok(())
}

/// A Byte property that must be 0 or 1.
fn zero_or_one(v: u8) -> Result<u8, u8> {
    if v > 1 { Err(PROTOCOL_ERROR) } else { Ok(v) }
}

/// A Two Byte Integer property that must not be 0.
fn non_zero_u16(v: u16) -> Result<u16, u8> {
    if v == 0 { Err(PROTOCOL_ERROR) } else { Ok(v) }
}

impl PropertySet {
    /// Parses and checks the property block `block` of `ctx` (see the
    /// module documentation).
    ///
    /// # Errors
    /// The Reason Code of the violation: [`MALFORMED_PACKET`],
    /// [`PROTOCOL_ERROR`] or, for a Topic Alias of 0,
    /// [`TOPIC_ALIAS_INVALID`].
    pub fn parse(block: &[u8], ctx: PropertyContext) -> Result<Self, u8> {
        use PropertyId as P;
        let mut set = Self::default();
        let mut r = Reader {
            bytes: block,
            at: 0,
        };
        while r.at < block.len() {
            let id = PropertyId::from_value(r.vbi()?);
            if id.value_kind().is_none() || !allowed_in(id, ctx) {
                return Err(MALFORMED_PACKET);
            }
            match id {
                P::PayloadFormatIndicator => {
                    once(&mut set.payload_format_indicator, zero_or_one(r.byte()?)?)?;
                }
                P::MessageExpiryInterval => once(&mut set.message_expiry_interval, r.four()?)?,
                P::ContentType => once(&mut set.content_type, r.string()?)?,
                P::ResponseTopic => once(&mut set.response_topic, r.string()?)?,
                P::CorrelationData => once(&mut set.correlation_data, r.binary()?)?,
                P::SubscriptionIdentifier => {
                    let v = r.vbi()?;
                    if v == 0 {
                        return Err(PROTOCOL_ERROR);
                    }
                    // §3.8.2.1.2: at most one in a SUBSCRIBE.
                    if matches!(ctx, PropertyContext::Packet(ControlPacketType::Subscribe))
                        && !set.subscription_identifiers.is_empty()
                    {
                        return Err(PROTOCOL_ERROR);
                    }
                    set.subscription_identifiers.push(v);
                }
                P::SessionExpiryInterval => once(&mut set.session_expiry_interval, r.four()?)?,
                P::AssignedClientIdentifier => {
                    once(&mut set.assigned_client_identifier, r.string()?)?;
                }
                P::ServerKeepAlive => once(&mut set.server_keep_alive, r.two()?)?,
                P::AuthenticationMethod => once(&mut set.authentication_method, r.string()?)?,
                P::AuthenticationData => once(&mut set.authentication_data, r.binary()?)?,
                P::RequestProblemInformation => once(
                    &mut set.request_problem_information,
                    zero_or_one(r.byte()?)?,
                )?,
                P::WillDelayInterval => once(&mut set.will_delay_interval, r.four()?)?,
                P::RequestResponseInformation => once(
                    &mut set.request_response_information,
                    zero_or_one(r.byte()?)?,
                )?,
                P::ResponseInformation => once(&mut set.response_information, r.string()?)?,
                P::ServerReference => once(&mut set.server_reference, r.string()?)?,
                P::ReasonString => once(&mut set.reason_string, r.string()?)?,
                P::ReceiveMaximum => once(&mut set.receive_maximum, non_zero_u16(r.two()?)?)?,
                P::TopicAliasMaximum => once(&mut set.topic_alias_maximum, r.two()?)?,
                P::TopicAlias => {
                    let v = r.two()?;
                    // §3.3.2.3.4: "A Topic Alias value of 0 [...] is a
                    // Protocol Error, the receiver uses DISCONNECT with
                    // Reason Code of 0x94 (Topic Alias invalid)".
                    if v == 0 {
                        return Err(TOPIC_ALIAS_INVALID);
                    }
                    once(&mut set.topic_alias, v)?;
                }
                P::MaximumQos => once(&mut set.maximum_qos, zero_or_one(r.byte()?)?)?,
                P::RetainAvailable => once(&mut set.retain_available, zero_or_one(r.byte()?)?)?,
                P::UserProperty => {
                    let k = r.string()?;
                    let v = r.string()?;
                    set.user_properties.push((k, v));
                }
                P::MaximumPacketSize => {
                    let v = r.four()?;
                    if v == 0 {
                        return Err(PROTOCOL_ERROR);
                    }
                    once(&mut set.maximum_packet_size, v)?;
                }
                P::WildcardSubscriptionAvailable => once(
                    &mut set.wildcard_subscription_available,
                    zero_or_one(r.byte()?)?,
                )?,
                P::SubscriptionIdentifierAvailable => once(
                    &mut set.subscription_identifier_available,
                    zero_or_one(r.byte()?)?,
                )?,
                P::SharedSubscriptionAvailable => once(
                    &mut set.shared_subscription_available,
                    zero_or_one(r.byte()?)?,
                )?,
                P::Other(_) => return Err(MALFORMED_PACKET),
            }
        }
        Ok(set)
    }

    /// The property block (without its length prefix) in the order of
    /// Table 2-4. A string or binary value over 65,535 octets cannot be
    /// written and is an error.
    ///
    /// # Errors
    /// `()` for a value too long for its length prefix.
    #[allow(clippy::result_unit_err)]
    pub fn encode(&self) -> Result<Vec<u8>, ()> {
        let mut out = Vec::new();
        let id = |out: &mut Vec<u8>, p: PropertyId| {
            out.extend_from_slice(&encode_vbi(p.to_value()).unwrap_or_default());
        };
        let bin = |out: &mut Vec<u8>, b: &[u8]| -> Result<(), ()> {
            let n = u16::try_from(b.len()).map_err(|_| ())?;
            out.extend_from_slice(&n.to_be_bytes());
            out.extend_from_slice(b);
            Ok(())
        };
        macro_rules! byte {
            ($f:expr, $p:expr) => {
                if let Some(v) = $f {
                    id(&mut out, $p);
                    out.push(v);
                }
            };
        }
        macro_rules! two {
            ($f:expr, $p:expr) => {
                if let Some(v) = $f {
                    id(&mut out, $p);
                    out.extend_from_slice(&v.to_be_bytes());
                }
            };
        }
        macro_rules! four {
            ($f:expr, $p:expr) => {
                if let Some(v) = $f {
                    id(&mut out, $p);
                    out.extend_from_slice(&v.to_be_bytes());
                }
            };
        }
        macro_rules! blob {
            ($f:expr, $p:expr) => {
                if let Some(v) = &$f {
                    id(&mut out, $p);
                    bin(&mut out, v.as_ref())?;
                }
            };
        }
        use PropertyId as P;
        byte!(self.payload_format_indicator, P::PayloadFormatIndicator);
        four!(self.message_expiry_interval, P::MessageExpiryInterval);
        blob!(self.content_type, P::ContentType);
        blob!(self.response_topic, P::ResponseTopic);
        blob!(self.correlation_data, P::CorrelationData);
        for v in &self.subscription_identifiers {
            id(&mut out, P::SubscriptionIdentifier);
            out.extend_from_slice(&encode_vbi(*v).ok_or(())?);
        }
        four!(self.session_expiry_interval, P::SessionExpiryInterval);
        blob!(self.assigned_client_identifier, P::AssignedClientIdentifier);
        two!(self.server_keep_alive, P::ServerKeepAlive);
        blob!(self.authentication_method, P::AuthenticationMethod);
        blob!(self.authentication_data, P::AuthenticationData);
        byte!(
            self.request_problem_information,
            P::RequestProblemInformation
        );
        four!(self.will_delay_interval, P::WillDelayInterval);
        byte!(
            self.request_response_information,
            P::RequestResponseInformation
        );
        blob!(self.response_information, P::ResponseInformation);
        blob!(self.server_reference, P::ServerReference);
        blob!(self.reason_string, P::ReasonString);
        two!(self.receive_maximum, P::ReceiveMaximum);
        two!(self.topic_alias_maximum, P::TopicAliasMaximum);
        two!(self.topic_alias, P::TopicAlias);
        byte!(self.maximum_qos, P::MaximumQos);
        byte!(self.retain_available, P::RetainAvailable);
        for (k, v) in &self.user_properties {
            id(&mut out, P::UserProperty);
            bin(&mut out, k.as_bytes())?;
            bin(&mut out, v.as_bytes())?;
        }
        four!(self.maximum_packet_size, P::MaximumPacketSize);
        byte!(
            self.wildcard_subscription_available,
            P::WildcardSubscriptionAvailable
        );
        byte!(
            self.subscription_identifier_available,
            P::SubscriptionIdentifierAvailable
        );
        byte!(
            self.shared_subscription_available,
            P::SharedSubscriptionAvailable
        );
        Ok(out)
    }
}

// The value kinds the parser reads must be those of Table 2-4; this keeps
// the two tables from drifting apart.
const _: () = {
    assert!(matches!(
        PropertyId::SubscriptionIdentifier.value_kind(),
        Some(PropertyValueKind::VariableByteInt)
    ));
};

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use ControlPacketType as T;
    use PropertyContext::{Packet, Will};

    #[test]
    fn mqtt5_2_2_2_a_set_round_trips_and_keeps_the_order_of_user_properties() {
        let set = PropertySet {
            payload_format_indicator: Some(1),
            message_expiry_interval: Some(60),
            content_type: Some("text/plain".into()),
            response_topic: Some("r/t".into()),
            correlation_data: Some(alloc::vec![1, 2, 3]),
            subscription_identifiers: alloc::vec![7, 300],
            topic_alias: Some(4),
            user_properties: alloc::vec![
                ("z".into(), "1".into()),
                ("a".into(), "2".into()),
                ("z".into(), "3".into()),
            ],
            ..PropertySet::default()
        };
        let bytes = set.encode().unwrap();
        assert_eq!(PropertySet::parse(&bytes, Packet(T::Publish)).unwrap(), set);
    }

    #[test]
    fn mqtt5_2_2_2_an_identifier_not_listed_for_the_packet_is_malformed() {
        // Session Expiry Interval in a PUBLISH, Will Delay Interval in a
        // CONNECT, Topic Alias in the Will Properties, an unknown 0x04.
        for (block, ctx) in [
            (&[0x11, 0, 0, 0, 1][..], Packet(T::Publish)),
            (&[0x18, 0, 0, 0, 1][..], Packet(T::Connect)),
            (&[0x23, 0, 1][..], Will),
            (&[0x04, 0][..], Packet(T::Publish)),
        ] {
            assert_eq!(
                PropertySet::parse(block, ctx),
                Err(MALFORMED_PACKET),
                "{block:?}"
            );
        }
        assert!(PropertySet::parse(&[0x18, 0, 0, 0, 1], Will).is_ok());
    }

    #[test]
    fn mqtt5_2_2_2_a_property_twice_is_a_protocol_error_user_property_is_not() {
        let twice = [0x11, 0, 0, 0, 1, 0x11, 0, 0, 0, 2];
        assert_eq!(
            PropertySet::parse(&twice, Packet(T::Connect)),
            Err(PROTOCOL_ERROR)
        );
        let users = [0x26, 0, 1, b'k', 0, 1, b'v', 0x26, 0, 1, b'k', 0, 1, b'w'];
        let set = PropertySet::parse(&users, Packet(T::Connect)).unwrap();
        assert_eq!(set.user_properties.len(), 2);
        // Two Subscription Identifiers: a PUBLISH of the server may carry
        // them, a SUBSCRIBE may not (§3.8.2.1.2).
        let ids = [0x0B, 1, 0x0B, 2];
        assert!(PropertySet::parse(&ids, Packet(T::Publish)).is_ok());
        assert_eq!(
            PropertySet::parse(&ids, Packet(T::Subscribe)),
            Err(PROTOCOL_ERROR)
        );
    }

    #[test]
    fn mqtt5_2_2_2_forbidden_values_are_protocol_errors() {
        for (block, ctx) in [
            (&[0x21, 0, 0][..], Packet(T::Connect)), // Receive Maximum 0
            (&[0x27, 0, 0, 0, 0][..], Packet(T::Connect)), // Maximum Packet Size 0
            (&[0x0B, 0][..], Packet(T::Subscribe)),  // Subscription Identifier 0
            (&[0x01, 2][..], Packet(T::Publish)),    // Payload Format Indicator 2
            (&[0x17, 2][..], Packet(T::Connect)),    // Request Problem Information 2
            (&[0x19, 2][..], Packet(T::Connect)),    // Request Response Information 2
        ] {
            assert_eq!(
                PropertySet::parse(block, ctx),
                Err(PROTOCOL_ERROR),
                "{block:?}"
            );
        }
        // Topic Alias 0: the specific Reason Code of §3.3.2.3.4.
        assert_eq!(
            PropertySet::parse(&[0x23, 0, 0], Packet(T::Publish)),
            Err(TOPIC_ALIAS_INVALID)
        );
    }

    #[test]
    fn mqtt5_1_5_4_a_string_with_u0000_or_bad_utf8_and_a_cut_value_are_malformed() {
        for block in [
            &[0x03, 0, 2, b'a', 0][..],      // U+0000
            &[0x03, 0, 2, 0xC3, 0x28][..],   // ill-formed UTF-8
            &[0x03, 0, 3, 0xED, 0xA0, 0x80], // a surrogate, U+D800
            &[0x02, 0, 0][..],               // a Four Byte Integer cut short
            &[0x26, 0, 1, b'k'][..],         // a pair without its value
        ] {
            assert_eq!(
                PropertySet::parse(block, Packet(T::Publish)),
                Err(MALFORMED_PACKET),
                "{block:?}"
            );
        }
    }
}
