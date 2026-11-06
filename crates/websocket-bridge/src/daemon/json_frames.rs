// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The JSON text frames of `zerodds-ws-bridge-1.0` §4.3 and the control
//! messages of §4.4.
//!
//! A sample frame names its topic, its operation (`publish`, `dispose`,
//! `register`, `unregister`), the instance's `key_hash`, the sample as
//! JSON in `data` and its source time in `timestamp_ns`:
//!
//! ```json
//! {"topic":"Chat::Message","op":"publish","key_hash":"a1d0…",
//!  "data":{"id":7,"text":"hi"},"timestamp_ns":1730000000000000000}
//! ```
//!
//! `data` is the sample of the topic's type, which the daemon reads from
//! the topic's `idl_file` (§3): a DDS sample is decoded from the format its
//! encapsulation header names (DDSI-RTPS 2.5 §10.5, DDS-XTypes 1.3 Table
//! 60), a client's `data` is encoded under XCDR2 little endian in the
//! format the type's extensibility takes (DDS-XTypes 1.3 §7.4.2). The
//! value of a member is the JSON of `zerodds_wire_codec::json`.
//!
//! The control messages are `subscribe` and `unsubscribe` with a list of
//! `topics`, `ping`; the daemon answers `subscribed` with the topics it
//! took, `pong`, and `error` with a `code` and a `message`.

use std::string::{String, ToString};
use std::sync::Arc;
use std::vec::Vec;

use serde_json::{Map, Value};
use zerodds_ast_layout::{Encoding, Endianness};
use zerodds_types::dynamic::descriptor::ExtensibilityKind;
use zerodds_types::dynamic::type_::DynamicType;
use zerodds_wire_codec::{Emit, ProbeRequest};

/// The type of a topic, read from its IDL file.
#[derive(Clone)]
pub struct TopicType {
    ty: DynamicType,
    keyed: bool,
}

impl core::fmt::Debug for TopicType {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("TopicType")
            .field("type", &self.ty.qualified_name())
            .field("keyed", &self.keyed)
            .finish()
    }
}

impl TopicType {
    /// Reads `idl_file` (its `#include`s from its own directory) and finds
    /// `type_name` in it.
    ///
    /// # Errors
    /// The file does not read or compile, or declares no such type.
    pub fn load(idl_file: &str, type_name: &str) -> Result<Self, String> {
        let dir = std::path::Path::new(idl_file)
            .parent()
            .map(std::path::Path::to_path_buf)
            .unwrap_or_default();
        let loader = zerodds_idl_compose::PathLoader::new(vec![dir]);
        let composed = zerodds_idl_compose::compose(idl_file, &loader)
            .map_err(|e| format!("{idl_file}: {e}"))?;
        if composed.failed() {
            let messages: Vec<String> = composed
                .diagnostics
                .iter()
                .map(|d| d.message.clone())
                .collect();
            return Err(format!("{idl_file}: {}", messages.join("; ")));
        }
        let ast = Arc::new(composed.ast);
        let ty = DynamicType::find(&ast, type_name)
            .map_err(|_| format!("{idl_file}: declares no type {type_name}"))?;
        Ok(Self::with(ty))
    }

    /// `type_name` in the IDL text `source`.
    ///
    /// # Errors
    /// The text does not compile, or declares no such type.
    pub fn from_idl(source: &str, type_name: &str) -> Result<Self, String> {
        let composed = zerodds_idl_compose::compose_source_with(
            "<idl>",
            source,
            &zerodds_idl_compose::PathLoader::bare(),
            zerodds_idl_compose::ComposeOptions::default(),
        );
        if composed.failed() {
            let messages: Vec<String> = composed
                .diagnostics
                .iter()
                .map(|d| d.message.clone())
                .collect();
            return Err(messages.join("; "));
        }
        let ast = Arc::new(composed.ast);
        let ty = DynamicType::find(&ast, type_name)
            .map_err(|_| format!("declares no type {type_name}"))?;
        Ok(Self::with(ty))
    }

    fn with(ty: DynamicType) -> Self {
        let keyed = ty
            .all_members_by_name()
            .values()
            .any(|m| m.descriptor().is_ok_and(|d| d.is_key));
        Self { ty, keyed }
    }

    /// Whether the type has a key member: its samples belong to instances.
    #[must_use]
    pub fn keyed(&self) -> bool {
        self.keyed
    }

    fn request(&self, encoding: Encoding, endianness: Endianness) -> ProbeRequest {
        ProbeRequest::new(&self.ty.qualified_name(), encoding, endianness)
    }

    /// The JSON of a DDS sample's `body`, read in the format and byte order
    /// `identifier` names.
    ///
    /// # Errors
    /// An identifier of no CDR form, or a body that is no sample of the type.
    pub fn decode(&self, body: &[u8], identifier: [u8; 2]) -> Result<Value, String> {
        let (encoding, endianness) = form_of(identifier).ok_or_else(|| {
            format!(
                "encapsulation {:02x}{:02x} is no CDR representation",
                identifier[0], identifier[1]
            )
        })?;
        zerodds_wire_codec::decode::<Value>(
            self.ty.ast(),
            &self.request(encoding, endianness),
            body,
        )
        .map_err(|e| e.to_string())
    }

    /// A client's `data` as a DDS sample: the body under XCDR2 little
    /// endian in the format of the type's extensibility (DDS-XTypes 1.3
    /// §7.4.2), the representation identifier of that format, and the
    /// instance's KeyHash (DDSI-RTPS 2.5 §9.6.4.8) for a keyed type.
    ///
    /// # Errors
    /// A value that is no sample of the type.
    pub fn encode(&self, data: &Value) -> Result<Encoded, String> {
        let (encoding, identifier) = match self.ty.extensibility() {
            Some(ExtensibilityKind::Mutable) => (Encoding::PlCdr2, [0x00, 0x0b]),
            Some(ExtensibilityKind::Appendable) => (Encoding::DelimitedCdr, [0x00, 0x09]),
            _ => (Encoding::PlainCdr2, [0x00, 0x07]),
        };
        let request = self.request(encoding, Endianness::Little);
        let body =
            zerodds_wire_codec::encode(self.ty.ast(), &request, data).map_err(|e| e.to_string())?;
        let key_hash = if self.keyed {
            Some(self.key_hash(data)?)
        } else {
            None
        };
        Ok(Encoded {
            body,
            identifier,
            key_hash,
        })
    }

    /// The KeyHash of the instance `data` names (DDSI-RTPS 2.5 §9.6.4.8).
    ///
    /// # Errors
    /// A value that is no sample of the type.
    pub fn key_hash(&self, data: &Value) -> Result<[u8; 16], String> {
        let mut request = self.request(Encoding::PlainCdr2, Endianness::Big);
        request.emit = Emit::KeyHash;
        let hash =
            zerodds_wire_codec::encode(self.ty.ast(), &request, data).map_err(|e| e.to_string())?;
        hash.try_into()
            .map_err(|_| "a KeyHash is sixteen octets".to_string())
    }
}

/// A client's sample as the DDS writer sends it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Encoded {
    /// The body, without the encapsulation header.
    pub body: Vec<u8>,
    /// The representation identifier of the header it goes under.
    pub identifier: [u8; 2],
    /// The KeyHash of its instance, for a keyed type.
    pub key_hash: Option<[u8; 16]>,
}

/// The encoding and byte order a representation identifier names
/// (DDS-XTypes 1.3 Table 60); `None` for one of no CDR form.
#[must_use]
pub fn form_of(identifier: [u8; 2]) -> Option<(Encoding, Endianness)> {
    if identifier[0] != 0x00 {
        return None;
    }
    let encoding = match identifier[1] & !0x01 {
        0x00 => Encoding::PlainCdr1,
        0x02 => Encoding::PlCdr1,
        0x06 => Encoding::PlainCdr2,
        0x08 => Encoding::DelimitedCdr,
        0x0a => Encoding::PlCdr2,
        _ => return None,
    };
    let endianness = if identifier[1] & 0x01 == 0 {
        Endianness::Big
    } else {
        Endianness::Little
    };
    Some((encoding, endianness))
}

/// The operation of a sample frame (§4.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SampleOp {
    /// A sample.
    Publish,
    /// The instance is disposed.
    Dispose,
    /// The instance is registered; no sample.
    Register,
    /// The writer unregisters the instance.
    Unregister,
}

impl SampleOp {
    /// The `op` of the frame.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Publish => "publish",
            Self::Dispose => "dispose",
            Self::Register => "register",
            Self::Unregister => "unregister",
        }
    }
}

/// A sample frame (§4.3), in either direction.
#[derive(Debug, Clone, PartialEq)]
pub struct SampleFrame {
    /// The DDS topic.
    pub topic: String,
    /// What happens to the instance.
    pub op: SampleOp,
    /// The instance's KeyHash, where there is one.
    pub key_hash: Option<[u8; 16]>,
    /// The sample, for `publish`.
    pub data: Option<Value>,
    /// The source time in nanoseconds since the Unix epoch.
    pub timestamp_ns: Option<i64>,
}

impl SampleFrame {
    /// The text of the frame. Fields without a value are left out.
    #[must_use]
    pub fn render(&self) -> String {
        let mut object = Map::new();
        object.insert("topic".into(), Value::String(self.topic.clone()));
        object.insert("op".into(), Value::String(self.op.as_str().into()));
        if let Some(hash) = self.key_hash {
            object.insert("key_hash".into(), Value::String(hex(&hash)));
        }
        if let Some(data) = &self.data {
            object.insert("data".into(), data.clone());
        }
        if let Some(ns) = self.timestamp_ns {
            object.insert("timestamp_ns".into(), Value::from(ns));
        }
        Value::Object(object).to_string()
    }
}

/// What a client sends (§4.3, §4.4).
#[derive(Debug, Clone, PartialEq)]
pub enum ClientMessage {
    /// `{"op":"subscribe","topics":[…]}`.
    Subscribe(Vec<String>),
    /// `{"op":"unsubscribe","topics":[…]}`.
    Unsubscribe(Vec<String>),
    /// `{"op":"ping"}`.
    Ping,
    /// A sample frame.
    Sample(SampleFrame),
}

/// Reads a client's text frame.
///
/// # Errors
/// Text that is no JSON object, an `op` the protocol does not have, or a
/// field of the wrong form: `topics` a list of names, `topic` a name,
/// `key_hash` 32 hexadecimal digits, `timestamp_ns` an integer, `data`
/// present for `publish`.
pub fn parse_client(text: &str) -> Result<ClientMessage, String> {
    let value: Value = serde_json::from_str(text).map_err(|e| format!("not JSON: {e}"))?;
    let Value::Object(object) = value else {
        return Err("a message is a JSON object".into());
    };
    let op = object
        .get("op")
        .and_then(Value::as_str)
        .ok_or("a message names its op")?;
    let topics = || -> Result<Vec<String>, String> {
        object
            .get("topics")
            .and_then(Value::as_array)
            .ok_or_else(|| format!("{op} names its topics as a list"))?
            .iter()
            .map(|t| {
                t.as_str()
                    .map(ToString::to_string)
                    .ok_or_else(|| "a topic is a string".to_string())
            })
            .collect()
    };
    let sample_op = match op {
        "subscribe" => return topics().map(ClientMessage::Subscribe),
        "unsubscribe" => return topics().map(ClientMessage::Unsubscribe),
        "ping" => return Ok(ClientMessage::Ping),
        "publish" => SampleOp::Publish,
        "dispose" => SampleOp::Dispose,
        "register" => SampleOp::Register,
        "unregister" => SampleOp::Unregister,
        other => return Err(format!("no op {other}")),
    };
    let topic = object
        .get("topic")
        .and_then(Value::as_str)
        .filter(|t| !t.is_empty())
        .ok_or("a sample frame names its topic")?
        .to_string();
    let key_hash = match object.get("key_hash") {
        None | Some(Value::Null) => None,
        Some(Value::String(h)) => Some(unhex16(h).ok_or("key_hash is 32 hexadecimal digits")?),
        Some(_) => return Err("key_hash is a string".into()),
    };
    let data = object.get("data").filter(|d| !d.is_null()).cloned();
    if sample_op == SampleOp::Publish && data.is_none() {
        return Err("publish carries the sample in data".into());
    }
    let timestamp_ns = match object.get("timestamp_ns") {
        None | Some(Value::Null) => None,
        Some(v) => Some(v.as_i64().ok_or("timestamp_ns is an integer")?),
    };
    Ok(ClientMessage::Sample(SampleFrame {
        topic,
        op: sample_op,
        key_hash,
        data,
        timestamp_ns,
    }))
}

/// `{"op":"subscribed","topics":[…]}`.
#[must_use]
pub fn subscribed(topics: &[String]) -> String {
    serde_json::json!({ "op": "subscribed", "topics": topics }).to_string()
}

/// `{"op":"error","code":…,"message":…}`.
#[must_use]
pub fn error(code: u16, message: &str) -> String {
    serde_json::json!({ "op": "error", "code": code, "message": message }).to_string()
}

/// `{"op":"pong"}`.
#[must_use]
pub fn pong() -> String {
    serde_json::json!({ "op": "pong" }).to_string()
}

fn hex(bytes: &[u8]) -> String {
    use core::fmt::Write as _;
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(s, "{b:02x}");
    }
    s
}

fn unhex16(text: &str) -> Option<[u8; 16]> {
    if text.len() != 32 {
        return None;
    }
    let mut out = [0u8; 16];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(text.get(2 * i..2 * i + 2)?, 16).ok()?;
    }
    Some(out)
}

/// A source time of DDSI-RTPS 2.5 §9.3.2.2 (seconds and 2^-32 s) in
/// nanoseconds since the Unix epoch.
#[must_use]
pub fn nanos_of(seconds: u32, fraction: u32) -> i64 {
    i64::from(seconds) * 1_000_000_000 + ((u64::from(fraction) * 1_000_000_000) >> 32) as i64
}

/// Nanoseconds since the Unix epoch as a source time of DDSI-RTPS 2.5
/// §9.3.2.2; `None` before the epoch or past 2106.
#[must_use]
pub fn time_of(nanos: i64) -> Option<(u32, u32)> {
    let nanos = u64::try_from(nanos).ok()?;
    let seconds = u32::try_from(nanos / 1_000_000_000).ok()?;
    let fraction = (((nanos % 1_000_000_000) << 32) / 1_000_000_000) as u32;
    Some((seconds, fraction))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use serde_json::json;

    const IDL: &str = "@appendable struct Reading { @key long id; string text; };";

    /// §4.3: a sample frame names its topic, its op, the instance's
    /// `key_hash` in hexadecimal, the sample in `data` and its source time
    /// in `timestamp_ns`; a client's frame of that form reads back as it.
    #[test]
    fn wsbridge_4_3_a_sample_frame_is_topic_op_key_hash_data_and_time() {
        let frame = SampleFrame {
            topic: "Chat::Message".into(),
            op: SampleOp::Publish,
            key_hash: Some([0xa1; 16]),
            data: Some(json!({ "text": "hi" })),
            timestamp_ns: Some(1_730_000_000_000_000_000),
        };
        let text = frame.render();
        assert_eq!(
            serde_json::from_str::<Value>(&text).unwrap(),
            json!({
                "topic": "Chat::Message",
                "op": "publish",
                "key_hash": "a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1",
                "data": { "text": "hi" },
                "timestamp_ns": 1_730_000_000_000_000_000_i64
            })
        );
        assert_eq!(parse_client(&text).unwrap(), ClientMessage::Sample(frame));
        for op in ["dispose", "register", "unregister"] {
            let text = format!(
                r#"{{"topic":"T","op":"{op}","key_hash":"00000007000000000000000000000000"}}"#
            );
            let ClientMessage::Sample(f) = parse_client(&text).unwrap() else {
                panic!("{op}")
            };
            assert_eq!((f.op.as_str(), f.key_hash.unwrap()[3]), (op, 7));
        }
    }

    /// §4.4: the control messages as the spec writes them.
    #[test]
    fn wsbridge_4_4_the_control_messages() {
        assert_eq!(
            parse_client(
                r#"{ "op": "subscribe", "topics": ["Chat::Message", "Sensor::Reading"] }"#
            )
            .unwrap(),
            ClientMessage::Subscribe(vec!["Chat::Message".into(), "Sensor::Reading".into()])
        );
        assert_eq!(
            parse_client(r#"{ "op": "unsubscribe", "topics": ["Chat::Message"] }"#).unwrap(),
            ClientMessage::Unsubscribe(vec!["Chat::Message".into()])
        );
        assert_eq!(
            parse_client(r#"{ "op": "ping" }"#).unwrap(),
            ClientMessage::Ping
        );
        let parse = |t: &str| serde_json::from_str::<Value>(t).unwrap();
        assert_eq!(
            parse(&subscribed(&["Chat::Message".into()])),
            json!({ "op": "subscribed", "topics": ["Chat::Message"] })
        );
        assert_eq!(
            parse(&error(403, "scope dds.write required for Chat::Message")),
            json!({ "op": "error", "code": 403, "message": "scope dds.write required for Chat::Message" })
        );
        assert_eq!(parse(&pong()), json!({ "op": "pong" }));
    }

    /// A message of another form is refused, not read as something else:
    /// no op, an op the protocol does not have, a publish without data, a
    /// key hash that is not 16 octets, topics that are not a list.
    #[test]
    fn wsbridge_4_4_a_message_of_another_form_is_refused() {
        for text in [
            "not json",
            r#"["op","ping"]"#,
            r#"{"topic":"T"}"#,
            r#"{"op":"notify","topic":"T","data":{}}"#,
            r#"{"op":"publish","topic":"T"}"#,
            r#"{"op":"dispose","topic":"T","key_hash":"0007"}"#,
            r#"{"op":"subscribe","topic":"T","id":"sub-1"}"#,
            r#"{"op":"publish","topic":"","data":{}}"#,
        ] {
            assert!(parse_client(text).is_err(), "{text}");
        }
    }

    /// The sample of a client's `data` under XCDR2 little endian: an
    /// `@appendable` struct is DELIMITED_CDR (DDS-XTypes 1.3 §7.4.2, a
    /// DHEADER, then the members), the identifier `00 09` (Table 60); the
    /// KeyHash of `@key long id = 7` is its PLAIN_CDR2 big-endian holder
    /// padded to 16 octets (DDSI-RTPS 2.5 §9.6.4.8). A DDS sample of the
    /// big-endian form decodes to the same `data`.
    #[test]
    fn xtypes_7_4_2_a_client_sample_is_the_delimited_body_of_its_type() {
        let ty = TopicType::from_idl(IDL, "Reading").unwrap();
        assert!(ty.keyed());
        let data = json!({ "id": 7, "text": "hi" });
        let encoded = ty.encode(&data).unwrap();
        assert_eq!(
            encoded,
            Encoded {
                body: vec![0x0b, 0, 0, 0, 0x07, 0, 0, 0, 0x03, 0, 0, 0, b'h', b'i', 0],
                identifier: [0x00, 0x09],
                key_hash: Some([0, 0, 0, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
            }
        );
        let big = [0, 0, 0, 0x0b, 0, 0, 0, 0x07, 0, 0, 0, 0x03, b'h', b'i', 0];
        assert_eq!(ty.decode(&big, [0x00, 0x08]).unwrap(), data);
        assert!(ty.decode(&big, [0x00, 0x04]).is_err(), "XML is no CDR form");
        assert!(TopicType::from_idl(IDL, "Missing").is_err());
    }

    /// DDS-XTypes 1.3 Table 60: the CDR identifiers and the encoding and
    /// byte order each names.
    #[test]
    fn xtypes_7_6_3_1_2_the_identifier_names_encoding_and_byte_order() {
        for (second, encoding) in [
            (0x00, Encoding::PlainCdr1),
            (0x02, Encoding::PlCdr1),
            (0x06, Encoding::PlainCdr2),
            (0x08, Encoding::DelimitedCdr),
            (0x0a, Encoding::PlCdr2),
        ] {
            assert_eq!(form_of([0x00, second]), Some((encoding, Endianness::Big)));
            assert_eq!(
                form_of([0x00, second | 1]),
                Some((encoding, Endianness::Little))
            );
        }
        assert_eq!(form_of([0x00, 0x04]), None);
        assert_eq!(form_of([0x80, 0x00]), None);
    }

    /// DDSI-RTPS 2.5 §9.3.2.2: a time is seconds and a fraction of 2^-32 s;
    /// one and a half seconds is `(1, 2^31)`.
    #[test]
    fn rtps_9_3_2_2_a_source_time_in_nanoseconds_and_back() {
        assert_eq!(nanos_of(1, 0x8000_0000), 1_500_000_000);
        assert_eq!(time_of(1_500_000_000), Some((1, 0x8000_0000)));
        assert_eq!(time_of(-1), None);
    }
}
