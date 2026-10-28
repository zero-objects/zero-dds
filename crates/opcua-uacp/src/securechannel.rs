// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! OPC-UA SecureChannel (Part 6 §6.7) on top of the UACP framing — the
//! `OPN` / `MSG` / `CLO` message chunks, their security/sequence headers, the
//! common service `RequestHeader`/`ResponseHeader`, and the
//! `OpenSecureChannel` service.
//!
//! A SecureChannel chunk is
//! `[MessageHeader][SecureChannelId:u32][SecurityHeader][SequenceHeader][body]`
//! — for `OPN` the security header is the
//! [`AsymmetricAlgorithmSecurityHeader`], for `MSG`/`CLO` it is the
//! `SymmetricAlgorithmSecurityHeader` (just a `TokenId`). With
//! `SecurityMode::None` (this stage) there is no padding/signature/encryption;
//! the secured policies (feature `crypto`) layer on top.
//!
//! Each service message body is `[TypeId:NodeId][fields]`, where `TypeId` is
//! the message type's DefaultBinary encoding NodeId (see [`node_ids`]).

use alloc::string::String;
use alloc::vec::Vec;

use zerodds_opcua_gateway::data_value::{ExtensionObject, ExtensionObjectBody};
use zerodds_opcua_gateway::node_id::NodeId;
use zerodds_opcua_pubsub::uadp::datatypes::MessageSecurityMode;
use zerodds_opcua_pubsub::{DecodeError, EncodeError, UaDecode, UaEncode, UaReader, UaWriter};

use crate::connection::{ChunkType, MessageHeader, MessageType};

/// Well-known DefaultBinary encoding NodeIds (ns0) of the service messages
/// (Part 6 Annex / Part 4). The body of a SecureChannel message begins with
/// the matching id.
pub mod node_ids {
    use zerodds_opcua_gateway::node_id::NodeId;

    /// `OpenSecureChannelRequest_Encoding_DefaultBinary`.
    pub const OPEN_SECURE_CHANNEL_REQUEST: NodeId = NodeId::numeric(0, 446);
    /// `OpenSecureChannelResponse_Encoding_DefaultBinary`.
    pub const OPEN_SECURE_CHANNEL_RESPONSE: NodeId = NodeId::numeric(0, 449);
    /// `CloseSecureChannelRequest_Encoding_DefaultBinary`.
    pub const CLOSE_SECURE_CHANNEL_REQUEST: NodeId = NodeId::numeric(0, 452);
    /// `CreateSessionRequest_Encoding_DefaultBinary`.
    pub const CREATE_SESSION_REQUEST: NodeId = NodeId::numeric(0, 461);
    /// `CreateSessionResponse_Encoding_DefaultBinary`.
    pub const CREATE_SESSION_RESPONSE: NodeId = NodeId::numeric(0, 464);
    /// `ActivateSessionRequest_Encoding_DefaultBinary`.
    pub const ACTIVATE_SESSION_REQUEST: NodeId = NodeId::numeric(0, 467);
    /// `ActivateSessionResponse_Encoding_DefaultBinary`.
    pub const ACTIVATE_SESSION_RESPONSE: NodeId = NodeId::numeric(0, 470);
    /// `CloseSessionRequest_Encoding_DefaultBinary`.
    pub const CLOSE_SESSION_REQUEST: NodeId = NodeId::numeric(0, 473);
    /// `CloseSessionResponse_Encoding_DefaultBinary`.
    pub const CLOSE_SESSION_RESPONSE: NodeId = NodeId::numeric(0, 476);
    /// `FindServersRequest_Encoding_DefaultBinary`.
    pub const FIND_SERVERS_REQUEST: NodeId = NodeId::numeric(0, 422);
    /// `FindServersResponse_Encoding_DefaultBinary`.
    pub const FIND_SERVERS_RESPONSE: NodeId = NodeId::numeric(0, 425);
    /// `GetEndpointsRequest_Encoding_DefaultBinary`.
    ///
    /// 426 and 429 are the `GetEndpointsRequest`/`GetEndpointsResponse`
    /// **DataType** nodes; the wire carries the DefaultBinary encoding node,
    /// two higher. Every other id in this table was already the encoding node,
    /// so nothing but a foreign server could tell the two apart.
    pub const GET_ENDPOINTS_REQUEST: NodeId = NodeId::numeric(0, 428);
    /// `GetEndpointsResponse_Encoding_DefaultBinary`.
    pub const GET_ENDPOINTS_RESPONSE: NodeId = NodeId::numeric(0, 431);
    /// `ReadRequest_Encoding_DefaultBinary`.
    pub const READ_REQUEST: NodeId = NodeId::numeric(0, 631);
    /// `ReadResponse_Encoding_DefaultBinary`.
    pub const READ_RESPONSE: NodeId = NodeId::numeric(0, 634);
    /// `WriteRequest_Encoding_DefaultBinary`.
    pub const WRITE_REQUEST: NodeId = NodeId::numeric(0, 673);
    /// `WriteResponse_Encoding_DefaultBinary`.
    pub const WRITE_RESPONSE: NodeId = NodeId::numeric(0, 676);
    /// `BrowseRequest_Encoding_DefaultBinary`.
    pub const BROWSE_REQUEST: NodeId = NodeId::numeric(0, 527);
    /// `BrowseResponse_Encoding_DefaultBinary`.
    pub const BROWSE_RESPONSE: NodeId = NodeId::numeric(0, 530);
    /// `BrowseNextRequest_Encoding_DefaultBinary`.
    pub const BROWSE_NEXT_REQUEST: NodeId = NodeId::numeric(0, 533);
    /// `BrowseNextResponse_Encoding_DefaultBinary`.
    pub const BROWSE_NEXT_RESPONSE: NodeId = NodeId::numeric(0, 536);
    /// `CallRequest_Encoding_DefaultBinary`.
    pub const CALL_REQUEST: NodeId = NodeId::numeric(0, 712);
    /// `CallResponse_Encoding_DefaultBinary`.
    pub const CALL_RESPONSE: NodeId = NodeId::numeric(0, 715);
    /// `CreateSubscriptionRequest_Encoding_DefaultBinary`.
    pub const CREATE_SUBSCRIPTION_REQUEST: NodeId = NodeId::numeric(0, 787);
    /// `CreateSubscriptionResponse_Encoding_DefaultBinary`.
    pub const CREATE_SUBSCRIPTION_RESPONSE: NodeId = NodeId::numeric(0, 790);
    /// `SetPublishingModeRequest_Encoding_DefaultBinary`.
    pub const SET_PUBLISHING_MODE_REQUEST: NodeId = NodeId::numeric(0, 799);
    /// `SetPublishingModeResponse_Encoding_DefaultBinary`.
    pub const SET_PUBLISHING_MODE_RESPONSE: NodeId = NodeId::numeric(0, 802);
    /// `CreateMonitoredItemsRequest_Encoding_DefaultBinary`.
    pub const CREATE_MONITORED_ITEMS_REQUEST: NodeId = NodeId::numeric(0, 751);
    /// `CreateMonitoredItemsResponse_Encoding_DefaultBinary`.
    pub const CREATE_MONITORED_ITEMS_RESPONSE: NodeId = NodeId::numeric(0, 754);
    /// `ModifyMonitoredItemsRequest_Encoding_DefaultBinary`.
    pub const MODIFY_MONITORED_ITEMS_REQUEST: NodeId = NodeId::numeric(0, 763);
    /// `ModifyMonitoredItemsResponse_Encoding_DefaultBinary`.
    pub const MODIFY_MONITORED_ITEMS_RESPONSE: NodeId = NodeId::numeric(0, 766);
    /// `SetMonitoringModeRequest_Encoding_DefaultBinary`.
    pub const SET_MONITORING_MODE_REQUEST: NodeId = NodeId::numeric(0, 769);
    /// `SetMonitoringModeResponse_Encoding_DefaultBinary`.
    pub const SET_MONITORING_MODE_RESPONSE: NodeId = NodeId::numeric(0, 772);
    /// `SetTriggeringRequest_Encoding_DefaultBinary`.
    pub const SET_TRIGGERING_REQUEST: NodeId = NodeId::numeric(0, 775);
    /// `SetTriggeringResponse_Encoding_DefaultBinary`.
    pub const SET_TRIGGERING_RESPONSE: NodeId = NodeId::numeric(0, 778);
    /// `DeleteMonitoredItemsRequest_Encoding_DefaultBinary`.
    pub const DELETE_MONITORED_ITEMS_REQUEST: NodeId = NodeId::numeric(0, 781);
    /// `DeleteMonitoredItemsResponse_Encoding_DefaultBinary`.
    pub const DELETE_MONITORED_ITEMS_RESPONSE: NodeId = NodeId::numeric(0, 784);
    /// `ModifySubscriptionRequest_Encoding_DefaultBinary`.
    pub const MODIFY_SUBSCRIPTION_REQUEST: NodeId = NodeId::numeric(0, 793);
    /// `ModifySubscriptionResponse_Encoding_DefaultBinary`.
    pub const MODIFY_SUBSCRIPTION_RESPONSE: NodeId = NodeId::numeric(0, 796);
    /// `StatusChangeNotification_Encoding_DefaultBinary`.
    pub const STATUS_CHANGE_NOTIFICATION: NodeId = NodeId::numeric(0, 820);
    /// `RepublishRequest_Encoding_DefaultBinary`.
    pub const REPUBLISH_REQUEST: NodeId = NodeId::numeric(0, 832);
    /// `RepublishResponse_Encoding_DefaultBinary`.
    pub const REPUBLISH_RESPONSE: NodeId = NodeId::numeric(0, 835);
    /// `TransferSubscriptionsRequest_Encoding_DefaultBinary`.
    pub const TRANSFER_SUBSCRIPTIONS_REQUEST: NodeId = NodeId::numeric(0, 841);
    /// `TransferSubscriptionsResponse_Encoding_DefaultBinary`.
    pub const TRANSFER_SUBSCRIPTIONS_RESPONSE: NodeId = NodeId::numeric(0, 844);
    /// `PublishRequest_Encoding_DefaultBinary`.
    pub const PUBLISH_REQUEST: NodeId = NodeId::numeric(0, 826);
    /// `PublishResponse_Encoding_DefaultBinary`.
    pub const PUBLISH_RESPONSE: NodeId = NodeId::numeric(0, 829);
    /// `DeleteSubscriptionsRequest_Encoding_DefaultBinary`.
    pub const DELETE_SUBSCRIPTIONS_REQUEST: NodeId = NodeId::numeric(0, 847);
    /// `DeleteSubscriptionsResponse_Encoding_DefaultBinary`.
    pub const DELETE_SUBSCRIPTIONS_RESPONSE: NodeId = NodeId::numeric(0, 850);
    /// `DataChangeNotification_Encoding_DefaultBinary`.
    pub const DATA_CHANGE_NOTIFICATION: NodeId = NodeId::numeric(0, 811);
    /// `ServiceFault_Encoding_DefaultBinary`.
    pub const SERVICE_FAULT: NodeId = NodeId::numeric(0, 397);
    /// `HistoryReadRequest_Encoding_DefaultBinary` (Part 4 §5.10.3).
    pub const HISTORY_READ_REQUEST: NodeId = NodeId::numeric(0, 664);
    /// `HistoryReadResponse_Encoding_DefaultBinary`.
    pub const HISTORY_READ_RESPONSE: NodeId = NodeId::numeric(0, 667);
    /// `ReadRawModifiedDetails_Encoding_DefaultBinary` (Part 11 §6.4.3) —
    /// the `HistoryReadDetails` a raw/time-range HistoryRead carries.
    pub const READ_RAW_MODIFIED_DETAILS: NodeId = NodeId::numeric(0, 649);
    /// `ReadAtTimeDetails_Encoding_DefaultBinary` (Part 11 §6.4.4).
    pub const READ_AT_TIME_DETAILS: NodeId = NodeId::numeric(0, 655);
    /// `HistoryData_Encoding_DefaultBinary` (Part 11 §6.6.2) — the
    /// `HistoryReadResult.historyData` extensible parameter for a
    /// HistoricalDataNode (as opposed to `HistoryEvent`, which DDS has no
    /// counterpart for and this crate does not implement).
    pub const HISTORY_DATA: NodeId = NodeId::numeric(0, 658);
}

/// SecurityPolicy URI for no message security.
pub const SECURITY_POLICY_NONE: &str = "http://opcfoundation.org/UA/SecurityPolicy#None";

/// A null/empty `ExtensionObject` (the usual `AdditionalHeader`).
#[must_use]
pub fn null_extension_object() -> ExtensionObject {
    ExtensionObject {
        type_id: NodeId::numeric(0, 0),
        body: ExtensionObjectBody::None,
    }
}

fn write_string(w: &mut UaWriter, s: &str) -> Result<(), EncodeError> {
    let len = i32::try_from(s.len()).map_err(|_| EncodeError::LengthOverflow {
        what: "String",
        len: s.len(),
    })?;
    w.write_i32(len);
    w.write_bytes(s.as_bytes());
    Ok(())
}

fn read_string(r: &mut UaReader<'_>) -> Result<String, DecodeError> {
    let len = r.read_i32()?;
    if len < 0 {
        return Ok(String::new());
    }
    let bytes = r.read_bytes(len as usize)?;
    core::str::from_utf8(bytes)
        .map(String::from)
        .map_err(|_| DecodeError::InvalidUtf8)
}

fn write_byte_string(w: &mut UaWriter, b: &[u8]) -> Result<(), EncodeError> {
    if b.is_empty() {
        w.write_i32(-1); // null ByteString
        return Ok(());
    }
    let len = i32::try_from(b.len()).map_err(|_| EncodeError::LengthOverflow {
        what: "ByteString",
        len: b.len(),
    })?;
    w.write_i32(len);
    w.write_bytes(b);
    Ok(())
}

fn read_byte_string(r: &mut UaReader<'_>) -> Result<Vec<u8>, DecodeError> {
    let len = r.read_i32()?;
    if len < 0 {
        return Ok(Vec::new());
    }
    Ok(r.read_bytes(len as usize)?.to_vec())
}

// ---------------------------------------------------------------------------
// DiagnosticInfo (Part 6 §5.2.2.12) — minimal: encode empty, decode/skip all.
// ---------------------------------------------------------------------------

/// Encodes an empty `DiagnosticInfo` (encoding mask `0`).
fn write_empty_diagnostic_info(w: &mut UaWriter) {
    w.write_u8(0);
}

/// Hard cap on `InnerDiagnosticInfo` nesting — untrusted input could otherwise
/// nest arbitrarily deep and overflow the stack. Real `DiagnosticInfo` never
/// nests beyond a handful of levels.
const MAX_DIAGNOSTIC_INFO_DEPTH: u8 = 16;

/// Reads (and discards) a `DiagnosticInfo`, advancing past all present fields.
fn skip_diagnostic_info(r: &mut UaReader<'_>) -> Result<(), DecodeError> {
    skip_diagnostic_info_depth(r, 0)
}

/// zerodds-lint: recursion-depth 16 (InnerDiagnosticInfo nesting; hard-capped by
/// `MAX_DIAGNOSTIC_INFO_DEPTH`, untrusted input rejected beyond it).
fn skip_diagnostic_info_depth(r: &mut UaReader<'_>, depth: u8) -> Result<(), DecodeError> {
    if depth >= MAX_DIAGNOSTIC_INFO_DEPTH {
        return Err(DecodeError::MalformedMessage {
            message: "DiagnosticInfo nested beyond the allowed depth",
        });
    }
    let mask = r.read_u8()?;
    if mask & 0x01 != 0 {
        r.read_i32()?; // SymbolicId
    }
    if mask & 0x02 != 0 {
        r.read_i32()?; // NamespaceUri
    }
    if mask & 0x04 != 0 {
        r.read_i32()?; // LocalizedText
    }
    if mask & 0x08 != 0 {
        r.read_i32()?; // Locale
    }
    if mask & 0x10 != 0 {
        read_string(r)?; // AdditionalInfo
    }
    if mask & 0x20 != 0 {
        r.read_u32()?; // InnerStatusCode
    }
    if mask & 0x40 != 0 {
        skip_diagnostic_info_depth(r, depth + 1)?; // InnerDiagnosticInfo
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Common service headers (Part 4 §7.32 / §7.33).
// ---------------------------------------------------------------------------

/// The `RequestHeader` common to every service request (Part 4 §7.32).
#[derive(Debug, Clone, PartialEq)]
pub struct RequestHeader {
    /// Session authentication token (null NodeId before a session exists).
    pub authentication_token: NodeId,
    /// Client send time (DateTime ticks).
    pub timestamp: i64,
    /// Client-assigned request handle.
    pub request_handle: u32,
    /// Diagnostics requested bitmask.
    pub return_diagnostics: u32,
    /// Audit entry id.
    pub audit_entry_id: String,
    /// Timeout hint in milliseconds.
    pub timeout_hint: u32,
    /// Extensible additional header (usually null).
    pub additional_header: ExtensionObject,
}

/// The OPC-UA `DateTime` epoch is 1601-01-01T00:00:00Z; this is the number of
/// 100 ns ticks between it and the Unix epoch (Part 6 §5.2.2.5).
pub const DATETIME_TICKS_AT_UNIX_EPOCH: i64 = 116_444_736_000_000_000;

impl RequestHeader {
    /// A minimal header with the given authentication token and handle.
    ///
    /// The timestamp is left at zero — call
    /// [`with_timestamp`](Self::with_timestamp) before sending. Part 4 §7.33
    /// specifies it as the time the client sent the request, and a server that
    /// verifies it (open62541 does, by default) answers a zero with
    /// `Bad_InvalidTimestamp`.
    #[must_use]
    pub fn new(authentication_token: NodeId, request_handle: u32) -> Self {
        Self {
            authentication_token,
            timestamp: 0,
            request_handle,
            return_diagnostics: 0,
            audit_entry_id: String::new(),
            timeout_hint: 0,
            additional_header: null_extension_object(),
        }
    }

    /// Sets the client send time (Part 4 §7.33), in `DateTime` ticks.
    #[must_use]
    pub const fn with_timestamp(mut self, timestamp: i64) -> Self {
        self.timestamp = timestamp;
        self
    }

    fn encode(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        self.authentication_token.encode(w)?;
        w.write_i64(self.timestamp);
        w.write_u32(self.request_handle);
        w.write_u32(self.return_diagnostics);
        write_string(w, &self.audit_entry_id)?;
        w.write_u32(self.timeout_hint);
        self.additional_header.encode(w)?;
        Ok(())
    }

    fn decode(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            authentication_token: NodeId::decode(r)?,
            timestamp: r.read_i64()?,
            request_handle: r.read_u32()?,
            return_diagnostics: r.read_u32()?,
            audit_entry_id: read_string(r)?,
            timeout_hint: r.read_u32()?,
            additional_header: ExtensionObject::decode(r)?,
        })
    }
}

/// The `ResponseHeader` common to every service response (Part 4 §7.33).
#[derive(Debug, Clone, PartialEq)]
pub struct ResponseHeader {
    /// Server send time (DateTime ticks).
    pub timestamp: i64,
    /// Echoes the request handle.
    pub request_handle: u32,
    /// Overall service result StatusCode.
    pub service_result: u32,
    /// String table referenced by diagnostics.
    pub string_table: Vec<String>,
    /// Extensible additional header (usually null).
    pub additional_header: ExtensionObject,
}

impl ResponseHeader {
    /// A `Good` (or given `service_result`) header echoing `request_handle`.
    #[must_use]
    pub fn new(request_handle: u32, service_result: u32) -> Self {
        Self {
            timestamp: 0,
            request_handle,
            service_result,
            string_table: Vec::new(),
            additional_header: null_extension_object(),
        }
    }

    fn encode(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        w.write_i64(self.timestamp);
        w.write_u32(self.request_handle);
        w.write_u32(self.service_result);
        write_empty_diagnostic_info(w);
        w.write_i32(i32::try_from(self.string_table.len()).map_err(|_| {
            EncodeError::LengthOverflow {
                what: "StringTable",
                len: self.string_table.len(),
            }
        })?);
        for s in &self.string_table {
            write_string(w, s)?;
        }
        self.additional_header.encode(w)?;
        Ok(())
    }

    fn decode(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        let timestamp = r.read_i64()?;
        let request_handle = r.read_u32()?;
        let service_result = r.read_u32()?;
        skip_diagnostic_info(r)?;
        let n = r.read_i32()?;
        let mut string_table = Vec::new();
        if n > 0 {
            for _ in 0..n {
                string_table.push(read_string(r)?);
            }
        }
        let additional_header = ExtensionObject::decode(r)?;
        Ok(Self {
            timestamp,
            request_handle,
            service_result,
            string_table,
            additional_header,
        })
    }
}

// ---------------------------------------------------------------------------
// SecureChannel headers + token.
// ---------------------------------------------------------------------------

/// `AsymmetricAlgorithmSecurityHeader` (Part 6 §6.7.2.3) — carried by `OPN`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AsymmetricAlgorithmSecurityHeader {
    /// SecurityPolicy URI.
    pub security_policy_uri: String,
    /// Sender's application certificate (empty for SecurityPolicy `None`).
    pub sender_certificate: Vec<u8>,
    /// Thumbprint of the receiver's certificate (empty for `None`).
    pub receiver_certificate_thumbprint: Vec<u8>,
}

impl AsymmetricAlgorithmSecurityHeader {
    /// The header for SecurityPolicy `None`.
    #[must_use]
    pub fn none() -> Self {
        Self {
            security_policy_uri: String::from(SECURITY_POLICY_NONE),
            sender_certificate: Vec::new(),
            receiver_certificate_thumbprint: Vec::new(),
        }
    }

    fn encode(&self, w: &mut UaWriter) -> Result<(), EncodeError> {
        write_string(w, &self.security_policy_uri)?;
        write_byte_string(w, &self.sender_certificate)?;
        write_byte_string(w, &self.receiver_certificate_thumbprint)?;
        Ok(())
    }

    fn decode(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            security_policy_uri: read_string(r)?,
            sender_certificate: read_byte_string(r)?,
            receiver_certificate_thumbprint: read_byte_string(r)?,
        })
    }
}

/// `SequenceHeader` (Part 6 §6.7.2.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SequenceHeader {
    /// Per-chunk sequence number.
    pub sequence_number: u32,
    /// Request id correlating a response to its request.
    pub request_id: u32,
}

/// `ChannelSecurityToken` (Part 4 §5.6.2) returned by OpenSecureChannel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChannelSecurityToken {
    /// SecureChannel id.
    pub channel_id: u32,
    /// Token id (used by `MSG`/`CLO` symmetric headers).
    pub token_id: u32,
    /// Creation time (DateTime ticks).
    pub created_at: i64,
    /// Revised token lifetime in milliseconds.
    pub revised_lifetime: u32,
}

impl ChannelSecurityToken {
    fn encode(&self, w: &mut UaWriter) {
        w.write_u32(self.channel_id);
        w.write_u32(self.token_id);
        w.write_i64(self.created_at);
        w.write_u32(self.revised_lifetime);
    }

    fn decode(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            channel_id: r.read_u32()?,
            token_id: r.read_u32()?,
            created_at: r.read_i64()?,
            revised_lifetime: r.read_u32()?,
        })
    }
}

// ---------------------------------------------------------------------------
// OpenSecureChannel service (Part 4 §5.6.2).
// ---------------------------------------------------------------------------

/// SecurityTokenRequestType (Part 4 §5.6.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecurityTokenRequestType {
    /// Create a new SecureChannel.
    Issue,
    /// Renew the token of an existing SecureChannel.
    Renew,
}

impl SecurityTokenRequestType {
    const fn to_i32(self) -> i32 {
        match self {
            Self::Issue => 0,
            Self::Renew => 1,
        }
    }

    fn from_i32(v: i32) -> Result<Self, DecodeError> {
        match v {
            0 => Ok(Self::Issue),
            1 => Ok(Self::Renew),
            other => Err(DecodeError::InvalidDiscriminant {
                field: "SecurityTokenRequestType",
                value: other as u32,
            }),
        }
    }
}

/// `OpenSecureChannelRequest` (Part 4 §5.6.2).
#[derive(Debug, Clone, PartialEq)]
pub struct OpenSecureChannelRequest {
    /// Common request header.
    pub request_header: RequestHeader,
    /// Client protocol version.
    pub client_protocol_version: u32,
    /// Issue or Renew.
    pub request_type: SecurityTokenRequestType,
    /// Requested message security mode.
    pub security_mode: MessageSecurityMode,
    /// Client nonce (empty for SecurityMode `None`).
    pub client_nonce: Vec<u8>,
    /// Requested token lifetime in milliseconds.
    pub requested_lifetime: u32,
}

impl OpenSecureChannelRequest {
    /// Encodes the message body (`TypeId` + fields).
    ///
    /// # Errors
    /// [`EncodeError`] on an oversized string/byte-string.
    pub fn encode(&self) -> Result<Vec<u8>, EncodeError> {
        let mut w = UaWriter::new();
        node_ids::OPEN_SECURE_CHANNEL_REQUEST.encode(&mut w)?;
        self.request_header.encode(&mut w)?;
        w.write_u32(self.client_protocol_version);
        w.write_i32(self.request_type.to_i32());
        w.write_i32(self.security_mode.to_i32());
        write_byte_string(&mut w, &self.client_nonce)?;
        w.write_u32(self.requested_lifetime);
        Ok(w.into_vec())
    }

    /// Decodes the body after the `TypeId` has been read.
    ///
    /// # Errors
    /// [`DecodeError`] on truncation or a bad enum value.
    pub fn decode_body(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            request_header: RequestHeader::decode(r)?,
            client_protocol_version: r.read_u32()?,
            request_type: SecurityTokenRequestType::from_i32(r.read_i32()?)?,
            security_mode: MessageSecurityMode::from_i32(r.read_i32()?)?,
            client_nonce: read_byte_string(r)?,
            requested_lifetime: r.read_u32()?,
        })
    }
}

/// `OpenSecureChannelResponse` (Part 4 §5.6.2).
#[derive(Debug, Clone, PartialEq)]
pub struct OpenSecureChannelResponse {
    /// Common response header.
    pub response_header: ResponseHeader,
    /// Server protocol version.
    pub server_protocol_version: u32,
    /// The issued security token.
    pub security_token: ChannelSecurityToken,
    /// Server nonce (empty for SecurityMode `None`).
    pub server_nonce: Vec<u8>,
}

impl OpenSecureChannelResponse {
    /// Encodes the message body (`TypeId` + fields).
    ///
    /// # Errors
    /// [`EncodeError`] on an oversized field.
    pub fn encode(&self) -> Result<Vec<u8>, EncodeError> {
        let mut w = UaWriter::new();
        node_ids::OPEN_SECURE_CHANNEL_RESPONSE.encode(&mut w)?;
        self.response_header.encode(&mut w)?;
        w.write_u32(self.server_protocol_version);
        self.security_token.encode(&mut w);
        write_byte_string(&mut w, &self.server_nonce)?;
        Ok(w.into_vec())
    }

    /// Decodes the body after the `TypeId` has been read.
    ///
    /// # Errors
    /// [`DecodeError`] on truncation.
    pub fn decode_body(r: &mut UaReader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            response_header: ResponseHeader::decode(r)?,
            server_protocol_version: r.read_u32()?,
            security_token: ChannelSecurityToken::decode(r)?,
            server_nonce: read_byte_string(r)?,
        })
    }
}

/// The parsed contents of one SecureChannel chunk.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedChunk {
    /// Message type (`OPN` / `MSG` / `CLO`).
    pub message_type: MessageType,
    /// SecureChannel id.
    pub channel_id: u32,
    /// Asymmetric header (present for `OPN`).
    pub asymmetric_header: Option<AsymmetricAlgorithmSecurityHeader>,
    /// Token id (present for `MSG`/`CLO` symmetric header).
    pub token_id: Option<u32>,
    /// Sequence header.
    pub sequence_header: SequenceHeader,
    /// The service message body (`TypeId` + fields).
    pub body: Vec<u8>,
}

/// The installed symmetric security for a secured SecureChannel: the negotiated
/// policy/mode and the §6.7.6-derived send/receive keys. After a secured
/// OpenSecureChannel, `MSG`/`CLO` chunks are protected with these.
#[cfg(feature = "crypto")]
#[derive(Debug, Clone)]
pub struct SecuritySession {
    /// Negotiated SecurityPolicy.
    pub policy: crate::crypto::SecurityPolicy,
    /// Sign or SignAndEncrypt.
    pub mode: crate::crypto::SecuredMode,
    /// Keys protecting messages we send.
    pub send_keys: crate::crypto::DerivedKeys,
    /// Keys verifying/decrypting messages we receive.
    pub recv_keys: crate::crypto::DerivedKeys,
}

/// A SecureChannel endpoint: frames service message bodies into `OPN`/`MSG`/
/// `CLO` chunks and parses them back. With a `SecuritySession` installed
/// (feature `crypto`) the symmetric chunks are signed/encrypted; otherwise
/// they are plaintext (SecurityMode `None`).
#[derive(Debug, Clone)]
pub struct SecureChannel {
    channel_id: u32,
    token_id: u32,
    send_sequence_number: u32,
    recv_sequence_number: Option<u32>,
    /// The peer's `ReceiveBufferSize` from Hello/Acknowledge (Part 6 §7.1.2.3):
    /// the largest MessageChunk we may send it.
    send_buffer_size: u32,
    /// The bodies of the `C` chunks received so far for the Message currently
    /// being reassembled (Part 6 §6.7.1).
    reassembly: Vec<u8>,
    #[cfg(feature = "crypto")]
    security: Option<SecuritySession>,
}

/// What one received MessageChunk yielded (Part 6 §6.7.2.2 `IsFinal`).
#[derive(Debug, Clone, PartialEq)]
pub enum ChunkOutcome {
    /// An intermediate (`C`) chunk: the Message is not complete yet.
    Incomplete,
    /// A final (`F`) chunk: the reassembled Message.
    Complete(ParsedChunk),
    /// An abort (`A`) chunk (Part 6 §6.7.3). The partial Message has been
    /// discarded; the SecureChannel stays open.
    Abort {
        /// The numeric error code the sender reported.
        error: u32,
        /// The sender's description of the error.
        reason: alloc::string::String,
    },
}

/// Part 6 §6.7.1 requires a TransportProtocol buffer of at least 8192 bytes for
/// RSA-based asymmetric cryptography; that is the floor a chunk size may take.
pub const MIN_SEND_BUFFER_SIZE: u32 = 8192;

/// The largest Message a receiver will reassemble from `C` chunks before it
/// gives up. Without it a peer could stream intermediate chunks forever and
/// grow the reassembly buffer without bound.
const MAX_REASSEMBLED_MESSAGE: usize = 16 * 1024 * 1024;

/// Bytes a symmetric (`MSG`/`CLO`) chunk spends on framing before the body:
/// MessageHeader (8) + SecureChannelId (4) + TokenId (4) + SequenceHeader (8).
const SYMMETRIC_CHUNK_OVERHEAD: usize = 24;

/// Additional bytes the secured layouts spend per chunk: the HMAC-SHA256
/// signature plus, for `SignAndEncrypt`, up to one AES block of padding.
#[cfg(feature = "crypto")]
const SECURED_CHUNK_OVERHEAD: usize = crate::crypto::HMAC_SHA256_LEN + crate::crypto::CBC_BLOCK + 1;

/// Part 6 §6.7.2.3 — above this value the SequenceNumber rolls over and the
/// next one is in `1..=1023` instead of `previous + 1`.
const SEQUENCE_ROLLOVER_THRESHOLD: u32 = 4_294_966_271;

impl SecureChannel {
    /// Creates a SecureChannel with the given ids (token `0` until issued).
    #[must_use]
    pub fn new(channel_id: u32, token_id: u32) -> Self {
        Self {
            channel_id,
            token_id,
            send_sequence_number: 0,
            recv_sequence_number: None,
            send_buffer_size: MIN_SEND_BUFFER_SIZE,
            reassembly: Vec::new(),
            #[cfg(feature = "crypto")]
            security: None,
        }
    }

    /// Sets the peer's `ReceiveBufferSize` (Part 6 §7.1.2.3) — the largest
    /// MessageChunk this endpoint may send. Values below
    /// [`MIN_SEND_BUFFER_SIZE`] are raised to it, as §6.7.1 requires.
    pub fn set_send_buffer_size(&mut self, size: u32) {
        self.send_buffer_size = size.max(MIN_SEND_BUFFER_SIZE);
    }

    /// The negotiated chunk size.
    #[must_use]
    pub const fn send_buffer_size(&self) -> u32 {
        self.send_buffer_size
    }

    /// Restarts both SequenceNumber counters and drops any partial Message.
    /// Part 6 §6.7.2.3 counts per SecureChannel, so a channel that is opened
    /// (not renewed) starts over.
    pub fn reset_sequences(&mut self) {
        self.send_sequence_number = 0;
        self.recv_sequence_number = None;
        self.reassembly.clear();
    }

    /// The number of body bytes one `MSG`/`CLO` chunk can carry.
    fn max_body_per_chunk(&self) -> usize {
        // The signature and padding a secured chunk carries, which only
        // exist when the crypto backend is compiled in. Written as a
        // conditional binding rather than a `mut` the `#[cfg]` block
        // adds to: without the feature nothing assigns to it again, and
        // `unused_mut` is denied — the build under a feature set nobody
        // runs locally is exactly where that goes unnoticed.
        #[cfg(feature = "crypto")]
        let overhead = if self.security.is_some() {
            SYMMETRIC_CHUNK_OVERHEAD + SECURED_CHUNK_OVERHEAD
        } else {
            SYMMETRIC_CHUNK_OVERHEAD
        };
        #[cfg(not(feature = "crypto"))]
        let overhead = SYMMETRIC_CHUNK_OVERHEAD;
        (self.send_buffer_size as usize)
            .saturating_sub(overhead)
            .max(1)
    }

    /// Installs the negotiated symmetric security; subsequent `MSG`/`CLO` chunks
    /// are protected and [`open_incoming`](Self::open_incoming) verifies them.
    #[cfg(feature = "crypto")]
    pub fn install_security(&mut self, session: SecuritySession) {
        self.security = Some(session);
    }

    /// Advances and returns the next send sequence number (shared across `OPN`
    /// and `MSG`/`CLO` so numbers stay monotonic across the channel).
    pub fn next_send_sequence(&mut self) -> u32 {
        self.next_sequence()
    }

    /// The SecureChannel id.
    #[must_use]
    pub const fn channel_id(&self) -> u32 {
        self.channel_id
    }

    /// Sets the active token id (after OpenSecureChannel).
    pub fn set_token_id(&mut self, token_id: u32) {
        self.token_id = token_id;
    }

    /// Adopts the ids the peer issued in its OpenSecureChannelResponse without
    /// disturbing the sequence counters.
    ///
    /// Part 6 §6.7.2.3 counts SequenceNumbers per channel across `OPN` and
    /// `MSG`/`CLO` alike, so the `OPN` that opened the channel has already
    /// consumed number 1. Replacing the whole `SecureChannel` with a fresh
    /// [`new`](Self::new) at that point restarts the counter and makes the
    /// first `MSG` repeat a number the peer has already seen.
    pub fn adopt_token(&mut self, channel_id: u32, token_id: u32) {
        self.channel_id = channel_id;
        self.token_id = token_id;
    }

    fn next_sequence(&mut self) -> u32 {
        self.send_sequence_number = self.send_sequence_number.wrapping_add(1);
        self.send_sequence_number
    }

    /// Records the SequenceNumber of a chunk that was parsed outside
    /// [`open_incoming`](Self::open_incoming) — the asymmetric `OPN`, which
    /// carries no symmetric security header — so that the `MSG` chunks that
    /// follow are checked against a continuous counter.
    ///
    /// # Errors
    /// [`DecodeError`] if the number does not continue the peer's sequence.
    pub fn note_incoming_sequence(&mut self, sequence_number: u32) -> Result<(), DecodeError> {
        self.accept_sequence(sequence_number)
    }

    /// Part 6 §6.7.2.3: the peer increments the SequenceNumber by exactly one
    /// per MessageChunk. The first chunk observed on the channel sets the
    /// baseline; every later one has to continue it, with the rollover from
    /// [`SEQUENCE_ROLLOVER_THRESHOLD`] into `1..=1023`.
    fn accept_sequence(&mut self, sequence_number: u32) -> Result<(), DecodeError> {
        if let Some(previous) = self.recv_sequence_number {
            let continues = sequence_number == previous.wrapping_add(1)
                || (previous >= SEQUENCE_ROLLOVER_THRESHOLD && sequence_number < 1024);
            if !continues {
                return Err(DecodeError::MalformedMessage {
                    message: "SecureChannel SequenceNumber does not continue the peer's sequence",
                });
            }
        }
        self.recv_sequence_number = Some(sequence_number);
        Ok(())
    }

    /// Frames an `OPN` chunk around the OpenSecureChannel body (SecurityPolicy
    /// `None`).
    ///
    /// # Errors
    /// [`EncodeError`] on an oversized message.
    pub fn open_chunk(&mut self, request_id: u32, body: &[u8]) -> Result<Vec<u8>, EncodeError> {
        let mut inner = UaWriter::new();
        inner.write_u32(self.channel_id);
        AsymmetricAlgorithmSecurityHeader::none().encode(&mut inner)?;
        let seq = self.next_sequence();
        inner.write_u32(seq);
        inner.write_u32(request_id);
        inner.write_bytes(body);
        frame_chunk(
            MessageType::OpenSecureChannel,
            ChunkType::Final,
            &inner.into_vec(),
        )
    }

    /// Frames a service message body into `MSG` chunks, splitting it across as
    /// many MessageChunks as the negotiated buffer needs (Part 6 §6.7.1: "OPC
    /// UA Secure Conversation will break OPC UA Messages into several pieces
    /// (called 'MessageChunks') that are smaller than the buffer size allowed
    /// by the TransportProtocol"). The chunks are returned in send order; all
    /// but the last carry `IsFinal` = `C`.
    ///
    /// # Errors
    /// [`EncodeError`] on an oversized chunk.
    pub fn message_chunks(
        &mut self,
        request_id: u32,
        body: &[u8],
    ) -> Result<Vec<Vec<u8>>, EncodeError> {
        self.symmetric_chunks(MessageType::Message, request_id, body)
    }

    /// [`message_chunks`](Self::message_chunks) concatenated into the byte
    /// stream to write to the transport.
    ///
    /// # Errors
    /// [`EncodeError`] on an oversized chunk.
    pub fn message_chunk(&mut self, request_id: u32, body: &[u8]) -> Result<Vec<u8>, EncodeError> {
        Ok(self.message_chunks(request_id, body)?.concat())
    }

    /// Frames a `CLO` chunk (CloseSecureChannel).
    ///
    /// # Errors
    /// [`EncodeError`] on an oversized message.
    pub fn close_chunk(&mut self, request_id: u32, body: &[u8]) -> Result<Vec<u8>, EncodeError> {
        Ok(self
            .symmetric_chunks(MessageType::CloseSecureChannel, request_id, body)?
            .concat())
    }

    /// Builds the abort (`A`) chunk of Part 6 §6.7.3: it tells the receiver
    /// that an error occurred and that it should discard the previous chunks.
    ///
    /// # Errors
    /// [`EncodeError`] if the reason string is oversized.
    pub fn abort_chunk(
        &mut self,
        request_id: u32,
        error: u32,
        reason: &str,
    ) -> Result<Vec<u8>, EncodeError> {
        // §6.7.3 Table 63: "This string shall not be more than 4096 bytes."
        let mut cut = reason.len().min(4096);
        while cut > 0 && !reason.is_char_boundary(cut) {
            cut -= 1;
        }
        let clipped = &reason[..cut];
        let mut body = UaWriter::new();
        body.write_u32(error);
        write_string(&mut body, clipped)?;
        self.one_symmetric_chunk(
            MessageType::Message,
            ChunkType::Abort,
            request_id,
            &body.into_vec(),
        )
    }

    fn symmetric_chunks(
        &mut self,
        message_type: MessageType,
        request_id: u32,
        body: &[u8],
    ) -> Result<Vec<Vec<u8>>, EncodeError> {
        let limit = self.max_body_per_chunk();
        if body.len() <= limit {
            return Ok(alloc::vec![self.one_symmetric_chunk(
                message_type,
                ChunkType::Final,
                request_id,
                body
            )?]);
        }
        let mut out = Vec::new();
        let mut offset = 0;
        while offset < body.len() {
            let end = (offset + limit).min(body.len());
            let chunk_type = if end == body.len() {
                ChunkType::Final
            } else {
                ChunkType::Intermediate
            };
            out.push(self.one_symmetric_chunk(
                message_type,
                chunk_type,
                request_id,
                &body[offset..end],
            )?);
            offset = end;
        }
        Ok(out)
    }

    fn one_symmetric_chunk(
        &mut self,
        message_type: MessageType,
        chunk_type: ChunkType,
        request_id: u32,
        body: &[u8],
    ) -> Result<Vec<u8>, EncodeError> {
        let seq = self.next_sequence();
        #[cfg(feature = "crypto")]
        if let Some(sess) = &self.security {
            return crate::crypto::build_symmetric_chunk_typed(
                sess.mode,
                &sess.send_keys,
                message_type,
                chunk_type,
                self.channel_id,
                self.token_id,
                seq,
                request_id,
                body,
            )
            .map_err(|_| EncodeError::ValueOutOfRange {
                message: "secured chunk encryption failed",
            });
        }
        let mut inner = UaWriter::new();
        inner.write_u32(self.channel_id);
        inner.write_u32(self.token_id); // SymmetricAlgorithmSecurityHeader
        inner.write_u32(seq);
        inner.write_u32(request_id);
        inner.write_bytes(body);
        frame_chunk(message_type, chunk_type, &inner.into_vec())
    }

    /// Parses one incoming `MSG`/`CLO` MessageChunk, transparently verifying
    /// and decrypting it when a `SecuritySession` is installed, checking its
    /// SequenceNumber against the peer's sequence (Part 6 §6.7.2.3) and
    /// reassembling the Message across `C` chunks (§6.7.1).
    ///
    /// # Errors
    /// [`DecodeError`] on truncation, a bad signature, bad padding, a
    /// SequenceNumber that does not continue the peer's sequence, or a Message
    /// that grows past the reassembly cap.
    pub fn open_incoming(&mut self, bytes: &[u8]) -> Result<ChunkOutcome, DecodeError> {
        #[cfg(feature = "crypto")]
        if self.security.is_some() {
            let opened = {
                let Some(sess) = &self.security else {
                    return Err(DecodeError::MalformedMessage {
                        message: "no SecuritySession installed",
                    });
                };
                crate::crypto::open_symmetric_chunk(sess.mode, &sess.recv_keys, bytes).map_err(
                    |_| DecodeError::MalformedMessage {
                        message: "secured chunk verification/decryption failed",
                    },
                )?
            };
            self.accept_sequence(opened.sequence_number)?;
            return self.reassemble(
                opened.chunk_type,
                ParsedChunk {
                    message_type: opened.message_type,
                    channel_id: opened.channel_id,
                    asymmetric_header: None,
                    token_id: opened.token_id,
                    sequence_header: SequenceHeader {
                        sequence_number: opened.sequence_number,
                        request_id: opened.request_id,
                    },
                    body: opened.body,
                },
            );
        }
        let chunk_type = {
            let mut r = UaReader::new(bytes);
            MessageHeader::decode(&mut r)?.chunk_type
        };
        let parsed = parse_chunk(bytes)?;
        self.accept_sequence(parsed.sequence_header.sequence_number)?;
        self.reassemble(chunk_type, parsed)
    }

    /// Folds one chunk into the Message under reassembly (Part 6 §6.7.1 /
    /// §6.7.3).
    fn reassemble(
        &mut self,
        chunk_type: ChunkType,
        chunk: ParsedChunk,
    ) -> Result<ChunkOutcome, DecodeError> {
        match chunk_type {
            ChunkType::Abort => {
                // §6.7.3: "the receiver shall ignore the Message but shall not
                // close the SecureChannel."
                self.reassembly.clear();
                let mut r = UaReader::new(&chunk.body);
                let error = r.read_u32()?;
                let reason = read_string(&mut r)?;
                Ok(ChunkOutcome::Abort { error, reason })
            }
            ChunkType::Intermediate => {
                if self.reassembly.len() + chunk.body.len() > MAX_REASSEMBLED_MESSAGE {
                    self.reassembly.clear();
                    return Err(DecodeError::MalformedMessage {
                        message: "reassembled Message exceeds MAX_REASSEMBLED_MESSAGE",
                    });
                }
                self.reassembly.extend_from_slice(&chunk.body);
                Ok(ChunkOutcome::Incomplete)
            }
            ChunkType::Final => {
                if self.reassembly.is_empty() {
                    return Ok(ChunkOutcome::Complete(chunk));
                }
                if self.reassembly.len() + chunk.body.len() > MAX_REASSEMBLED_MESSAGE {
                    self.reassembly.clear();
                    return Err(DecodeError::MalformedMessage {
                        message: "reassembled Message exceeds MAX_REASSEMBLED_MESSAGE",
                    });
                }
                let mut body = core::mem::take(&mut self.reassembly);
                body.extend_from_slice(&chunk.body);
                Ok(ChunkOutcome::Complete(ParsedChunk { body, ..chunk }))
            }
        }
    }
}

/// Splits a byte stream that holds one or more consecutive UACP messages into
/// the individual messages, using each `MessageSize` header field (Part 6
/// §7.1.2.2). A caller that received a chunked Message in one read gets the
/// chunks back one by one.
///
/// # Errors
/// [`DecodeError`] if a `MessageSize` is below the 8-byte header or runs past
/// the end of the buffer.
pub fn split_messages(bytes: &[u8]) -> Result<Vec<&[u8]>, DecodeError> {
    let mut out = Vec::new();
    let mut offset = 0;
    while offset < bytes.len() {
        if bytes.len() - offset < 8 {
            return Err(DecodeError::MalformedMessage {
                message: "UACP message shorter than its 8-byte header",
            });
        }
        let size = u32::from_le_bytes([
            bytes[offset + 4],
            bytes[offset + 5],
            bytes[offset + 6],
            bytes[offset + 7],
        ]) as usize;
        if size < 8 || offset + size > bytes.len() {
            return Err(DecodeError::MalformedMessage {
                message: "UACP MessageSize does not fit the buffer",
            });
        }
        out.push(&bytes[offset..offset + size]);
        offset += size;
    }
    Ok(out)
}

/// Parses one SecureChannel chunk (SecurityMode `None`).
///
/// # Errors
/// [`DecodeError`] on truncation, an unknown message type, or a non-`OPN`/
/// `MSG`/`CLO` chunk.
pub fn parse_chunk(bytes: &[u8]) -> Result<ParsedChunk, DecodeError> {
    let mut r = UaReader::new(bytes);
    let header = MessageHeader::decode(&mut r)?;
    let channel_id = r.read_u32()?;
    let (asymmetric_header, token_id) = match header.message_type {
        MessageType::OpenSecureChannel => (
            Some(AsymmetricAlgorithmSecurityHeader::decode(&mut r)?),
            None,
        ),
        MessageType::Message | MessageType::CloseSecureChannel => (None, Some(r.read_u32()?)),
        _ => {
            return Err(DecodeError::MalformedMessage {
                message: "not a SecureChannel chunk (expected OPN/MSG/CLO)",
            });
        }
    };
    let sequence_header = SequenceHeader {
        sequence_number: r.read_u32()?,
        request_id: r.read_u32()?,
    };
    let body = r.read_bytes(r.remaining())?.to_vec();
    Ok(ParsedChunk {
        message_type: header.message_type,
        channel_id,
        asymmetric_header,
        token_id,
        sequence_header,
        body,
    })
}

fn frame_chunk(
    message_type: MessageType,
    chunk_type: ChunkType,
    inner: &[u8],
) -> Result<Vec<u8>, EncodeError> {
    const HEADER_LEN: usize = 8;
    let total = HEADER_LEN + inner.len();
    let message_size = u32::try_from(total).map_err(|_| EncodeError::LengthOverflow {
        what: "SecureChannel MessageSize",
        len: total,
    })?;
    let mut w = UaWriter::with_capacity(total);
    MessageHeader {
        message_type,
        chunk_type,
        message_size,
    }
    .encode(&mut w);
    w.write_bytes(inner);
    Ok(w.into_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_secure_channel_round_trip_none() {
        let req = OpenSecureChannelRequest {
            request_header: RequestHeader::new(NodeId::numeric(0, 0), 1),
            client_protocol_version: 0,
            request_type: SecurityTokenRequestType::Issue,
            security_mode: MessageSecurityMode::None,
            client_nonce: Vec::new(),
            requested_lifetime: 3_600_000,
        };
        let body = req.encode().expect("encode");

        let mut ch = SecureChannel::new(0, 0);
        let chunk = ch.open_chunk(1, &body).expect("chunk");

        let parsed = parse_chunk(&chunk).expect("parse");
        assert_eq!(parsed.message_type, MessageType::OpenSecureChannel);
        assert_eq!(parsed.sequence_header.request_id, 1);
        assert_eq!(
            parsed
                .asymmetric_header
                .as_ref()
                .unwrap()
                .security_policy_uri,
            SECURITY_POLICY_NONE
        );

        // Body: TypeId then fields.
        let mut br = UaReader::new(&parsed.body);
        let type_id = NodeId::decode(&mut br).expect("type id");
        assert_eq!(type_id, node_ids::OPEN_SECURE_CHANNEL_REQUEST);
        let back = OpenSecureChannelRequest::decode_body(&mut br).expect("body");
        assert_eq!(back, req);
    }

    #[test]
    fn open_response_round_trip() {
        let resp = OpenSecureChannelResponse {
            response_header: ResponseHeader::new(1, 0),
            server_protocol_version: 0,
            security_token: ChannelSecurityToken {
                channel_id: 42,
                token_id: 7,
                created_at: 132_000_000_000_000_000,
                revised_lifetime: 600_000,
            },
            server_nonce: Vec::new(),
        };
        let body = resp.encode().expect("encode");
        let mut br = UaReader::new(&body);
        assert_eq!(
            NodeId::decode(&mut br).expect("id"),
            node_ids::OPEN_SECURE_CHANNEL_RESPONSE
        );
        assert_eq!(
            OpenSecureChannelResponse::decode_body(&mut br).expect("b"),
            resp
        );
    }

    #[test]
    fn message_chunk_round_trip() {
        let mut ch = SecureChannel::new(42, 7);
        let chunk = ch.message_chunk(5, &[0xAA, 0xBB]).expect("chunk");
        let parsed = parse_chunk(&chunk).expect("parse");
        assert_eq!(parsed.message_type, MessageType::Message);
        assert_eq!(parsed.channel_id, 42);
        assert_eq!(parsed.token_id, Some(7));
        assert_eq!(parsed.sequence_header.request_id, 5);
        assert_eq!(parsed.body, alloc::vec![0xAA, 0xBB]);
    }

    #[test]
    fn chunked_message_round_trips_across_c_and_f() {
        // Part 6 §6.7.1: a Message larger than the negotiated buffer is broken
        // into MessageChunks; §6.7.2.2 Table 57 marks all but the last `C`.
        let mut sender = SecureChannel::new(42, 7);
        sender.set_send_buffer_size(MIN_SEND_BUFFER_SIZE);
        let body: Vec<u8> = (0..30_000u32).map(|i| (i % 251) as u8).collect();
        let chunks = sender.message_chunks(5, &body).expect("chunks");
        assert!(
            chunks.len() > 1,
            "expected several chunks, got {}",
            chunks.len()
        );
        for (i, c) in chunks.iter().enumerate() {
            let last = i + 1 == chunks.len();
            assert_eq!(c[3], if last { b'F' } else { b'C' }, "chunk {i} IsFinal");
            assert!(
                c.len() <= MIN_SEND_BUFFER_SIZE as usize,
                "chunk {i} is {} bytes, above the negotiated buffer",
                c.len()
            );
        }

        let mut receiver = SecureChannel::new(42, 7);
        let mut done = None;
        for c in &chunks {
            match receiver.open_incoming(c).expect("open") {
                ChunkOutcome::Incomplete => {}
                ChunkOutcome::Complete(chunk) => done = Some(chunk),
                ChunkOutcome::Abort { .. } => panic!("unexpected abort"),
            }
        }
        let reassembled = done.expect("final chunk");
        assert_eq!(reassembled.body, body);
        assert_eq!(reassembled.sequence_header.request_id, 5);

        // A body that fits stays one final chunk.
        let mut small = SecureChannel::new(1, 1);
        assert_eq!(small.message_chunks(1, &[1, 2, 3]).expect("one").len(), 1);
    }

    #[test]
    fn abort_chunk_reports_the_error_and_keeps_the_channel() {
        // Part 6 §6.7.3 Table 63 — the abort body is `Error` + `Reason`.
        let mut sender = SecureChannel::new(9, 3);
        let mut receiver = SecureChannel::new(9, 3);
        let first = sender
            .message_chunks(11, &alloc::vec![0u8; 20_000])
            .expect("chunks");
        assert!(matches!(
            receiver.open_incoming(&first[0]).expect("open"),
            ChunkOutcome::Incomplete
        ));
        // A real sender aborts *instead of* emitting the remaining chunks, so
        // the abort carries the SequenceNumber that would have followed the one
        // intermediate chunk the receiver saw. `sender` has already numbered
        // all three chunks, so the abort is framed from a channel positioned
        // where the sender would have been when it gave up.
        let mut aborting = SecureChannel::new(9, 3);
        aborting.next_send_sequence();
        let abort = aborting
            .abort_chunk(11, 0x8086_0000, "encoder ran out of buffer")
            .expect("abort");
        assert_eq!(abort[3], b'A');
        match receiver.open_incoming(&abort).expect("open abort") {
            ChunkOutcome::Abort { error, reason } => {
                assert_eq!(error, 0x8086_0000);
                assert_eq!(reason, "encoder ran out of buffer");
            }
            other => panic!("expected an abort, got {other:?}"),
        }
        // "the receiver shall ignore the Message but shall not close the
        // SecureChannel" — the next Message still reassembles cleanly.
        let next = aborting.message_chunks(12, &[7, 8, 9]).expect("chunks");
        match receiver.open_incoming(&next[0]).expect("open") {
            ChunkOutcome::Complete(c) => assert_eq!(c.body, alloc::vec![7, 8, 9]),
            other => panic!("expected a complete Message, got {other:?}"),
        }
    }

    #[test]
    fn split_messages_splits_by_message_size() {
        let mut ch = SecureChannel::new(1, 1);
        let a = ch.message_chunk(1, &[0xAA]).expect("a");
        let b = ch.message_chunk(2, &[0xBB, 0xCC]).expect("b");
        let mut stream = a.clone();
        stream.extend_from_slice(&b);
        let parts = split_messages(&stream).expect("split");
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0], a.as_slice());
        assert_eq!(parts[1], b.as_slice());
        // A MessageSize that runs past the buffer is rejected, not truncated.
        let mut broken = a.clone();
        broken[4] = 0xFF;
        assert!(split_messages(&broken).is_err());
    }

    #[test]
    fn response_header_with_string_table_round_trips() {
        let h = ResponseHeader {
            timestamp: 5,
            request_handle: 9,
            service_result: 0x8000_0000,
            string_table: alloc::vec![String::from("a"), String::from("b")],
            additional_header: null_extension_object(),
        };
        let mut w = UaWriter::new();
        h.encode(&mut w).expect("enc");
        let mut r = UaReader::new(w.as_slice());
        assert_eq!(ResponseHeader::decode(&mut r).expect("dec"), h);
    }
}
