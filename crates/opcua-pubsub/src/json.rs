// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! JSON message mapping for OPC-UA PubSub (Part 14 §7.2.3) plus the OPC-UA
//! reversible JSON value encoding it builds on (Part 6 §5.4).
//!
//! UADP is the binary wire format; over broker transports (MQTT/AMQP) OPC-UA
//! PubSub is frequently carried as JSON instead. A [`JsonNetworkMessage`]
//! (`MessageType: "ua-data"`) carries named-payload [`JsonDataSetMessage`]s;
//! each payload value is the reversible JSON encoding of a `Variant` (or a
//! `DataValue` when the writer included status/timestamps), so messages
//! round-trip without external metadata.
//!
//! This module is behind the `json` feature (which pulls `serde_json` and is
//! therefore `std`).

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use serde_json::{Map, Number, Value};
use zerodds_opcua_gateway::data_value::{
    DataValue, ExtensionObject, ExtensionObjectBody, Variant, VariantValue,
};
use zerodds_opcua_gateway::node_id::{NodeId, NodeIdentifier};
use zerodds_opcua_gateway::types::{BuiltinTypeKind, Guid, LocalizedText, QualifiedName};

use crate::config::ConfigurationVersion;
use crate::error::{DecodeError, EncodeError};
use crate::uadp::dataset_message::DataSetMessageKind;

// ---------------------------------------------------------------------------
// DateTime (OPC-UA ticks: 100 ns since 1601-01-01) ↔ ISO 8601 UTC.
// ---------------------------------------------------------------------------

const TICKS_PER_SEC: i64 = 10_000_000;
const SECS_PER_DAY: i64 = 86_400;
// Whole days between 1601-01-01 and 1970-01-01.
const EPOCH_DELTA_DAYS: i64 = 134_774;

/// Civil date `(year, month, day)` from days since the Unix epoch (Hinnant's
/// algorithm).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = (if z >= 0 { z } else { z - 146_096 }) / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m as u32, d as u32)
}

/// Days since the Unix epoch for a civil date (inverse of [`civil_from_days`]).
fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = (if y >= 0 { y } else { y - 399 }) / 400;
    let yoe = y - era * 400;
    let m = m as i64;
    let d = d as i64;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn ticks_to_iso8601(ticks: i64) -> String {
    let secs = ticks.div_euclid(TICKS_PER_SEC);
    let frac = ticks.rem_euclid(TICKS_PER_SEC);
    let days = secs.div_euclid(SECS_PER_DAY) - EPOCH_DELTA_DAYS;
    let tod = secs.rem_euclid(SECS_PER_DAY);
    let (y, m, d) = civil_from_days(days);
    let (hh, mm, ss) = (tod / 3600, (tod % 3600) / 60, tod % 60);
    format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}.{frac:07}Z")
}

fn iso8601_to_ticks(s: &str) -> Result<i64, DecodeError> {
    let bad = || DecodeError::MalformedMessage {
        message: "invalid ISO 8601 DateTime in JSON",
    };
    let s = s.strip_suffix('Z').unwrap_or(s);
    let (date, time) = s.split_once('T').ok_or_else(bad)?;
    let mut dparts = date.split('-');
    let y: i64 = dparts.next().ok_or_else(bad)?.parse().map_err(|_| bad())?;
    let mo: u32 = dparts.next().ok_or_else(bad)?.parse().map_err(|_| bad())?;
    let da: u32 = dparts.next().ok_or_else(bad)?.parse().map_err(|_| bad())?;

    let (hms, frac_str) = time.split_once('.').unwrap_or((time, ""));
    let mut tparts = hms.split(':');
    let hh: i64 = tparts.next().ok_or_else(bad)?.parse().map_err(|_| bad())?;
    let mm: i64 = tparts.next().ok_or_else(bad)?.parse().map_err(|_| bad())?;
    let ss: i64 = tparts.next().ok_or_else(bad)?.parse().map_err(|_| bad())?;

    // Fractional seconds → 100 ns ticks (pad/truncate to 7 digits).
    let mut frac_digits = String::from(frac_str);
    frac_digits.truncate(7);
    while frac_digits.len() < 7 {
        frac_digits.push('0');
    }
    let frac: i64 = if frac_digits.is_empty() {
        0
    } else {
        frac_digits.parse().map_err(|_| bad())?
    };

    let days = days_from_civil(y, mo, da) + EPOCH_DELTA_DAYS;
    let secs = days * SECS_PER_DAY + hh * 3600 + mm * 60 + ss;
    Ok(secs * TICKS_PER_SEC + frac)
}

// ---------------------------------------------------------------------------
// Guid ↔ canonical string.
// ---------------------------------------------------------------------------

fn guid_to_string(g: &Guid) -> String {
    let mut d4 = String::with_capacity(16);
    for b in g.data4 {
        d4.push_str(&format!("{b:02X}"));
    }
    format!(
        "{:08X}-{:04X}-{:04X}-{}-{}",
        g.data1,
        g.data2,
        g.data3,
        &d4[..4],
        &d4[4..]
    )
}

fn guid_from_string(s: &str) -> Result<Guid, DecodeError> {
    let bad = || DecodeError::MalformedMessage {
        message: "invalid Guid string in JSON",
    };
    let hex: String = s.chars().filter(|c| *c != '-').collect();
    if hex.len() != 32 {
        return Err(bad());
    }
    let byte = |i: usize| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).map_err(|_| bad());
    let data1 = u32::from_str_radix(&hex[0..8], 16).map_err(|_| bad())?;
    let data2 = u16::from_str_radix(&hex[8..12], 16).map_err(|_| bad())?;
    let data3 = u16::from_str_radix(&hex[12..16], 16).map_err(|_| bad())?;
    let mut data4 = [0u8; 8];
    for (i, b) in data4.iter_mut().enumerate() {
        *b = byte(8 + i)?;
    }
    Ok(Guid {
        data1,
        data2,
        data3,
        data4,
    })
}

// ---------------------------------------------------------------------------
// Scalar value ↔ JSON (Part 6 §5.4.2). The bare value carries no type, so
// decoding needs the BuiltinTypeKind.
// ---------------------------------------------------------------------------

fn float_to_json(f: f64) -> Value {
    if f.is_nan() {
        Value::String(String::from("NaN"))
    } else if f.is_infinite() {
        Value::String(String::from(if f > 0.0 { "Infinity" } else { "-Infinity" }))
    } else {
        Number::from_f64(f).map_or(Value::Null, Value::Number)
    }
}

fn json_to_f64(v: &Value) -> Result<f64, DecodeError> {
    let bad = || DecodeError::MalformedMessage {
        message: "invalid floating-point value in JSON",
    };
    match v {
        Value::Number(n) => n.as_f64().ok_or_else(bad),
        Value::String(s) => match s.as_str() {
            "NaN" => Ok(f64::NAN),
            "Infinity" => Ok(f64::INFINITY),
            "-Infinity" => Ok(f64::NEG_INFINITY),
            _ => Err(bad()),
        },
        _ => Err(bad()),
    }
}

fn node_id_to_json(n: &NodeId) -> Value {
    let mut m = Map::new();
    match &n.identifier_type {
        NodeIdentifier::Numeric(id) => {
            m.insert(String::from("Id"), Value::Number((*id).into()));
        }
        NodeIdentifier::String(s) => {
            m.insert(String::from("IdType"), Value::Number(1.into()));
            m.insert(String::from("Id"), Value::String(s.clone()));
        }
        NodeIdentifier::Guid(g) => {
            m.insert(String::from("IdType"), Value::Number(2.into()));
            m.insert(String::from("Id"), Value::String(guid_to_string(g)));
        }
        NodeIdentifier::Opaque(b) => {
            m.insert(String::from("IdType"), Value::Number(3.into()));
            m.insert(String::from("Id"), Value::String(BASE64.encode(b)));
        }
    }
    if n.namespace_index != 0 {
        m.insert(
            String::from("Namespace"),
            Value::Number(n.namespace_index.into()),
        );
    }
    Value::Object(m)
}

fn node_id_from_json(v: &Value) -> Result<NodeId, DecodeError> {
    let bad = || DecodeError::MalformedMessage {
        message: "invalid NodeId object in JSON",
    };
    let obj = v.as_object().ok_or_else(bad)?;
    let ns = obj.get("Namespace").and_then(Value::as_u64).unwrap_or(0) as u16;
    let id_type = obj.get("IdType").and_then(Value::as_u64).unwrap_or(0);
    let id = obj.get("Id").ok_or_else(bad)?;
    Ok(match id_type {
        0 => NodeId::numeric(ns, id.as_u64().ok_or_else(bad)? as u32),
        1 => NodeId {
            namespace_index: ns,
            identifier_type: NodeIdentifier::String(id.as_str().ok_or_else(bad)?.to_string()),
        },
        2 => NodeId::guid(ns, guid_from_string(id.as_str().ok_or_else(bad)?)?),
        3 => NodeId {
            namespace_index: ns,
            identifier_type: NodeIdentifier::Opaque(
                BASE64
                    .decode(id.as_str().ok_or_else(bad)?)
                    .map_err(|_| bad())?,
            ),
        },
        _ => return Err(bad()),
    })
}

fn scalar_to_json(v: &VariantValue) -> Value {
    match v {
        VariantValue::Boolean(b) => Value::Bool(*b),
        VariantValue::SByte(x) => Value::Number((*x).into()),
        VariantValue::Byte(x) => Value::Number((*x).into()),
        VariantValue::Int16(x) => Value::Number((*x).into()),
        VariantValue::UInt16(x) => Value::Number((*x).into()),
        VariantValue::Int32(x) => Value::Number((*x).into()),
        VariantValue::UInt32(x) => Value::Number((*x).into()),
        // Int64/UInt64 are JSON strings to preserve full precision (§5.4.2).
        VariantValue::Int64(x) => Value::String(x.to_string()),
        VariantValue::UInt64(x) => Value::String(x.to_string()),
        VariantValue::Float(f) => float_to_json(f64::from(*f)),
        VariantValue::Double(d) => float_to_json(*d),
        VariantValue::String(s) | VariantValue::XmlElement(s) => Value::String(s.clone()),
        VariantValue::DateTime(t) => Value::String(ticks_to_iso8601(*t)),
        VariantValue::Guid(g) => Value::String(guid_to_string(g)),
        VariantValue::ByteString(b) => Value::String(BASE64.encode(b)),
        VariantValue::NodeId(n) => node_id_to_json(n),
        VariantValue::StatusCode(c) => Value::Number((*c).into()),
        VariantValue::QualifiedName(q) => {
            let mut m = Map::new();
            m.insert(String::from("Name"), Value::String(q.name.clone()));
            if q.namespace_index != 0 {
                m.insert(String::from("Uri"), Value::Number(q.namespace_index.into()));
            }
            Value::Object(m)
        }
        VariantValue::LocalizedText(l) => {
            let mut m = Map::new();
            if let Some(loc) = &l.locale {
                m.insert(String::from("Locale"), Value::String(loc.clone()));
            }
            if let Some(t) = &l.text {
                m.insert(String::from("Text"), Value::String(t.clone()));
            }
            Value::Object(m)
        }
        VariantValue::ExtensionObject(e) => ext_to_json(e),
    }
}

fn ext_to_json(e: &ExtensionObject) -> Value {
    let mut m = Map::new();
    m.insert(String::from("TypeId"), node_id_to_json(&e.type_id));
    match &e.body {
        ExtensionObjectBody::None => {}
        ExtensionObjectBody::ByteString(b) => {
            m.insert(String::from("Encoding"), Value::Number(1.into()));
            m.insert(String::from("Body"), Value::String(BASE64.encode(b)));
        }
        ExtensionObjectBody::XmlElement(s) => {
            m.insert(String::from("Encoding"), Value::Number(2.into()));
            m.insert(String::from("Body"), Value::String(s.clone()));
        }
    }
    Value::Object(m)
}

fn ext_from_json(v: &Value) -> Result<ExtensionObject, DecodeError> {
    let bad = || DecodeError::MalformedMessage {
        message: "invalid ExtensionObject in JSON",
    };
    let obj = v.as_object().ok_or_else(bad)?;
    let type_id = node_id_from_json(obj.get("TypeId").ok_or_else(bad)?)?;
    let encoding = obj.get("Encoding").and_then(Value::as_u64).unwrap_or(0);
    let body = match encoding {
        0 => ExtensionObjectBody::None,
        1 => ExtensionObjectBody::ByteString(
            BASE64
                .decode(obj.get("Body").and_then(Value::as_str).ok_or_else(bad)?)
                .map_err(|_| bad())?,
        ),
        2 => ExtensionObjectBody::XmlElement(
            obj.get("Body")
                .and_then(Value::as_str)
                .ok_or_else(bad)?
                .to_string(),
        ),
        _ => return Err(bad()),
    };
    Ok(ExtensionObject { type_id, body })
}

fn scalar_from_json(v: &Value, kind: BuiltinTypeKind) -> Result<VariantValue, DecodeError> {
    let bad = || DecodeError::MalformedMessage {
        message: "JSON value does not match its declared built-in type",
    };
    Ok(match kind {
        BuiltinTypeKind::Boolean => VariantValue::Boolean(v.as_bool().ok_or_else(bad)?),
        BuiltinTypeKind::SByte => {
            VariantValue::SByte(i8::try_from(v.as_i64().ok_or_else(bad)?).map_err(|_| bad())?)
        }
        BuiltinTypeKind::Byte => {
            VariantValue::Byte(u8::try_from(v.as_u64().ok_or_else(bad)?).map_err(|_| bad())?)
        }
        BuiltinTypeKind::Int16 => {
            VariantValue::Int16(i16::try_from(v.as_i64().ok_or_else(bad)?).map_err(|_| bad())?)
        }
        BuiltinTypeKind::UInt16 => {
            VariantValue::UInt16(u16::try_from(v.as_u64().ok_or_else(bad)?).map_err(|_| bad())?)
        }
        BuiltinTypeKind::Int32 => {
            VariantValue::Int32(i32::try_from(v.as_i64().ok_or_else(bad)?).map_err(|_| bad())?)
        }
        BuiltinTypeKind::UInt32 => {
            VariantValue::UInt32(u32::try_from(v.as_u64().ok_or_else(bad)?).map_err(|_| bad())?)
        }
        BuiltinTypeKind::Int64 => VariantValue::Int64(parse_int_str(v)?),
        BuiltinTypeKind::UInt64 => VariantValue::UInt64(parse_uint_str(v)?),
        BuiltinTypeKind::Float => VariantValue::Float(json_to_f64(v)? as f32),
        BuiltinTypeKind::Double => VariantValue::Double(json_to_f64(v)?),
        BuiltinTypeKind::String => VariantValue::String(v.as_str().ok_or_else(bad)?.to_string()),
        BuiltinTypeKind::XmlElement => {
            VariantValue::XmlElement(v.as_str().ok_or_else(bad)?.to_string())
        }
        BuiltinTypeKind::DateTime => {
            VariantValue::DateTime(iso8601_to_ticks(v.as_str().ok_or_else(bad)?)?)
        }
        BuiltinTypeKind::Guid => VariantValue::Guid(guid_from_string(v.as_str().ok_or_else(bad)?)?),
        BuiltinTypeKind::ByteString => VariantValue::ByteString(
            BASE64
                .decode(v.as_str().ok_or_else(bad)?)
                .map_err(|_| bad())?,
        ),
        BuiltinTypeKind::NodeId => VariantValue::NodeId(node_id_from_json(v)?),
        BuiltinTypeKind::StatusCode => {
            VariantValue::StatusCode(u32::try_from(v.as_u64().ok_or_else(bad)?).map_err(|_| bad())?)
        }
        BuiltinTypeKind::QualifiedName => {
            let obj = v.as_object().ok_or_else(bad)?;
            VariantValue::QualifiedName(QualifiedName {
                namespace_index: obj.get("Uri").and_then(Value::as_u64).unwrap_or(0) as u16,
                name: obj
                    .get("Name")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
            })
        }
        BuiltinTypeKind::LocalizedText => {
            let obj = v.as_object().ok_or_else(bad)?;
            VariantValue::LocalizedText(LocalizedText {
                locale: obj
                    .get("Locale")
                    .and_then(Value::as_str)
                    .map(ToString::to_string),
                text: obj
                    .get("Text")
                    .and_then(Value::as_str)
                    .map(ToString::to_string),
            })
        }
        BuiltinTypeKind::ExtensionObject => VariantValue::ExtensionObject(ext_from_json(v)?),
        other => {
            let _ = other;
            return Err(DecodeError::MalformedMessage {
                message: "built-in type is not representable as a JSON value",
            });
        }
    })
}

fn parse_int_str(v: &Value) -> Result<i64, DecodeError> {
    let bad = || DecodeError::MalformedMessage {
        message: "invalid Int64 in JSON",
    };
    match v {
        Value::String(s) => s.parse().map_err(|_| bad()),
        Value::Number(n) => n.as_i64().ok_or_else(bad),
        _ => Err(bad()),
    }
}

fn parse_uint_str(v: &Value) -> Result<u64, DecodeError> {
    let bad = || DecodeError::MalformedMessage {
        message: "invalid UInt64 in JSON",
    };
    match v {
        Value::String(s) => s.parse().map_err(|_| bad()),
        Value::Number(n) => n.as_u64().ok_or_else(bad),
        _ => Err(bad()),
    }
}

// ---------------------------------------------------------------------------
// Variant ↔ reversible JSON (Part 6 §5.4.3): {"Type": id, "Body": value,
// optional "Dimensions": [...]}.
// ---------------------------------------------------------------------------

fn variant_to_json(v: &Variant) -> Value {
    let mut m = Map::new();
    let kind = v.value.first().map(VariantValue::kind);
    let type_id = kind.map_or(0u8, BuiltinTypeKind::value);
    m.insert(String::from("Type"), Value::Number(type_id.into()));

    let body = if v.array_dimensions.is_empty() && v.value.len() == 1 {
        scalar_to_json(&v.value[0])
    } else {
        Value::Array(v.value.iter().map(scalar_to_json).collect())
    };
    m.insert(String::from("Body"), body);

    if !v.array_dimensions.is_empty() {
        m.insert(
            String::from("Dimensions"),
            Value::Array(
                v.array_dimensions
                    .iter()
                    .map(|d| Value::Number((*d).into()))
                    .collect(),
            ),
        );
    }
    Value::Object(m)
}

fn variant_from_json(v: &Value) -> Result<Variant, DecodeError> {
    let bad = || DecodeError::MalformedMessage {
        message: "invalid reversible Variant in JSON",
    };
    let obj = v.as_object().ok_or_else(bad)?;
    let type_id = obj.get("Type").and_then(Value::as_u64).ok_or_else(bad)? as u8;
    if type_id == 0 {
        return Ok(Variant {
            array_dimensions: Vec::new(),
            value: Vec::new(),
        });
    }
    let kind = crate::binary::builtin_type_from_value(type_id)?;
    let body = obj.get("Body").ok_or_else(bad)?;
    let value = match body {
        Value::Array(items) => items
            .iter()
            .map(|it| scalar_from_json(it, kind))
            .collect::<Result<Vec<_>, _>>()?,
        scalar => alloc::vec![scalar_from_json(scalar, kind)?],
    };
    let array_dimensions = obj
        .get("Dimensions")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_u64)
                .map(|d| d as u32)
                .collect()
        })
        .unwrap_or_default();
    Ok(Variant {
        array_dimensions,
        value,
    })
}

// ---------------------------------------------------------------------------
// JSON PubSub messages (Part 14 §7.2.3).
// ---------------------------------------------------------------------------

fn message_type_str(kind: DataSetMessageKind) -> &'static str {
    match kind {
        DataSetMessageKind::KeyFrame => "ua-keyframe",
        DataSetMessageKind::DeltaFrame => "ua-deltaframe",
        DataSetMessageKind::Event => "ua-event",
        DataSetMessageKind::KeepAlive => "ua-keepalive",
    }
}

fn message_type_from_str(s: &str) -> Result<DataSetMessageKind, DecodeError> {
    Ok(match s {
        "ua-keyframe" => DataSetMessageKind::KeyFrame,
        "ua-deltaframe" => DataSetMessageKind::DeltaFrame,
        "ua-event" => DataSetMessageKind::Event,
        "ua-keepalive" => DataSetMessageKind::KeepAlive,
        _ => {
            return Err(DecodeError::MalformedMessage {
                message: "unknown JSON DataSetMessage MessageType",
            });
        }
    })
}

/// A JSON DataSetMessage (Part 14 §7.2.3.3) with a named field payload.
#[derive(Debug, Clone, PartialEq)]
pub struct JsonDataSetMessage {
    /// The producing DataSetWriter.
    pub data_set_writer_id: u16,
    /// Optional message sequence number.
    pub sequence_number: Option<u32>,
    /// Optional DataSet configuration version.
    pub meta_data_version: Option<ConfigurationVersion>,
    /// Optional source timestamp (DateTime ticks).
    pub timestamp: Option<i64>,
    /// Optional 32-bit status code.
    pub status: Option<u32>,
    /// KeyFrame / DeltaFrame / Event / KeepAlive.
    pub kind: DataSetMessageKind,
    /// Named field values (in DataSet field order).
    pub payload: Vec<(String, DataValue)>,
}

/// A JSON NetworkMessage (Part 14 §7.2.3.2, `MessageType: "ua-data"`).
#[derive(Debug, Clone, PartialEq)]
pub struct JsonNetworkMessage {
    /// Globally unique message id.
    pub message_id: String,
    /// Optional Publisher identifier.
    pub publisher_id: Option<String>,
    /// Optional DataSetClass id.
    pub data_set_class_id: Option<Guid>,
    /// The carried DataSetMessages.
    pub messages: Vec<JsonDataSetMessage>,
}

/// Encodes a payload field value: a reversible `Variant` when only the value
/// is set, or a JSON `DataValue` object when status/timestamps are present.
fn payload_value_to_json(dv: &DataValue) -> Value {
    let has_meta = dv.status.is_some()
        || dv.source_timestamp.is_some()
        || dv.server_timestamp.is_some()
        || dv.source_pico_sec.is_some()
        || dv.server_pico_sec.is_some();
    let variant = dv.value.clone().unwrap_or(Variant {
        array_dimensions: Vec::new(),
        value: Vec::new(),
    });
    if !has_meta {
        return variant_to_json(&variant);
    }
    let mut m = Map::new();
    m.insert(String::from("Value"), variant_to_json(&variant));
    if let Some(s) = dv.status {
        m.insert(String::from("Status"), Value::Number(s.into()));
    }
    if let Some(t) = dv.source_timestamp {
        m.insert(
            String::from("SourceTimestamp"),
            Value::String(ticks_to_iso8601(t)),
        );
    }
    if let Some(t) = dv.server_timestamp {
        m.insert(
            String::from("ServerTimestamp"),
            Value::String(ticks_to_iso8601(t)),
        );
    }
    Value::Object(m)
}

fn payload_value_from_json(v: &Value) -> Result<DataValue, DecodeError> {
    // A DataValue object has a "Value" member; a reversible Variant has "Type".
    if let Some(obj) = v.as_object() {
        if let Some(value_node) = obj.get("Value") {
            return Ok(DataValue {
                value: Some(variant_from_json(value_node)?),
                status: obj.get("Status").and_then(Value::as_u64).map(|s| s as u32),
                source_timestamp: obj
                    .get("SourceTimestamp")
                    .and_then(Value::as_str)
                    .map(iso8601_to_ticks)
                    .transpose()?,
                server_timestamp: obj
                    .get("ServerTimestamp")
                    .and_then(Value::as_str)
                    .map(iso8601_to_ticks)
                    .transpose()?,
                source_pico_sec: None,
                server_pico_sec: None,
            });
        }
    }
    Ok(DataValue {
        value: Some(variant_from_json(v)?),
        status: None,
        source_timestamp: None,
        server_timestamp: None,
        source_pico_sec: None,
        server_pico_sec: None,
    })
}

impl JsonNetworkMessage {
    /// Serialises the message to a JSON string (Part 14 §7.2.3.2).
    ///
    /// # Errors
    /// [`EncodeError`] is currently never returned (the signature mirrors the
    /// binary codec for symmetry).
    pub fn to_json_string(&self) -> Result<String, EncodeError> {
        let mut root = Map::new();
        root.insert(
            String::from("MessageId"),
            Value::String(self.message_id.clone()),
        );
        root.insert(
            String::from("MessageType"),
            Value::String(String::from("ua-data")),
        );
        if let Some(p) = &self.publisher_id {
            root.insert(String::from("PublisherId"), Value::String(p.clone()));
        }
        if let Some(g) = &self.data_set_class_id {
            root.insert(
                String::from("DataSetClassId"),
                Value::String(guid_to_string(g)),
            );
        }

        let messages = self
            .messages
            .iter()
            .map(|m| {
                let mut jm = Map::new();
                jm.insert(
                    String::from("DataSetWriterId"),
                    Value::Number(m.data_set_writer_id.into()),
                );
                if let Some(sn) = m.sequence_number {
                    jm.insert(String::from("SequenceNumber"), Value::Number(sn.into()));
                }
                if let Some(v) = &m.meta_data_version {
                    let mut mv = Map::new();
                    mv.insert(
                        String::from("MajorVersion"),
                        Value::Number(v.major_version.into()),
                    );
                    mv.insert(
                        String::from("MinorVersion"),
                        Value::Number(v.minor_version.into()),
                    );
                    jm.insert(String::from("MetaDataVersion"), Value::Object(mv));
                }
                if let Some(t) = m.timestamp {
                    jm.insert(
                        String::from("Timestamp"),
                        Value::String(ticks_to_iso8601(t)),
                    );
                }
                if let Some(s) = m.status {
                    jm.insert(String::from("Status"), Value::Number(s.into()));
                }
                jm.insert(
                    String::from("MessageType"),
                    Value::String(String::from(message_type_str(m.kind))),
                );
                let mut payload = Map::new();
                for (name, dv) in &m.payload {
                    payload.insert(name.clone(), payload_value_to_json(dv));
                }
                jm.insert(String::from("Payload"), Value::Object(payload));
                Value::Object(jm)
            })
            .collect();
        root.insert(String::from("Messages"), Value::Array(messages));

        serde_json::to_string(&Value::Object(root)).map_err(|_| EncodeError::ValueOutOfRange {
            message: "JSON serialisation failed",
        })
    }

    /// Parses a JSON NetworkMessage string.
    ///
    /// # Errors
    /// [`DecodeError`] if the JSON is malformed or not a `ua-data` message.
    pub fn from_json_str(s: &str) -> Result<Self, DecodeError> {
        let bad = || DecodeError::MalformedMessage {
            message: "malformed JSON NetworkMessage",
        };
        let root: Value = serde_json::from_str(s).map_err(|_| bad())?;
        let obj = root.as_object().ok_or_else(bad)?;
        if obj.get("MessageType").and_then(Value::as_str) != Some("ua-data") {
            return Err(DecodeError::MalformedMessage {
                message: "JSON message is not of MessageType ua-data",
            });
        }
        let message_id = obj
            .get("MessageId")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let publisher_id = obj
            .get("PublisherId")
            .and_then(Value::as_str)
            .map(ToString::to_string);
        let data_set_class_id = obj
            .get("DataSetClassId")
            .and_then(Value::as_str)
            .map(guid_from_string)
            .transpose()?;

        let messages = obj
            .get("Messages")
            .and_then(Value::as_array)
            .ok_or_else(bad)?
            .iter()
            .map(parse_dataset_message)
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self {
            message_id,
            publisher_id,
            data_set_class_id,
            messages,
        })
    }
}

fn parse_dataset_message(v: &Value) -> Result<JsonDataSetMessage, DecodeError> {
    let bad = || DecodeError::MalformedMessage {
        message: "malformed JSON DataSetMessage",
    };
    let obj = v.as_object().ok_or_else(bad)?;
    let data_set_writer_id = obj
        .get("DataSetWriterId")
        .and_then(Value::as_u64)
        .ok_or_else(bad)? as u16;
    let sequence_number = obj
        .get("SequenceNumber")
        .and_then(Value::as_u64)
        .map(|n| n as u32);
    let meta_data_version = obj
        .get("MetaDataVersion")
        .and_then(Value::as_object)
        .map(|mv| ConfigurationVersion {
            major_version: mv.get("MajorVersion").and_then(Value::as_u64).unwrap_or(0) as u32,
            minor_version: mv.get("MinorVersion").and_then(Value::as_u64).unwrap_or(0) as u32,
        });
    let timestamp = obj
        .get("Timestamp")
        .and_then(Value::as_str)
        .map(iso8601_to_ticks)
        .transpose()?;
    let status = obj.get("Status").and_then(Value::as_u64).map(|s| s as u32);
    let kind = obj
        .get("MessageType")
        .and_then(Value::as_str)
        .map_or(Ok(DataSetMessageKind::KeyFrame), message_type_from_str)?;

    let mut payload = Vec::new();
    if let Some(p) = obj.get("Payload").and_then(Value::as_object) {
        for (name, val) in p {
            payload.push((name.clone(), payload_value_from_json(val)?));
        }
    }

    Ok(JsonDataSetMessage {
        data_set_writer_id,
        sequence_number,
        meta_data_version,
        timestamp,
        status,
        kind,
        payload,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn datetime_round_trips_through_iso8601() {
        // 2026-06-14T12:34:56.7654321Z (ticks since 1601).
        let ticks = days_from_civil(2026, 6, 14).wrapping_add(EPOCH_DELTA_DAYS)
            * SECS_PER_DAY
            * TICKS_PER_SEC
            + (12 * 3600 + 34 * 60 + 56) * TICKS_PER_SEC
            + 7_654_321;
        let s = ticks_to_iso8601(ticks);
        assert_eq!(s, "2026-06-14T12:34:56.7654321Z");
        assert_eq!(iso8601_to_ticks(&s).expect("parse"), ticks);
    }

    #[test]
    fn guid_round_trips() {
        let g = Guid::from_bytes([
            0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC, 0xDD,
            0xEE, 0xFF,
        ]);
        assert_eq!(guid_from_string(&guid_to_string(&g)).expect("guid"), g);
    }

    #[test]
    fn variant_reversible_round_trips_many_types() {
        for v in [
            VariantValue::Boolean(true),
            VariantValue::Int32(-42),
            VariantValue::UInt64(18_000_000_000_000_000_000),
            VariantValue::Double(1.5),
            VariantValue::String(String::from("héllo")),
            VariantValue::ByteString(alloc::vec![1, 2, 3, 250]),
            VariantValue::DateTime(132_000_000_000_000_000),
        ] {
            let variant = Variant::scalar(v.clone());
            let json = variant_to_json(&variant);
            let back = variant_from_json(&json).expect("variant");
            assert_eq!(back, variant, "round trip failed for {v:?}");
        }
    }

    #[test]
    fn special_floats_round_trip() {
        for f in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let v = Variant::scalar(VariantValue::Double(f));
            let back = variant_from_json(&variant_to_json(&v)).expect("f");
            let VariantValue::Double(d) = back.value[0] else {
                panic!("double");
            };
            assert_eq!(d.is_nan(), f.is_nan());
            if !f.is_nan() {
                assert_eq!(d, f);
            }
        }
    }

    fn dv(v: VariantValue) -> DataValue {
        DataValue {
            value: Some(Variant::scalar(v)),
            status: None,
            source_timestamp: None,
            server_timestamp: None,
            source_pico_sec: None,
            server_pico_sec: None,
        }
    }

    #[test]
    fn network_message_round_trips() {
        let nm = JsonNetworkMessage {
            message_id: String::from("msg-1"),
            publisher_id: Some(String::from("pub-1")),
            data_set_class_id: None,
            messages: alloc::vec![JsonDataSetMessage {
                data_set_writer_id: 5,
                sequence_number: Some(7),
                meta_data_version: Some(ConfigurationVersion {
                    major_version: 1,
                    minor_version: 0,
                }),
                timestamp: Some(132_000_000_000_000_000),
                status: None,
                kind: DataSetMessageKind::KeyFrame,
                payload: alloc::vec![
                    (String::from("temp"), dv(VariantValue::Double(21.5))),
                    (
                        String::from("name"),
                        dv(VariantValue::String(String::from("sensor")))
                    ),
                ],
            }],
        };
        let s = nm.to_json_string().expect("encode");
        let back = JsonNetworkMessage::from_json_str(&s).expect("decode");
        assert_eq!(back, nm);
    }

    #[test]
    fn datavalue_payload_with_status_round_trips() {
        let mut d = dv(VariantValue::Int32(7));
        d.status = Some(0x8000_0000);
        d.source_timestamp = Some(132_000_000_000_000_000);
        let v = payload_value_to_json(&d);
        assert!(v.as_object().is_some_and(|o| o.contains_key("Value")));
        let back = payload_value_from_json(&v).expect("dv");
        assert_eq!(back.status, Some(0x8000_0000));
        assert_eq!(back.source_timestamp, Some(132_000_000_000_000_000));
    }

    #[test]
    fn rejects_non_ua_data() {
        let s = r#"{"MessageType":"ua-metadata","MessageId":"x"}"#;
        assert!(JsonNetworkMessage::from_json_str(s).is_err());
    }
}
