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
//! [`SymmetricAlgorithmSecurityHeader`] (just a `TokenId`). With
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
    pub const GET_ENDPOINTS_REQUEST: NodeId = NodeId::numeric(0, 426);
    /// `GetEndpointsResponse_Encoding_DefaultBinary`.
    pub const GET_ENDPOINTS_RESPONSE: NodeId = NodeId::numeric(0, 429);
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

impl RequestHeader {
    /// A minimal header with the given authentication token and handle.
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
/// `CLO` chunks and parses them back. With a [`SecuritySession`] installed
/// (feature `crypto`) the symmetric chunks are signed/encrypted; otherwise
/// they are plaintext (SecurityMode `None`).
#[derive(Debug, Clone)]
pub struct SecureChannel {
    channel_id: u32,
    token_id: u32,
    send_sequence_number: u32,
    #[cfg(feature = "crypto")]
    security: Option<SecuritySession>,
}

impl SecureChannel {
    /// Creates a SecureChannel with the given ids (token `0` until issued).
    #[must_use]
    pub fn new(channel_id: u32, token_id: u32) -> Self {
        Self {
            channel_id,
            token_id,
            send_sequence_number: 0,
            #[cfg(feature = "crypto")]
            security: None,
        }
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

    fn next_sequence(&mut self) -> u32 {
        self.send_sequence_number = self.send_sequence_number.wrapping_add(1);
        self.send_sequence_number
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
        frame_chunk(MessageType::OpenSecureChannel, &inner.into_vec())
    }

    /// Frames a `MSG` chunk around a service message body (SecurityMode
    /// `None`).
    ///
    /// # Errors
    /// [`EncodeError`] on an oversized message.
    pub fn message_chunk(&mut self, request_id: u32, body: &[u8]) -> Result<Vec<u8>, EncodeError> {
        self.symmetric_chunk(MessageType::Message, request_id, body)
    }

    /// Frames a `CLO` chunk (CloseSecureChannel).
    ///
    /// # Errors
    /// [`EncodeError`] on an oversized message.
    pub fn close_chunk(&mut self, request_id: u32, body: &[u8]) -> Result<Vec<u8>, EncodeError> {
        self.symmetric_chunk(MessageType::CloseSecureChannel, request_id, body)
    }

    fn symmetric_chunk(
        &mut self,
        message_type: MessageType,
        request_id: u32,
        body: &[u8],
    ) -> Result<Vec<u8>, EncodeError> {
        let seq = self.next_sequence();
        #[cfg(feature = "crypto")]
        if let Some(sess) = &self.security {
            return crate::crypto::build_symmetric_chunk(
                sess.mode,
                &sess.send_keys,
                message_type,
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
        frame_chunk(message_type, &inner.into_vec())
    }

    /// Parses an incoming `MSG`/`CLO` chunk, transparently verifying and
    /// decrypting it when a [`SecuritySession`] is installed. Returns the same
    /// [`ParsedChunk`] shape as [`parse_chunk`] either way.
    ///
    /// # Errors
    /// [`DecodeError`] on truncation, a bad signature, or bad padding.
    pub fn open_incoming(&self, bytes: &[u8]) -> Result<ParsedChunk, DecodeError> {
        #[cfg(feature = "crypto")]
        if let Some(sess) = &self.security {
            let opened = crate::crypto::open_symmetric_chunk(sess.mode, &sess.recv_keys, bytes)
                .map_err(|_| DecodeError::MalformedMessage {
                    message: "secured chunk verification/decryption failed",
                })?;
            return Ok(ParsedChunk {
                message_type: opened.message_type,
                channel_id: opened.channel_id,
                asymmetric_header: None,
                token_id: opened.token_id,
                sequence_header: SequenceHeader {
                    sequence_number: opened.sequence_number,
                    request_id: opened.request_id,
                },
                body: opened.body,
            });
        }
        parse_chunk(bytes)
    }
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

fn frame_chunk(message_type: MessageType, inner: &[u8]) -> Result<Vec<u8>, EncodeError> {
    const HEADER_LEN: usize = 8;
    let total = HEADER_LEN + inner.len();
    let message_size = u32::try_from(total).map_err(|_| EncodeError::LengthOverflow {
        what: "SecureChannel MessageSize",
        len: total,
    })?;
    let mut w = UaWriter::with_capacity(total);
    MessageHeader {
        message_type,
        chunk_type: ChunkType::Final,
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
