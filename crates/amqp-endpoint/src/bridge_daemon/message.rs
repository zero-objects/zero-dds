// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The message form of a DDS sample (spec §4.3, §4.4; DDS-AMQP 1.0 §8):
//! a sample becomes one AMQP message, a message becomes a sample.

use std::string::{String, ToString};
use std::vec::Vec;

use zerodds_amqp_bridge::extended_types::AmqpExtValue;
use zerodds_amqp_bridge::sections::MessageSection;

use super::config::TopicMapping;
use crate::coexistence::{CoexistenceConfig, stamp_outbound};
use crate::properties::{
    DdsOperation, SampleHeader, app_keys, produce_application_properties, produce_properties,
};

/// The content type of a Pass-Through body (DDS-AMQP 1.0 §8.1.1).
pub const CONTENT_TYPE_XCDR2: &str = "application/vnd.dds.xcdr2";

/// The content type this bridge gives an XCDR1 sample. DDS-AMQP 1.0 §8.1.1
/// defines Pass-Through for XCDR2; an XCDR1 sample travels with its XCDR1
/// encapsulation header under this type instead of being relabelled.
pub const CONTENT_TYPE_XCDR1: &str = "application/vnd.dds.xcdr1";

/// The encapsulation header of a body (RTPS 2.5 §10.5): the representation
/// identifier the sample came under, and options. The body is passed on
/// unchanged, so the identifier is too: it names the XCDR version, the
/// format (plain, delimited, parameter list) and the byte order.
fn encapsulation(representation_identifier: [u8; 2]) -> [u8; 4] {
    [
        representation_identifier[0],
        representation_identifier[1],
        0,
        0,
    ]
}

/// Whether a representation identifier names XCDR2 (DDS-XTypes 1.3 Table 60
/// and Table 39: `0x0006` and up), else XCDR1.
fn is_xcdr2(representation_identifier: [u8; 2]) -> bool {
    representation_identifier[1] >= 0x06
}

/// The extensibility the DDS writer frames a body under `identifier` with
/// (DDS-XTypes 1.3 Table 60, Table 39): a parameter list (`PL_CDR`,
/// `PL_CDR2`) is `@mutable`, a delimited body (`D_CDR2`) `@appendable`,
/// anything else `@final`. Without it the writer's header named every body
/// plain, also one that carries a DHEADER or member headers.
#[must_use]
pub fn wire_extensibility_of(
    representation_identifier: [u8; 2],
) -> zerodds_types::qos::ExtensibilityForRepr {
    use zerodds_types::qos::ExtensibilityForRepr as Ext;
    match representation_identifier[1] & !0x01 {
        0x02 | 0x0a | 0x12 => Ext::Mutable,
        0x08 | 0x14 => Ext::Appendable,
        _ => Ext::Final,
    }
}

/// What a DDS sample contributes to its message.
#[derive(Debug, Clone)]
pub struct OutSample<'a> {
    /// CDR payload without the encapsulation header.
    pub payload: &'a [u8],
    /// The representation identifier of the header the sample came under
    /// (`UserSample::Alive::representation_identifier`, DDSI-RTPS 2.5 §10.5).
    pub representation_identifier: [u8; 2],
    /// Key hash; `None` for an unkeyed topic.
    pub key_hash: Option<[u8; 16]>,
    /// Writer GUID.
    pub writer_guid: [u8; 16],
    /// RTPS sequence number.
    pub sequence: i64,
    /// Source timestamp in nanoseconds since the epoch.
    pub source_timestamp_ns: Option<u64>,
    /// `dds:operation`.
    pub operation: DdsOperation,
    /// The writer's LIFESPAN in milliseconds; `None` for infinite.
    pub lifespan_ms: Option<u64>,
    /// The writer's PARTITION (`dds:partition`, DDS-AMQP 1.0 §8.3); empty
    /// for the default partition.
    pub partitions: &'a [String],
}

/// Encodes a DDS sample as the message of §4.3: header, properties,
/// application properties, a Pass-Through data body.
///
/// # Errors
/// An encoding error of the AMQP codec.
pub fn encode_message(
    topic: &TopicMapping,
    domain: i32,
    coexistence: &CoexistenceConfig,
    s: &OutSample<'_>,
) -> Result<Vec<u8>, String> {
    let now_ns = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_nanos()).unwrap_or(u64::MAX));
    let ts = s.source_timestamp_ns.unwrap_or(now_ns);
    let hdr = SampleHeader {
        writer_guid: s.writer_guid,
        seqnum: u64::try_from(s.sequence.max(0)).unwrap_or(0),
        source_timestamp_ms: i64::try_from(ts / 1_000_000).unwrap_or(i64::MAX),
        source_nsec_remainder: u32::try_from(ts % 1_000_000).unwrap_or(0),
        keyhash: s.key_hash.map(|k| k.to_vec()),
        instance_handle: s.key_hash.unwrap_or([0; 16]),
        lifespan_remaining_ms: s.lifespan_ms.map(|l| i64::try_from(l).unwrap_or(i64::MAX)),
        operation: s.operation,
        type_id_hex: None,
        domain_id: u32::try_from(domain).unwrap_or(0),
        partitions: s.partitions.to_vec(),
    };
    let props = produce_properties(&hdr);
    let content_type = if is_xcdr2(s.representation_identifier) {
        CONTENT_TYPE_XCDR2
    } else {
        CONTENT_TYPE_XCDR1
    };
    let durable = !matches!(topic.durability, zerodds_qos::DurabilityKind::Volatile);
    // §6: LIFESPAN is the message's ttl.
    let ttl = s.lifespan_ms.map_or(AmqpExtValue::Null, |l| {
        AmqpExtValue::Uint(u32::try_from(l).unwrap_or(u32::MAX))
    });
    let header = MessageSection::Header(AmqpExtValue::List(vec![
        AmqpExtValue::Boolean(durable),
        AmqpExtValue::Ubyte(4),
        ttl,
        AmqpExtValue::Boolean(true),
    ]));
    let properties = MessageSection::Properties(AmqpExtValue::List(vec![
        AmqpExtValue::Binary(props.message_id),
        AmqpExtValue::Null, // user-id
        AmqpExtValue::Str(topic.amqp_address.clone()),
        AmqpExtValue::Str(topic.dds_name.clone()),
        AmqpExtValue::Null, // reply-to
        AmqpExtValue::Null, // correlation-id
        AmqpExtValue::Symbol(content_type.to_string()),
        AmqpExtValue::Null, // content-encoding
        props
            .absolute_expiry_time_ms
            .map_or(AmqpExtValue::Null, AmqpExtValue::Timestamp),
        AmqpExtValue::Timestamp(props.creation_time_ms),
        props.group_id.map_or(AmqpExtValue::Null, AmqpExtValue::Str),
    ]));
    let mut app = produce_application_properties(&hdr);
    stamp_outbound(coexistence, &mut app);
    let mut body = Vec::with_capacity(4 + s.payload.len());
    body.extend_from_slice(&encapsulation(s.representation_identifier));
    body.extend_from_slice(s.payload);
    let mut out = Vec::with_capacity(body.len() + 256);
    for section in [
        header,
        properties,
        MessageSection::ApplicationProperties(app),
        MessageSection::Data(body),
    ] {
        out.extend_from_slice(&section.encode().map_err(|e| format!("{e:?}"))?);
    }
    Ok(out)
}

/// A message the broker gave back with `released` or `modified`, as it
/// goes out again (AMQP 1.0 Part 3 §3.2.1, §3.4.4, §3.4.5):
///
/// * `header.first-acquirer` is false: the broker's link acquired it;
/// * `header.delivery-count` is one more when `delivery_failed` (a
///   `modified` with delivery-failed);
/// * `annotations` (a `modified`'s message-annotations) are combined with
///   the message's: an entry of the same key is replaced, another added; a
///   message without message-annotations gets the section after its header.
///
/// Every other section keeps its octets.
///
/// # Errors
/// A section the codec cannot read or write.
pub(super) fn redelivered(
    message: &[u8],
    delivery_failed: bool,
    annotations: &[(AmqpExtValue, AmqpExtValue)],
) -> Result<Vec<u8>, String> {
    rewrite(message, Some(delivery_failed), annotations)
}

/// `message` with `annotations` combined into its message-annotations, as
/// [`redelivered`] does, the header and every other section unchanged.
///
/// # Errors
/// A section the codec cannot read or write.
pub(super) fn annotated(
    message: &[u8],
    annotations: &[(AmqpExtValue, AmqpExtValue)],
) -> Result<Vec<u8>, String> {
    rewrite(message, None, annotations)
}

/// The sections of `message` again: with `again` (`Some(delivery_failed)`)
/// the header of a message sent again, and `annotations` combined.
fn rewrite(
    message: &[u8],
    again: Option<bool>,
    annotations: &[(AmqpExtValue, AmqpExtValue)],
) -> Result<Vec<u8>, String> {
    use zerodds_amqp_bridge::sections::MessageSection as S;
    let enc = |s: &S| s.encode().map_err(|e| format!("{e:?}"));
    let mut sections: Vec<(S, &[u8])> = Vec::new();
    let mut rest = message;
    while !rest.is_empty() {
        let (s, n) = S::decode(rest).map_err(|e| format!("message section: {e:?}"))?;
        if n == 0 {
            return Err("a message section of no octets".into());
        }
        sections.push((s, &rest[..n]));
        rest = &rest[n..];
    }
    let mut out = Vec::with_capacity(message.len() + 64);
    let mut header_seen = false;
    let mut annotated = annotations.is_empty();
    for (s, raw) in &sections {
        match s {
            S::Header(AmqpExtValue::List(fields)) if again.is_some() => {
                let delivery_failed = again == Some(true);
                header_seen = true;
                let mut f = fields.clone();
                if f.len() < 5 {
                    f.resize(5, AmqpExtValue::Null);
                }
                f[3] = AmqpExtValue::Boolean(false);
                if delivery_failed {
                    let count = match &f[4] {
                        AmqpExtValue::Uint(c) => *c,
                        AmqpExtValue::Null => 0,
                        other => return Err(format!("header.delivery-count {other:?}")),
                    };
                    f[4] = AmqpExtValue::Uint(count.saturating_add(1));
                }
                out.extend_from_slice(&enc(&S::Header(AmqpExtValue::List(f)))?);
            }
            S::MessageAnnotations(existing) if !annotated => {
                let mut merged = match existing {
                    AmqpExtValue::Map(m) => m.clone(),
                    AmqpExtValue::Null => Vec::new(),
                    other => return Err(format!("message-annotations {other:?}")),
                };
                merge_annotations(&mut merged, annotations);
                out.extend_from_slice(&enc(&S::MessageAnnotations(AmqpExtValue::Map(merged)))?);
                annotated = true;
            }
            S::DeliveryAnnotations(_) | S::Header(_) => out.extend_from_slice(raw),
            _ => {
                if !annotated {
                    // The first section after header and delivery
                    // annotations: the message had no message-annotations.
                    out.extend_from_slice(&enc(&S::MessageAnnotations(AmqpExtValue::Map(
                        annotations.to_vec(),
                    )))?);
                    annotated = true;
                }
                out.extend_from_slice(raw);
            }
        }
    }
    if let (false, Some(delivery_failed)) = (header_seen, again) {
        // A header of defaults with first-acquirer false (Part 3 §3.2.1).
        let mut f = vec![
            AmqpExtValue::Null,
            AmqpExtValue::Null,
            AmqpExtValue::Null,
            AmqpExtValue::Boolean(false),
        ];
        if delivery_failed {
            f.push(AmqpExtValue::Uint(1));
        }
        let mut with_header = enc(&S::Header(AmqpExtValue::List(f)))?;
        with_header.extend_from_slice(&out);
        out = with_header;
    }
    if !annotated {
        out.extend_from_slice(&enc(&S::MessageAnnotations(AmqpExtValue::Map(
            annotations.to_vec(),
        )))?);
    }
    Ok(out)
}

/// Combines `add` into the annotation map `into` (Part 3 §3.4.5): an
/// entry of the same key is replaced, another appended.
pub(super) fn merge_annotations(
    into: &mut Vec<(AmqpExtValue, AmqpExtValue)>,
    add: &[(AmqpExtValue, AmqpExtValue)],
) {
    for (k, v) in add {
        match into.iter_mut().find(|(ek, _)| ek == k) {
            Some(e) => e.1 = v.clone(),
            None => into.push((k.clone(), v.clone())),
        }
    }
}

/// A message from the broker, as far as the bridge reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InMessage {
    /// `properties.content-type`.
    pub content_type: Option<String>,
    /// `application-properties`.
    pub application_properties: AmqpExtValue,
    /// The body: all data sections in order, or a binary amqp-value.
    pub body: Vec<u8>,
    /// `dds:operation`.
    pub operation: DdsOperation,
    /// `dds:instance-handle`: the key hash of the instance.
    pub instance_handle: Option<[u8; 16]>,
    /// `dds:partition` (DDS-AMQP 1.0 §8.3, a string or a list of strings);
    /// `None` when the message does not carry it.
    pub partitions: Option<Vec<String>>,
}

fn app_prop<'a>(props: &'a AmqpExtValue, key: &str) -> Option<&'a AmqpExtValue> {
    match props {
        AmqpExtValue::Map(entries) => entries.iter().find_map(|(k, v)| match k {
            AmqpExtValue::Str(s) | AmqpExtValue::Symbol(s) if s == key => Some(v),
            _ => None,
        }),
        _ => None,
    }
}

/// Decodes the sections of a message.
///
/// # Errors
/// A section the codec cannot read, a body that is neither data nor a
/// binary value, an unknown `dds:operation`.
pub fn decode_message(bytes: &[u8]) -> Result<InMessage, String> {
    let mut rest = bytes;
    let mut msg = InMessage {
        content_type: None,
        application_properties: AmqpExtValue::Map(Vec::new()),
        body: Vec::new(),
        operation: DdsOperation::Write,
        instance_handle: None,
        partitions: None,
    };
    let mut has_body = false;
    while !rest.is_empty() {
        let (section, n) =
            MessageSection::decode(rest).map_err(|e| format!("message section: {e:?}"))?;
        if n == 0 {
            break;
        }
        rest = &rest[n..];
        match section {
            MessageSection::Properties(AmqpExtValue::List(fields)) => {
                msg.content_type = match fields.get(6) {
                    Some(AmqpExtValue::Symbol(s) | AmqpExtValue::Str(s)) => Some(s.clone()),
                    _ => None,
                };
            }
            MessageSection::ApplicationProperties(v) => msg.application_properties = v,
            MessageSection::Data(b) => {
                msg.body.extend_from_slice(&b);
                has_body = true;
            }
            MessageSection::AmqpValue(AmqpExtValue::Binary(b)) => {
                msg.body = b;
                has_body = true;
            }
            MessageSection::AmqpValue(_) | MessageSection::AmqpSequence(_) => {
                return Err("an amqp-value or amqp-sequence body; the bridge reads Pass-Through data bodies".into());
            }
            _ => {}
        }
    }
    if !has_body {
        return Err("a message without a body".into());
    }
    if let Some(v) = app_prop(&msg.application_properties, app_keys::OPERATION) {
        let text = match v {
            AmqpExtValue::Str(s) | AmqpExtValue::Symbol(s) => s.as_str(),
            _ => return Err("dds:operation is not a string".into()),
        };
        msg.operation =
            DdsOperation::parse(text).map_err(|o| format!("unknown dds:operation {o}"))?;
    }
    if let Some(v) = app_prop(&msg.application_properties, app_keys::PARTITION) {
        let name = |v: &AmqpExtValue| match v {
            AmqpExtValue::Str(s) | AmqpExtValue::Symbol(s) => Ok(s.clone()),
            other => Err(format!("dds:partition holds {other:?}, not a string")),
        };
        msg.partitions = Some(match v {
            AmqpExtValue::List(items) | AmqpExtValue::Array(items) => {
                items.iter().map(name).collect::<Result<Vec<_>, _>>()?
            }
            single => vec![name(single)?],
        });
    }
    if let Some(AmqpExtValue::Binary(b)) =
        app_prop(&msg.application_properties, app_keys::INSTANCE_HANDLE)
        && b.len() == 16
    {
        let mut k = [0u8; 16];
        k.copy_from_slice(b);
        if k != [0; 16] {
            msg.instance_handle = Some(k);
        }
    }
    Ok(msg)
}

/// The representation and byte order an encapsulation header names, and
/// the payload behind it.
///
/// # Errors
/// A body shorter than the header, or a representation identifier the
/// bridge does not write (RTPS 2.5 §10.5).
pub fn split_encapsulation(body: &[u8]) -> Result<(u8, bool, &[u8]), String> {
    if body.len() < 4 {
        return Err("a body shorter than the encapsulation header".into());
    }
    let id = u16::from_be_bytes([body[0], body[1]]);
    let (rep, be) = match id {
        0x0000 => (0, true),
        0x0001 => (0, false),
        0x0002 => (0, true),  // PL_CDR_BE
        0x0003 => (0, false), // PL_CDR_LE
        0x0006 | 0x0008 | 0x000a => (1, true),
        0x0007 | 0x0009 | 0x000b => (1, false),
        other => {
            return Err(format!(
                "encapsulation {other:#06x} is not a CDR representation"
            ));
        }
    };
    Ok((rep, be, &body[4..]))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::bridge_daemon::test_support::topic;
    use crate::coexistence::{InboundDecision, inspect_inbound};

    /// A sample goes out as the §4.3 message and comes back as the same
    /// body, key and operation.
    #[test]
    fn amqpbridge_4_3_a_sample_round_trips_through_the_message_form() {
        let t = topic();
        let coex = CoexistenceConfig::new("11111111-2222-4333-8444-555555555555".into());
        let key = [9u8; 16];
        let m = encode_message(
            &t,
            3,
            &coex,
            &OutSample {
                payload: &[1, 2, 3, 4],
                representation_identifier: [0x00, 0x07],
                key_hash: Some(key),
                writer_guid: [7; 16],
                sequence: 42,
                source_timestamp_ns: Some(1_700_000_000_123_456_789),
                operation: DdsOperation::Write,
                lifespan_ms: None,
                partitions: &[],
            },
        )
        .unwrap();
        let d = decode_message(&m).unwrap();
        assert_eq!(d.content_type.as_deref(), Some(CONTENT_TYPE_XCDR2));
        assert_eq!(d.body, vec![0x00, 0x07, 0, 0, 1, 2, 3, 4]);
        assert_eq!(d.instance_handle, Some(key));
        assert_eq!(d.operation, DdsOperation::Write);
        assert_eq!(
            split_encapsulation(&d.body).unwrap(),
            (1, false, &[1u8, 2, 3, 4][..])
        );
        // This bridge's own stamp: a receiver of the same address drops it.
        assert_eq!(
            inspect_inbound(&coex, &d.application_properties),
            InboundDecision::DropLoop
        );
        let other = CoexistenceConfig::new("99999999-2222-4333-8444-555555555555".into());
        assert_eq!(
            inspect_inbound(&other, &d.application_properties),
            InboundDecision::Forward
        );
    }

    #[test]
    fn amqpbridge_4_3_an_xcdr1_sample_keeps_its_header_and_says_so() {
        let t = topic();
        let coex = CoexistenceConfig::new("x".into());
        let m = encode_message(
            &t,
            0,
            &coex,
            &OutSample {
                payload: &[5],
                representation_identifier: [0x00, 0x00],
                key_hash: None,
                writer_guid: [1; 16],
                sequence: 1,
                source_timestamp_ns: None,
                operation: DdsOperation::Write,
                lifespan_ms: None,
                partitions: &[],
            },
        )
        .unwrap();
        let d = decode_message(&m).unwrap();
        assert_eq!(d.content_type.as_deref(), Some(CONTENT_TYPE_XCDR1));
        assert_eq!(d.body[..2], [0x00, 0x00], "CDR_BE");
    }

    /// DDSI-RTPS 2.5 §10.5: a body passed on unchanged goes under the
    /// identifier it came with, delimited and parameter-list bodies
    /// included (DDS-XTypes 1.3 Table 60); before, the header named every
    /// XCDR2 body `CDR2` and every XCDR1 body `CDR`.
    #[test]
    fn rtps_10_5_a_sample_keeps_the_identifier_of_its_format() {
        let t = topic();
        let coex = CoexistenceConfig::new("x".into());
        for (identifier, content_type) in [
            ([0x00, 0x03], CONTENT_TYPE_XCDR1), // PL_CDR_LE
            ([0x00, 0x08], CONTENT_TYPE_XCDR2), // D_CDR2_BE
            ([0x00, 0x0b], CONTENT_TYPE_XCDR2), // PL_CDR2_LE
        ] {
            let m = encode_message(
                &t,
                0,
                &coex,
                &OutSample {
                    payload: &[5, 0, 0, 0],
                    representation_identifier: identifier,
                    key_hash: None,
                    writer_guid: [1; 16],
                    sequence: 1,
                    source_timestamp_ns: None,
                    operation: DdsOperation::Write,
                    lifespan_ms: None,
                    partitions: &[],
                },
            )
            .unwrap();
            let d = decode_message(&m).unwrap();
            assert_eq!(d.body[..2], identifier, "{identifier:02x?}");
            assert_eq!(d.content_type.as_deref(), Some(content_type));
        }
    }

    /// DDS-XTypes 1.3 Table 60 and Table 39: the format an identifier names
    /// is the extensibility the DDS writer frames the body with.
    #[test]
    fn xtypes_7_6_3_1_2_the_identifier_names_the_extensibility_of_the_body() {
        use zerodds_types::qos::ExtensibilityForRepr as Ext;
        for (identifier, ext) in [
            ([0x00, 0x00], Ext::Final),
            ([0x00, 0x01], Ext::Final),
            ([0x00, 0x02], Ext::Mutable),
            ([0x00, 0x03], Ext::Mutable),
            ([0x00, 0x06], Ext::Final),
            ([0x00, 0x07], Ext::Final),
            ([0x00, 0x08], Ext::Appendable),
            ([0x00, 0x09], Ext::Appendable),
            ([0x00, 0x0a], Ext::Mutable),
            ([0x00, 0x0b], Ext::Mutable),
            ([0x00, 0x10], Ext::Final),
            ([0x00, 0x13], Ext::Mutable),
            ([0x00, 0x15], Ext::Appendable),
        ] {
            assert_eq!(wire_extensibility_of(identifier), ext, "{identifier:02x?}");
        }
    }

    fn field_of(v: &AmqpExtValue, i: usize) -> &AmqpExtValue {
        match v {
            AmqpExtValue::List(f) => f.get(i).unwrap_or(&AmqpExtValue::Null),
            other => panic!("not a list: {other:?}"),
        }
    }

    fn sections(bytes: &[u8]) -> Vec<MessageSection> {
        let mut rest = bytes;
        let mut out = Vec::new();
        while !rest.is_empty() {
            let (s, n) = MessageSection::decode(rest).unwrap();
            out.push(s);
            rest = &rest[n..];
        }
        out
    }

    /// §4.3: header, properties and application properties of a sample's
    /// message, field by field: durable for a non-VOLATILE topic, priority
    /// 4, ttl and absolute-expiry-time from LIFESPAN, first-acquirer,
    /// message-id = writer GUID || sequence number (big-endian), to,
    /// subject, content-type, creation-time = source timestamp in ms,
    /// group-id for a keyed topic only, `dds:domain-id`, `dds:nsec`,
    /// `dds:instance-handle`, no `dds:operation` for a write, `dds:operation`
    /// for a dispose.
    #[test]
    fn amqpbridge_4_3_the_header_and_properties_are_those_of_the_spec() {
        let mut t = topic();
        t.durability = zerodds_qos::DurabilityKind::TransientLocal;
        let coex = CoexistenceConfig::new("11111111-2222-4333-8444-555555555555".into());
        let mut guid = [0u8; 16];
        for (i, b) in guid.iter_mut().enumerate() {
            *b = i as u8 + 1;
        }
        let sample = OutSample {
            payload: &[1, 2],
            representation_identifier: [0x00, 0x07],
            key_hash: Some([5; 16]),
            writer_guid: guid,
            sequence: 0x0102_0304,
            source_timestamp_ns: Some(1_700_000_000_123_456_789),
            operation: DdsOperation::Write,
            lifespan_ms: Some(1500),
            partitions: &[],
        };
        let m = encode_message(&t, 7, &coex, &sample).unwrap();
        let s = sections(&m);
        let MessageSection::Header(h) = &s[0] else {
            panic!("{:?}", s[0])
        };
        assert_eq!(field_of(h, 0), &AmqpExtValue::Boolean(true), "durable");
        assert_eq!(field_of(h, 1), &AmqpExtValue::Ubyte(4), "priority");
        assert_eq!(field_of(h, 2), &AmqpExtValue::Uint(1500), "ttl");
        assert_eq!(
            field_of(h, 3),
            &AmqpExtValue::Boolean(true),
            "first-acquirer"
        );
        let MessageSection::Properties(p) = &s[1] else {
            panic!("{:?}", s[1])
        };
        let mut id = guid.to_vec();
        id.extend_from_slice(&0x0102_0304u64.to_be_bytes());
        assert_eq!(field_of(p, 0), &AmqpExtValue::Binary(id), "message-id");
        assert_eq!(field_of(p, 2), &AmqpExtValue::Str("/queues/t".into()), "to");
        assert_eq!(field_of(p, 3), &AmqpExtValue::Str("T".into()), "subject");
        assert_eq!(
            field_of(p, 6),
            &AmqpExtValue::Symbol(CONTENT_TYPE_XCDR2.into()),
            "content-type"
        );
        assert_eq!(
            field_of(p, 9),
            &AmqpExtValue::Timestamp(1_700_000_000_123),
            "creation-time"
        );
        assert_eq!(
            field_of(p, 8),
            &AmqpExtValue::Timestamp(1_700_000_000_123 + 1500),
            "absolute-expiry-time"
        );
        assert!(
            matches!(field_of(p, 10), AmqpExtValue::Str(g) if !g.is_empty()),
            "group-id"
        );
        let MessageSection::ApplicationProperties(a) = &s[2] else {
            panic!("{:?}", s[2])
        };
        assert_eq!(
            app_prop(a, app_keys::DOMAIN_ID),
            Some(&AmqpExtValue::Uint(7))
        );
        assert_eq!(
            app_prop(a, app_keys::NSEC),
            Some(&AmqpExtValue::Uint(456_789))
        );
        assert_eq!(
            app_prop(a, app_keys::INSTANCE_HANDLE),
            Some(&AmqpExtValue::Binary(vec![5; 16]))
        );
        assert_eq!(
            app_prop(a, app_keys::OPERATION),
            None,
            "write has no dds:operation"
        );
        assert!(app_prop(a, "dds:bridge-id").is_some());
        assert!(matches!(&s[3], MessageSection::Data(b) if b[..] == [0x00, 0x07, 0, 0, 1, 2]));

        // VOLATILE, unkeyed, no LIFESPAN, a dispose.
        let mut plain = topic();
        plain.keyed = false;
        let m = encode_message(
            &plain,
            0,
            &coex,
            &OutSample {
                key_hash: None,
                lifespan_ms: None,
                partitions: &[],
                operation: DdsOperation::Dispose,
                ..sample
            },
        )
        .unwrap();
        let s = sections(&m);
        let MessageSection::Header(h) = &s[0] else {
            panic!()
        };
        assert_eq!(field_of(h, 0), &AmqpExtValue::Boolean(false), "VOLATILE");
        assert_eq!(
            field_of(h, 2),
            &AmqpExtValue::Null,
            "infinite LIFESPAN: no ttl"
        );
        let MessageSection::Properties(p) = &s[1] else {
            panic!()
        };
        assert_eq!(field_of(p, 8), &AmqpExtValue::Null);
        assert_eq!(field_of(p, 10), &AmqpExtValue::Null, "unkeyed: no group-id");
        let MessageSection::ApplicationProperties(a) = &s[2] else {
            panic!()
        };
        assert_eq!(
            app_prop(a, app_keys::OPERATION),
            Some(&AmqpExtValue::Str("dispose".into()))
        );
    }
}
