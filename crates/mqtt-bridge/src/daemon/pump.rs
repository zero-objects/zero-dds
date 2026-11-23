// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The mapping between a DDS sample and an MQTT PUBLISH — Spec §4.2.
//!
//! DDS → MQTT: the payload is the encapsulation header the sample came
//! under and its CDR bytes; the properties name the format and the sample
//! ([`publish_properties`]). MQTT → DDS: the payload is read as such a
//! header and CDR bytes, the properties that carry a source timestamp or a
//! key hash go to the DDS write ([`inbound_sample`]).
//!
//! The dispose and the unregister of an instance (Spec §4.4) are a PUBLISH
//! with the user property `zerodds_op` ([`lifecycle_properties`]); a
//! PUBLISH of the broker with it is written as that change
//! ([`inbound_message`]).
//!
//! The DDS pump of a topic with a DataReader (`Pump`, one thread per topic):
//! its samples wait in the topic's history (§6), go out one at a time while
//! the broker connection stands (§9.3), pass the ACL (§7.3) and go into the
//! replay buffer once sent (§6). Under MQTT 3.1.1 the `EchoGuard` keeps the
//! broker's copy of the daemon's own PUBLISH on a bidir topic out of DDS
//! (§4.3).

use std::collections::BTreeMap;
use std::string::{String, ToString};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use zerodds_dcps::runtime::UserSample;
use zerodds_rtps::header_extension::HeTimestamp;

use super::acl::LiveAcl;
use super::client::{ClientError, MqttClient};
use super::config::TopicConfig;
use super::history::HistoryQueue;
use super::log::{self, Level};
use super::metrics::{BridgeMetrics, acl_reason};
use super::replay::ReplayBuffer;
use crate::property_set::PropertySet;
use crate::version::ProtocolVersion;

/// The Content Type of an XCDR1 payload (`CDR`, `PL_CDR`; DDS-XTypes 1.3
/// Table 60).
pub const CONTENT_TYPE_CDR: &str = "application/x-dds-cdr";
/// The Content Type of an XCDR2 payload (`CDR2`, `D_CDR2`, `PL_CDR2`).
pub const CONTENT_TYPE_CDR2: &str = "application/x-dds-cdr2";

/// User property: the DDS type name of the topic.
pub const PROP_TYPE: &str = "zerodds_type";
/// User property: the DDS topic name.
pub const PROP_TOPIC: &str = "zerodds_topic";
/// User property: the status flags of the change, `0x` and two hex digits.
pub const PROP_FLAGS: &str = "zerodds_flags";
/// User property: the instance's key hash, 32 hex digits.
pub const PROP_KEY_HASH: &str = "zerodds_key_hash";
/// User property: the source timestamp, nanoseconds since the Unix epoch.
pub const PROP_SOURCE_TS: &str = "zerodds_source_ts_ns";
/// User property: the operation of a PUBLISH (Spec §4.4).
pub const PROP_OP: &str = "zerodds_op";

/// The `zerodds_flags` of a dispose (PID_STATUS_INFO, DDSI-RTPS 2.5
/// §9.6.3.9).
pub const FLAG_DISPOSED: u8 = 0x01;
/// The `zerodds_flags` of an unregister.
pub const FLAG_UNREGISTERED: u8 = 0x02;

/// The encapsulation header in front of the key holder of a lifecycle
/// PUBLISH: `CDR2_BE`, the big-endian PLAIN_CDR2 of the key holder of
/// DDS-XTypes 1.3 §7.6.8.
pub const LIFECYCLE_ENCAPSULATION: [u8; 4] = [0x00, 0x06, 0x00, 0x00];

/// The values of `zerodds_op` (Spec §4.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    /// `sample`, the default: a sample with data.
    Sample,
    /// `dispose`: the instance is disposed.
    Dispose,
    /// `unregister`: the writer unregisters the instance.
    Unregister,
    /// `register`: the instance is registered; DDS sends no message for it
    /// (DDS 1.4 §2.2.2.4.2.5).
    Register,
}

impl Op {
    /// Its value of `zerodds_op`.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Sample => "sample",
            Self::Dispose => "dispose",
            Self::Unregister => "unregister",
            Self::Register => "register",
        }
    }

    /// The operation `text` names.
    ///
    /// # Errors
    /// A value that is none of the four.
    pub fn parse(text: &str) -> Result<Self, String> {
        match text {
            "sample" => Ok(Self::Sample),
            "dispose" => Ok(Self::Dispose),
            "unregister" => Ok(Self::Unregister),
            "register" => Ok(Self::Register),
            other => Err(format!(
                "{PROP_OP} `{other}` is not sample, dispose, unregister or register"
            )),
        }
    }
}

/// The operation and the `zerodds_flags` of a DDS lifecycle change (Spec
/// §4.4): a dispose is `dispose` 0x01, an unregister `unregister` 0x02, the
/// two together `dispose` 0x03; `None` for a kind that is no lifecycle
/// change.
#[must_use]
pub fn op_of(kind: zerodds_rtps::history_cache::ChangeKind) -> Option<(Op, u8)> {
    use zerodds_rtps::history_cache::ChangeKind as K;
    match kind {
        K::NotAliveDisposed => Some((Op::Dispose, FLAG_DISPOSED)),
        K::NotAliveUnregistered => Some((Op::Unregister, FLAG_UNREGISTERED)),
        K::NotAliveDisposedUnregistered => Some((Op::Dispose, FLAG_DISPOSED | FLAG_UNREGISTERED)),
        K::Alive | K::AliveFiltered => None,
    }
}

/// What of a DDS sample a PUBLISH carries besides its CDR bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SampleMeta {
    /// The representation identifier of its encapsulation header.
    pub representation_identifier: [u8; 2],
    /// The instance's key hash, when the sample has one.
    pub key_hash: Option<[u8; 16]>,
    /// The writer's source timestamp, when the sample carries one.
    pub source_timestamp: Option<HeTimestamp>,
    /// The writer's LIFESPAN in nanoseconds, 0 for infinite.
    pub lifespan_nanos: u64,
    /// The status flags of the change as PID_STATUS_INFO carries them
    /// (DDSI-RTPS 2.5 §9.6.3.9): 0 for a sample, 0x01 disposed, 0x02
    /// unregistered.
    pub status_flags: u8,
}

/// The encapsulation header of a serialized payload (DDSI-RTPS 2.5 §10.5,
/// DDS-XTypes 1.3 §7.6.3.1.2): the representation identifier the sample came
/// under, and options. The CDR bytes are passed on unchanged, so the
/// identifier is too; it names the XCDR version, the format (plain,
/// delimited, parameter list) and the byte order. This is what
/// zerodds-mqtt-bridge-1.0 §4.2 puts in front of the CDR bytes of a PUBLISH.
#[must_use]
pub fn encapsulation(representation_identifier: [u8; 2]) -> [u8; 4] {
    [
        representation_identifier[0],
        representation_identifier[1],
        0,
        0,
    ]
}

/// Whether `identifier` is an XCDR1 representation (`CDR_BE`, `CDR_LE`,
/// `PL_CDR_BE`, `PL_CDR_LE`; DDS-XTypes 1.3 Table 60).
#[must_use]
pub fn is_xcdr1(identifier: [u8; 2]) -> bool {
    identifier[0] == 0 && identifier[1] <= 0x03
}

/// The Content Type of a payload under `identifier` (Spec §4.2).
#[must_use]
pub fn content_type_of(identifier: [u8; 2]) -> &'static str {
    if is_xcdr1(identifier) {
        CONTENT_TYPE_CDR
    } else {
        CONTENT_TYPE_CDR2
    }
}

/// The representation identifier and the CDR bytes of a PUBLISH payload
/// (zerodds-mqtt-bridge-1.0 §4.2: the encapsulation header, then the CDR
/// bytes).
///
/// # Errors
/// A payload shorter than the header, or an identifier that is not a CDR
/// representation (DDSI-RTPS 2.5 §10.5, DDS-XTypes 1.3 Table 60).
pub fn split_encapsulation(payload: &[u8]) -> Result<([u8; 2], &[u8]), String> {
    if payload.len() < 4 {
        return Err("a payload shorter than the encapsulation header".into());
    }
    let identifier = [payload[0], payload[1]];
    match u16::from_be_bytes(identifier) {
        0x0000..=0x0003 | 0x0006..=0x000b => Ok((identifier, &payload[4..])),
        other => Err(format!(
            "encapsulation {other:#06x} is not a CDR representation"
        )),
    }
}

/// A source timestamp in nanoseconds since the Unix epoch (`seconds` and
/// `fraction` in units of 2⁻³² s, DDSI-RTPS 2.5 §9.3.2.2).
#[must_use]
pub fn timestamp_ns(ts: HeTimestamp) -> u64 {
    u64::from(ts.seconds) * 1_000_000_000 + ((u64::from(ts.fraction) * 1_000_000_000) >> 32)
}

/// The source timestamp of `ns` nanoseconds since the Unix epoch; `None`
/// beyond the seconds an RTPS time holds.
#[must_use]
pub fn timestamp_of_ns(ns: u64) -> Option<HeTimestamp> {
    let seconds = u32::try_from(ns / 1_000_000_000).ok()?;
    // Rounded up, so the conversion back gives `ns` again.
    let fraction = ((ns % 1_000_000_000) << 32).div_ceil(1_000_000_000);
    Some(HeTimestamp {
        seconds,
        fraction: u32::try_from(fraction).ok()?,
    })
}

/// A Message Expiry Interval of `lifespan_nanos` (Spec §4.2, §6): seconds,
/// rounded, at least 1, at most `u32::MAX`; `None` for an infinite
/// lifespan (0).
#[must_use]
pub fn message_expiry_of(lifespan_nanos: u64) -> Option<u32> {
    if lifespan_nanos == 0 {
        return None;
    }
    // Rounded without the sum `lifespan_nanos + 0.5 s`, which overflows
    // within half a second of `u64::MAX`.
    let secs =
        lifespan_nanos / 1_000_000_000 + u64::from(lifespan_nanos % 1_000_000_000 >= 500_000_000);
    Some(u32::try_from(secs.max(1)).unwrap_or(u32::MAX))
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(out, "{b:02x}");
    }
    out
}

/// The properties of the PUBLISH of a sample of `topic` (Spec §4.2):
/// Payload Format Indicator 0, the Content Type of its representation, the
/// user properties `zerodds_type`, `zerodds_topic`, `zerodds_flags`, the key
/// hash and the source timestamp when the sample has them, and the Message
/// Expiry Interval of its LIFESPAN.
#[must_use]
pub fn publish_properties(topic: &TopicConfig, sample: &SampleMeta) -> PropertySet {
    let mut user = vec![
        (PROP_TYPE.to_string(), topic.dds_type.clone()),
        (PROP_TOPIC.to_string(), topic.dds_name.clone()),
        (
            PROP_FLAGS.to_string(),
            format!("0x{:02x}", sample.status_flags),
        ),
    ];
    if let Some(k) = sample.key_hash {
        user.push((PROP_KEY_HASH.to_string(), hex(&k)));
    }
    if let Some(ts) = sample.source_timestamp {
        user.push((PROP_SOURCE_TS.to_string(), timestamp_ns(ts).to_string()));
    }
    PropertySet {
        payload_format_indicator: Some(0),
        message_expiry_interval: message_expiry_of(sample.lifespan_nanos),
        content_type: Some(content_type_of(sample.representation_identifier).to_string()),
        user_properties: user,
        ..PropertySet::default()
    }
}

/// The payload of the PUBLISH of a lifecycle change (Spec §4.4): the header
/// [`LIFECYCLE_ENCAPSULATION`], then the instance's key holder when the
/// DataReader recovered one. Never empty, so it does not clear the retained
/// message of its topic (MQTT 5.0 §3.3.1.3).
#[must_use]
pub fn lifecycle_message(key_holder: Option<&[u8]>) -> Vec<u8> {
    let holder = key_holder.unwrap_or_default();
    let mut out = Vec::with_capacity(4 + holder.len());
    out.extend_from_slice(&LIFECYCLE_ENCAPSULATION);
    out.extend_from_slice(holder);
    out
}

/// The properties of the PUBLISH of a lifecycle change of an instance of
/// `topic` (Spec §4.4): Payload Format Indicator 0, the Content Type of
/// XCDR2, `zerodds_type`, `zerodds_topic`, `zerodds_op`, `zerodds_flags`,
/// `zerodds_key_hash`, and the source timestamp when the change has one.
#[must_use]
pub fn lifecycle_properties(
    topic: &TopicConfig,
    op: Op,
    flags: u8,
    key_hash: [u8; 16],
    source_timestamp: Option<HeTimestamp>,
) -> PropertySet {
    let mut user = vec![
        (PROP_TYPE.to_string(), topic.dds_type.clone()),
        (PROP_TOPIC.to_string(), topic.dds_name.clone()),
        (PROP_OP.to_string(), op.as_str().to_string()),
        (PROP_FLAGS.to_string(), format!("0x{flags:02x}")),
        (PROP_KEY_HASH.to_string(), hex(&key_hash)),
    ];
    if let Some(ts) = source_timestamp {
        user.push((PROP_SOURCE_TS.to_string(), timestamp_ns(ts).to_string()));
    }
    PropertySet {
        payload_format_indicator: Some(0),
        content_type: Some(CONTENT_TYPE_CDR2.to_string()),
        user_properties: user,
        ..PropertySet::default()
    }
}

/// What a PUBLISH of the broker is in DDS (Spec §4.2, §4.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InboundMessage<'a> {
    /// A sample with data (`zerodds_op` `sample` or none).
    Sample(InboundSample<'a>),
    /// A dispose or an unregister of an instance.
    Lifecycle {
        /// The instance (`zerodds_key_hash`).
        key_hash: [u8; 16],
        /// Its key holder behind the header, when the payload carries one.
        key_holder: Option<&'a [u8]>,
        /// PID_STATUS_INFO: `DISPOSED`, `UNREGISTERED` or both.
        status_bits: u32,
        /// `zerodds_source_ts_ns`, when present.
        source_timestamp: Option<HeTimestamp>,
    },
    /// `register`: DDS sends no message for it, nothing is written.
    Register,
}

/// A PUBLISH of the broker as what it is in DDS (Spec §4.4): `zerodds_op`
/// absent or `sample` is a sample ([`inbound_sample`]); `dispose` and
/// `unregister` name their instance by `zerodds_key_hash` and carry the key
/// holder behind [`LIFECYCLE_ENCAPSULATION`] or nothing; `register` writes
/// nothing. `zerodds_flags`, when present, agrees with the operation:
/// 0x00 for a sample, 0x01 or 0x03 for a dispose (0x03: dispose and
/// unregister), 0x02 for an unregister.
///
/// # Errors
/// An unknown operation, flags that contradict it, a lifecycle change
/// without key hash or with a payload that is no key holder, and what
/// [`inbound_sample`] refuses.
pub fn inbound_message<'a>(
    payload: &'a [u8],
    props: &PropertySet,
) -> Result<InboundMessage<'a>, String> {
    use zerodds_rtps::inline_qos::status_info::{DISPOSED, UNREGISTERED};
    let op = match user_property(props, PROP_OP) {
        None => Op::Sample,
        Some(text) => Op::parse(text)?,
    };
    let flags = match user_property(props, PROP_FLAGS) {
        None => None,
        Some(text) => Some(
            text.strip_prefix("0x")
                .filter(|h| h.len() == 2)
                .and_then(|h| u8::from_str_radix(h, 16).ok())
                .ok_or_else(|| format!("{PROP_FLAGS} `{text}` is not 0x and two hex digits"))?,
        ),
    };
    let allowed: &[u8] = match op {
        Op::Sample | Op::Register => &[0x00],
        Op::Dispose => &[FLAG_DISPOSED, FLAG_DISPOSED | FLAG_UNREGISTERED],
        Op::Unregister => &[FLAG_UNREGISTERED],
    };
    if let Some(f) = flags {
        if !allowed.contains(&f) {
            return Err(format!(
                "{PROP_FLAGS} 0x{f:02x} contradicts {PROP_OP} {}",
                op.as_str()
            ));
        }
    }
    match op {
        Op::Sample => inbound_sample(payload, props).map(InboundMessage::Sample),
        Op::Register => Ok(InboundMessage::Register),
        Op::Dispose | Op::Unregister => {
            if props.payload_format_indicator == Some(1) {
                return Err("Payload Format Indicator 1 (UTF-8 text) is no key holder".into());
            }
            if let Some(ct) = &props.content_type {
                if ct != CONTENT_TYPE_CDR2 {
                    return Err(format!(
                        "Content Type `{ct}` of a {} is not {CONTENT_TYPE_CDR2}",
                        op.as_str()
                    ));
                }
            }
            let key_hash = match user_property(props, PROP_KEY_HASH) {
                None => {
                    return Err(format!(
                        "a {} without {PROP_KEY_HASH} names no instance",
                        op.as_str()
                    ));
                }
                Some(text) => parse_key_hash(text)
                    .ok_or_else(|| format!("{PROP_KEY_HASH} `{text}` is not 32 hex digits"))?,
            };
            let key_holder = if payload.is_empty() {
                None
            } else {
                let (identifier, body) = split_encapsulation(payload)?;
                if identifier != [LIFECYCLE_ENCAPSULATION[0], LIFECYCLE_ENCAPSULATION[1]] {
                    return Err(format!(
                        "the key holder of a {} is big-endian PLAIN_CDR2 (0x0006), not {:#06x}",
                        op.as_str(),
                        u16::from_be_bytes(identifier)
                    ));
                }
                (!body.is_empty()).then_some(body)
            };
            let source_timestamp = match user_property(props, PROP_SOURCE_TS) {
                None => None,
                Some(text) => Some(
                    text.parse::<u64>()
                        .ok()
                        .and_then(timestamp_of_ns)
                        .ok_or_else(|| {
                            format!("{PROP_SOURCE_TS} `{text}` is not a time in nanoseconds")
                        })?,
                ),
            };
            let status_bits = match (op, flags) {
                (Op::Unregister, _) => UNREGISTERED,
                (_, Some(f)) if f == FLAG_DISPOSED | FLAG_UNREGISTERED => DISPOSED | UNREGISTERED,
                _ => DISPOSED,
            };
            Ok(InboundMessage::Lifecycle {
                key_hash,
                key_holder,
                status_bits,
                source_timestamp,
            })
        }
    }
}

/// What a PUBLISH of the broker gives the DDS write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InboundSample<'a> {
    /// The representation identifier of its encapsulation header.
    pub identifier: [u8; 2],
    /// The CDR bytes behind the header.
    pub body: &'a [u8],
    /// `zerodds_key_hash`, when present.
    pub key_hash: Option<[u8; 16]>,
    /// `zerodds_source_ts_ns`, when present.
    pub source_timestamp: Option<HeTimestamp>,
}

/// The value of the user property `name`, the last one when it is there
/// more than once.
fn user_property<'p>(props: &'p PropertySet, name: &str) -> Option<&'p str> {
    props
        .user_properties
        .iter()
        .rev()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.as_str())
}

/// The DDS sample of a broker's PUBLISH (Spec §4.2 MQTT → DDS): the payload
/// is an encapsulation header and CDR bytes; `zerodds_key_hash` and
/// `zerodds_source_ts_ns` go to the write.
///
/// # Errors
/// What makes the PUBLISH no DDS sample: a Payload Format Indicator 1 (the
/// payload is UTF-8 text, MQTT 5.0 §3.3.2.3.2), a Content Type other than
/// the two of §4.2 or one that names the other XCDR version than the
/// header, a payload that is no encapsulation header and CDR bytes, a key
/// hash that is not 32 hex digits, a source timestamp that is not a
/// number of nanoseconds an RTPS time holds.
pub fn inbound_sample<'a>(
    payload: &'a [u8],
    props: &PropertySet,
) -> Result<InboundSample<'a>, String> {
    if props.payload_format_indicator == Some(1) {
        return Err("Payload Format Indicator 1 (UTF-8 text) is no CDR payload".into());
    }
    let (identifier, body) = split_encapsulation(payload)?;
    if let Some(ct) = &props.content_type {
        let want = content_type_of(identifier);
        if ct != CONTENT_TYPE_CDR && ct != CONTENT_TYPE_CDR2 {
            return Err(format!(
                "Content Type `{ct}` is not {CONTENT_TYPE_CDR} or {CONTENT_TYPE_CDR2}"
            ));
        }
        if ct != want {
            return Err(format!(
                "Content Type `{ct}` with the encapsulation {:#06x}, which is {want}",
                u16::from_be_bytes(identifier)
            ));
        }
    }
    let key_hash = match user_property(props, PROP_KEY_HASH) {
        None => None,
        Some(text) => Some(
            parse_key_hash(text)
                .ok_or_else(|| format!("{PROP_KEY_HASH} `{text}` is not 32 hex digits"))?,
        ),
    };
    let source_timestamp = match user_property(props, PROP_SOURCE_TS) {
        None => None,
        Some(text) => Some(
            text.parse::<u64>()
                .ok()
                .and_then(timestamp_of_ns)
                .ok_or_else(|| format!("{PROP_SOURCE_TS} `{text}` is not a time in nanoseconds"))?,
        ),
    };
    Ok(InboundSample {
        identifier,
        body,
        key_hash,
        source_timestamp,
    })
}

fn parse_key_hash(text: &str) -> Option<[u8; 16]> {
    if text.len() != 32 || !text.is_ascii() {
        return None;
    }
    let mut out = [0u8; 16];
    for (i, b) in out.iter_mut().enumerate() {
        *b = u8::from_str_radix(&text[2 * i..2 * i + 2], 16).ok()?;
    }
    Some(out)
}

/// The data representation an identifier names (DDS-XTypes 1.3 Table 60):
/// `CDR`, `PL_CDR` are XCDR1, the others XCDR2.
#[must_use]
pub fn offer_of(identifier: [u8; 2]) -> i16 {
    use zerodds_rtps::publication_data::data_representation as dr;
    if is_xcdr1(identifier) {
        dr::XCDR
    } else {
        dr::XCDR2
    }
}

/// The extensibility the DDS writer frames a body under `identifier` with
/// (DDS-XTypes 1.3 Table 60): a parameter list (`PL_CDR`, `PL_CDR2`) is
/// `@mutable`, a delimited body (`D_CDR2`) `@appendable`, anything else
/// `@final`.
#[must_use]
pub fn wire_extensibility_of(identifier: [u8; 2]) -> zerodds_types::qos::ExtensibilityForRepr {
    use zerodds_types::qos::ExtensibilityForRepr as Ext;
    match identifier[1] & !0x01 {
        0x02 | 0x0a => Ext::Mutable,
        0x08 => Ext::Appendable,
        _ => Ext::Final,
    }
}

/// How long the echo guard waits for the broker's copy of a PUBLISH.
const ECHO_WINDOW: Duration = Duration::from_secs(30);

/// Spec §4.3 under MQTT 3.1.1, which has no No Local: the bridge's PUBLISH
/// packets on bidir topics, by topic and a hash of the payload, whose copy
/// from the broker has not come. The copy is recognised and not written
/// into DDS a second time; a record waits [`ECHO_WINDOW`] at most.
#[derive(Default)]
pub(super) struct EchoGuard {
    sent: Mutex<BTreeMap<String, std::collections::VecDeque<(u64, std::time::Instant)>>>,
}

impl EchoGuard {
    fn hash(payload: &[u8]) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        payload.hash(&mut h);
        h.finish()
    }

    /// The bridge publishes `payload` on `topic`: noted before the PUBLISH
    /// goes out, so that the broker's copy finds the record.
    pub(super) fn sent(&self, topic: &str, payload: &[u8]) {
        if let Ok(mut sent) = self.sent.lock() {
            let q = sent.entry(topic.to_string()).or_default();
            let now = std::time::Instant::now();
            while q
                .front()
                .is_some_and(|(_, t)| now.duration_since(*t) > ECHO_WINDOW)
            {
                q.pop_front();
            }
            q.push_back((Self::hash(payload), now));
        }
    }

    /// The PUBLISH of `payload` on `topic` noted by [`Self::sent`] did not
    /// go out; its record (the newest of that payload) is taken out.
    pub(super) fn unsent(&self, topic: &str, payload: &[u8]) {
        let Ok(mut sent) = self.sent.lock() else {
            return;
        };
        let Some(q) = sent.get_mut(topic) else {
            return;
        };
        let h = Self::hash(payload);
        if let Some(i) = q.iter().rposition(|(x, _)| *x == h) {
            q.remove(i);
        }
        if q.is_empty() {
            sent.remove(topic);
        }
    }

    /// Whether a PUBLISH of the broker on `topic` with `payload` is the copy
    /// of one the bridge sent; its record is taken out.
    pub(super) fn take(&self, topic: &str, payload: &[u8]) -> bool {
        let Ok(mut sent) = self.sent.lock() else {
            return false;
        };
        let Some(q) = sent.get_mut(topic) else {
            return false;
        };
        let now = std::time::Instant::now();
        while q
            .front()
            .is_some_and(|(_, t)| now.duration_since(*t) > ECHO_WINDOW)
        {
            q.pop_front();
        }
        let h = Self::hash(payload);
        let found = q.iter().position(|(x, _)| *x == h);
        if let Some(i) = found {
            q.remove(i);
        }
        if q.is_empty() {
            sent.remove(topic);
        }
        found.is_some()
    }
}

/// A DDS change of a topic as the PUBLISH it becomes (Spec §4.2, §4.4).
struct Outgoing {
    /// The encapsulation header and the CDR bytes (a sample), or the
    /// header and the key holder (a dispose or an unregister).
    message: Vec<u8>,
    props: crate::property_set::PropertySet,
    kind: OutKind,
    /// When the Message Expiry Interval of a sample ends (§6): its arrival
    /// plus its lifespan; `None` for an infinite lifespan.
    expires: Option<std::time::Instant>,
    /// When the pump took the change from the DataReader: `latency_us` of
    /// `publish_out` counts from here (§8.1).
    arrived: std::time::Instant,
}

/// What an [`Outgoing`] is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OutKind {
    /// A sample (§4.2); once it went out it is the newest sample of its
    /// instance in the replay buffer (§6).
    Sample,
    /// A dispose or an unregister (§4.4); once it went out its instance
    /// leaves the replay buffer.
    Lifecycle,
    /// A dispose or an unregister under MQTT 3.1.1, whose PUBLISH carries
    /// no `zerodds_op` (§4.4): it is not published; in its turn in the
    /// history its instance leaves the replay buffer.
    NotPublished,
}

/// The pump of one topic with a DataReader (`out`, `bidir`): its DDS
/// samples wait in a [`HistoryQueue`] by the topic's `qos.history` (§6) and
/// go out one at a time, the oldest first.
pub(super) struct Pump {
    /// The GUID of the bridge's own writer of the topic, whose samples are
    /// not published (§4.3).
    pub(super) own_writer: Option<[u8; 16]>,
    /// Under MQTT 3.1.1 on a bidir topic: where each PUBLISH is noted, so
    /// that its copy from the broker is not written into DDS (§4.3).
    pub(super) echo: Option<Arc<EchoGuard>>,
    /// The protocol of the connection: under MQTT 3.1.1 a dispose or an
    /// unregister is not published (§4.4).
    pub(super) version: ProtocolVersion,
    pub(super) topic: TopicConfig,
    pub(super) rx: std::sync::mpsc::Receiver<UserSample>,
    pub(super) client: Arc<Mutex<MqttClient>>,
    pub(super) metrics: BridgeMetrics,
    /// The ACL and the subject of the connection (§7.3).
    pub(super) acl: Arc<LiveAcl>,
    /// The tenant's name, empty for the top level (§8.1 `tenant`).
    pub(super) tenant: String,
    pub(super) stop: Arc<AtomicBool>,
    /// False while the broker connection is lost (§9.3): the samples wait
    /// in the history.
    pub(super) connected: Arc<AtomicBool>,
    /// The replay buffer of the transient and persistent topics (§6).
    pub(super) replay: Arc<Mutex<ReplayBuffer>>,
    /// Whether a lifecycle change not published under MQTT 3.1.1 was
    /// logged; the further ones are counted.
    pub(super) lifecycle_logged: std::cell::Cell<bool>,
    /// Whether an expired sample was logged; the further ones are counted.
    pub(super) expired_logged: std::cell::Cell<bool>,
    /// Whether a sample the ACL refused was logged; the further ones are
    /// counted.
    pub(super) refused_logged: std::cell::Cell<bool>,
}

impl Pump {
    pub(super) fn run(self) {
        let (mqtt_qos, retain) = (
            self.topic.effective_mqtt_qos(),
            self.topic.effective_retain(),
        );
        let mut queue: HistoryQueue<Outgoing> = HistoryQueue::new(self.topic.history_depth());
        let mut reader_gone = false;
        while !self.stop.load(Ordering::SeqCst) {
            // Wait for a sample only while none waits; then take every one
            // that has arrived, so the history decides what waits.
            if queue.is_empty() {
                if reader_gone {
                    break;
                }
                match self.rx.recv_timeout(Duration::from_millis(200)) {
                    Ok(sample) => self.take(sample, &mut queue),
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                }
            }
            loop {
                match self.rx.try_recv() {
                    Ok(sample) => self.take(sample, &mut queue),
                    Err(std::sync::mpsc::TryRecvError::Empty) => break,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        reader_gone = true;
                        break;
                    }
                }
            }
            // §9.3: while the connection is lost the samples wait, by the
            // history of the topic.
            if !self.connected.load(Ordering::SeqCst) {
                thread::sleep(Duration::from_millis(50));
                continue;
            }
            let Some((instance, out)) = queue.pop() else {
                continue;
            };
            if out.kind == OutKind::NotPublished {
                self.kept(instance, None);
                continue;
            }
            let topic = self.topic.mqtt_topic.as_str();
            let sent = match self.client.lock() {
                Ok(mut c) => {
                    // §4.3: the record is made before the PUBLISH, under the
                    // client's lock, so the broker's copy cannot come first.
                    if let Some(echo) = &self.echo {
                        echo.sent(topic, &out.message);
                    }
                    let sent = c.publish_until(
                        topic,
                        &out.message,
                        mqtt_qos,
                        retain,
                        &out.props,
                        out.expires,
                    );
                    if sent.is_err() {
                        if let Some(echo) = &self.echo {
                            echo.unsent(topic, &out.message);
                        }
                    } else if c.tracing() {
                        // §8.3: the span of the roundtrip, from the sample.
                        c.trace_publish(super::trace::PublishOut {
                            start_ns: super::trace::unix_ns_since(out.arrived),
                            mqtt_topic: topic.to_string(),
                            dds_topic: self.topic.dds_name.clone(),
                            qos: mqtt_qos,
                            bytes: out.message.len(),
                            packet_id: None,
                        });
                    }
                    sent
                }
                Err(_) => break,
            };
            match sent {
                Ok(expiry) => {
                    // §6: the replay buffer holds what went out.
                    match out.kind {
                        OutKind::Sample => {
                            let mut props = out.props.clone();
                            props.message_expiry_interval = expiry;
                            self.kept(instance, Some((&out.message, &props)));
                        }
                        OutKind::Lifecycle => self.kept(instance, None),
                        OutKind::NotPublished => {}
                    }
                    self.metrics
                        .publish_out(&self.topic.dds_name, mqtt_qos, out.message.len());
                    if log::enabled(Level::Debug) {
                        log::event(Level::Debug, "publish_out")
                            .tenant(&self.tenant)
                            .field("mqtt_topic", topic)
                            .field("dds_topic", &self.topic.dds_name)
                            .number("bytes", out.message.len() as u64)
                            .number("mqtt_qos", u64::from(mqtt_qos))
                            .latency(out.arrived)
                            .emit();
                    }
                }
                Err(ClientError::Expired(why)) => {
                    // §6: the lifespan of the sample passed while it waited.
                    self.metrics.dds_dropped(&self.topic.dds_name);
                    if !self.expired_logged.replace(true) {
                        log::event(Level::Warn, "sample_expired")
                            .tenant(&self.tenant)
                            .field("dds_topic", &self.topic.dds_name)
                            .field("mqtt_topic", topic)
                            .field(
                                "reason",
                                &format!("{why}; not published, further ones are counted in dds_samples_dropped_total"),
                            )
                            .emit();
                    }
                }
                Err(ClientError::Io(why)) => {
                    // §9.3: the connection is lost (the inbound loop
                    // connects again); the sample waits on as the oldest.
                    if queue.put_back(instance, out) {
                        self.metrics.dds_dropped(&self.topic.dds_name);
                    }
                    if self.connected.load(Ordering::SeqCst) {
                        log::event(Level::Warn, "samples_waiting")
                            .tenant(&self.tenant)
                            .field("dds_topic", &self.topic.dds_name)
                            .field("mqtt_topic", topic)
                            .number("waiting", queue.len() as u64)
                            .field("reason", &why)
                            .emit();
                    }
                    thread::sleep(Duration::from_millis(50));
                }
                Err(ClientError::Busy(why)) => {
                    // §6: the broker acknowledges nothing; the sample waits
                    // on as the oldest, unless keep_last drops it now.
                    if queue.put_back(instance, out) {
                        self.metrics.dds_dropped(&self.topic.dds_name);
                    }
                    log::event(Level::Warn, "samples_waiting")
                        .tenant(&self.tenant)
                        .field("dds_topic", &self.topic.dds_name)
                        .field("mqtt_topic", topic)
                        .number("waiting", queue.len() as u64)
                        .field("reason", &why)
                        .emit();
                }
                Err(e) => {
                    self.metrics.errors_total.inc();
                    log::event(Level::Warn, "publish_failed")
                        .tenant(&self.tenant)
                        .field("dds_topic", &self.topic.dds_name)
                        .field("mqtt_topic", topic)
                        .number("bytes", out.message.len() as u64)
                        .number("mqtt_qos", u64::from(mqtt_qos))
                        .field("reason", &e.to_string())
                        .emit();
                }
            }
        }
    }

    /// A sample of the reader into `queue`: the ACL read check (§7.3), the
    /// PUBLISH it becomes (§4.2); keep_last may drop an older one.
    fn take(&self, sample: UserSample, queue: &mut HistoryQueue<Outgoing>) {
        match sample {
            UserSample::Alive {
                payload,
                representation_identifier,
                key_hash,
                source_timestamp,
                lifespan_nanos,
                writer_guid,
                ..
            } => {
                if self.own_writer == Some(writer_guid) {
                    return;
                }
                self.metrics.dds_received(&self.topic.dds_name);
                if !self.may_publish() {
                    return;
                }
                // Spec §4.2: the encapsulation header, then the CDR bytes,
                // as the sample travelled over RTPS; the properties name the
                // format and the sample.
                let mut message = Vec::with_capacity(payload.len() + 4);
                message.extend_from_slice(&encapsulation(representation_identifier));
                message.extend_from_slice(&payload);
                // §6: the writer's lifespan, else `qos.lifespan_ms`; the
                // Message Expiry Interval counts from the arrival.
                let lifespan = super::qos_translation::lifespan_nanos(&self.topic, lifespan_nanos);
                let expires = (lifespan > 0)
                    .then(|| std::time::Instant::now().checked_add(Duration::from_nanos(lifespan)))
                    .flatten();
                let props = publish_properties(
                    &self.topic,
                    &SampleMeta {
                        representation_identifier,
                        key_hash,
                        source_timestamp,
                        lifespan_nanos: lifespan,
                        status_flags: 0,
                    },
                );
                let out = Outgoing {
                    message,
                    props,
                    kind: OutKind::Sample,
                    expires,
                    arrived: std::time::Instant::now(),
                };
                if queue.push(key_hash, out) {
                    self.metrics.dds_dropped(&self.topic.dds_name);
                }
            }
            // §4.4: a dispose or an unregister is a PUBLISH with
            // `zerodds_op`, in the history of its instance.
            UserSample::Lifecycle {
                key_hash,
                kind,
                key_holder,
                source_timestamp,
                writer_guid,
                ..
            } => {
                if self.own_writer == Some(writer_guid) {
                    return;
                }
                let Some((op, flags)) = op_of(kind) else {
                    self.metrics.errors_total.inc();
                    log::event(Level::Warn, "lifecycle_not_published")
                        .tenant(&self.tenant)
                        .field("dds_topic", &self.topic.dds_name)
                        .field("mqtt_topic", &self.topic.mqtt_topic)
                        .field(
                            "reason",
                            &format!(
                                "a lifecycle marker of kind {kind:?} is no dispose or unregister"
                            ),
                        )
                        .emit();
                    return;
                };
                if !self.may_publish() {
                    return;
                }
                let out = if self.version == ProtocolVersion::V311 {
                    // §4.4, §1 L1: a PUBLISH of MQTT 3.1.1 has no properties;
                    // without `zerodds_op` it would be read as a sample.
                    self.metrics.errors_total.inc();
                    if !self.lifecycle_logged.replace(true) {
                        log::event(Level::Warn, "lifecycle_not_published")
                            .tenant(&self.tenant)
                            .field("dds_topic", &self.topic.dds_name)
                            .field("mqtt_topic", &self.topic.mqtt_topic)
                            .field(
                                "reason",
                                &format!(
                                    "a {} is not published under MQTT 3.1.1, which has no zerodds_op; further ones are counted as errors",
                                    op.as_str()
                                ),
                            )
                            .emit();
                    }
                    Outgoing {
                        message: Vec::new(),
                        props: crate::property_set::PropertySet::default(),
                        kind: OutKind::NotPublished,
                        expires: None,
                        arrived: std::time::Instant::now(),
                    }
                } else {
                    Outgoing {
                        message: lifecycle_message(key_holder.as_deref()),
                        props: lifecycle_properties(
                            &self.topic,
                            op,
                            flags,
                            key_hash,
                            source_timestamp,
                        ),
                        kind: OutKind::Lifecycle,
                        expires: None,
                        arrived: std::time::Instant::now(),
                    }
                };
                if queue.push(Some(key_hash), out) {
                    self.metrics.dds_dropped(&self.topic.dds_name);
                }
            }
            // The end-of-coherent-set marker (DDSI-RTPS 2.5 §8.7.6) carries
            // no user data — nothing to forward.
            UserSample::CoherentSetEnd { .. } => {}
        }
    }
}

impl Pump {
    /// §7.3: whether `allow_publish` grants the topic's MQTT topic to the
    /// bridge's subject; a refusal is counted, the first one logged.
    fn may_publish(&self) -> bool {
        let live = self.acl.current();
        if live.0.may_publish(&live.1, &self.topic.mqtt_topic) {
            return true;
        }
        self.metrics.acl_denial(acl_reason::PUBLISH);
        if !self.refused_logged.replace(true) {
            log::event(Level::Warn, "acl_denied")
                .tenant(&self.tenant)
                .field("action", "publish")
                .field("mqtt_topic", &self.topic.mqtt_topic)
                .field("dds_topic", &self.topic.dds_name)
                .field("subject", live.1.display())
                .emit();
        }
        false
    }

    /// §6: the newest sample of `instance` that went out into the replay
    /// buffer (`sample` `Some`), or the instance out of it (a dispose or
    /// unregister).
    fn kept(
        &self,
        instance: Option<[u8; 16]>,
        sample: Option<(&[u8], &crate::property_set::PropertySet)>,
    ) {
        let Ok(mut replay) = self.replay.lock() else {
            return;
        };
        if !replay.keeps(&self.topic.dds_name) {
            return;
        }
        let done = match sample {
            Some((message, props)) => replay.keep(&self.topic.dds_name, instance, message, props),
            None => replay.forget(&self.topic.dds_name, instance),
        };
        if let Err(e) = done {
            self.metrics.errors_total.inc();
            log::event(Level::Warn, "replay_failed")
                .tenant(&self.tenant)
                .field("dds_topic", &self.topic.dds_name)
                .field("mqtt_topic", &self.topic.mqtt_topic)
                .field("reason", &format!("replay buffer: {e}"))
                .emit();
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn topic() -> TopicConfig {
        TopicConfig {
            dds_name: "Chat::Message".into(),
            dds_type: "chat::Message".into(),
            mqtt_topic: "chat/message".into(),
            ..TopicConfig::default()
        }
    }

    /// Spec §4.2: the properties of the PUBLISH of a sample, in the octets
    /// of MQTT 5.0 §2.2.2.2 (order of Table 2-4, the user properties in the
    /// order of the table of §4.2).
    #[test]
    fn mqttbridge_4_2_the_properties_of_a_sample() {
        let key = [0xab; 16];
        let meta = SampleMeta {
            representation_identifier: [0x00, 0x07],
            key_hash: Some(key),
            source_timestamp: Some(HeTimestamp {
                seconds: 1,
                fraction: 0x8000_0000,
            }),
            lifespan_nanos: 2_400_000_000,
            status_flags: 0,
        };
        let props = publish_properties(&topic(), &meta);
        let mut want = vec![0x01, 0x00]; // Payload Format Indicator 0
        want.extend([0x02, 0, 0, 0, 2]); // Message Expiry Interval 2 s
        want.push(0x03); // Content Type
        want.extend((CONTENT_TYPE_CDR2.len() as u16).to_be_bytes());
        want.extend(CONTENT_TYPE_CDR2.as_bytes());
        for (k, v) in [
            ("zerodds_type", "chat::Message"),
            ("zerodds_topic", "Chat::Message"),
            ("zerodds_flags", "0x00"),
            ("zerodds_key_hash", "abababababababababababababababab"),
            ("zerodds_source_ts_ns", "1500000000"),
        ] {
            want.push(0x26);
            want.extend((k.len() as u16).to_be_bytes());
            want.extend(k.as_bytes());
            want.extend((v.len() as u16).to_be_bytes());
            want.extend(v.as_bytes());
        }
        assert_eq!(props.encode().unwrap(), want);
    }

    /// Without a key hash, a source timestamp and a lifespan the PUBLISH
    /// carries none of them; an XCDR1 sample is `application/x-dds-cdr`.
    #[test]
    fn mqttbridge_4_2_what_a_sample_does_not_have_is_left_out() {
        let meta = SampleMeta {
            representation_identifier: [0x00, 0x01],
            key_hash: None,
            source_timestamp: None,
            lifespan_nanos: 0,
            status_flags: 0,
        };
        let props = publish_properties(&topic(), &meta);
        assert_eq!(props.content_type.as_deref(), Some(CONTENT_TYPE_CDR));
        assert_eq!(props.message_expiry_interval, None);
        let names: Vec<&str> = props
            .user_properties
            .iter()
            .map(|(k, _)| k.as_str())
            .collect();
        assert_eq!(names, ["zerodds_type", "zerodds_topic", "zerodds_flags"]);
        for (id, ct) in [
            ([0, 0], CONTENT_TYPE_CDR),
            ([0, 3], CONTENT_TYPE_CDR),
            ([0, 6], CONTENT_TYPE_CDR2),
            ([0, 0x0b], CONTENT_TYPE_CDR2),
        ] {
            assert_eq!(content_type_of(id), ct);
        }
        assert_eq!(message_expiry_of(300_000_000), Some(1));
        assert_eq!(message_expiry_of(1_499_999_999), Some(1));
        assert_eq!(message_expiry_of(1_500_000_000), Some(2));
    }

    /// A source timestamp goes through nanoseconds and back unchanged.
    #[test]
    fn mqttbridge_4_2_the_source_timestamp_round_trips_through_nanoseconds() {
        for ns in [0u64, 1, 999_999_999, 1_700_000_000_123_456_789] {
            let ts = timestamp_of_ns(ns).unwrap();
            assert_eq!(timestamp_ns(ts), ns, "{ns}");
        }
        assert_eq!(timestamp_of_ns(u64::MAX), None);
    }

    /// Spec §4.2 MQTT → DDS: the header and the body, the key hash and the
    /// source timestamp of the user properties; what makes a PUBLISH no DDS
    /// sample is refused.
    #[test]
    fn mqttbridge_4_2_an_inbound_publish_and_what_is_refused() {
        let payload = [0x00, 0x07, 0x00, 0x00, 9, 8];
        let props = PropertySet {
            content_type: Some(CONTENT_TYPE_CDR2.into()),
            user_properties: vec![
                (
                    PROP_KEY_HASH.into(),
                    "000102030405060708090a0b0c0d0e0f".into(),
                ),
                (PROP_SOURCE_TS.into(), "1500000000".into()),
            ],
            ..PropertySet::default()
        };
        let s = inbound_sample(&payload, &props).unwrap();
        assert_eq!((s.identifier, s.body), ([0x00, 0x07], &[9u8, 8][..]));
        assert_eq!(
            s.key_hash,
            Some([0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15])
        );
        assert_eq!(s.source_timestamp.map(timestamp_ns), Some(1_500_000_000));
        // A plain MQTT client sends no property: the payload alone.
        let plain = inbound_sample(&payload, &PropertySet::default()).unwrap();
        assert_eq!((plain.key_hash, plain.source_timestamp), (None, None));
        let with = |f: &dyn Fn(&mut PropertySet)| {
            let mut p = PropertySet::default();
            f(&mut p);
            inbound_sample(&payload, &p).unwrap_err()
        };
        assert!(with(&|p| p.payload_format_indicator = Some(1)).contains("UTF-8"));
        assert!(with(&|p| p.content_type = Some("application/json".into())).contains("json"));
        assert!(with(&|p| p.content_type = Some(CONTENT_TYPE_CDR.into())).contains("0x0007"));
        assert!(
            with(&|p| p.user_properties = vec![(PROP_KEY_HASH.into(), "zz".into())])
                .contains("32 hex")
        );
        assert!(
            with(&|p| p.user_properties = vec![(PROP_SOURCE_TS.into(), "-1".into())])
                .contains("nanoseconds")
        );
        assert!(inbound_sample(&[0x00, 0x20, 0, 0], &PropertySet::default()).is_err());
        assert!(inbound_sample(&[0x00], &PropertySet::default()).is_err());
    }

    /// Spec §4.4: a dispose of DDS is a PUBLISH with `zerodds_op` dispose,
    /// `zerodds_flags` 0x01 and the key hash, its payload the header 00 06
    /// and the key holder; an unregister is `unregister` 0x02, the two
    /// together `dispose` 0x03.
    #[test]
    fn mqttbridge_4_4_a_dispose_marker_is_op_dispose_with_the_key_hash() {
        use zerodds_rtps::history_cache::ChangeKind as K;
        assert_eq!(op_of(K::NotAliveDisposed), Some((Op::Dispose, 0x01)));
        assert_eq!(op_of(K::NotAliveUnregistered), Some((Op::Unregister, 0x02)));
        assert_eq!(
            op_of(K::NotAliveDisposedUnregistered),
            Some((Op::Dispose, 0x03))
        );
        assert_eq!(op_of(K::Alive), None);
        let props = lifecycle_properties(
            &topic(),
            Op::Dispose,
            0x01,
            [0xcd; 16],
            Some(HeTimestamp {
                seconds: 2,
                fraction: 0,
            }),
        );
        assert_eq!(props.payload_format_indicator, Some(0));
        assert_eq!(props.content_type.as_deref(), Some(CONTENT_TYPE_CDR2));
        let user: Vec<(&str, &str)> = props
            .user_properties
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        assert_eq!(
            user,
            [
                ("zerodds_type", "chat::Message"),
                ("zerodds_topic", "Chat::Message"),
                ("zerodds_op", "dispose"),
                ("zerodds_flags", "0x01"),
                ("zerodds_key_hash", "cdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd"),
                ("zerodds_source_ts_ns", "2000000000"),
            ]
        );
        assert_eq!(lifecycle_message(None), [0x00, 0x06, 0x00, 0x00]);
        assert_eq!(
            lifecycle_message(Some(&[0, 0, 0, 7])),
            [0x00, 0x06, 0x00, 0x00, 0, 0, 0, 7]
        );
        // What the bridge sends, it reads back as the same change.
        let mut back = props.clone();
        back.user_properties.retain(|(k, _)| k != PROP_TYPE);
        match inbound_message(&lifecycle_message(Some(&[0, 0, 0, 7])), &back).unwrap() {
            InboundMessage::Lifecycle {
                key_hash,
                key_holder,
                status_bits,
                source_timestamp,
            } => {
                assert_eq!(key_hash, [0xcd; 16]);
                assert_eq!(key_holder, Some(&[0u8, 0, 0, 7][..]));
                assert_eq!(status_bits, 0x01);
                assert_eq!(source_timestamp.map(timestamp_ns), Some(2_000_000_000));
            }
            other => panic!("{other:?}"),
        }
    }

    fn op_props(op: Option<&str>, flags: Option<&str>, key: bool) -> PropertySet {
        let mut user = Vec::new();
        if let Some(op) = op {
            user.push((PROP_OP.to_string(), op.to_string()));
        }
        if let Some(f) = flags {
            user.push((PROP_FLAGS.to_string(), f.to_string()));
        }
        if key {
            user.push((PROP_KEY_HASH.to_string(), "0f".repeat(16)));
        }
        PropertySet {
            user_properties: user,
            ..PropertySet::default()
        }
    }

    /// Spec §4.4: without `zerodds_op` (and with `sample`) a PUBLISH is a
    /// sample; `register` writes nothing; `unregister` and `dispose` with
    /// 0x03 carry their status bits, with no payload or the header alone no
    /// key holder.
    #[test]
    fn mqttbridge_4_4_a_missing_op_is_sample() {
        let payload = [0x00, 0x07, 0x00, 0x00, 5];
        for p in [
            op_props(None, None, false),
            op_props(Some("sample"), None, false),
            op_props(None, Some("0x00"), false),
        ] {
            match inbound_message(&payload, &p).unwrap() {
                InboundMessage::Sample(s) => assert_eq!(s.body, [5]),
                other => panic!("{other:?}"),
            }
        }
        assert_eq!(
            inbound_message(&payload, &op_props(Some("register"), None, true)).unwrap(),
            InboundMessage::Register
        );
        for (op, flags, payload, bits) in [
            ("unregister", None, &[][..], 0x02),
            (
                "unregister",
                Some("0x02"),
                &[0x00, 0x06, 0x00, 0x00][..],
                0x02,
            ),
            ("dispose", Some("0x03"), &[][..], 0x03),
            ("dispose", None, &[][..], 0x01),
        ] {
            match inbound_message(payload, &op_props(Some(op), flags, true)).unwrap() {
                InboundMessage::Lifecycle {
                    key_hash,
                    key_holder,
                    status_bits,
                    ..
                } => {
                    assert_eq!(key_hash, [0x0f; 16]);
                    assert_eq!(key_holder, None, "{op}");
                    assert_eq!(status_bits, bits, "{op} {flags:?}");
                }
                other => panic!("{other:?}"),
            }
        }
    }

    /// Spec §4.4: an unknown `zerodds_op`, flags that contradict the
    /// operation, a dispose without key hash, a key holder under another
    /// header are refused, not written.
    #[test]
    fn mqttbridge_4_4_an_unknown_op_is_refused() {
        let payload = [0x00, 0x07, 0x00, 0x00, 5];
        for (p, payload, needle) in [
            (
                op_props(Some("delete"), None, true),
                &payload[..],
                "is not sample",
            ),
            (
                op_props(Some("dispose"), Some("0x02"), true),
                &[][..],
                "contradicts",
            ),
            (
                op_props(Some("sample"), Some("0x01"), false),
                &payload[..],
                "contradicts",
            ),
            (
                op_props(Some("unregister"), Some("0x03"), true),
                &[][..],
                "contradicts",
            ),
            (
                op_props(Some("dispose"), Some("1"), true),
                &[][..],
                "two hex digits",
            ),
            (
                op_props(Some("dispose"), None, false),
                &[][..],
                "names no instance",
            ),
            (
                op_props(Some("dispose"), None, true),
                &payload[..],
                "PLAIN_CDR2",
            ),
        ] {
            let err = inbound_message(payload, &p).unwrap_err();
            assert!(err.contains(needle), "{err}");
        }
    }

    /// §4.3 under MQTT 3.1.1: the record of a PUBLISH is made before it
    /// goes out; the broker's copy finds it once, a second copy is not
    /// taken for one; a PUBLISH that did not go out takes its record back,
    /// so a later message of the same payload from another client is
    /// written.
    #[test]
    fn mqttbridge_4_3_the_echo_record_comes_before_the_publish_and_goes_when_it_fails() {
        let echo = EchoGuard::default();
        echo.sent("t", b"a");
        assert!(echo.take("t", b"a"), "the copy of a noted PUBLISH");
        assert!(!echo.take("t", b"a"), "one record, one copy");
        echo.sent("t", b"b");
        echo.sent("t", b"b");
        echo.unsent("t", b"b");
        assert!(echo.take("t", b"b"));
        assert!(!echo.take("t", b"b"), "the failed PUBLISH left a record");
        assert!(!echo.take("u", b"b"), "another topic");
    }
}
