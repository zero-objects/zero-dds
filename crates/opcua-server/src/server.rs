// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The OPC-UA [`Server`]: a request-driven state machine that runs the
//! Hello → OpenSecureChannel → Session handshake and dispatches the Discovery,
//! Attribute, View, Method, MonitoredItem and Subscription service sets against
//! an [`AddressSpace`], in SecurityMode `None` and — with the `crypto` feature —
//! in the secured SecurityPolicies.

use alloc::string::String;
use alloc::vec::Vec;

use zerodds_opcua_gateway::data_value::DataValue;
use zerodds_opcua_gateway::historical::adapter::{HistoryReadError, ReadHistoryQuery};
use zerodds_opcua_gateway::node_id::{ExpandedNodeId, NodeId};
use zerodds_opcua_gateway::types::{LocalizedText, QualifiedName};
use zerodds_opcua_pubsub::uadp::datatypes::{
    ApplicationType, EndpointDescription, MessageSecurityMode,
};
use zerodds_opcua_pubsub::{DecodeError, EncodeError, UaDecode, UaReader};
use zerodds_opcua_uacp::connection::{
    AcknowledgeMessage, HelloMessage, MessageHeader, MessageType,
};
use zerodds_opcua_uacp::securechannel::{
    ChannelSecurityToken, ChunkOutcome, OpenSecureChannelRequest, OpenSecureChannelResponse,
    ResponseHeader, SECURITY_POLICY_NONE, SecureChannel, SecurityTokenRequestType, parse_chunk,
};

use crate::address_space::AddressSpace;
use crate::attributes::{MonitoringMode, NumericRange, TimestampsToReturn};
use crate::identity::{ClientUserId, UserIdentityToken, UserStore};
use crate::services::{
    ActivateSessionResponse, BrowseDescription, BrowseNextResponse, BrowseResponse, BrowseResult,
    CallMethodResult, CallResponse, CloseSessionResponse, CreateMonitoredItemsResponse,
    CreateSessionResponse, CreateSubscriptionResponse, DataChangeNotification,
    DeleteMonitoredItemsResponse, DeleteSubscriptionsResponse, FindServersResponse,
    GetEndpointsResponse, HistoryData, HistoryReadDetails, HistoryReadResponse, HistoryReadResult,
    ModifyMonitoredItemsResponse, ModifySubscriptionResponse, MonitoredItemCreateResult,
    MonitoredItemModifyResult, MonitoredItemNotification, NotificationMessage, PublishResponse,
    ReadResponse, ReferenceDescription, RepublishResponse, ServiceFault, ServiceRequest,
    ServiceResponse, SetMonitoringModeResponse, SetPublishingModeResponse, SetTriggeringResponse,
    SignatureData, StatusChangeNotification, TransferResult, TransferSubscriptionsResponse,
    WriteResponse, null_filter,
};
use crate::status;

use alloc::boxed::Box;
#[cfg(feature = "crypto")]
use zerodds_opcua_uacp::crypto::{
    AsymmetricContext, CryptoRngCore, RsaPrivateKey, RsaPublicKey, SecuredMode, SecurityPolicy,
    build_asymmetric_chunk, derive_keys, open_asymmetric_chunk,
};
#[cfg(feature = "crypto")]
use zerodds_opcua_uacp::securechannel::SecuritySession;

/// The server's secured-SecurityPolicy configuration (feature `crypto`): the
/// server's own RSA key + certificate, the trusted client certificate +
/// public key, and a caller-supplied CSPRNG. With this set, the server runs the
/// secured OpenSecureChannel handshake and protects `MSG`/`CLO`.
#[cfg(feature = "crypto")]
pub struct ServerSecurity {
    /// Negotiated SecurityPolicy.
    pub policy: SecurityPolicy,
    /// Sign or SignAndEncrypt.
    pub mode: SecuredMode,
    /// The server's RSA private key.
    pub private_key: RsaPrivateKey,
    /// The server's DER application certificate.
    pub certificate: Vec<u8>,
    /// The trusted client's DER certificate.
    pub client_certificate: Vec<u8>,
    /// The trusted client's RSA public key (verifies the client's OPN).
    pub client_public_key: RsaPublicKey,
    /// A cryptographically secure RNG (OS RNG under `std`).
    pub rng: Box<dyn CryptoRngCore + Send>,
}

/// StatusCode `Good`.
pub const GOOD: u32 = status::GOOD;
/// StatusCode `Bad_NodeIdUnknown`.
pub const BAD_NODE_ID_UNKNOWN: u32 = status::BAD_NODE_ID_UNKNOWN;
/// StatusCode `Bad_MethodInvalid`.
pub const BAD_METHOD_INVALID: u32 = status::BAD_METHOD_INVALID;
/// StatusCode `Bad_AttributeIdInvalid`.
pub const BAD_ATTRIBUTE_ID_INVALID: u32 = status::BAD_ATTRIBUTE_ID_INVALID;
/// StatusCode `Bad_SubscriptionIdInvalid`.
pub const BAD_SUBSCRIPTION_ID_INVALID: u32 = status::BAD_SUBSCRIPTION_ID_INVALID;
/// StatusCode `Bad_InvalidTimestamp`.
pub const BAD_INVALID_TIMESTAMP: u32 = status::BAD_INVALID_TIMESTAMP;

/// Part 4 §5.14.1 (i): the retransmission queue holds sent
/// NotificationMessages until they are acknowledged. This is its depth; the
/// oldest message is discarded when a new one does not fit.
const RETRANSMISSION_QUEUE_DEPTH: usize = 32;

/// Part 4 §5.9.2.4 `Bad_NoContinuationPoints`: a Server allocates a bounded
/// number of Browse continuation points.
const MAX_CONTINUATION_POINTS: usize = 16;

/// A monitored item the server samples on Publish (Part 4 §5.13.1).
#[derive(Debug, Clone)]
struct MonitoredItem {
    id: u32,
    client_handle: u32,
    node_id: NodeId,
    attribute_id: u32,
    index_range: Option<NumericRange>,
    monitoring_mode: MonitoringMode,
    sampling_interval: f64,
    queue_size: u32,
    discard_oldest: bool,
    timestamps_to_return: TimestampsToReturn,
    last_value: Option<DataValue>,
    /// Notifications sampled but not yet reported (Part 4 §5.13.1.5).
    queue: Vec<DataValue>,
    /// The items this one triggers (Part 4 §5.13.5).
    triggered_items: Vec<u32>,
}

/// A server-side subscription (Part 4 §5.14.1).
#[derive(Debug, Clone)]
struct Subscription {
    id: u32,
    /// The session that owns it (Part 4 §5.14.7 transfers this).
    session_id: u32,
    publishing_enabled: bool,
    publishing_interval: f64,
    lifetime_count: u32,
    max_keep_alive_count: u32,
    max_notifications_per_publish: u32,
    priority: u8,
    seq: u32,
    next_item_id: u32,
    items: Vec<MonitoredItem>,
    /// Part 4 §5.14.1 (i) — sent messages awaiting acknowledgement.
    retransmission_queue: Vec<NotificationMessage>,
    /// Set once by a TransferSubscriptions that took this subscription over,
    /// so the losing session's next Publish carries the §7.25.4
    /// `Good_SubscriptionTransferred` StatusChangeNotification.
    transferred_away_from: Option<u32>,
    /// Part 4 §5.14.7.2 `sendInitialValues`.
    send_initial_values: bool,
}

/// A Session (Part 4 §5.7).
#[derive(Debug, Clone)]
struct Session {
    id: u32,
    authentication_token: NodeId,
    activated: bool,
    user: ClientUserId,
    nonce: Vec<u8>,
}

/// A Browse continuation point (Part 4 §7.9).
#[derive(Debug, Clone)]
struct ContinuationPoint {
    id: Vec<u8>,
    remaining: Vec<ReferenceDescription>,
}

/// An error from the server state machine.
#[derive(Debug, Clone, PartialEq)]
pub enum ServerError {
    /// A message could not be decoded.
    Decode(DecodeError),
    /// A response could not be encoded.
    Encode(EncodeError),
    /// A protocol-state violation.
    Protocol(&'static str),
}

impl From<DecodeError> for ServerError {
    fn from(e: DecodeError) -> Self {
        Self::Decode(e)
    }
}

impl From<EncodeError> for ServerError {
    fn from(e: EncodeError) -> Self {
        Self::Encode(e)
    }
}

impl core::fmt::Display for ServerError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Decode(e) => write!(f, "decode error: {e}"),
            Self::Encode(e) => write!(f, "encode error: {e}"),
            Self::Protocol(m) => write!(f, "protocol error: {m}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ServerError {}

/// An OPC-UA server serving an [`AddressSpace`] over the SecureChannel/UACP
/// transport. Drive it by feeding each received UACP message to
/// [`process`](Self::process); the returned bytes (if any) are the response to
/// send back.
pub struct Server {
    address_space: AddressSpace,
    endpoint_url: String,
    /// The open SecureChannels (Part 4 §5.6.2: a Server holds several, one per
    /// connected application instance), keyed by their SecureChannelId.
    channels: Vec<SecureChannel>,
    next_channel_id: u32,
    next_token_id: u32,
    next_session: u32,
    sessions: Vec<Session>,
    subscriptions: Vec<Subscription>,
    next_subscription_id: u32,
    continuation_points: Vec<ContinuationPoint>,
    next_continuation_point: u32,
    /// The peer ReceiveBufferSize the last Hello negotiated (Part 6 §7.1.2.3);
    /// it caps the chunk size of every SecureChannel opened afterwards.
    pending_send_buffer_size: u32,
    verify_request_timestamp: bool,
    users: UserStore,
    #[cfg(feature = "crypto")]
    security: Option<ServerSecurity>,
    write_hook: Option<WriteHook>,
}

/// A hook [`Server`] calls on every `Write` service target, before it ever
/// reaches the AddressSpace: the target `WriteValue` plus the calling
/// session's identity in, `Some(status)` refuses the write with that
/// StatusCode (the AddressSpace is never touched), `None` lets it proceed
/// as normal.
///
/// Until this existed, nothing in this crate carried a Write's calling
/// session identity anywhere a caller could act on it — a daemon wanting
/// per-session ACL on Write had no hook to attach to (see
/// `docs/specs/zerodds-opcua-bridge-1.0.md` §7's named limitation, which
/// this closes).
pub type WriteHook =
    Box<dyn FnMut(&crate::services::WriteValue, &ClientUserId) -> Option<u32> + Send>;

impl core::fmt::Debug for Server {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Server")
            .field("endpoint_url", &self.endpoint_url)
            .field("channels", &self.channels.len())
            .field("sessions", &self.sessions.len())
            .field("subscriptions", &self.subscriptions.len())
            .finish_non_exhaustive()
    }
}

impl Server {
    /// Creates a server serving `address_space` at `endpoint_url`, accepting
    /// the anonymous UserIdentityToken.
    #[must_use]
    pub fn new(endpoint_url: impl Into<String>, address_space: AddressSpace) -> Self {
        Self {
            address_space,
            endpoint_url: endpoint_url.into(),
            channels: Vec::new(),
            next_channel_id: 1,
            next_token_id: 1,
            next_session: 1,
            sessions: Vec::new(),
            subscriptions: Vec::new(),
            next_subscription_id: 1,
            continuation_points: Vec::new(),
            next_continuation_point: 1,
            pending_send_buffer_size: 8192,
            verify_request_timestamp: true,
            users: UserStore::anonymous_only(),
            #[cfg(feature = "crypto")]
            security: None,
            write_hook: None,
        }
    }

    /// Installs (or replaces) the [`WriteHook`] every subsequent `Write`
    /// service target is offered to first.
    pub fn set_write_hook(&mut self, hook: WriteHook) {
        self.write_hook = Some(hook);
    }

    /// Whether a request whose header carries no send time (Part 4 §7.33) is
    /// rejected with `Bad_InvalidTimestamp`. On by default, matching
    /// open62541's `verifyRequestTimestamp`; turn it off for clients without a
    /// clock, deliberately and in one place.
    pub fn set_verify_request_timestamp(&mut self, verify: bool) {
        self.verify_request_timestamp = verify;
    }

    /// Replaces the accepted user identities (Part 4 §7.40/§7.41). Their
    /// [`UserTokenPolicy`](crate::identity::UserTokenPolicy)s are published in
    /// the `EndpointDescription` that GetEndpoints and CreateSession return.
    pub fn set_user_store(&mut self, users: UserStore) {
        self.users = users;
    }

    /// The accepted user identities.
    #[must_use]
    pub fn user_store(&self) -> &UserStore {
        &self.users
    }

    /// Enables secured SecurityPolicies for the OpenSecureChannel handshake.
    #[cfg(feature = "crypto")]
    pub fn set_security(&mut self, security: ServerSecurity) {
        self.security = Some(security);
    }

    /// Mutable access to the served AddressSpace.
    pub fn address_space_mut(&mut self) -> &mut AddressSpace {
        &mut self.address_space
    }

    /// Processes one received UACP message, returning the response to send
    /// (or `None` for messages that need no reply, e.g. CloseSecureChannel and
    /// the non-final chunks of a multi-chunk Message).
    ///
    /// # Errors
    /// [`ServerError`] on a malformed message or protocol-state violation.
    pub fn process(&mut self, incoming: &[u8]) -> Result<Option<Vec<u8>>, ServerError> {
        // A chunked Message may arrive as one buffer of consecutive
        // MessageChunks (Part 6 §6.7.1); handle each in turn and concatenate
        // whatever they produce.
        let messages = zerodds_opcua_uacp::securechannel::split_messages(incoming)?;
        if messages.len() > 1 {
            let mut out = Vec::new();
            for message in messages {
                if let Some(part) = self.process_one(message)? {
                    out.extend_from_slice(&part);
                }
            }
            return Ok(if out.is_empty() { None } else { Some(out) });
        }
        self.process_one(incoming)
    }

    fn process_one(&mut self, incoming: &[u8]) -> Result<Option<Vec<u8>>, ServerError> {
        let mut r = UaReader::new(incoming);
        let header = MessageHeader::decode(&mut r)?;
        match header.message_type {
            MessageType::Hello => {
                let hello = HelloMessage::decode_body(&mut r)?;
                let receive_buffer_size = hello.receive_buffer_size.max(8192);
                let send_buffer_size = hello.send_buffer_size.max(8192);
                let ack = AcknowledgeMessage {
                    protocol_version: 0,
                    receive_buffer_size,
                    send_buffer_size,
                    max_message_size: hello.max_message_size,
                    max_chunk_count: hello.max_chunk_count,
                };
                // Part 6 §7.1.2.3: the peer's ReceiveBufferSize caps the chunks
                // we may send it, on this connection's channels and the next.
                for channel in &mut self.channels {
                    channel.set_send_buffer_size(receive_buffer_size);
                }
                self.pending_send_buffer_size = receive_buffer_size;
                Ok(Some(ack.encode()?))
            }
            MessageType::OpenSecureChannel => {
                let chunk = parse_chunk(incoming)?;
                let secured = chunk
                    .asymmetric_header
                    .as_ref()
                    .is_some_and(|h| h.security_policy_uri != SECURITY_POLICY_NONE);
                #[cfg(feature = "crypto")]
                if secured && self.security.is_some() {
                    return Ok(Some(self.open_secure_channel_secured(incoming)?));
                }
                if secured {
                    return Err(ServerError::Protocol(
                        "secured OpenSecureChannel but server has no SecurityPolicy configured",
                    ));
                }
                let mut br = UaReader::new(&chunk.body);
                let _type_id = NodeId::decode(&mut br)?;
                let req = OpenSecureChannelRequest::decode_body(&mut br)?;
                Ok(Some(self.open_secure_channel(
                    &req,
                    chunk.channel_id,
                    chunk.sequence_header.request_id,
                    chunk.sequence_header.sequence_number,
                )?))
            }
            MessageType::Message => {
                // Part 6 §6.7.2.2: the SecureChannelId in the header names the
                // channel this chunk belongs to.
                let channel_id = chunk_channel_id(incoming)?;
                let outcome = self
                    .channel_mut(channel_id)
                    .ok_or(ServerError::Protocol("MSG for an unknown SecureChannel"))?
                    .open_incoming(incoming)?;
                let chunk = match outcome {
                    // Part 6 §6.7.2.2: an intermediate chunk carries no
                    // complete Message yet, so there is nothing to answer.
                    ChunkOutcome::Incomplete => return Ok(None),
                    // Part 6 §6.7.3: "the receiver shall ignore the Message but
                    // shall not close the SecureChannel."
                    ChunkOutcome::Abort { .. } => return Ok(None),
                    ChunkOutcome::Complete(chunk) => chunk,
                };
                let req = ServiceRequest::decode(&chunk.body)?;
                let resp = if self.verify_request_timestamp && req.request_header().timestamp == 0 {
                    ServiceResponse::ServiceFault(ServiceFault::new(
                        req.request_header().request_handle,
                        BAD_INVALID_TIMESTAMP,
                    ))
                } else {
                    self.handle_service(req)
                };
                let body = resp.encode()?;
                let channel = self
                    .channel_mut(channel_id)
                    .ok_or(ServerError::Protocol("MSG for an unknown SecureChannel"))?;
                Ok(Some(
                    channel.message_chunk(chunk.sequence_header.request_id, &body)?,
                ))
            }
            MessageType::CloseSecureChannel => {
                // Part 4 §5.6.3: closing one SecureChannel leaves the others.
                let channel_id = chunk_channel_id(incoming)?;
                self.channels.retain(|c| c.channel_id() != channel_id);
                Ok(None)
            }
            other => {
                let _ = other;
                Err(ServerError::Protocol("unexpected message type"))
            }
        }
    }

    fn open_secure_channel(
        &mut self,
        req: &OpenSecureChannelRequest,
        incoming_channel_id: u32,
        request_id: u32,
        request_sequence_number: u32,
    ) -> Result<Vec<u8>, ServerError> {
        // Part 4 §5.6.2: RENEW "creates a new SecurityToken for an existing
        // SecureChannel" — the channel id survives, the token id does not.
        let renew = req.request_type == SecurityTokenRequestType::Renew;
        let existing = (incoming_channel_id != 0).then_some(incoming_channel_id);
        if renew && existing.is_none() {
            return Err(ServerError::Protocol(
                "OpenSecureChannel RENEW without an existing SecureChannel",
            ));
        }
        let channel_id = if renew {
            existing.unwrap_or(0)
        } else {
            let id = self.next_channel_id;
            self.next_channel_id += 1;
            id
        };
        let token_id = self.next_token_id;
        self.next_token_id += 1;

        let mut channel = if renew {
            self.take_channel(channel_id)
                .unwrap_or_else(|| SecureChannel::new(channel_id, token_id))
        } else {
            SecureChannel::new(channel_id, token_id)
        };
        channel.set_send_buffer_size(self.pending_send_buffer_size);
        // The client's OPN is the channel's first chunk (Part 6 §6.7.2.3), so
        // it sets the baseline every later MSG has to continue. On a renew the
        // counter continues instead of restarting.
        if renew {
            channel.note_incoming_sequence(request_sequence_number)?;
        } else {
            channel.reset_sequences();
            channel.note_incoming_sequence(request_sequence_number)?;
        }

        let resp = OpenSecureChannelResponse {
            response_header: ResponseHeader::new(req.request_header.request_handle, GOOD),
            server_protocol_version: 0,
            security_token: ChannelSecurityToken {
                channel_id,
                token_id,
                created_at: 0,
                revised_lifetime: req.requested_lifetime.max(60_000),
            },
            server_nonce: Vec::new(),
        };
        let body = resp.encode()?;
        let out = channel.open_chunk(request_id, &body)?;
        channel.set_token_id(token_id);
        self.insert_channel(channel);
        Ok(out)
    }

    /// The secured OpenSecureChannel handshake: opens the client's asymmetric
    /// OPN, derives the §6.7.6 keys, and returns the secured OPN response while
    /// installing the symmetric [`SecuritySession`] on the channel. Handles
    /// both ISSUE and RENEW (Part 4 §5.6.2).
    #[cfg(feature = "crypto")]
    fn open_secure_channel_secured(&mut self, incoming: &[u8]) -> Result<Vec<u8>, ServerError> {
        let sec = self
            .security
            .as_mut()
            .ok_or(ServerError::Protocol("no SecurityPolicy configured"))?;
        let policy = sec.policy;
        let mode = sec.mode;

        // 1. Open the client's asymmetric OPN (decrypt with our key, verify with
        //    the trusted client public key).
        let opened =
            open_asymmetric_chunk(policy, &sec.private_key, &sec.client_public_key, incoming)
                .map_err(|_| ServerError::Protocol("secured OPN open/verify failed"))?;
        let mut br = UaReader::new(&opened.body);
        let _type_id = NodeId::decode(&mut br)?;
        let req = OpenSecureChannelRequest::decode_body(&mut br)?;
        let client_nonce = req.client_nonce;
        let renew = req.request_type == SecurityTokenRequestType::Renew;
        let existing = chunk_channel_id(incoming).ok().filter(|id| *id != 0);
        if renew && existing.is_none() {
            return Err(ServerError::Protocol(
                "OpenSecureChannel RENEW without an existing SecureChannel",
            ));
        }

        // 2. Generate the server nonce.
        let nonce_len = policy.sym_enc_key_len().max(32);
        let mut server_nonce = alloc::vec![0u8; nonce_len];
        sec.rng.fill_bytes(&mut server_nonce);

        // 3. Derive the §6.7.6 key sets (server send / receive).
        let send_keys = derive_keys(policy, &client_nonce, &server_nonce)
            .map_err(|_| ServerError::Protocol("key derivation failed"))?;
        let recv_keys = derive_keys(policy, &server_nonce, &client_nonce)
            .map_err(|_| ServerError::Protocol("key derivation failed"))?;

        // 4. Allocate channel + token ids (disjoint field borrows from `sec`).
        let channel_id = if renew {
            existing.unwrap_or(0)
        } else {
            let id = self.next_channel_id;
            self.next_channel_id += 1;
            id
        };
        let token_id = self.next_token_id;
        self.next_token_id += 1;

        // 5. Build the OpenSecureChannelResponse and frame it as a secured OPN.
        let resp = OpenSecureChannelResponse {
            response_header: ResponseHeader::new(req.request_header.request_handle, GOOD),
            server_protocol_version: 0,
            security_token: ChannelSecurityToken {
                channel_id,
                token_id,
                created_at: 0,
                revised_lifetime: req.requested_lifetime.max(60_000),
            },
            server_nonce,
        };
        let resp_body = resp.encode()?;
        let mut channel = if renew {
            self.take_channel(channel_id)
                .unwrap_or_else(|| SecureChannel::new(channel_id, token_id))
        } else {
            SecureChannel::new(channel_id, token_id)
        };
        channel.set_send_buffer_size(self.pending_send_buffer_size);
        if !renew {
            channel.reset_sequences();
        }
        channel.adopt_token(channel_id, token_id);
        channel.note_incoming_sequence(opened.sequence_number)?;
        let seq = channel.next_send_sequence();
        let sec = self
            .security
            .as_mut()
            .ok_or(ServerError::Protocol("no SecurityPolicy configured"))?;
        let ctx = AsymmetricContext {
            policy,
            sender_certificate: &sec.certificate,
            sender_private_key: &sec.private_key,
            receiver_certificate: &sec.client_certificate,
            receiver_public_key: &sec.client_public_key,
        };
        // `&mut dyn` is Sized and implements CryptoRngCore, satisfying the
        // generic `R: CryptoRngCore` bound of the (Sized) chunk builder.
        let mut rng_ref: &mut dyn CryptoRngCore = sec.rng.as_mut();
        let out = build_asymmetric_chunk(
            &mut rng_ref,
            &ctx,
            channel_id,
            seq,
            opened.request_id,
            &resp_body,
        )
        .map_err(|_| ServerError::Protocol("secured OPN response build failed"))?;

        // 6. Install the symmetric session for subsequent MSG/CLO.
        channel.install_security(SecuritySession {
            policy,
            mode,
            send_keys,
            recv_keys,
        });
        self.insert_channel(channel);
        Ok(out)
    }
}

impl Server {
    /// Part 4 §5.6.2: "A Server application should limit the number of
    /// SecureChannels." Above this the oldest channel with no Session assigned
    /// is closed to make room.
    const MAX_SECURE_CHANNELS: usize = 64;

    fn channel_mut(&mut self, channel_id: u32) -> Option<&mut SecureChannel> {
        self.channels
            .iter_mut()
            .find(|c| c.channel_id() == channel_id)
    }

    fn take_channel(&mut self, channel_id: u32) -> Option<SecureChannel> {
        let pos = self
            .channels
            .iter()
            .position(|c| c.channel_id() == channel_id)?;
        Some(self.channels.remove(pos))
    }

    fn insert_channel(&mut self, channel: SecureChannel) {
        if self.channels.len() >= Self::MAX_SECURE_CHANNELS {
            // "the Server shall close the oldest unused SecureChannel that has
            // no Session assigned before reaching the maximum number of
            // supported SecureChannels."
            self.channels.remove(0);
        }
        self.channels.push(channel);
    }

    fn session_of(&self, token: &NodeId) -> Option<&Session> {
        self.sessions
            .iter()
            .find(|s| s.authentication_token == *token)
    }

    /// Part 4 §5.7.3.1: every Service other than CloseSession requires an
    /// activated Session.
    fn require_activated_session(&self, token: &NodeId) -> Result<u32, u32> {
        match self.session_of(token) {
            None => Err(status::BAD_SESSION_ID_INVALID),
            Some(s) if !s.activated => Err(status::BAD_SESSION_NOT_ACTIVATED),
            Some(s) => Ok(s.id),
        }
    }

    #[allow(clippy::too_many_lines)]
    fn handle_service(&mut self, req: ServiceRequest) -> ServiceResponse {
        let handle = req.request_header().request_handle;
        let token = req.request_header().authentication_token.clone();
        // Part 4 §5.5.1: the Discovery Services need only a SecureChannel;
        // OpenSecureChannel/CreateSession precede any Session, and CloseSession
        // is explicitly allowed before activation (§5.7.4.1).
        let session_less = matches!(
            req,
            ServiceRequest::CreateSession(_)
                | ServiceRequest::ActivateSession(_)
                | ServiceRequest::CloseSession(_)
                | ServiceRequest::GetEndpoints(_)
                | ServiceRequest::FindServers(_)
        );
        let session_id = if session_less {
            0
        } else {
            match self.require_activated_session(&token) {
                Ok(id) => id,
                Err(code) => {
                    return ServiceResponse::ServiceFault(ServiceFault::new(handle, code));
                }
            }
        };

        match req {
            ServiceRequest::CreateSession(r) => self.create_session(&r),
            ServiceRequest::ActivateSession(r) => {
                let (result, nonce) = self.activate_session(&token, &r.user_identity_token);
                match result {
                    Err(code) => ServiceResponse::ServiceFault(ServiceFault::new(handle, code)),
                    Ok(()) => ServiceResponse::ActivateSession(ActivateSessionResponse {
                        response_header: ResponseHeader::new(handle, GOOD),
                        server_nonce: nonce,
                        // Part 4 §5.7.3.2: one result per clientSoftwareCertificate;
                        // the request carries none, so the list is empty.
                        results: Vec::new(),
                    }),
                }
            }
            ServiceRequest::CloseSession(r) => {
                self.close_session(&token, r.delete_subscriptions);
                ServiceResponse::CloseSession(CloseSessionResponse {
                    response_header: ResponseHeader::new(handle, GOOD),
                })
            }
            ServiceRequest::Read(r) => self.read(handle, &r),
            ServiceRequest::Write(r) => {
                let user = self
                    .session_of(&token)
                    .map_or(ClientUserId::Anonymous, |s| s.user.clone());
                let results = r
                    .nodes_to_write
                    .iter()
                    .map(|wv| self.write_one(wv, &user))
                    .collect();
                ServiceResponse::Write(WriteResponse {
                    response_header: ResponseHeader::new(handle, GOOD),
                    results,
                })
            }
            ServiceRequest::Browse(r) => {
                if r.nodes_to_browse.is_empty() {
                    return ServiceResponse::ServiceFault(ServiceFault::new(
                        handle,
                        status::BAD_NOTHING_TO_DO,
                    ));
                }
                let max = r.requested_max_references_per_node as usize;
                let results = r
                    .nodes_to_browse
                    .clone()
                    .iter()
                    .map(|bd| self.browse_one(bd, max))
                    .collect();
                ServiceResponse::Browse(BrowseResponse {
                    response_header: ResponseHeader::new(handle, GOOD),
                    results,
                })
            }
            ServiceRequest::BrowseNext(r) => {
                if r.continuation_points.is_empty() {
                    return ServiceResponse::ServiceFault(ServiceFault::new(
                        handle,
                        status::BAD_NOTHING_TO_DO,
                    ));
                }
                let results = r
                    .continuation_points
                    .iter()
                    .map(|cp| self.browse_next_one(cp, r.release_continuation_points))
                    .collect();
                ServiceResponse::BrowseNext(BrowseNextResponse {
                    response_header: ResponseHeader::new(handle, GOOD),
                    results,
                })
            }
            ServiceRequest::GetEndpoints(_) => {
                ServiceResponse::GetEndpoints(GetEndpointsResponse {
                    response_header: ResponseHeader::new(handle, GOOD),
                    endpoints: alloc::vec![self.endpoint()],
                })
            }
            ServiceRequest::FindServers(_) => ServiceResponse::FindServers(FindServersResponse {
                response_header: ResponseHeader::new(handle, GOOD),
                servers: alloc::vec![self.endpoint().server],
            }),
            ServiceRequest::CreateSubscription(r) => {
                let id = self.next_subscription_id;
                self.next_subscription_id += 1;
                let interval = revise_publishing_interval(r.requested_publishing_interval);
                let keep_alive = revise_keep_alive(r.requested_max_keep_alive_count);
                let lifetime = revise_lifetime(r.requested_lifetime_count, keep_alive);
                self.subscriptions.push(Subscription {
                    id,
                    session_id,
                    publishing_enabled: r.publishing_enabled,
                    publishing_interval: interval,
                    lifetime_count: lifetime,
                    max_keep_alive_count: keep_alive,
                    max_notifications_per_publish: r.max_notifications_per_publish,
                    priority: r.priority,
                    seq: 0,
                    next_item_id: 1,
                    items: Vec::new(),
                    retransmission_queue: Vec::new(),
                    transferred_away_from: None,
                    send_initial_values: false,
                });
                ServiceResponse::CreateSubscription(CreateSubscriptionResponse {
                    response_header: ResponseHeader::new(handle, GOOD),
                    subscription_id: id,
                    revised_publishing_interval: interval,
                    revised_lifetime_count: lifetime,
                    revised_max_keep_alive_count: keep_alive,
                })
            }
            ServiceRequest::ModifySubscription(r) => {
                let interval = revise_publishing_interval(r.requested_publishing_interval);
                let keep_alive = revise_keep_alive(r.requested_max_keep_alive_count);
                let lifetime = revise_lifetime(r.requested_lifetime_count, keep_alive);
                let Some(sub) = self
                    .subscriptions
                    .iter_mut()
                    .find(|s| s.id == r.subscription_id && s.session_id == session_id)
                else {
                    return ServiceResponse::ServiceFault(ServiceFault::new(
                        handle,
                        BAD_SUBSCRIPTION_ID_INVALID,
                    ));
                };
                sub.publishing_interval = interval;
                sub.max_keep_alive_count = keep_alive;
                sub.lifetime_count = lifetime;
                sub.max_notifications_per_publish = r.max_notifications_per_publish;
                sub.priority = r.priority;
                ServiceResponse::ModifySubscription(ModifySubscriptionResponse {
                    response_header: ResponseHeader::new(handle, GOOD),
                    revised_publishing_interval: interval,
                    revised_lifetime_count: lifetime,
                    revised_max_keep_alive_count: keep_alive,
                })
            }
            ServiceRequest::SetPublishingMode(r) => {
                let enabled = r.publishing_enabled;
                let results = r
                    .subscription_ids
                    .iter()
                    .map(|sid| {
                        if let Some(s) = self
                            .subscriptions
                            .iter_mut()
                            .find(|s| s.id == *sid && s.session_id == session_id)
                        {
                            s.publishing_enabled = enabled;
                            GOOD
                        } else {
                            BAD_SUBSCRIPTION_ID_INVALID
                        }
                    })
                    .collect();
                ServiceResponse::SetPublishingMode(SetPublishingModeResponse {
                    response_header: ResponseHeader::new(handle, GOOD),
                    results,
                })
            }
            ServiceRequest::CreateMonitoredItems(r) => {
                self.create_monitored_items(handle, session_id, &r)
            }
            ServiceRequest::ModifyMonitoredItems(r) => {
                self.modify_monitored_items(handle, session_id, &r)
            }
            ServiceRequest::SetMonitoringMode(r) => {
                let mode = match MonitoringMode::from_i32(r.monitoring_mode) {
                    Ok(m) => m,
                    Err(code) => {
                        return ServiceResponse::ServiceFault(ServiceFault::new(handle, code));
                    }
                };
                if r.monitored_item_ids.is_empty() {
                    return ServiceResponse::ServiceFault(ServiceFault::new(
                        handle,
                        status::BAD_NOTHING_TO_DO,
                    ));
                }
                let Some(sub) = self
                    .subscriptions
                    .iter_mut()
                    .find(|s| s.id == r.subscription_id && s.session_id == session_id)
                else {
                    return ServiceResponse::ServiceFault(ServiceFault::new(
                        handle,
                        BAD_SUBSCRIPTION_ID_INVALID,
                    ));
                };
                let results = r
                    .monitored_item_ids
                    .iter()
                    .map(|mid| match sub.items.iter_mut().find(|i| i.id == *mid) {
                        None => status::BAD_MONITORED_ITEM_ID_INVALID,
                        Some(item) => {
                            item.monitoring_mode = mode;
                            // §5.13.4.1: "Setting the mode to DISABLED causes
                            // all queued Notifications to be deleted."
                            if mode == MonitoringMode::Disabled {
                                item.queue.clear();
                                item.last_value = None;
                            }
                            GOOD
                        }
                    })
                    .collect();
                ServiceResponse::SetMonitoringMode(SetMonitoringModeResponse {
                    response_header: ResponseHeader::new(handle, GOOD),
                    results,
                })
            }
            ServiceRequest::SetTriggering(r) => {
                let Some(sub) = self
                    .subscriptions
                    .iter_mut()
                    .find(|s| s.id == r.subscription_id && s.session_id == session_id)
                else {
                    return ServiceResponse::ServiceFault(ServiceFault::new(
                        handle,
                        BAD_SUBSCRIPTION_ID_INVALID,
                    ));
                };
                if r.links_to_add.is_empty() && r.links_to_remove.is_empty() {
                    return ServiceResponse::ServiceFault(ServiceFault::new(
                        handle,
                        status::BAD_NOTHING_TO_DO,
                    ));
                }
                if !sub.items.iter().any(|i| i.id == r.triggering_item_id) {
                    return ServiceResponse::ServiceFault(ServiceFault::new(
                        handle,
                        status::BAD_MONITORED_ITEM_ID_INVALID,
                    ));
                }
                let known: Vec<u32> = sub.items.iter().map(|i| i.id).collect();
                // §5.13.5.2: "The list of linksToRemove is processed before the
                // linksToAdd."
                let mut remove_results = Vec::with_capacity(r.links_to_remove.len());
                let mut add_results = Vec::with_capacity(r.links_to_add.len());
                {
                    let Some(trigger) = sub.items.iter_mut().find(|i| i.id == r.triggering_item_id)
                    else {
                        return ServiceResponse::ServiceFault(ServiceFault::new(
                            handle,
                            status::BAD_MONITORED_ITEM_ID_INVALID,
                        ));
                    };
                    for id in &r.links_to_remove {
                        if let Some(pos) = trigger.triggered_items.iter().position(|x| x == id) {
                            trigger.triggered_items.remove(pos);
                            remove_results.push(GOOD);
                        } else {
                            remove_results.push(status::BAD_MONITORED_ITEM_ID_INVALID);
                        }
                    }
                    for id in &r.links_to_add {
                        if known.contains(id) {
                            if !trigger.triggered_items.contains(id) {
                                trigger.triggered_items.push(*id);
                            }
                            add_results.push(GOOD);
                        } else {
                            add_results.push(status::BAD_MONITORED_ITEM_ID_INVALID);
                        }
                    }
                }
                ServiceResponse::SetTriggering(SetTriggeringResponse {
                    response_header: ResponseHeader::new(handle, GOOD),
                    add_results,
                    remove_results,
                })
            }
            ServiceRequest::DeleteMonitoredItems(r) => {
                if r.monitored_item_ids.is_empty() {
                    return ServiceResponse::ServiceFault(ServiceFault::new(
                        handle,
                        status::BAD_NOTHING_TO_DO,
                    ));
                }
                let Some(sub) = self
                    .subscriptions
                    .iter_mut()
                    .find(|s| s.id == r.subscription_id && s.session_id == session_id)
                else {
                    return ServiceResponse::ServiceFault(ServiceFault::new(
                        handle,
                        BAD_SUBSCRIPTION_ID_INVALID,
                    ));
                };
                let results = r
                    .monitored_item_ids
                    .iter()
                    .map(|mid| match sub.items.iter().position(|i| i.id == *mid) {
                        None => status::BAD_MONITORED_ITEM_ID_INVALID,
                        Some(pos) => {
                            sub.items.remove(pos);
                            // §5.13.6.1: "When a MonitoredItem is deleted, its
                            // triggered item links are also deleted."
                            for other in &mut sub.items {
                                other.triggered_items.retain(|x| x != mid);
                            }
                            GOOD
                        }
                    })
                    .collect();
                ServiceResponse::DeleteMonitoredItems(DeleteMonitoredItemsResponse {
                    response_header: ResponseHeader::new(handle, GOOD),
                    results,
                })
            }
            ServiceRequest::Publish(r) => self.publish(handle, session_id, &r),
            ServiceRequest::Republish(r) => {
                let Some(sub) = self
                    .subscriptions
                    .iter()
                    .find(|s| s.id == r.subscription_id && s.session_id == session_id)
                else {
                    return ServiceResponse::ServiceFault(ServiceFault::new(
                        handle,
                        BAD_SUBSCRIPTION_ID_INVALID,
                    ));
                };
                match sub
                    .retransmission_queue
                    .iter()
                    .find(|m| m.sequence_number == r.retransmit_sequence_number)
                {
                    // §5.14.6.3: "If the Server does not have the requested
                    // Message in its retransmission queue, it returns an error
                    // response" — Bad_MessageNotAvailable.
                    None => ServiceResponse::ServiceFault(ServiceFault::new(
                        handle,
                        status::BAD_MESSAGE_NOT_AVAILABLE,
                    )),
                    Some(message) => ServiceResponse::Republish(RepublishResponse {
                        response_header: ResponseHeader::new(handle, GOOD),
                        notification_message: message.clone(),
                    }),
                }
            }
            ServiceRequest::TransferSubscriptions(r) => {
                if r.subscription_ids.is_empty() {
                    return ServiceResponse::ServiceFault(ServiceFault::new(
                        handle,
                        status::BAD_NOTHING_TO_DO,
                    ));
                }
                let send_initial_values = r.send_initial_values;
                let results = r
                    .subscription_ids
                    .iter()
                    .map(|sid| self.transfer_one(*sid, session_id, send_initial_values))
                    .collect();
                ServiceResponse::TransferSubscriptions(TransferSubscriptionsResponse {
                    response_header: ResponseHeader::new(handle, GOOD),
                    results,
                })
            }
            ServiceRequest::DeleteSubscriptions(r) => {
                if r.subscription_ids.is_empty() {
                    return ServiceResponse::ServiceFault(ServiceFault::new(
                        handle,
                        status::BAD_NOTHING_TO_DO,
                    ));
                }
                let results = r
                    .subscription_ids
                    .iter()
                    .map(|sid| {
                        match self
                            .subscriptions
                            .iter()
                            .position(|s| s.id == *sid && s.session_id == session_id)
                        {
                            Some(pos) => {
                                self.subscriptions.remove(pos);
                                GOOD
                            }
                            None => BAD_SUBSCRIPTION_ID_INVALID,
                        }
                    })
                    .collect();
                ServiceResponse::DeleteSubscriptions(DeleteSubscriptionsResponse {
                    response_header: ResponseHeader::new(handle, GOOD),
                    results,
                })
            }
            ServiceRequest::Call(r) => {
                let results = r
                    .methods_to_call
                    .iter()
                    .map(
                        |m| match self.address_space.call(&m.method_id, &m.input_arguments) {
                            Some(outcome) => CallMethodResult {
                                status_code: outcome.status_code,
                                input_argument_results: m
                                    .input_arguments
                                    .iter()
                                    .map(|_| GOOD)
                                    .collect(),
                                output_arguments: outcome.output_arguments,
                            },
                            None => CallMethodResult {
                                status_code: BAD_METHOD_INVALID,
                                input_argument_results: Vec::new(),
                                output_arguments: Vec::new(),
                            },
                        },
                    )
                    .collect();
                ServiceResponse::Call(CallResponse {
                    response_header: ResponseHeader::new(handle, GOOD),
                    results,
                })
            }
            ServiceRequest::HistoryRead(r) => self.history_read(handle, &r),
        }
    }

    // -----------------------------------------------------------------------
    // Session Service Set (Part 4 §5.7).
    // -----------------------------------------------------------------------

    fn create_session(&mut self, r: &crate::services::CreateSessionRequest) -> ServiceResponse {
        let session = self.next_session;
        self.next_session += 1;
        // Part 4 §5.7.2.2: a nonce of 32..=128 bytes the client signs back in
        // ActivateSession. It is derived from the ids that identify the
        // session so a no_std server needs no entropy source of its own; a
        // secured deployment supplies its RNG through `set_security`.
        let nonce = self.session_nonce(session, 0);
        self.sessions.push(Session {
            id: session,
            authentication_token: NodeId::numeric(0, 0x1000_0000 + session),
            activated: false,
            user: ClientUserId::Anonymous,
            nonce: nonce.clone(),
        });
        ServiceResponse::CreateSession(CreateSessionResponse {
            response_header: ResponseHeader::new(r.request_header.request_handle, GOOD),
            session_id: NodeId::numeric(1, session),
            authentication_token: NodeId::numeric(0, 0x1000_0000 + session),
            revised_session_timeout: if r.requested_session_timeout > 0.0 {
                r.requested_session_timeout
            } else {
                1_200_000.0
            },
            server_nonce: nonce,
            server_certificate: self.server_certificate(),
            server_endpoints: alloc::vec![self.endpoint()],
            server_signature: SignatureData::default(),
            max_request_message_size: 0,
        })
    }

    fn activate_session(
        &mut self,
        token: &NodeId,
        user_identity_token: &zerodds_opcua_gateway::data_value::ExtensionObject,
    ) -> (Result<(), u32>, Vec<u8>) {
        let Some(index) = self
            .sessions
            .iter()
            .position(|s| s.authentication_token == *token)
        else {
            return (Err(status::BAD_SESSION_ID_INVALID), Vec::new());
        };
        let identity = match UserIdentityToken::from_extension_object(user_identity_token) {
            Ok(t) => t,
            Err(_) => return (Err(status::BAD_IDENTITY_TOKEN_INVALID), Vec::new()),
        };
        // Part 4 §7.40.5: the X.509 proof of possession is only required when
        // the SecurityPolicy asks for it; under SecurityPolicy `None` there is
        // no signature to check.
        let signature_ok = self.user_token_signature_expected().then_some(true);
        let user = match self.users.validate(&identity, signature_ok) {
            Ok(u) => u,
            Err(code) => return (Err(code), Vec::new()),
        };
        // §5.7.3.1: "Once used, a serverNonce cannot be used again. For that
        // reason, the Server returns a new serverNonce each time the
        // ActivateSession Service is called."
        let session_id = self.sessions[index].id;
        let nonce = self.session_nonce(session_id, self.next_token_id);
        let session = &mut self.sessions[index];
        session.activated = true;
        session.user = user;
        session.nonce.clone_from(&nonce);
        (Ok(()), nonce)
    }

    fn close_session(&mut self, token: &NodeId, delete_subscriptions: bool) {
        let Some(pos) = self
            .sessions
            .iter()
            .position(|s| s.authentication_token == *token)
        else {
            return;
        };
        let id = self.sessions[pos].id;
        self.sessions.remove(pos);
        if delete_subscriptions {
            // §5.14.8.1 via §5.7.4.2: "If the value is TRUE, the Server deletes
            // all Subscriptions associated with the Session."
            self.subscriptions.retain(|s| s.session_id != id);
        }
    }

    /// A per-session nonce of the 32-byte minimum Part 4 §5.7.2.2 sets. Under
    /// the `crypto` feature the configured CSPRNG fills it; without security
    /// configured, the SecurityPolicy is `None`, where §7.40.4/§7.40.5 require
    /// no proof of possession and the nonce is only an opaque echo.
    fn session_nonce(&mut self, session_id: u32, salt: u32) -> Vec<u8> {
        let mut nonce = alloc::vec![0u8; 32];
        #[cfg(feature = "crypto")]
        if let Some(sec) = self.security.as_mut() {
            sec.rng.fill_bytes(&mut nonce);
            return nonce;
        }
        for (i, b) in nonce.iter_mut().enumerate() {
            let mix = session_id
                .wrapping_mul(0x9E37_79B9)
                .wrapping_add(salt.wrapping_mul(0x85EB_CA6B))
                .wrapping_add(i as u32);
            *b = (mix >> 8) as u8;
        }
        nonce
    }

    /// Whether the SecureChannel's SecurityPolicy makes the ActivateSession
    /// `userTokenSignature` mandatory (Part 4 §7.40.5).
    fn user_token_signature_expected(&self) -> bool {
        #[cfg(feature = "crypto")]
        {
            self.security.is_some()
        }
        #[cfg(not(feature = "crypto"))]
        {
            false
        }
    }

    fn server_certificate(&self) -> Vec<u8> {
        #[cfg(feature = "crypto")]
        {
            self.security
                .as_ref()
                .map(|s| s.certificate.clone())
                .unwrap_or_default()
        }
        #[cfg(not(feature = "crypto"))]
        {
            Vec::new()
        }
    }

    // -----------------------------------------------------------------------
    // Attribute Service Set (Part 4 §5.11).
    // -----------------------------------------------------------------------

    fn read(&mut self, handle: u32, r: &crate::services::ReadRequest) -> ServiceResponse {
        // §5.11.2.3: "Bad_MaxAgeInvalid — The max age parameter is invalid."
        // §5.11.2.2: "Negative values are invalid for maxAge."
        if r.max_age < 0.0 {
            return ServiceResponse::ServiceFault(ServiceFault::new(
                handle,
                status::BAD_MAX_AGE_INVALID,
            ));
        }
        let timestamps = match TimestampsToReturn::from_i32(r.timestamps_to_return) {
            Ok(t) => t,
            Err(code) => return ServiceResponse::ServiceFault(ServiceFault::new(handle, code)),
        };
        if r.nodes_to_read.is_empty() {
            return ServiceResponse::ServiceFault(ServiceFault::new(
                handle,
                status::BAD_NOTHING_TO_DO,
            ));
        }
        let results = r
            .nodes_to_read
            .iter()
            .map(|rv| {
                let mut dv = read_one(&self.address_space, rv);
                timestamps.apply(&mut dv);
                dv
            })
            .collect();
        ServiceResponse::Read(ReadResponse {
            response_header: ResponseHeader::new(handle, GOOD),
            results,
        })
    }

    fn write_one(&mut self, wv: &crate::services::WriteValue, user: &ClientUserId) -> u32 {
        if let Some(hook) = self.write_hook.as_mut() {
            if let Some(status) = hook(wv, user) {
                return status;
            }
        }
        let range = match NumericRange::parse(&wv.index_range) {
            Ok(r) => r,
            Err(code) => return code,
        };
        let Some(range) = range else {
            return self.address_space.write_attribute(
                &wv.node_id,
                wv.attribute_id,
                wv.value.clone(),
            );
        };
        // §5.11.4.2: an indexRange writes a slice of the current value, so the
        // current value has to be read, patched and written back whole.
        let current = match self
            .address_space
            .read_attribute(&wv.node_id, wv.attribute_id)
        {
            Ok(dv) => dv,
            Err(code) => return code,
        };
        let Some(mut target) = current.value else {
            return status::BAD_INDEX_RANGE_NO_DATA;
        };
        let Some(source) = wv.value.value.clone() else {
            return status::BAD_TYPE_MISMATCH;
        };
        if let Err(code) = range.replace(&mut target, &source) {
            return code;
        }
        let mut patched = wv.value.clone();
        patched.value = Some(target);
        self.address_space
            .write_attribute(&wv.node_id, wv.attribute_id, patched)
    }

    // -----------------------------------------------------------------------
    // HistoryRead Service (Part 4 §5.10.3; Historical Access is Part 11).
    // -----------------------------------------------------------------------

    /// Part 4 §5.10.3 HistoryRead. Spec §9.3.4.4: the DDS-side retained
    /// samples of a historizing node ([`AddressSpace::set_historizing`])
    /// answer either a Raw time-range read (`ReadRawModifiedDetails` with
    /// `is_read_modified == false`) or a discrete ReadAtTime
    /// (`ReadAtTimeDetails`) — DDS keeps neither Modified-value history nor
    /// Events, so both are refused per-request, not silently reinterpreted.
    #[allow(clippy::too_many_lines)]
    fn history_read(
        &mut self,
        handle: u32,
        r: &crate::services::HistoryReadRequest,
    ) -> ServiceResponse {
        if r.nodes_to_read.is_empty() {
            return ServiceResponse::ServiceFault(ServiceFault::new(
                handle,
                status::BAD_NOTHING_TO_DO,
            ));
        }
        let timestamps = match TimestampsToReturn::from_i32(r.timestamps_to_return) {
            Ok(t) => t,
            Err(code) => return ServiceResponse::ServiceFault(ServiceFault::new(handle, code)),
        };
        let base_query = match &r.history_read_details {
            HistoryReadDetails::RawModified(d) => {
                if d.is_read_modified {
                    return ServiceResponse::ServiceFault(ServiceFault::new(
                        handle,
                        status::BAD_HISTORY_OPERATION_UNSUPPORTED,
                    ));
                }
                ReadHistoryQuery::Raw {
                    start_time: d.start_time,
                    end_time: d.end_time,
                    num_values_per_node: d.num_values_per_node,
                    return_bounds: d.return_bounds,
                }
            }
            HistoryReadDetails::AtTime(d) => {
                if d.req_times.is_empty() {
                    return ServiceResponse::ServiceFault(ServiceFault::new(
                        handle,
                        status::BAD_HISTORY_OPERATION_INVALID,
                    ));
                }
                ReadHistoryQuery::AtTime {
                    req_times: d.req_times.clone(),
                    use_simple_bounds: d.use_simple_bounds,
                }
            }
        };

        let results = r
            .nodes_to_read
            .iter()
            .map(|nr| {
                // Part 4 §5.10.2.2: releasing a continuation point stops the
                // series without returning more data — it is not a normal
                // empty-result read, so it never touches the AddressSpace.
                if r.release_continuation_points {
                    return HistoryReadResult {
                        status_code: GOOD,
                        continuation_point: Vec::new(),
                        history_data: HistoryData::default(),
                    };
                }
                let query = if nr.continuation_point.is_empty() {
                    base_query.clone()
                } else {
                    match (
                        &base_query,
                        decode_continuation_point(&nr.continuation_point),
                    ) {
                        (
                            ReadHistoryQuery::Raw {
                                end_time,
                                num_values_per_node,
                                return_bounds,
                                ..
                            },
                            Some(after),
                        ) => ReadHistoryQuery::Raw {
                            start_time: after.saturating_add(1),
                            end_time: *end_time,
                            num_values_per_node: *num_values_per_node,
                            return_bounds: *return_bounds,
                        },
                        // A ReadAtTime request carries no meaningful resume
                        // point (a discrete point list is answered whole or
                        // not at all), and a malformed continuation point
                        // decodes to `None` either way — both are refused,
                        // never silently answered as a fresh read.
                        _ => {
                            return HistoryReadResult {
                                status_code: status::BAD_CONTINUATION_POINT_INVALID,
                                continuation_point: Vec::new(),
                                history_data: HistoryData::default(),
                            };
                        }
                    }
                };
                match self.address_space.read_history(&nr.node_id, &query) {
                    Ok(result) => {
                        let continuation_point = if result.more_available {
                            result
                                .samples
                                .last()
                                .map(|s| encode_continuation_point(s.source_timestamp))
                                .unwrap_or_default()
                        } else {
                            Vec::new()
                        };
                        let data_values: Vec<DataValue> = result
                            .samples
                            .into_iter()
                            .map(|s| {
                                let mut dv = s.value;
                                dv.source_timestamp = Some(s.source_timestamp);
                                timestamps.apply(&mut dv);
                                dv
                            })
                            .collect();
                        let status_code = if data_values.is_empty() {
                            status::BAD_NO_DATA
                        } else {
                            GOOD
                        };
                        HistoryReadResult {
                            status_code,
                            continuation_point,
                            history_data: HistoryData { data_values },
                        }
                    }
                    Err(HistoryReadError::UnknownInstance) => HistoryReadResult {
                        status_code: BAD_NODE_ID_UNKNOWN,
                        continuation_point: Vec::new(),
                        history_data: HistoryData::default(),
                    },
                    Err(HistoryReadError::InvalidTimeRange) => HistoryReadResult {
                        status_code: status::BAD_HISTORY_OPERATION_INVALID,
                        continuation_point: Vec::new(),
                        history_data: HistoryData::default(),
                    },
                }
            })
            .collect();

        ServiceResponse::HistoryRead(HistoryReadResponse {
            response_header: ResponseHeader::new(handle, GOOD),
            results,
        })
    }

    // -----------------------------------------------------------------------
    // View Service Set (Part 4 §5.9).
    // -----------------------------------------------------------------------

    /// Browses one [`BrowseDescription`] against the AddressSpace, honouring the
    /// reference-type filter, node-class mask, result mask and
    /// `requestedMaxReferencesPerNode` (Part 4 §5.9.2.2).
    fn browse_one(&mut self, bd: &BrowseDescription, max_references: usize) -> BrowseResult {
        // §5.9.2.4 Bad_BrowseDirectionInvalid — the BrowseDirection enum
        // (§7.5) has exactly Forward/Inverse/Both/Invalid.
        if !(0..=2).contains(&bd.browse_direction) {
            return browse_fault(status::BAD_BROWSE_DIRECTION_INVALID);
        }
        // An unknown source node is reported per Part 4 §5.9.2.
        if self.address_space.node_meta(&bd.node_id).is_none() {
            return browse_fault(BAD_NODE_ID_UNKNOWN);
        }
        let null_id = NodeId::numeric(0, 0);
        let filter = if bd.reference_type_id == null_id {
            None
        } else {
            // §5.9.2.4 Bad_ReferenceTypeIdInvalid: a referenceTypeId that is
            // not a ReferenceType node in this AddressSpace, unless it is one
            // of the well-known ns0 types the filter understands.
            if !is_known_reference_type(&self.address_space, &bd.reference_type_id) {
                return browse_fault(0x804C_0000);
            }
            Some(&bd.reference_type_id)
        };
        let matches = self.address_space.browse(
            &bd.node_id,
            bd.browse_direction,
            filter,
            bd.include_subtypes,
            bd.node_class_mask,
        );
        let mut references: Vec<ReferenceDescription> = matches
            .into_iter()
            .map(|m| project_reference(&m, bd.result_mask))
            .collect();

        // §5.9.2.2 requestedMaxReferencesPerNode: "The value 0 indicates that
        // the Client is imposing no limitation."
        if max_references == 0 || references.len() <= max_references {
            return BrowseResult {
                status_code: GOOD,
                continuation_point: Vec::new(),
                references,
            };
        }
        let rest = references.split_off(max_references);
        match self.allocate_continuation_point(rest) {
            Some(id) => BrowseResult {
                status_code: GOOD,
                continuation_point: id,
                references,
            },
            // §5.9.2.4 Bad_NoContinuationPoints — all continuation points are
            // allocated, so the truncated result cannot be continued.
            None => browse_fault(status::BAD_NO_CONTINUATION_POINTS),
        }
    }

    /// Continues (or, with `release`, frees) one Browse continuation point
    /// (Part 4 §5.9.3).
    fn browse_next_one(&mut self, cp: &[u8], release: bool) -> BrowseResult {
        let Some(pos) = self.continuation_points.iter().position(|c| c.id == cp) else {
            return browse_fault(status::BAD_CONTINUATION_POINT_INVALID);
        };
        if release {
            // §5.9.3.2: "TRUE — passed continuationPoints shall be reset to
            // free resources in the Server. The continuation points are
            // released and the results and diagnosticInfos arrays are empty."
            self.continuation_points.remove(pos);
            return BrowseResult {
                status_code: GOOD,
                continuation_point: Vec::new(),
                references: Vec::new(),
            };
        }
        let point = self.continuation_points.remove(pos);
        BrowseResult {
            status_code: GOOD,
            continuation_point: Vec::new(),
            references: point.remaining,
        }
    }

    fn allocate_continuation_point(
        &mut self,
        remaining: Vec<ReferenceDescription>,
    ) -> Option<Vec<u8>> {
        if self.continuation_points.len() >= MAX_CONTINUATION_POINTS {
            return None;
        }
        let id = self.next_continuation_point;
        self.next_continuation_point += 1;
        let bytes = id.to_be_bytes().to_vec();
        self.continuation_points.push(ContinuationPoint {
            id: bytes.clone(),
            remaining,
        });
        Some(bytes)
    }

    fn endpoint(&self) -> EndpointDescription {
        use zerodds_opcua_pubsub::uadp::datatypes::ApplicationDescription;
        let (security_mode, security_policy_uri) = self.endpoint_security();
        EndpointDescription {
            endpoint_url: self.endpoint_url.clone(),
            server: ApplicationDescription {
                application_uri: String::from("urn:zerodds:opcua-server"),
                product_uri: String::from("urn:zerodds"),
                application_name: LocalizedText {
                    locale: None,
                    text: Some(String::from("ZeroDDS OPC-UA Server")),
                },
                application_type: ApplicationType::Server,
                gateway_server_uri: String::new(),
                discovery_profile_uri: String::new(),
                discovery_urls: alloc::vec![self.endpoint_url.clone()],
            },
            server_certificate: self.server_certificate(),
            security_mode,
            security_policy_uri,
            user_identity_tokens: self.users.policies().to_vec(),
            transport_profile_uri: String::from(
                "http://opcfoundation.org/UA-Profile/Transport/uatcp-uasc-uabinary",
            ),
            security_level: 0,
        }
    }

    fn endpoint_security(&self) -> (MessageSecurityMode, String) {
        #[cfg(feature = "crypto")]
        if let Some(sec) = self.security.as_ref() {
            let mode = match sec.mode {
                SecuredMode::Sign => MessageSecurityMode::Sign,
                SecuredMode::SignAndEncrypt => MessageSecurityMode::SignAndEncrypt,
            };
            return (mode, String::from(sec.policy.uri()));
        }
        (
            MessageSecurityMode::None,
            String::from(SECURITY_POLICY_NONE),
        )
    }
}

/// The `SecureChannelId` of a MessageChunk (Part 6 §6.7.2.2: the `UInt32`
/// right after the 8-byte Message header).
fn chunk_channel_id(bytes: &[u8]) -> Result<u32, ServerError> {
    if bytes.len() < 12 {
        return Err(ServerError::Protocol(
            "MessageChunk shorter than its SecureChannelId",
        ));
    }
    Ok(u32::from_le_bytes([
        bytes[8], bytes[9], bytes[10], bytes[11],
    ]))
}

/// Encodes a HistoryRead continuation point: the SourceTimestamp (i64
/// ticks) of the last sample already delivered for a Raw range read, so a
/// follow-up request resumes strictly after it. Part 4 §5.10.2.2 leaves the
/// continuation point's content to the Server — it is opaque to the client,
/// echoed back verbatim on the next call.
fn encode_continuation_point(after_timestamp: i64) -> Vec<u8> {
    after_timestamp.to_be_bytes().to_vec()
}

/// Decodes a continuation point produced by [`encode_continuation_point`].
/// Anything not exactly 8 bytes is not one this server issued — `None`,
/// never a best-effort guess.
fn decode_continuation_point(bytes: &[u8]) -> Option<i64> {
    let arr: [u8; 8] = bytes.try_into().ok()?;
    Some(i64::from_be_bytes(arr))
}

/// A `BrowseResult` that carries only an operation-level StatusCode.
fn browse_fault(status_code: u32) -> BrowseResult {
    BrowseResult {
        status_code,
        continuation_point: Vec::new(),
        references: Vec::new(),
    }
}

/// Whether `id` names a ReferenceType this AddressSpace can filter on: a
/// ReferenceType node it holds, or one of the well-known ns0 types the
/// subtype filter knows (Part 3 §7).
fn is_known_reference_type(space: &AddressSpace, id: &NodeId) -> bool {
    use crate::address_space::{NodeClass, reference_types as rt};
    if [
        rt::REFERENCES,
        rt::HIERARCHICAL_REFERENCES,
        rt::HAS_CHILD,
        rt::ORGANIZES,
        rt::HAS_SUBTYPE,
        rt::HAS_PROPERTY,
        rt::HAS_COMPONENT,
        rt::HAS_TYPE_DEFINITION,
    ]
    .iter()
    .any(|k| k == id)
    {
        return true;
    }
    space
        .node_meta(id)
        .is_some_and(|m| m.node_class == NodeClass::ReferenceType)
}

/// Projects a browse hit onto the fields `resultMask` asks for
/// (Part 4 §5.9.2.2).
fn project_reference(m: &crate::address_space::BrowseMatch, mask: u32) -> ReferenceDescription {
    let null_id = NodeId::numeric(0, 0);
    let expanded = |id: NodeId| ExpandedNodeId {
        node_id: id,
        namespace_uri: String::new(),
        server_index: 0,
    };
    ReferenceDescription {
        reference_type_id: if mask & 0x01 != 0 {
            m.reference_type.clone()
        } else {
            null_id.clone()
        },
        is_forward: mask & 0x02 != 0 && m.is_forward,
        node_id: expanded(m.target.node_id.clone()),
        browse_name: if mask & 0x08 != 0 {
            m.target.browse_name.clone()
        } else {
            QualifiedName {
                namespace_index: 0,
                name: String::new(),
            }
        },
        display_name: if mask & 0x10 != 0 {
            m.target.display_name.clone()
        } else {
            LocalizedText {
                locale: None,
                text: None,
            }
        },
        node_class: if mask & 0x04 != 0 {
            m.target.node_class.as_i32()
        } else {
            0
        },
        type_definition: expanded(if mask & 0x20 != 0 {
            m.target.type_definition.clone()
        } else {
            null_id
        }),
    }
}

/// Reads one `ReadValueId` (Part 4 §5.11.2), applying its `indexRange`.
fn read_one(space: &AddressSpace, rv: &crate::services::ReadValueId) -> DataValue {
    // §5.11.2.4 Bad_DataEncodingInvalid: this stack serves the default binary
    // encoding only, so a named dataEncoding cannot be honoured.
    if !rv.data_encoding.name.is_empty() {
        return data_value_status(status::BAD_DATA_ENCODING_INVALID);
    }
    let range = match NumericRange::parse(&rv.index_range) {
        Ok(r) => r,
        Err(code) => return data_value_status(code),
    };
    let mut dv = match space.read_attribute(&rv.node_id, rv.attribute_id) {
        Ok(dv) => dv,
        Err(code) => return data_value_status(code),
    };
    let Some(range) = range else { return dv };
    let Some(value) = dv.value.take() else {
        return data_value_status(status::BAD_INDEX_RANGE_NO_DATA);
    };
    match range.apply(&value) {
        Ok(selected) => {
            dv.value = Some(selected);
            dv
        }
        Err(code) => data_value_status(code),
    }
}

fn data_value_status(status_code: u32) -> DataValue {
    DataValue {
        value: None,
        status: Some(status_code),
        source_timestamp: None,
        server_timestamp: None,
        source_pico_sec: None,
        server_pico_sec: None,
    }
}

/// Part 4 §5.14.2.2: "If the requested value is 0 or negative, the Server shall
/// revise with the fastest supported publishing interval."
fn revise_publishing_interval(requested: f64) -> f64 {
    if requested > 0.0 { requested } else { 100.0 }
}

/// Part 4 §5.14.3.2: "If the requested value is 0, the Server shall revise with
/// the smallest supported keep-alive count."
fn revise_keep_alive(requested: u32) -> u32 {
    if requested == 0 { 1 } else { requested }
}

/// Part 4 §5.14.3.2: "The lifetime count shall be a minimum of three times the
/// keep-alive count."
fn revise_lifetime(requested: u32, keep_alive: u32) -> u32 {
    requested.max(keep_alive.saturating_mul(3))
}

impl Server {
    // -----------------------------------------------------------------------
    // MonitoredItem Service Set (Part 4 §5.13).
    // -----------------------------------------------------------------------

    fn create_monitored_items(
        &mut self,
        handle: u32,
        session_id: u32,
        r: &crate::services::CreateMonitoredItemsRequest,
    ) -> ServiceResponse {
        let timestamps = match TimestampsToReturn::from_i32(r.timestamps_to_return) {
            Ok(t) => t,
            Err(code) => return ServiceResponse::ServiceFault(ServiceFault::new(handle, code)),
        };
        if r.items_to_create.is_empty() {
            return ServiceResponse::ServiceFault(ServiceFault::new(
                handle,
                status::BAD_NOTHING_TO_DO,
            ));
        }
        let known_nodes: Vec<(NodeId, u32, bool)> = r
            .items_to_create
            .iter()
            .map(|it| {
                let rv = &it.item_to_monitor;
                (
                    rv.node_id.clone(),
                    rv.attribute_id,
                    self.address_space
                        .read_attribute(&rv.node_id, rv.attribute_id)
                        .is_ok(),
                )
            })
            .collect();
        let Some(sub) = self
            .subscriptions
            .iter_mut()
            .find(|s| s.id == r.subscription_id && s.session_id == session_id)
        else {
            return ServiceResponse::ServiceFault(ServiceFault::new(
                handle,
                BAD_SUBSCRIPTION_ID_INVALID,
            ));
        };
        let mut results = Vec::with_capacity(r.items_to_create.len());
        for (it, (node_id, attribute_id, resolvable)) in
            r.items_to_create.iter().zip(known_nodes.into_iter())
        {
            let mode = match MonitoringMode::from_i32(it.monitoring_mode) {
                Ok(m) => m,
                Err(code) => {
                    results.push(monitored_item_fault(code));
                    continue;
                }
            };
            let range = match NumericRange::parse(&it.item_to_monitor.index_range) {
                Ok(r) => r,
                Err(code) => {
                    results.push(monitored_item_fault(code));
                    continue;
                }
            };
            if !resolvable {
                // §5.13.2.4 Bad_NodeIdUnknown / Bad_AttributeIdInvalid — a
                // monitored item that could never sample is rejected here
                // instead of silently never reporting.
                results.push(monitored_item_fault(BAD_NODE_ID_UNKNOWN));
                continue;
            }
            let p = &it.requested_parameters;
            // §7.22: the only MonitoringFilter this stack applies is "none";
            // any other filter is refused rather than ignored.
            if p.filter.type_id != NodeId::numeric(0, 0) {
                results.push(monitored_item_fault(
                    status::BAD_MONITORED_ITEM_FILTER_UNSUPPORTED,
                ));
                continue;
            }
            let mid = sub.next_item_id;
            sub.next_item_id += 1;
            let sampling = revise_sampling_interval(p.sampling_interval, sub.publishing_interval);
            let queue_size = p.queue_size.max(1);
            sub.items.push(MonitoredItem {
                id: mid,
                client_handle: p.client_handle,
                node_id,
                attribute_id,
                index_range: range,
                monitoring_mode: mode,
                sampling_interval: sampling,
                queue_size,
                discard_oldest: p.discard_oldest,
                timestamps_to_return: timestamps,
                last_value: None,
                queue: Vec::new(),
                triggered_items: Vec::new(),
            });
            results.push(MonitoredItemCreateResult {
                status_code: GOOD,
                monitored_item_id: mid,
                revised_sampling_interval: sampling,
                revised_queue_size: queue_size,
                filter_result: null_filter(),
            });
        }
        ServiceResponse::CreateMonitoredItems(CreateMonitoredItemsResponse {
            response_header: ResponseHeader::new(handle, GOOD),
            results,
        })
    }

    fn modify_monitored_items(
        &mut self,
        handle: u32,
        session_id: u32,
        r: &crate::services::ModifyMonitoredItemsRequest,
    ) -> ServiceResponse {
        let timestamps = match TimestampsToReturn::from_i32(r.timestamps_to_return) {
            Ok(t) => t,
            Err(code) => return ServiceResponse::ServiceFault(ServiceFault::new(handle, code)),
        };
        if r.items_to_modify.is_empty() {
            return ServiceResponse::ServiceFault(ServiceFault::new(
                handle,
                status::BAD_NOTHING_TO_DO,
            ));
        }
        let Some(sub) = self
            .subscriptions
            .iter_mut()
            .find(|s| s.id == r.subscription_id && s.session_id == session_id)
        else {
            return ServiceResponse::ServiceFault(ServiceFault::new(
                handle,
                BAD_SUBSCRIPTION_ID_INVALID,
            ));
        };
        let publishing_interval = sub.publishing_interval;
        let results = r
            .items_to_modify
            .iter()
            .map(|m| {
                let p = &m.requested_parameters;
                if p.filter.type_id != NodeId::numeric(0, 0) {
                    return modify_fault(status::BAD_MONITORED_ITEM_FILTER_UNSUPPORTED);
                }
                let Some(item) = sub.items.iter_mut().find(|i| i.id == m.monitored_item_id) else {
                    return modify_fault(status::BAD_MONITORED_ITEM_ID_INVALID);
                };
                let sampling = revise_sampling_interval(p.sampling_interval, publishing_interval);
                let queue_size = p.queue_size.max(1);
                item.client_handle = p.client_handle;
                item.sampling_interval = sampling;
                item.discard_oldest = p.discard_oldest;
                item.timestamps_to_return = timestamps;
                item.queue_size = queue_size;
                // §5.13.3.2: "If the number of notifications in the queue
                // exceeds the new queue size, the notifications exceeding the
                // size shall be discarded following the configured discard
                // policy."
                trim_queue(&mut item.queue, queue_size as usize, item.discard_oldest);
                MonitoredItemModifyResult {
                    status_code: GOOD,
                    revised_sampling_interval: sampling,
                    revised_queue_size: queue_size,
                    filter_result: null_filter(),
                }
            })
            .collect();
        ServiceResponse::ModifyMonitoredItems(ModifyMonitoredItemsResponse {
            response_header: ResponseHeader::new(handle, GOOD),
            results,
        })
    }

    // -----------------------------------------------------------------------
    // Subscription Service Set (Part 4 §5.14).
    // -----------------------------------------------------------------------

    fn transfer_one(
        &mut self,
        subscription_id: u32,
        session_id: u32,
        send_initial_values: bool,
    ) -> TransferResult {
        let Some(sub) = self
            .subscriptions
            .iter_mut()
            .find(|s| s.id == subscription_id)
        else {
            return TransferResult {
                status_code: BAD_SUBSCRIPTION_ID_INVALID,
                available_sequence_numbers: Vec::new(),
            };
        };
        if sub.session_id == session_id {
            // §5.14.7.4: "This result code is returned if the Subscription is
            // already owned by the Session used to call TransferSubscription."
            return TransferResult {
                status_code: status::BAD_NOTHING_TO_DO,
                available_sequence_numbers: Vec::new(),
            };
        }
        let previous = sub.session_id;
        sub.session_id = session_id;
        sub.send_initial_values = send_initial_values;
        sub.transferred_away_from = Some(previous);
        TransferResult {
            status_code: GOOD,
            available_sequence_numbers: sub
                .retransmission_queue
                .iter()
                .map(|m| m.sequence_number)
                .collect(),
        }
    }

    /// Handles a Publish (Part 4 §5.14.5): processes the acknowledgements,
    /// samples the subscription's monitored items against the AddressSpace and
    /// returns either a `DataChangeNotification` or a keep-alive Message.
    fn publish(
        &mut self,
        handle: u32,
        session_id: u32,
        r: &crate::services::PublishRequest,
    ) -> ServiceResponse {
        // 1. Acknowledgements, before the availableSequenceNumbers list is
        //    prepared (§5.14.5.2).
        let ack_results: Vec<u32> = r
            .subscription_acknowledgements
            .iter()
            .map(|ack| {
                match self
                    .subscriptions
                    .iter_mut()
                    .find(|s| s.id == ack.subscription_id && s.session_id == session_id)
                {
                    None => BAD_SUBSCRIPTION_ID_INVALID,
                    Some(sub) => {
                        let before = sub.retransmission_queue.len();
                        sub.retransmission_queue
                            .retain(|m| m.sequence_number != ack.sequence_number);
                        if sub.retransmission_queue.len() == before {
                            status::BAD_SEQUENCE_NUMBER_UNKNOWN
                        } else {
                            GOOD
                        }
                    }
                }
            })
            .collect();

        // 2. §5.14.7.1: "If the Server transfers the Subscription to the new
        //    Session, the Server shall issue a StatusChangeNotification
        //    notificationMessage with the status code
        //    Good_SubscriptionTransferred to the old Session." That message is
        //    the losing session's next Publish response, before any check for
        //    Subscriptions it still owns.
        if let Some(index) = self
            .subscriptions
            .iter()
            .position(|s| s.transferred_away_from == Some(session_id))
        {
            self.subscriptions[index].transferred_away_from = None;
            let sub = &mut self.subscriptions[index];
            sub.seq += 1;
            let message = NotificationMessage {
                sequence_number: sub.seq,
                publish_time: 0,
                notification_data: (StatusChangeNotification {
                    status: status::GOOD_SUBSCRIPTION_TRANSFERRED,
                })
                .to_extension_object()
                .map(|eo| alloc::vec![eo])
                .unwrap_or_default(),
            };
            let sub_id = sub.id;
            return ServiceResponse::Publish(PublishResponse {
                response_header: ResponseHeader::new(handle, GOOD),
                subscription_id: sub_id,
                available_sequence_numbers: Vec::new(),
                more_notifications: false,
                notification_message: message,
                results: ack_results,
            });
        }

        // 3. Pick the subscription to serve: the highest-priority one owned by
        //    this session (§5.14.3.2 priority).
        let Some(index) = self.pick_subscription(session_id) else {
            // §5.14.5.3 Bad_NoSubscription — "There is no Subscription
            // available for this session."
            return ServiceResponse::ServiceFault(ServiceFault::new(
                handle,
                status::BAD_NO_SUBSCRIPTION,
            ));
        };

        // 4. Sample every enabled monitored item against the AddressSpace.
        self.sample_subscription(index);

        // 5. Collect what the Reporting items (and the items they trigger)
        //    have queued.
        let sub = &mut self.subscriptions[index];
        let publishing_enabled = sub.publishing_enabled;
        let reporting: Vec<u32> = sub
            .items
            .iter()
            .filter(|i| i.monitoring_mode == MonitoringMode::Reporting && !i.queue.is_empty())
            .map(|i| i.id)
            .collect();
        // §5.13.1.6: a triggering item that reports also releases the queues of
        // the items linked to it, even in Sampling mode.
        let mut releasing = reporting.clone();
        for id in &reporting {
            if let Some(item) = sub.items.iter().find(|i| i.id == *id) {
                for linked in &item.triggered_items {
                    if !releasing.contains(linked) {
                        releasing.push(*linked);
                    }
                }
            }
        }

        let max = if sub.max_notifications_per_publish == 0 {
            usize::MAX
        } else {
            sub.max_notifications_per_publish as usize
        };
        let mut notes: Vec<MonitoredItemNotification> = Vec::new();
        let mut more = false;
        if publishing_enabled {
            for item in &mut sub.items {
                if !releasing.contains(&item.id) || item.monitoring_mode == MonitoringMode::Disabled
                {
                    continue;
                }
                while let Some(value) = item.queue.first().cloned() {
                    if notes.len() >= max {
                        more = true;
                        break;
                    }
                    item.queue.remove(0);
                    notes.push(MonitoredItemNotification {
                        client_handle: item.client_handle,
                        value,
                    });
                }
                if more {
                    break;
                }
            }
        }

        sub.seq += 1;
        let seq = sub.seq;
        let sub_id = sub.id;
        let notification_data = if notes.is_empty() {
            // §5.14.1 (f): a keep-alive Message carries no Notifications and
            // the sequence number of the next NotificationMessage.
            Vec::new()
        } else {
            (DataChangeNotification {
                monitored_items: notes,
            })
            .to_extension_object()
            .map(|eo| alloc::vec![eo])
            .unwrap_or_default()
        };
        let message = NotificationMessage {
            sequence_number: seq,
            publish_time: 0,
            notification_data,
        };
        if !message.notification_data.is_empty() {
            // §5.14.1 (i): only real NotificationMessages are retained for
            // retransmission; keep-alives are not acknowledged.
            sub.retransmission_queue.push(message.clone());
            if sub.retransmission_queue.len() > RETRANSMISSION_QUEUE_DEPTH {
                sub.retransmission_queue.remove(0);
            }
        }
        let available_sequence_numbers = sub
            .retransmission_queue
            .iter()
            .map(|m| m.sequence_number)
            .collect();
        ServiceResponse::Publish(PublishResponse {
            response_header: ResponseHeader::new(handle, GOOD),
            subscription_id: sub_id,
            available_sequence_numbers,
            more_notifications: more,
            notification_message: message,
            results: ack_results,
        })
    }

    /// The subscription a Publish serves: the highest-priority one of this
    /// session (Part 4 §5.14.3.2 priority; ties keep creation order).
    fn pick_subscription(&self, session_id: u32) -> Option<usize> {
        let mut best: Option<usize> = None;
        for (i, s) in self.subscriptions.iter().enumerate() {
            if s.session_id != session_id {
                continue;
            }
            match best {
                None => best = Some(i),
                Some(b) if s.priority > self.subscriptions[b].priority => best = Some(i),
                _ => {}
            }
        }
        best
    }

    /// Samples every non-Disabled monitored item of `index` against the
    /// AddressSpace and queues the values that changed (Part 4 §5.13.1.5).
    fn sample_subscription(&mut self, index: usize) {
        let space = &self.address_space;
        let sub = &mut self.subscriptions[index];
        let send_initial_values = core::mem::take(&mut sub.send_initial_values);
        for item in &mut sub.items {
            if item.monitoring_mode == MonitoringMode::Disabled {
                continue;
            }
            let mut sampled = sample_item(space, item);
            item.timestamps_to_return.apply(&mut sampled);
            let changed = item.last_value.as_ref() != Some(&sampled);
            if changed || send_initial_values {
                item.last_value = Some(sampled.clone());
                item.queue.push(sampled);
                trim_queue(
                    &mut item.queue,
                    item.queue_size as usize,
                    item.discard_oldest,
                );
            }
        }
    }
}

/// Reads a monitored item's attribute, applying its `indexRange`.
fn sample_item(space: &AddressSpace, item: &MonitoredItem) -> DataValue {
    let mut dv = match space.read_attribute(&item.node_id, item.attribute_id) {
        Ok(dv) => dv,
        Err(code) => return data_value_status(code),
    };
    let Some(range) = item.index_range.as_ref() else {
        return dv;
    };
    let Some(value) = dv.value.take() else {
        return data_value_status(status::BAD_INDEX_RANGE_NO_DATA);
    };
    match range.apply(&value) {
        Ok(selected) => {
            dv.value = Some(selected);
            dv
        }
        Err(code) => data_value_status(code),
    }
}

/// Part 4 §5.13.1.5: a full queue drops its oldest or its newest entry,
/// depending on `discardOldest`.
fn trim_queue(queue: &mut Vec<DataValue>, queue_size: usize, discard_oldest: bool) {
    let size = queue_size.max(1);
    while queue.len() > size {
        if discard_oldest {
            queue.remove(0);
        } else {
            queue.pop();
        }
    }
}

/// Part 4 §5.13.2.2: "The negotiated value for [the publishing interval] is
/// used as the default sampling interval for MonitoredItems assigned to this
/// Subscription"; a negative request means "use the publishing interval".
fn revise_sampling_interval(requested: f64, publishing_interval: f64) -> f64 {
    if requested < 0.0 {
        publishing_interval
    } else if requested == 0.0 {
        // §5.13.1.2: 0 means the Server samples as fast as it can, which for a
        // sample-on-publish Server is once per publishing cycle.
        publishing_interval
    } else {
        requested
    }
}

fn monitored_item_fault(status_code: u32) -> MonitoredItemCreateResult {
    MonitoredItemCreateResult {
        status_code,
        monitored_item_id: 0,
        revised_sampling_interval: 0.0,
        revised_queue_size: 0,
        filter_result: null_filter(),
    }
}

fn modify_fault(status_code: u32) -> MonitoredItemModifyResult {
    MonitoredItemModifyResult {
        status_code,
        revised_sampling_interval: 0.0,
        revised_queue_size: 0,
        filter_result: null_filter(),
    }
}
