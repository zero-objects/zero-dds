// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! AMQP 0.9.1 class/method framing (§1.4–1.8). A method-frame payload is
//! `[class-id:short][method-id:short][arguments…]`. This module provides the
//! builders/parsers for the subset a publish/consume client needs.

extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;

use crate::types::{FieldValue, Reader, WireError, Writer, pack_bits};

/// Class identifiers (§1.4).
pub mod class {
    /// connection class.
    pub const CONNECTION: u16 = 10;
    /// channel class.
    pub const CHANNEL: u16 = 20;
    /// exchange class.
    pub const EXCHANGE: u16 = 40;
    /// queue class.
    pub const QUEUE: u16 = 50;
    /// basic class.
    pub const BASIC: u16 = 60;
    /// confirm class (RabbitMQ extension — publisher confirms).
    pub const CONFIRM: u16 = 85;
    /// tx (transaction) class.
    pub const TX: u16 = 90;
}

/// `(class-id, method-id)` pairs used here.
pub mod id {
    /// connection.start (server→client).
    pub const CONNECTION_START: (u16, u16) = (10, 10);
    /// connection.start-ok (client→server).
    pub const CONNECTION_START_OK: (u16, u16) = (10, 11);
    /// connection.tune (server→client).
    pub const CONNECTION_TUNE: (u16, u16) = (10, 30);
    /// connection.tune-ok (client→server).
    pub const CONNECTION_TUNE_OK: (u16, u16) = (10, 31);
    /// connection.open (client→server).
    pub const CONNECTION_OPEN: (u16, u16) = (10, 40);
    /// connection.open-ok (server→client).
    pub const CONNECTION_OPEN_OK: (u16, u16) = (10, 41);
    /// connection.close (either).
    pub const CONNECTION_CLOSE: (u16, u16) = (10, 50);
    /// connection.close-ok.
    pub const CONNECTION_CLOSE_OK: (u16, u16) = (10, 51);
    /// channel.open (client→server).
    pub const CHANNEL_OPEN: (u16, u16) = (20, 10);
    /// channel.open-ok (server→client).
    pub const CHANNEL_OPEN_OK: (u16, u16) = (20, 11);
    /// channel.flow (either).
    pub const CHANNEL_FLOW: (u16, u16) = (20, 20);
    /// channel.flow-ok (either).
    pub const CHANNEL_FLOW_OK: (u16, u16) = (20, 21);
    /// channel.close (either).
    pub const CHANNEL_CLOSE: (u16, u16) = (20, 40);
    /// channel.close-ok (either).
    pub const CHANNEL_CLOSE_OK: (u16, u16) = (20, 41);
    /// exchange.declare (client→server).
    pub const EXCHANGE_DECLARE: (u16, u16) = (40, 10);
    /// exchange.declare-ok (server→client).
    pub const EXCHANGE_DECLARE_OK: (u16, u16) = (40, 11);
    /// exchange.delete (client→server).
    pub const EXCHANGE_DELETE: (u16, u16) = (40, 20);
    /// exchange.delete-ok (server→client).
    pub const EXCHANGE_DELETE_OK: (u16, u16) = (40, 21);
    /// queue.declare (client→server).
    pub const QUEUE_DECLARE: (u16, u16) = (50, 10);
    /// queue.declare-ok (server→client).
    pub const QUEUE_DECLARE_OK: (u16, u16) = (50, 11);
    /// queue.bind (client→server).
    pub const QUEUE_BIND: (u16, u16) = (50, 20);
    /// queue.bind-ok (server→client).
    pub const QUEUE_BIND_OK: (u16, u16) = (50, 21);
    /// queue.purge (client→server).
    pub const QUEUE_PURGE: (u16, u16) = (50, 30);
    /// queue.purge-ok (server→client).
    pub const QUEUE_PURGE_OK: (u16, u16) = (50, 31);
    /// queue.delete (client→server).
    pub const QUEUE_DELETE: (u16, u16) = (50, 40);
    /// queue.delete-ok (server→client).
    pub const QUEUE_DELETE_OK: (u16, u16) = (50, 41);
    /// queue.unbind (client→server).
    pub const QUEUE_UNBIND: (u16, u16) = (50, 50);
    /// queue.unbind-ok (server→client).
    pub const QUEUE_UNBIND_OK: (u16, u16) = (50, 51);
    /// basic.qos (client→server).
    pub const BASIC_QOS: (u16, u16) = (60, 10);
    /// basic.qos-ok (server→client).
    pub const BASIC_QOS_OK: (u16, u16) = (60, 11);
    /// basic.consume (client→server).
    pub const BASIC_CONSUME: (u16, u16) = (60, 20);
    /// basic.consume-ok (server→client).
    pub const BASIC_CONSUME_OK: (u16, u16) = (60, 21);
    /// basic.cancel (client→server).
    pub const BASIC_CANCEL: (u16, u16) = (60, 30);
    /// basic.cancel-ok (server→client).
    pub const BASIC_CANCEL_OK: (u16, u16) = (60, 31);
    /// basic.publish (client→server).
    pub const BASIC_PUBLISH: (u16, u16) = (60, 40);
    /// basic.deliver (server→client; async push from a consume).
    pub const BASIC_DELIVER: (u16, u16) = (60, 60);
    /// basic.get (client→server).
    pub const BASIC_GET: (u16, u16) = (60, 70);
    /// basic.get-ok (server→client).
    pub const BASIC_GET_OK: (u16, u16) = (60, 71);
    /// basic.get-empty (server→client).
    pub const BASIC_GET_EMPTY: (u16, u16) = (60, 72);
    /// basic.ack (either; client→server to ack a delivery, server→client for
    /// publisher confirms).
    pub const BASIC_ACK: (u16, u16) = (60, 80);
    /// basic.reject (client→server).
    pub const BASIC_REJECT: (u16, u16) = (60, 90);
    /// basic.nack (RabbitMQ extension; either).
    pub const BASIC_NACK: (u16, u16) = (60, 120);
    /// confirm.select (client→server; RabbitMQ extension).
    pub const CONFIRM_SELECT: (u16, u16) = (85, 10);
    /// confirm.select-ok (server→client).
    pub const CONFIRM_SELECT_OK: (u16, u16) = (85, 11);
    /// tx.select (client→server).
    pub const TX_SELECT: (u16, u16) = (90, 10);
    /// tx.select-ok (server→client).
    pub const TX_SELECT_OK: (u16, u16) = (90, 11);
    /// tx.commit (client→server).
    pub const TX_COMMIT: (u16, u16) = (90, 20);
    /// tx.commit-ok (server→client).
    pub const TX_COMMIT_OK: (u16, u16) = (90, 21);
    /// tx.rollback (client→server).
    pub const TX_ROLLBACK: (u16, u16) = (90, 30);
    /// tx.rollback-ok (server→client).
    pub const TX_ROLLBACK_OK: (u16, u16) = (90, 31);
}

/// Reads the `(class-id, method-id)` from a method-frame payload (the first
/// 4 bytes).
///
/// # Errors
/// [`WireError::Truncated`] if the payload is shorter than 4 bytes.
pub fn method_id(payload: &[u8]) -> Result<(u16, u16), WireError> {
    let mut r = Reader::new(payload);
    Ok((r.u16()?, r.u16()?))
}

fn start(class: u16, method: u16) -> Writer {
    let mut w = Writer::new();
    w.u16(class).u16(method);
    w
}

/// connection.start-ok: client-properties (we advertise product/version),
/// mechanism (`PLAIN`), response (`\0user\0pass`), locale (`en_US`).
///
/// # Errors
/// [`WireError`] on a `shortstr` overflow.
pub fn connection_start_ok(user: &str, pass: &str) -> Result<Vec<u8>, WireError> {
    let mut w = start(id::CONNECTION_START_OK.0, id::CONNECTION_START_OK.1);
    w.field_table_strs(&[("product", "ZeroDDS"), ("version", "1.0")])?;
    w.short_str("PLAIN")?;
    let mut resp = Vec::with_capacity(user.len() + pass.len() + 2);
    resp.push(0);
    resp.extend_from_slice(user.as_bytes());
    resp.push(0);
    resp.extend_from_slice(pass.as_bytes());
    w.long_str(&resp);
    w.short_str("en_US")?;
    Ok(w.into_bytes())
}

/// connection.tune-ok: echo the broker's channel-max / frame-max / heartbeat.
#[must_use]
pub fn connection_tune_ok(channel_max: u16, frame_max: u32, heartbeat: u16) -> Vec<u8> {
    let mut w = start(id::CONNECTION_TUNE_OK.0, id::CONNECTION_TUNE_OK.1);
    w.u16(channel_max).u32(frame_max).u16(heartbeat);
    w.into_bytes()
}

/// connection.open: virtual-host, reserved shortstr, reserved bit.
///
/// # Errors
/// [`WireError`] on a `shortstr` overflow.
pub fn connection_open(vhost: &str) -> Result<Vec<u8>, WireError> {
    let mut w = start(id::CONNECTION_OPEN.0, id::CONNECTION_OPEN.1);
    w.short_str(vhost)?;
    w.short_str("")?; // reserved-1 (capabilities)
    w.u8(0); // reserved-2 (insist) bit
    Ok(w.into_bytes())
}

/// channel.open: one reserved shortstr.
///
/// # Errors
/// [`WireError`] on a `shortstr` overflow.
pub fn channel_open() -> Result<Vec<u8>, WireError> {
    let mut w = start(id::CHANNEL_OPEN.0, id::CHANNEL_OPEN.1);
    w.short_str("")?; // reserved-1
    Ok(w.into_bytes())
}

/// queue.declare: reserved short, queue name, bit flags
/// (passive, durable, exclusive, auto-delete, no-wait), empty arguments table.
///
/// # Errors
/// [`WireError`] on a `shortstr` overflow.
pub fn queue_declare(queue: &str, durable: bool) -> Result<Vec<u8>, WireError> {
    let mut w = start(id::QUEUE_DECLARE.0, id::QUEUE_DECLARE.1);
    w.u16(0); // reserved-1
    w.short_str(queue)?;
    w.u8(pack_bits(&[false, durable, false, false, false])); // flags
    w.empty_field_table(); // arguments
    Ok(w.into_bytes())
}

/// basic.publish: reserved short, exchange, routing-key, bits(mandatory,
/// immediate). The caller follows it with a content header + body frame.
///
/// # Errors
/// [`WireError`] on a `shortstr` overflow.
pub fn basic_publish(exchange: &str, routing_key: &str) -> Result<Vec<u8>, WireError> {
    let mut w = start(id::BASIC_PUBLISH.0, id::BASIC_PUBLISH.1);
    w.u16(0); // reserved-1
    w.short_str(exchange)?;
    w.short_str(routing_key)?;
    w.u8(pack_bits(&[false, false])); // mandatory, immediate
    Ok(w.into_bytes())
}

/// basic.get: reserved short, queue, no-ack bit.
///
/// # Errors
/// [`WireError`] on a `shortstr` overflow.
pub fn basic_get(queue: &str, no_ack: bool) -> Result<Vec<u8>, WireError> {
    let mut w = start(id::BASIC_GET.0, id::BASIC_GET.1);
    w.u16(0); // reserved-1
    w.short_str(queue)?;
    w.u8(pack_bits(&[no_ack]));
    Ok(w.into_bytes())
}

/// basic.ack: delivery-tag, multiple bit.
#[must_use]
pub fn basic_ack(delivery_tag: u64, multiple: bool) -> Vec<u8> {
    let mut w = start(id::BASIC_ACK.0, id::BASIC_ACK.1);
    w.u64(delivery_tag).u8(pack_bits(&[multiple]));
    w.into_bytes()
}

/// The `delivery-tag` from a `basic.get-ok` method payload (after the class/
/// method id): `delivery-tag(longlong)`, then redelivered/exchange/routing-key/
/// message-count which we do not need.
///
/// # Errors
/// [`WireError`] on truncation.
pub fn basic_get_ok_delivery_tag(payload: &[u8]) -> Result<u64, WireError> {
    let mut r = Reader::new(payload);
    let _class = r.u16()?;
    let _method = r.u16()?;
    r.u64()
}

/// A content header frame payload (§4.2.6.1) for a body of `body_size` bytes
/// with **no** properties set (property-flags = 0): `class-id`, `weight`(0),
/// `body-size`(longlong), `property-flags`(short = 0).
#[must_use]
pub fn content_header(class_id: u16, body_size: u64) -> Vec<u8> {
    let mut w = Writer::new();
    w.u16(class_id).u16(0).u64(body_size).u16(0);
    w.into_bytes()
}

/// Parses a content header frame payload, returning the `body-size`. The
/// property-flags + properties are skipped.
///
/// # Errors
/// [`WireError`] on truncation.
pub fn content_header_body_size(payload: &[u8]) -> Result<u64, WireError> {
    let mut r = Reader::new(payload);
    let _class = r.u16()?;
    let _weight = r.u16()?;
    r.u64()
}

/// AMQP basic content properties (§1.8.1 / §4.2.6.1). Every field is optional;
/// presence is encoded via the property-flags short. Covers the full standard
/// property set.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ContentProperties {
    /// MIME content-type (e.g. `application/json`).
    pub content_type: Option<String>,
    /// MIME content-encoding.
    pub content_encoding: Option<String>,
    /// Application message headers (a typed field-table).
    pub headers: Option<Vec<(String, FieldValue)>>,
    /// 1 = non-persistent, 2 = persistent.
    pub delivery_mode: Option<u8>,
    /// Message priority (0–9).
    pub priority: Option<u8>,
    /// Correlation identifier.
    pub correlation_id: Option<String>,
    /// Reply-to address.
    pub reply_to: Option<String>,
    /// Expiration (TTL) as a string of milliseconds.
    pub expiration: Option<String>,
    /// Application message identifier.
    pub message_id: Option<String>,
    /// POSIX timestamp.
    pub timestamp: Option<u64>,
    /// Application message type name.
    pub type_: Option<String>,
    /// Creating user id (validated by RabbitMQ against the login).
    pub user_id: Option<String>,
    /// Creating application id.
    pub app_id: Option<String>,
}

// Property-flag bit positions (MSB = bit 15 = first property).
const PF_CONTENT_TYPE: u16 = 1 << 15;
const PF_CONTENT_ENCODING: u16 = 1 << 14;
const PF_HEADERS: u16 = 1 << 13;
const PF_DELIVERY_MODE: u16 = 1 << 12;
const PF_PRIORITY: u16 = 1 << 11;
const PF_CORRELATION_ID: u16 = 1 << 10;
const PF_REPLY_TO: u16 = 1 << 9;
const PF_EXPIRATION: u16 = 1 << 8;
const PF_MESSAGE_ID: u16 = 1 << 7;
const PF_TIMESTAMP: u16 = 1 << 6;
const PF_TYPE: u16 = 1 << 5;
const PF_USER_ID: u16 = 1 << 4;
const PF_APP_ID: u16 = 1 << 3;

impl ContentProperties {
    /// Encodes the property-flags short + present-property body, in spec order.
    ///
    /// # Errors
    /// [`WireError`] on a `shortstr` overflow.
    fn encode(&self) -> Result<(u16, Vec<u8>), WireError> {
        let mut flags = 0u16;
        let mut w = Writer::new();
        if let Some(v) = &self.content_type {
            flags |= PF_CONTENT_TYPE;
            w.short_str(v)?;
        }
        if let Some(v) = &self.content_encoding {
            flags |= PF_CONTENT_ENCODING;
            w.short_str(v)?;
        }
        if let Some(h) = &self.headers {
            flags |= PF_HEADERS;
            let entries: Vec<(&str, FieldValue)> =
                h.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();
            w.field_table(&entries)?;
        }
        if let Some(v) = self.delivery_mode {
            flags |= PF_DELIVERY_MODE;
            w.u8(v);
        }
        if let Some(v) = self.priority {
            flags |= PF_PRIORITY;
            w.u8(v);
        }
        if let Some(v) = &self.correlation_id {
            flags |= PF_CORRELATION_ID;
            w.short_str(v)?;
        }
        if let Some(v) = &self.reply_to {
            flags |= PF_REPLY_TO;
            w.short_str(v)?;
        }
        if let Some(v) = &self.expiration {
            flags |= PF_EXPIRATION;
            w.short_str(v)?;
        }
        if let Some(v) = &self.message_id {
            flags |= PF_MESSAGE_ID;
            w.short_str(v)?;
        }
        if let Some(v) = self.timestamp {
            flags |= PF_TIMESTAMP;
            w.u64(v);
        }
        if let Some(v) = &self.type_ {
            flags |= PF_TYPE;
            w.short_str(v)?;
        }
        if let Some(v) = &self.user_id {
            flags |= PF_USER_ID;
            w.short_str(v)?;
        }
        if let Some(v) = &self.app_id {
            flags |= PF_APP_ID;
            w.short_str(v)?;
        }
        Ok((flags, w.into_bytes()))
    }

    /// Whether no property is set (the empty/flags-0 case).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        *self == ContentProperties::default()
    }
}

/// A content header frame payload (§4.2.6.1) carrying `props`: `class-id`,
/// `weight`(0), `body-size`(longlong), `property-flags`(short), properties.
///
/// # Errors
/// [`WireError`] on a `shortstr` overflow.
pub fn content_header_with_props(
    class_id: u16,
    body_size: u64,
    props: &ContentProperties,
) -> Result<Vec<u8>, WireError> {
    let (flags, body) = props.encode()?;
    let mut w = Writer::new();
    w.u16(class_id)
        .u16(0)
        .u64(body_size)
        .u16(flags)
        .bytes(&body);
    Ok(w.into_bytes())
}

/// Parses a content header frame payload into `(body-size, properties)`.
///
/// # Errors
/// [`WireError`] on truncation.
pub fn parse_content_header(payload: &[u8]) -> Result<(u64, ContentProperties), WireError> {
    let mut r = Reader::new(payload);
    let _class = r.u16()?;
    let _weight = r.u16()?;
    let body_size = r.u64()?;
    let flags = r.u16()?;
    let mut p = ContentProperties::default();
    if flags & PF_CONTENT_TYPE != 0 {
        p.content_type = Some(r.short_str()?);
    }
    if flags & PF_CONTENT_ENCODING != 0 {
        p.content_encoding = Some(r.short_str()?);
    }
    if flags & PF_HEADERS != 0 {
        p.headers = Some(r.field_table()?);
    }
    if flags & PF_DELIVERY_MODE != 0 {
        p.delivery_mode = Some(r.u8()?);
    }
    if flags & PF_PRIORITY != 0 {
        p.priority = Some(r.u8()?);
    }
    if flags & PF_CORRELATION_ID != 0 {
        p.correlation_id = Some(r.short_str()?);
    }
    if flags & PF_REPLY_TO != 0 {
        p.reply_to = Some(r.short_str()?);
    }
    if flags & PF_EXPIRATION != 0 {
        p.expiration = Some(r.short_str()?);
    }
    if flags & PF_MESSAGE_ID != 0 {
        p.message_id = Some(r.short_str()?);
    }
    if flags & PF_TIMESTAMP != 0 {
        p.timestamp = Some(r.u64()?);
    }
    if flags & PF_TYPE != 0 {
        p.type_ = Some(r.short_str()?);
    }
    if flags & PF_USER_ID != 0 {
        p.user_id = Some(r.short_str()?);
    }
    if flags & PF_APP_ID != 0 {
        p.app_id = Some(r.short_str()?);
    }
    Ok((body_size, p))
}

/// Reads a `connection.tune` payload, returning `(channel-max, frame-max,
/// heartbeat)`.
///
/// # Errors
/// [`WireError`] on truncation.
pub fn connection_tune_params(payload: &[u8]) -> Result<(u16, u32, u16), WireError> {
    let mut r = Reader::new(payload);
    let _class = r.u16()?;
    let _method = r.u16()?;
    Ok((r.u16()?, r.u32()?, r.u16()?))
}

/// The queue name echoed in a `queue.declare-ok` payload.
///
/// # Errors
/// [`WireError`] on truncation.
pub fn queue_declare_ok_name(payload: &[u8]) -> Result<String, WireError> {
    let mut r = Reader::new(payload);
    let _class = r.u16()?;
    let _method = r.u16()?;
    r.short_str()
}

// ---- channel.flow / channel.close (§1.5) -------------------------------

/// channel.flow: a single `active` bit (true = resume, false = pause).
#[must_use]
pub fn channel_flow(active: bool) -> Vec<u8> {
    let mut w = start(id::CHANNEL_FLOW.0, id::CHANNEL_FLOW.1);
    w.u8(pack_bits(&[active]));
    w.into_bytes()
}

/// channel.flow-ok: echoes the `active` bit.
#[must_use]
pub fn channel_flow_ok(active: bool) -> Vec<u8> {
    let mut w = start(id::CHANNEL_FLOW_OK.0, id::CHANNEL_FLOW_OK.1);
    w.u8(pack_bits(&[active]));
    w.into_bytes()
}

/// channel.close: reply-code, reply-text, offending class-id + method-id
/// (0/0 when not caused by a specific method).
///
/// # Errors
/// [`WireError`] on a `shortstr` overflow.
pub fn channel_close(reply_code: u16, reply_text: &str) -> Result<Vec<u8>, WireError> {
    let mut w = start(id::CHANNEL_CLOSE.0, id::CHANNEL_CLOSE.1);
    w.u16(reply_code);
    w.short_str(reply_text)?;
    w.u16(0).u16(0); // class-id, method-id
    Ok(w.into_bytes())
}

/// channel.close-ok: no arguments.
#[must_use]
pub fn channel_close_ok() -> Vec<u8> {
    start(id::CHANNEL_CLOSE_OK.0, id::CHANNEL_CLOSE_OK.1).into_bytes()
}

// ---- exchange class (§1.6) ---------------------------------------------

/// exchange.declare: reserved short, exchange name, type
/// (`direct`/`fanout`/`topic`/`headers`), bits
/// (passive, durable, auto-delete, internal, no-wait), empty arguments.
///
/// # Errors
/// [`WireError`] on a `shortstr` overflow.
pub fn exchange_declare(name: &str, kind: &str, durable: bool) -> Result<Vec<u8>, WireError> {
    let mut w = start(id::EXCHANGE_DECLARE.0, id::EXCHANGE_DECLARE.1);
    w.u16(0); // reserved-1
    w.short_str(name)?;
    w.short_str(kind)?;
    w.u8(pack_bits(&[false, durable, false, false, false]));
    w.empty_field_table();
    Ok(w.into_bytes())
}

/// exchange.delete: reserved short, exchange name, bits(if-unused, no-wait).
///
/// # Errors
/// [`WireError`] on a `shortstr` overflow.
pub fn exchange_delete(name: &str, if_unused: bool) -> Result<Vec<u8>, WireError> {
    let mut w = start(id::EXCHANGE_DELETE.0, id::EXCHANGE_DELETE.1);
    w.u16(0); // reserved-1
    w.short_str(name)?;
    w.u8(pack_bits(&[if_unused, false]));
    Ok(w.into_bytes())
}

// ---- queue bind / unbind / purge / delete (§1.7) -----------------------

/// queue.bind: reserved short, queue, exchange, routing-key, no-wait bit,
/// empty arguments.
///
/// # Errors
/// [`WireError`] on a `shortstr` overflow.
pub fn queue_bind(queue: &str, exchange: &str, routing_key: &str) -> Result<Vec<u8>, WireError> {
    let mut w = start(id::QUEUE_BIND.0, id::QUEUE_BIND.1);
    w.u16(0); // reserved-1
    w.short_str(queue)?;
    w.short_str(exchange)?;
    w.short_str(routing_key)?;
    w.u8(pack_bits(&[false])); // no-wait
    w.empty_field_table();
    Ok(w.into_bytes())
}

/// queue.unbind: reserved short, queue, exchange, routing-key, empty arguments
/// (note: unbind has **no** no-wait bit).
///
/// # Errors
/// [`WireError`] on a `shortstr` overflow.
pub fn queue_unbind(queue: &str, exchange: &str, routing_key: &str) -> Result<Vec<u8>, WireError> {
    let mut w = start(id::QUEUE_UNBIND.0, id::QUEUE_UNBIND.1);
    w.u16(0); // reserved-1
    w.short_str(queue)?;
    w.short_str(exchange)?;
    w.short_str(routing_key)?;
    w.empty_field_table();
    Ok(w.into_bytes())
}

/// queue.purge: reserved short, queue, no-wait bit.
///
/// # Errors
/// [`WireError`] on a `shortstr` overflow.
pub fn queue_purge(queue: &str) -> Result<Vec<u8>, WireError> {
    let mut w = start(id::QUEUE_PURGE.0, id::QUEUE_PURGE.1);
    w.u16(0); // reserved-1
    w.short_str(queue)?;
    w.u8(pack_bits(&[false])); // no-wait
    Ok(w.into_bytes())
}

/// queue.delete: reserved short, queue, bits(if-unused, if-empty, no-wait).
///
/// # Errors
/// [`WireError`] on a `shortstr` overflow.
pub fn queue_delete(queue: &str) -> Result<Vec<u8>, WireError> {
    let mut w = start(id::QUEUE_DELETE.0, id::QUEUE_DELETE.1);
    w.u16(0); // reserved-1
    w.short_str(queue)?;
    w.u8(pack_bits(&[false, false, false]));
    Ok(w.into_bytes())
}

/// The `message-count` from a `queue.purge-ok` / `queue.delete-ok` payload
/// (after the class/method id).
///
/// # Errors
/// [`WireError`] on truncation.
pub fn queue_op_ok_message_count(payload: &[u8]) -> Result<u32, WireError> {
    let mut r = Reader::new(payload);
    let _class = r.u16()?;
    let _method = r.u16()?;
    r.u32()
}

// ---- basic qos / consume / cancel / deliver / reject / nack (§1.8) ------

/// basic.qos: prefetch-size (long), prefetch-count (short), global bit.
#[must_use]
pub fn basic_qos(prefetch_size: u32, prefetch_count: u16, global: bool) -> Vec<u8> {
    let mut w = start(id::BASIC_QOS.0, id::BASIC_QOS.1);
    w.u32(prefetch_size)
        .u16(prefetch_count)
        .u8(pack_bits(&[global]));
    w.into_bytes()
}

/// basic.consume: reserved short, queue, consumer-tag, bits
/// (no-local, no-ack, exclusive, no-wait), empty arguments.
///
/// # Errors
/// [`WireError`] on a `shortstr` overflow.
pub fn basic_consume(queue: &str, consumer_tag: &str, no_ack: bool) -> Result<Vec<u8>, WireError> {
    let mut w = start(id::BASIC_CONSUME.0, id::BASIC_CONSUME.1);
    w.u16(0); // reserved-1
    w.short_str(queue)?;
    w.short_str(consumer_tag)?;
    w.u8(pack_bits(&[false, no_ack, false, false]));
    w.empty_field_table();
    Ok(w.into_bytes())
}

/// basic.cancel: consumer-tag, no-wait bit.
///
/// # Errors
/// [`WireError`] on a `shortstr` overflow.
pub fn basic_cancel(consumer_tag: &str, no_wait: bool) -> Result<Vec<u8>, WireError> {
    let mut w = start(id::BASIC_CANCEL.0, id::BASIC_CANCEL.1);
    w.short_str(consumer_tag)?;
    w.u8(pack_bits(&[no_wait]));
    Ok(w.into_bytes())
}

/// basic.reject: delivery-tag, requeue bit.
#[must_use]
pub fn basic_reject(delivery_tag: u64, requeue: bool) -> Vec<u8> {
    let mut w = start(id::BASIC_REJECT.0, id::BASIC_REJECT.1);
    w.u64(delivery_tag).u8(pack_bits(&[requeue]));
    w.into_bytes()
}

/// basic.nack (RabbitMQ extension): delivery-tag, bits(multiple, requeue).
#[must_use]
pub fn basic_nack(delivery_tag: u64, multiple: bool, requeue: bool) -> Vec<u8> {
    let mut w = start(id::BASIC_NACK.0, id::BASIC_NACK.1);
    w.u64(delivery_tag).u8(pack_bits(&[multiple, requeue]));
    w.into_bytes()
}

/// The consumer-tag echoed in a `basic.consume-ok` payload.
///
/// # Errors
/// [`WireError`] on truncation.
pub fn basic_consume_ok_tag(payload: &[u8]) -> Result<String, WireError> {
    let mut r = Reader::new(payload);
    let _class = r.u16()?;
    let _method = r.u16()?;
    r.short_str()
}

/// The `delivery-tag` from a `basic.deliver` payload (after the class/method
/// id): consumer-tag(shortstr), delivery-tag(longlong), redelivered/exchange/
/// routing-key follow but are not needed.
///
/// # Errors
/// [`WireError`] on truncation.
pub fn basic_deliver_delivery_tag(payload: &[u8]) -> Result<u64, WireError> {
    let mut r = Reader::new(payload);
    let _class = r.u16()?;
    let _method = r.u16()?;
    let _consumer_tag = r.short_str()?;
    r.u64()
}

/// The `delivery-tag` from a server-pushed `basic.ack` (publisher confirm).
///
/// # Errors
/// [`WireError`] on truncation.
pub fn basic_ack_delivery_tag(payload: &[u8]) -> Result<u64, WireError> {
    let mut r = Reader::new(payload);
    let _class = r.u16()?;
    let _method = r.u16()?;
    r.u64()
}

// ---- confirm class (§RabbitMQ) + tx class (§1.9) -----------------------

/// confirm.select: a single no-wait bit (puts the channel into confirm mode).
#[must_use]
pub fn confirm_select(no_wait: bool) -> Vec<u8> {
    let mut w = start(id::CONFIRM_SELECT.0, id::CONFIRM_SELECT.1);
    w.u8(pack_bits(&[no_wait]));
    w.into_bytes()
}

/// tx.select: no arguments (start a transaction).
#[must_use]
pub fn tx_select() -> Vec<u8> {
    start(id::TX_SELECT.0, id::TX_SELECT.1).into_bytes()
}

/// tx.commit: no arguments.
#[must_use]
pub fn tx_commit() -> Vec<u8> {
    start(id::TX_COMMIT.0, id::TX_COMMIT.1).into_bytes()
}

/// tx.rollback: no arguments.
#[must_use]
pub fn tx_rollback() -> Vec<u8> {
    start(id::TX_ROLLBACK.0, id::TX_ROLLBACK.1).into_bytes()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn start_ok_carries_plain_response() {
        let p = connection_start_ok("zerodds", "secret").unwrap();
        assert_eq!(method_id(&p).unwrap(), id::CONNECTION_START_OK);
        // The PLAIN response \0zerodds\0secret must appear verbatim.
        let needle = b"\x00zerodds\x00secret";
        assert!(
            p.windows(needle.len()).any(|w| w == needle),
            "PLAIN response must be present"
        );
    }

    #[test]
    fn tune_ok_roundtrips_params() {
        let tune = {
            // simulate a connection.tune payload: class/method + params.
            let mut w = Writer::new();
            w.u16(10).u16(30).u16(2047).u32(131072).u16(60);
            w.into_bytes()
        };
        assert_eq!(connection_tune_params(&tune).unwrap(), (2047, 131072, 60));
        let ok = connection_tune_ok(2047, 131072, 60);
        assert_eq!(method_id(&ok).unwrap(), id::CONNECTION_TUNE_OK);
    }

    #[test]
    fn queue_declare_and_publish_ids() {
        assert_eq!(
            method_id(&queue_declare("zd", true).unwrap()).unwrap(),
            id::QUEUE_DECLARE
        );
        assert_eq!(
            method_id(&basic_publish("", "zd").unwrap()).unwrap(),
            id::BASIC_PUBLISH
        );
        assert_eq!(
            method_id(&basic_get("zd", true).unwrap()).unwrap(),
            id::BASIC_GET
        );
    }

    #[test]
    fn content_header_roundtrip() {
        let h = content_header(class::BASIC, 11);
        assert_eq!(content_header_body_size(&h).unwrap(), 11);
    }

    #[test]
    fn channel_close_and_flow_ids() {
        assert_eq!(
            method_id(&channel_close(200, "bye").unwrap()).unwrap(),
            id::CHANNEL_CLOSE
        );
        assert_eq!(
            method_id(&channel_close_ok()).unwrap(),
            id::CHANNEL_CLOSE_OK
        );
        assert_eq!(method_id(&channel_flow(true)).unwrap(), id::CHANNEL_FLOW);
        assert_eq!(
            method_id(&channel_flow_ok(false)).unwrap(),
            id::CHANNEL_FLOW_OK
        );
    }

    #[test]
    fn exchange_declare_carries_type() {
        let p = exchange_declare("zd.ex", "topic", true).unwrap();
        assert_eq!(method_id(&p).unwrap(), id::EXCHANGE_DECLARE);
        // The "topic" type string must appear on the wire.
        let needle = b"\x05topic";
        assert!(p.windows(needle.len()).any(|w| w == needle));
        assert_eq!(
            method_id(&exchange_delete("zd.ex", true).unwrap()).unwrap(),
            id::EXCHANGE_DELETE
        );
    }

    #[test]
    fn queue_bind_unbind_purge_delete_ids() {
        assert_eq!(
            method_id(&queue_bind("q", "ex", "rk").unwrap()).unwrap(),
            id::QUEUE_BIND
        );
        assert_eq!(
            method_id(&queue_unbind("q", "ex", "rk").unwrap()).unwrap(),
            id::QUEUE_UNBIND
        );
        assert_eq!(
            method_id(&queue_purge("q").unwrap()).unwrap(),
            id::QUEUE_PURGE
        );
        assert_eq!(
            method_id(&queue_delete("q").unwrap()).unwrap(),
            id::QUEUE_DELETE
        );
    }

    #[test]
    fn queue_op_ok_count_parsed() {
        // simulate a queue.purge-ok: class/method + message-count.
        let mut w = Writer::new();
        w.u16(50).u16(31).u32(7);
        assert_eq!(queue_op_ok_message_count(&w.into_bytes()).unwrap(), 7);
    }

    #[test]
    fn basic_consume_cancel_qos_ids() {
        assert_eq!(
            method_id(&basic_consume("q", "ctag", false).unwrap()).unwrap(),
            id::BASIC_CONSUME
        );
        assert_eq!(
            method_id(&basic_cancel("ctag", false).unwrap()).unwrap(),
            id::BASIC_CANCEL
        );
        assert_eq!(method_id(&basic_qos(0, 1, false)).unwrap(), id::BASIC_QOS);
        assert_eq!(method_id(&basic_reject(3, true)).unwrap(), id::BASIC_REJECT);
        assert_eq!(
            method_id(&basic_nack(3, false, true)).unwrap(),
            id::BASIC_NACK
        );
    }

    #[test]
    fn basic_deliver_tag_skips_consumer_tag() {
        // class/method + consumer-tag(shortstr) + delivery-tag(longlong) + …
        let mut w = Writer::new();
        w.u16(60).u16(60);
        w.short_str("ctag").unwrap();
        w.u64(42).u8(0); // delivery-tag, redelivered
        assert_eq!(basic_deliver_delivery_tag(&w.into_bytes()).unwrap(), 42);
    }

    #[test]
    fn consume_ok_tag_parsed() {
        let mut w = Writer::new();
        w.u16(60).u16(21);
        w.short_str("server-tag").unwrap();
        assert_eq!(basic_consume_ok_tag(&w.into_bytes()).unwrap(), "server-tag");
    }

    #[test]
    fn confirm_and_tx_ids() {
        assert_eq!(
            method_id(&confirm_select(false)).unwrap(),
            id::CONFIRM_SELECT
        );
        assert_eq!(method_id(&tx_select()).unwrap(), id::TX_SELECT);
        assert_eq!(method_id(&tx_commit()).unwrap(), id::TX_COMMIT);
        assert_eq!(method_id(&tx_rollback()).unwrap(), id::TX_ROLLBACK);
    }

    #[test]
    fn ack_delivery_tag_parsed() {
        let mut w = Writer::new();
        w.u16(60).u16(80).u64(99).u8(0);
        assert_eq!(basic_ack_delivery_tag(&w.into_bytes()).unwrap(), 99);
    }

    #[test]
    fn content_properties_roundtrip() {
        let props = ContentProperties {
            content_type: Some("application/json".into()),
            delivery_mode: Some(2), // persistent
            priority: Some(5),
            message_id: Some("msg-1".into()),
            timestamp: Some(1_700_000_000),
            app_id: Some("zerodds".into()),
            headers: Some(vec![
                ("trace".into(), FieldValue::str("abc")),
                ("retry".into(), FieldValue::I32(3)),
            ]),
            ..ContentProperties::default()
        };
        let h = content_header_with_props(class::BASIC, 17, &props).unwrap();
        let (body_size, decoded) = parse_content_header(&h).unwrap();
        assert_eq!(body_size, 17);
        assert_eq!(decoded, props, "all properties must survive the round-trip");
    }

    #[test]
    fn content_header_no_props_matches_legacy() {
        // content_header_with_props(empty) must equal the legacy content_header.
        let legacy = content_header(class::BASIC, 11);
        let new =
            content_header_with_props(class::BASIC, 11, &ContentProperties::default()).unwrap();
        assert_eq!(legacy, new);
        assert!(ContentProperties::default().is_empty());
    }
}
