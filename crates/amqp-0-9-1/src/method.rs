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

    // The expected octets of each method are written from the protocol's
    // XML definitions (`amqp0-9-1.xml`, RabbitMQ's `amqp0-9-1.extended.xml`
    // for `basic.nack` and the `confirm` class): class-id and method-id as
    // shorts, then the fields in their order, a shortstr as one length
    // octet and the octets, adjacent bits in one octet from the low bit
    // (§4.2.5.2), an empty table as the long-uint 0.

    /// `class-id method-id` followed by `args`.
    fn method(class: u16, method: u16, args: &[u8]) -> Vec<u8> {
        let mut out = class.to_be_bytes().to_vec();
        out.extend_from_slice(&method.to_be_bytes());
        out.extend_from_slice(args);
        out
    }

    /// §1.4 (connection.start-ok 10/11): client-properties (table),
    /// mechanism (shortstr), response (longstr), locale (shortstr); the
    /// PLAIN response is `\0user\0password` (RFC 4616).
    #[test]
    fn amqp091_1_4_start_ok_is_properties_mechanism_plain_response_and_locale() {
        let mut args = vec![0x00, 0x00, 0x00, 0x24];
        args.extend_from_slice(b"\x07productS\x00\x00\x00\x07ZeroDDS");
        args.extend_from_slice(b"\x07versionS\x00\x00\x00\x031.0");
        args.extend_from_slice(b"\x05PLAIN");
        args.extend_from_slice(b"\x00\x00\x00\x0f\x00zerodds\x00secret");
        args.extend_from_slice(b"\x05en_US");
        assert_eq!(
            connection_start_ok("zerodds", "secret").unwrap(),
            method(10, 11, &args)
        );
    }

    /// §1.4 (connection.tune 10/30, tune-ok 10/31): channel-max (short),
    /// frame-max (long), heartbeat (short).
    #[test]
    fn amqp091_1_4_tune_and_tune_ok_carry_channel_max_frame_max_heartbeat() {
        let args = [0x07, 0xff, 0x00, 0x02, 0x00, 0x00, 0x00, 0x3c];
        assert_eq!(
            connection_tune_params(&method(10, 30, &args)).unwrap(),
            (2047, 131_072, 60)
        );
        assert_eq!(connection_tune_ok(2047, 131_072, 60), method(10, 31, &args));
    }

    /// §1.4 (connection.open 10/40): virtual-host (shortstr), reserved-1
    /// (shortstr), reserved-2 (bit).
    #[test]
    fn amqp091_1_4_open_is_the_virtual_host_and_two_reserved_fields() {
        assert_eq!(
            connection_open("/").unwrap(),
            method(10, 40, &[0x01, b'/', 0x00, 0x00])
        );
    }

    /// §1.5 (channel 20): open 10 (reserved shortstr), flow 20 and
    /// flow-ok 21 (active bit), close 40 (reply-code, reply-text,
    /// class-id, method-id), close-ok 41.
    #[test]
    fn amqp091_1_5_channel_open_flow_and_close_are_their_fields() {
        assert_eq!(channel_open().unwrap(), method(20, 10, &[0x00]));
        assert_eq!(channel_flow(true), method(20, 20, &[0x01]));
        assert_eq!(channel_flow_ok(false), method(20, 21, &[0x00]));
        assert_eq!(
            channel_close(200, "bye").unwrap(),
            method(20, 40, &[0x00, 0xc8, 0x03, b'b', b'y', b'e', 0, 0, 0, 0])
        );
        assert_eq!(channel_close_ok(), method(20, 41, &[]));
    }

    /// §1.6 (exchange 40): declare 10 (reserved-1 short, exchange, type,
    /// bits passive durable reserved-2 reserved-3 no-wait, arguments),
    /// delete 20 (reserved-1, exchange, bits if-unused no-wait).
    #[test]
    fn amqp091_1_6_exchange_declare_and_delete_are_their_fields() {
        let mut declare = vec![0x00, 0x00, 0x05];
        declare.extend_from_slice(b"zd.ex\x05topic");
        declare.extend_from_slice(&[0x02, 0x00, 0x00, 0x00, 0x00]);
        assert_eq!(
            exchange_declare("zd.ex", "topic", true).unwrap(),
            method(40, 10, &declare)
        );
        assert_eq!(
            exchange_delete("zd.ex", true).unwrap(),
            method(40, 20, b"\x00\x00\x05zd.ex\x01")
        );
    }

    /// §1.7 (queue 50): declare 10, bind 20, purge 30, delete 40, unbind 50
    /// with their fields; unbind has no no-wait bit.
    #[test]
    fn amqp091_1_7_queue_declare_bind_purge_delete_unbind_are_their_fields() {
        assert_eq!(
            queue_declare("zd", true).unwrap(),
            method(50, 10, b"\x00\x00\x02zd\x02\x00\x00\x00\x00")
        );
        assert_eq!(
            queue_bind("q", "ex", "rk").unwrap(),
            method(50, 20, b"\x00\x00\x01q\x02ex\x02rk\x00\x00\x00\x00\x00")
        );
        assert_eq!(
            queue_purge("q").unwrap(),
            method(50, 30, b"\x00\x00\x01q\x00")
        );
        assert_eq!(
            queue_delete("q").unwrap(),
            method(50, 40, b"\x00\x00\x01q\x00")
        );
        assert_eq!(
            queue_unbind("q", "ex", "rk").unwrap(),
            method(50, 50, b"\x00\x00\x01q\x02ex\x02rk\x00\x00\x00\x00")
        );
    }

    /// §1.7: declare-ok 11 names the queue; purge-ok 31 and delete-ok 41
    /// carry the message-count (long).
    #[test]
    fn amqp091_1_7_declare_ok_names_the_queue_purge_ok_and_delete_ok_count() {
        let declare_ok = method(50, 11, b"\x02zd\x00\x00\x00\x03\x00\x00\x00\x01");
        assert_eq!(queue_declare_ok_name(&declare_ok).unwrap(), "zd");
        assert_eq!(
            queue_op_ok_message_count(&method(50, 31, &[0, 0, 0, 7])).unwrap(),
            7
        );
        assert_eq!(
            queue_op_ok_message_count(&method(50, 41, &[0, 0, 1, 0])).unwrap(),
            256
        );
    }

    /// §1.8 (basic.publish 60/40): reserved-1, exchange, routing-key, bits
    /// mandatory immediate.
    #[test]
    fn amqp091_1_8_publish_is_its_fields() {
        assert_eq!(
            basic_publish("", "zd").unwrap(),
            method(60, 40, b"\x00\x00\x00\x02zd\x00")
        );
    }

    /// §4.2.6.1: a content header is class-id, weight 0, body-size
    /// (longlong), property-flags; without properties the flags are 0.
    #[test]
    fn amqp091_4_2_6_1_a_content_header_without_properties_has_flags_zero() {
        let wire = [
            0x00, 0x3c, 0x00, 0x00, 0, 0, 0, 0, 0, 0, 0, 0x0b, 0x00, 0x00,
        ];
        assert_eq!(content_header(class::BASIC, 11), wire);
        assert_eq!(
            content_header_with_props(class::BASIC, 11, &ContentProperties::default()).unwrap(),
            wire
        );
        assert_eq!(content_header_body_size(&wire).unwrap(), 11);
        assert!(ContentProperties::default().is_empty());
    }

    /// §4.2.6.1 with the basic class's properties of the XML definitions:
    /// one flag bit per property from bit 15 down (content-type 15,
    /// headers 13, delivery-mode 12, priority 11, message-id 7, timestamp
    /// 6, app-id 3), the present properties in that order.
    #[test]
    fn amqp091_4_2_6_1_the_properties_follow_their_flag_bits_in_order() {
        let mut wire = vec![0x00, 0x3c, 0x00, 0x00, 0, 0, 0, 0, 0, 0, 0, 0x11];
        wire.extend_from_slice(&[0xb8, 0xc8]);
        wire.extend_from_slice(b"\x10application/json");
        wire.extend_from_slice(b"\x00\x00\x00\x0e\x05traceS\x00\x00\x00\x03abc");
        wire.extend_from_slice(&[0x02, 0x05]);
        wire.extend_from_slice(b"\x05msg-1");
        wire.extend_from_slice(&[0x00, 0x00, 0x00, 0x00, 0x65, 0x53, 0xf1, 0x00]);
        wire.extend_from_slice(b"\x07zerodds");
        let props = ContentProperties {
            content_type: Some("application/json".into()),
            headers: Some(vec![("trace".into(), FieldValue::str("abc"))]),
            delivery_mode: Some(2),
            priority: Some(5),
            message_id: Some("msg-1".into()),
            timestamp: Some(1_700_000_000),
            app_id: Some("zerodds".into()),
            ..ContentProperties::default()
        };
        assert_eq!(
            content_header_with_props(class::BASIC, 17, &props).unwrap(),
            wire
        );
        assert_eq!(parse_content_header(&wire).unwrap(), (17, props));
    }

    /// §1.8: get 70 (reserved-1, queue, no-ack bit), ack 80
    /// (delivery-tag, multiple bit), reject 90 (delivery-tag, requeue
    /// bit), nack 120 (delivery-tag, bits multiple requeue; RabbitMQ).
    #[test]
    fn amqp091_1_8_get_ack_reject_nack_are_their_fields() {
        assert_eq!(
            basic_get("zd", true).unwrap(),
            method(60, 70, b"\x00\x00\x02zd\x01")
        );
        assert_eq!(
            basic_ack(99, false),
            method(60, 80, &[0, 0, 0, 0, 0, 0, 0, 99, 0x00])
        );
        assert_eq!(
            basic_reject(3, true),
            method(60, 90, &[0, 0, 0, 0, 0, 0, 0, 3, 0x01])
        );
        assert_eq!(
            basic_nack(3, false, true),
            method(60, 120, &[0, 0, 0, 0, 0, 0, 0, 3, 0x02])
        );
    }

    /// §1.8: get-ok 71 and a broker's ack 80 (publisher confirm) begin with
    /// the delivery-tag (longlong).
    #[test]
    fn amqp091_1_8_get_ok_and_ack_begin_with_the_delivery_tag() {
        let get_ok = method(
            60,
            71,
            b"\x00\x00\x00\x00\x00\x00\x00\x2a\x00\x00\x02zd\x00\x00\x00\x00",
        );
        assert_eq!(basic_get_ok_delivery_tag(&get_ok).unwrap(), 42);
        let ack = method(60, 80, &[0, 0, 0, 0, 0, 0, 0, 99, 0x00]);
        assert_eq!(basic_ack_delivery_tag(&ack).unwrap(), 99);
    }

    /// §1.8: qos 10 (prefetch-size long, prefetch-count short, global
    /// bit), consume 20 (reserved-1, queue, consumer-tag, bits no-local
    /// no-ack exclusive no-wait, arguments), cancel 30 (consumer-tag,
    /// no-wait bit).
    #[test]
    fn amqp091_1_8_qos_consume_and_cancel_are_their_fields() {
        assert_eq!(
            basic_qos(0, 1, false),
            method(60, 10, &[0, 0, 0, 0, 0x00, 0x01, 0x00])
        );
        assert_eq!(
            basic_consume("q", "ctag", true).unwrap(),
            method(60, 20, b"\x00\x00\x01q\x04ctag\x02\x00\x00\x00\x00")
        );
        assert_eq!(
            basic_cancel("ctag", false).unwrap(),
            method(60, 30, b"\x04ctag\x00")
        );
    }

    /// §1.8: consume-ok 21 returns the consumer-tag; deliver 60 is
    /// consumer-tag, delivery-tag, redelivered, exchange, routing-key.
    #[test]
    fn amqp091_1_8_consume_ok_and_deliver_carry_their_tags() {
        let consume_ok = method(60, 21, b"\x0aserver-tag");
        assert_eq!(basic_consume_ok_tag(&consume_ok).unwrap(), "server-tag");
        let deliver = method(
            60,
            60,
            b"\x04ctag\x00\x00\x00\x00\x00\x00\x00\x2a\x00\x00\x02zd",
        );
        assert_eq!(basic_deliver_delivery_tag(&deliver).unwrap(), 42);
    }

    /// RabbitMQ's `confirm` class (85, `amqp0-9-1.extended.xml`): select 10
    /// with its no-wait bit.
    #[test]
    fn amqp091_confirm_select_is_class_85_method_10_with_no_wait() {
        assert_eq!(confirm_select(false), method(85, 10, &[0x00]));
        assert_eq!(confirm_select(true), method(85, 10, &[0x01]));
    }

    /// §1.9 (tx 90): select 10, commit 20, rollback 30, no fields.
    #[test]
    fn amqp091_1_9_tx_select_commit_and_rollback_have_no_fields() {
        assert_eq!(tx_select(), method(90, 10, &[]));
        assert_eq!(tx_commit(), method(90, 20, &[]));
        assert_eq!(tx_rollback(), method(90, 30, &[]));
    }

    /// The class and method of a payload are its first two shorts.
    #[test]
    fn amqp091_1_4_a_method_payload_begins_with_class_and_method() {
        assert_eq!(
            method_id(&[0x00, 0x3c, 0x00, 0x50, 0xff]).unwrap(),
            (60, 80)
        );
        assert_eq!(method_id(&[0x00, 0x3c]), Err(WireError::Truncated));
    }
}
