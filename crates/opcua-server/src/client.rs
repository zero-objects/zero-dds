// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The OPC-UA [`Client`]: drives the Hello → OpenSecureChannel → CreateSession
//! → ActivateSession handshake and calls the Read / Call services over a
//! [`Transport`] (SecurityMode `None`).
//!
//! [`LoopbackTransport`] talks to an in-process [`crate::server::Server`];
//! [`TcpTransport`] (feature `std`) talks to a real `opc.tcp` peer.

use alloc::string::String;
use alloc::vec::Vec;

use zerodds_opcua_gateway::data_value::{DataValue, Variant};
use zerodds_opcua_gateway::node_id::NodeId;
use zerodds_opcua_pubsub::uadp::datatypes::{
    ApplicationDescription, ApplicationType, EndpointDescription, MessageSecurityMode,
};
use zerodds_opcua_pubsub::{DecodeError, EncodeError, UaDecode, UaReader};
use zerodds_opcua_uacp::connection::{
    AcknowledgeMessage, HelloMessage, MessageHeader, MessageType,
};
use zerodds_opcua_uacp::securechannel::{
    ChunkOutcome, OpenSecureChannelRequest, OpenSecureChannelResponse, RequestHeader,
    SecureChannel, SecurityTokenRequestType, null_extension_object, parse_chunk, split_messages,
};

use crate::attributes::{MonitoringMode, TimestampsToReturn};
use crate::identity::UserIdentityToken;
use crate::services::{
    ATTRIBUTE_VALUE, ActivateSessionRequest, BrowseDescription, BrowseNextRequest, BrowseRequest,
    CallMethodRequest, CallMethodResult, CallRequest, CloseSessionRequest,
    CreateMonitoredItemsRequest, CreateSessionRequest, CreateSubscriptionRequest,
    DataChangeNotification, DeleteMonitoredItemsRequest, DeleteSubscriptionsRequest,
    FindServersRequest, GetEndpointsRequest, HistoryReadDetails, HistoryReadRequest,
    HistoryReadResult, HistoryReadValueId, ModifyMonitoredItemsRequest, ModifySubscriptionRequest,
    MonitoredItemCreateRequest, MonitoredItemCreateResult, MonitoredItemModifyRequest,
    MonitoredItemModifyResult, MonitoredItemNotification, MonitoringParameters, PublishRequest,
    ReadRequest, ReadValueId, ReferenceDescription, RepublishRequest, ServiceRequest,
    ServiceResponse, SetMonitoringModeRequest, SetTriggeringRequest, SignatureData,
    StatusChangeNotification, SubscriptionAcknowledgement, TransferResult,
    TransferSubscriptionsRequest, ViewDescription, WriteRequest, WriteValue, null_filter,
};
use zerodds_opcua_uacp::securechannel::node_ids;

#[cfg(feature = "crypto")]
use alloc::boxed::Box;
#[cfg(feature = "crypto")]
use zerodds_opcua_uacp::crypto::{
    AsymmetricContext, CryptoRngCore, RsaPrivateKey, RsaPublicKey, SecuredMode, SecurityPolicy,
    build_asymmetric_chunk, derive_keys, open_asymmetric_chunk,
};
#[cfg(feature = "crypto")]
use zerodds_opcua_uacp::securechannel::SecuritySession;

/// The client's secured-SecurityPolicy configuration (feature `crypto`): the
/// client's own RSA key + certificate, the trusted server certificate +
/// public key, and a caller-supplied CSPRNG. With this set, [`Client::connect`]
/// runs the secured OpenSecureChannel handshake.
#[cfg(feature = "crypto")]
pub struct ClientSecurity {
    /// Negotiated SecurityPolicy.
    pub policy: SecurityPolicy,
    /// Sign or SignAndEncrypt.
    pub mode: SecuredMode,
    /// The client's RSA private key.
    pub private_key: RsaPrivateKey,
    /// The client's DER application certificate.
    pub certificate: Vec<u8>,
    /// The trusted server's DER certificate.
    pub server_certificate: Vec<u8>,
    /// The trusted server's RSA public key (verifies the server's OPN).
    pub server_public_key: RsaPublicKey,
    /// A cryptographically secure RNG (OS RNG under `std`).
    pub rng: Box<dyn CryptoRngCore + Send>,
}

/// An error from the client / a transport.
#[derive(Debug, Clone, PartialEq)]
pub enum ClientError {
    /// A message could not be decoded.
    Decode(DecodeError),
    /// A request could not be encoded.
    Encode(EncodeError),
    /// An unexpected message / wrong response type.
    Protocol(&'static str),
    /// A transport I/O failure.
    Io(String),
    /// The server answered with a ServiceFault (Part 4 §7.34); the payload is
    /// its `serviceResult` StatusCode.
    ServiceFault(u32),
}

impl From<DecodeError> for ClientError {
    fn from(e: DecodeError) -> Self {
        Self::Decode(e)
    }
}

impl From<EncodeError> for ClientError {
    fn from(e: EncodeError) -> Self {
        Self::Encode(e)
    }
}

impl core::fmt::Display for ClientError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Decode(e) => write!(f, "decode error: {e}"),
            Self::Encode(e) => write!(f, "encode error: {e}"),
            Self::Protocol(m) => write!(f, "protocol error: {m}"),
            Self::Io(m) => write!(f, "transport I/O error: {m}"),
            Self::ServiceFault(s) => write!(f, "server ServiceFault, StatusCode 0x{s:08x}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ClientError {}

/// A synchronous request/response byte transport for OPC-UA TCP.
pub trait Transport {
    /// Sends one UACP message and returns the single response message.
    ///
    /// # Errors
    /// [`ClientError`] on an I/O or protocol failure.
    fn request(&mut self, message: &[u8]) -> Result<Vec<u8>, ClientError>;

    /// Sends one UACP message without awaiting a response (e.g.
    /// CloseSecureChannel).
    ///
    /// # Errors
    /// [`ClientError`] on an I/O failure.
    fn send(&mut self, message: &[u8]) -> Result<(), ClientError>;
}

/// An in-process [`Transport`] that drives a [`crate::server::Server`]
/// directly (no sockets) — used for tests and embedding.
pub struct LoopbackTransport<'a> {
    server: &'a mut crate::server::Server,
}

impl<'a> LoopbackTransport<'a> {
    /// Wraps a server.
    pub fn new(server: &'a mut crate::server::Server) -> Self {
        Self { server }
    }
}

impl Transport for LoopbackTransport<'_> {
    fn request(&mut self, message: &[u8]) -> Result<Vec<u8>, ClientError> {
        self.server
            .process(message)
            .map_err(|_| ClientError::Protocol("server rejected the request"))?
            .ok_or(ClientError::Protocol("server produced no response"))
    }

    fn send(&mut self, message: &[u8]) -> Result<(), ClientError> {
        self.server
            .process(message)
            .map(|_| ())
            .map_err(|_| ClientError::Protocol("server rejected the message"))
    }
}

/// An OPC-UA client. SecurityMode `None` by default; call
/// `set_security` (feature `crypto`) for a secured
/// SecurityPolicy.
pub struct Client {
    channel: SecureChannel,
    auth_token: NodeId,
    session_id: NodeId,
    request_handle: u32,
    next_request_id: u32,
    channel_open: bool,
    connected: bool,
    clock: fn() -> i64,
    identity: Option<UserIdentityToken>,
    server_certificate: Vec<u8>,
    server_nonce: Vec<u8>,
    #[cfg(feature = "crypto")]
    security: Option<ClientSecurity>,
}

/// The default [`Client`] clock: the real wall clock under `std`, and a
/// constant zero without it — see [`Client::set_clock`].
#[cfg(feature = "std")]
fn default_clock() -> i64 {
    use zerodds_opcua_uacp::securechannel::DATETIME_TICKS_AT_UNIX_EPOCH;
    let Ok(since_epoch) = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) else {
        return DATETIME_TICKS_AT_UNIX_EPOCH;
    };
    let ticks = i64::try_from(since_epoch.as_nanos() / 100).unwrap_or(i64::MAX);
    DATETIME_TICKS_AT_UNIX_EPOCH.saturating_add(ticks)
}

#[cfg(not(feature = "std"))]
fn default_clock() -> i64 {
    0
}

impl core::fmt::Debug for Client {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Client")
            .field("session_id", &self.session_id)
            .field("connected", &self.connected)
            .finish_non_exhaustive()
    }
}

impl Default for Client {
    fn default() -> Self {
        Self::new()
    }
}

impl Client {
    /// Creates a fresh, unconnected client.
    #[must_use]
    pub fn new() -> Self {
        Self {
            channel: SecureChannel::new(0, 0),
            auth_token: NodeId::numeric(0, 0),
            session_id: NodeId::numeric(0, 0),
            request_handle: 0,
            next_request_id: 0,
            channel_open: false,
            connected: false,
            clock: default_clock,
            identity: None,
            server_certificate: Vec::new(),
            server_nonce: Vec::new(),
            #[cfg(feature = "crypto")]
            security: None,
        }
    }

    /// Replaces the clock stamped into every RequestHeader (Part 4 §7.33), in
    /// `DateTime` ticks — 100 ns intervals since 1601-01-01T00:00:00Z.
    ///
    /// Without `std` the default returns zero, and a server that verifies the
    /// timestamp answers `Bad_InvalidTimestamp`; a no_std caller has to supply
    /// its own source of time here.
    pub fn set_clock(&mut self, clock: fn() -> i64) {
        self.clock = clock;
    }

    /// Enables a secured SecurityPolicy for the next [`connect`](Self::connect).
    #[cfg(feature = "crypto")]
    pub fn set_security(&mut self, security: ClientSecurity) {
        self.security = Some(security);
    }

    /// The activated session id (null until connected).
    #[must_use]
    pub fn session_id(&self) -> &NodeId {
        &self.session_id
    }

    fn next_handle(&mut self) -> u32 {
        self.request_handle = self.request_handle.wrapping_add(1);
        self.request_handle
    }

    fn next_request_id(&mut self) -> u32 {
        self.next_request_id = self.next_request_id.wrapping_add(1);
        self.next_request_id
    }

    fn header(&mut self) -> RequestHeader {
        let token = self.auth_token.clone();
        self.header_with(token)
    }

    fn header_with(&mut self, token: NodeId) -> RequestHeader {
        let handle = self.next_handle();
        let now = (self.clock)();
        RequestHeader::new(token, handle).with_timestamp(now)
    }

    /// Opens just the transport + SecureChannel (Hello + OpenSecureChannel),
    /// without a session — enough for the session-less Discovery services
    /// ([`get_endpoints`](Self::get_endpoints) / [`find_servers`](Self::find_servers)).
    ///
    /// # Errors
    /// [`ClientError`] on a Hello/OpenSecureChannel failure.
    pub fn open_channel<T: Transport>(
        &mut self,
        transport: &mut T,
        endpoint_url: &str,
    ) -> Result<(), ClientError> {
        // 1. Hello / Acknowledge.
        let hello = HelloMessage {
            protocol_version: 0,
            receive_buffer_size: 65_535,
            send_buffer_size: 65_535,
            max_message_size: 0,
            max_chunk_count: 0,
            endpoint_url: String::from(endpoint_url),
        };
        let ack_bytes = transport.request(&hello.encode()?)?;
        let mut ar = UaReader::new(&ack_bytes);
        let ah = MessageHeader::decode(&mut ar)?;
        if ah.message_type != MessageType::Acknowledge {
            return Err(ClientError::Protocol("expected Acknowledge"));
        }
        // Part 6 §7.1.2.4: the Acknowledge carries the peer's buffer sizes;
        // its ReceiveBufferSize caps the chunks we may send.
        let ack = AcknowledgeMessage::decode_body(&mut ar)?;
        self.channel.set_send_buffer_size(ack.receive_buffer_size);

        // 2. OpenSecureChannel — secured if a SecurityPolicy is configured.
        let secured = {
            #[cfg(feature = "crypto")]
            {
                if self.security.is_some() {
                    self.open_secure_channel_secured(transport, SecurityTokenRequestType::Issue)?;
                    true
                } else {
                    false
                }
            }
            #[cfg(not(feature = "crypto"))]
            {
                false
            }
        };
        if !secured {
            self.open_secure_channel_plain(transport, SecurityTokenRequestType::Issue)?;
        }
        self.channel_open = true;
        Ok(())
    }

    /// Runs the full connect handshake: Hello, OpenSecureChannel,
    /// CreateSession, ActivateSession.
    ///
    /// # Errors
    /// [`ClientError`] on any handshake step failing.
    pub fn connect<T: Transport>(
        &mut self,
        transport: &mut T,
        endpoint_url: &str,
    ) -> Result<(), ClientError> {
        self.open_channel(transport, endpoint_url)?;

        // 3. CreateSession.
        let create = ServiceRequest::CreateSession(CreateSessionRequest {
            request_header: self.header(),
            client_description: client_description(),
            server_uri: String::new(),
            endpoint_url: String::from(endpoint_url),
            session_name: String::from("zerodds-client"),
            client_nonce: Vec::new(),
            client_certificate: Vec::new(),
            requested_session_timeout: 1_200_000.0,
            max_response_message_size: 0,
        });
        let resp = self.call_service(transport, create)?;
        let ServiceResponse::CreateSession(cs) = resp else {
            return Err(ClientError::Protocol("expected CreateSessionResponse"));
        };
        self.session_id = cs.session_id;
        self.auth_token = cs.authentication_token;

        self.server_certificate = cs.server_certificate;
        self.server_nonce = cs.server_nonce;

        // 4. ActivateSession with the configured UserIdentityToken (Part 4
        //    §5.7.3; the anonymous token unless `set_identity` said otherwise).
        self.activate_session(transport)
    }

    /// Sets the `UserIdentityToken` [`connect`](Self::connect) and
    /// [`change_user`](Self::change_user) send (Part 4 §7.40). Without this the
    /// client activates anonymously.
    pub fn set_identity(&mut self, identity: UserIdentityToken) {
        self.identity = Some(identity);
    }

    /// Changes the user identity of the running session by calling
    /// ActivateSession again (Part 4 §5.7.3.1: "Clients can change the identity
    /// of a user associated with a Session by calling the ActivateSession
    /// Service").
    ///
    /// # Errors
    /// [`ClientError`] if not connected or the server rejects the identity.
    pub fn change_user<T: Transport>(
        &mut self,
        transport: &mut T,
        identity: UserIdentityToken,
    ) -> Result<(), ClientError> {
        if !self.connected {
            return Err(ClientError::Protocol("not connected"));
        }
        self.identity = Some(identity);
        self.activate_session(transport)
    }

    fn activate_session<T: Transport>(&mut self, transport: &mut T) -> Result<(), ClientError> {
        let token = match self.identity.as_ref() {
            None => null_extension_object(),
            Some(identity) => identity.to_extension_object()?,
        };
        // Part 4 §6.1.8: the userTokenSignature proves possession of an X.509
        // identity; it is signed over the server certificate and the last
        // server nonce.
        let user_token_signature = self.user_token_signature()?;
        let activate = ServiceRequest::ActivateSession(ActivateSessionRequest {
            request_header: self.header(),
            client_signature: SignatureData::default(),
            locale_ids: Vec::new(),
            user_identity_token: token,
            user_token_signature,
        });
        let ServiceResponse::ActivateSession(ar) = self.call_service(transport, activate)? else {
            return Err(ClientError::Protocol("expected ActivateSessionResponse"));
        };
        // §5.7.3.2: "This number shall have a length between 32 and 128 bytes
        // inclusive. The Client shall check the length."
        if !ar.server_nonce.is_empty() && !(32..=128).contains(&ar.server_nonce.len()) {
            return Err(ClientError::Protocol(
                "ActivateSession serverNonce outside the 32..=128 byte range",
            ));
        }
        self.server_nonce = ar.server_nonce;
        self.connected = true;
        Ok(())
    }

    /// The `userTokenSignature` for the configured identity, empty when the
    /// token needs no proof of possession (Part 4 §7.40.5).
    fn user_token_signature(&mut self) -> Result<SignatureData, ClientError> {
        #[cfg(feature = "crypto")]
        {
            let Some(UserIdentityToken::X509 { .. }) = self.identity.as_ref() else {
                return Ok(SignatureData::default());
            };
            let Some(sec) = self.security.as_mut() else {
                return Ok(SignatureData::default());
            };
            let data = zerodds_opcua_uacp::crypto::signature_data(
                &self.server_certificate,
                &self.server_nonce,
            );
            let policy = sec.policy;
            let mut rng_ref: &mut dyn CryptoRngCore = sec.rng.as_mut();
            let signature = zerodds_opcua_uacp::crypto::sign_with_policy(
                &mut rng_ref,
                policy,
                &sec.private_key,
                &data,
            )
            .map_err(|_| ClientError::Protocol("userTokenSignature signing failed"))?;
            Ok(SignatureData {
                algorithm: String::from(policy.uri()),
                signature,
            })
        }
        #[cfg(not(feature = "crypto"))]
        {
            Ok(SignatureData::default())
        }
    }

    /// Renews the SecureChannel's SecurityToken (Part 4 §5.6.2: "Clients should
    /// request a new SecurityToken after 75 % of its lifetime has elapsed").
    ///
    /// # Errors
    /// [`ClientError`] if the channel is not open or the renew fails.
    pub fn renew_secure_channel<T: Transport>(
        &mut self,
        transport: &mut T,
    ) -> Result<(), ClientError> {
        if !self.channel_open {
            return Err(ClientError::Protocol("channel not open"));
        }
        #[cfg(feature = "crypto")]
        if self.security.is_some() {
            return self.open_secure_channel_secured(transport, SecurityTokenRequestType::Renew);
        }
        self.open_secure_channel_plain(transport, SecurityTokenRequestType::Renew)
    }

    /// The plaintext (SecurityMode `None`) OpenSecureChannel exchange.
    fn open_secure_channel_plain<T: Transport>(
        &mut self,
        transport: &mut T,
        request_type: SecurityTokenRequestType,
    ) -> Result<(), ClientError> {
        let open = OpenSecureChannelRequest {
            request_header: self.header_with(NodeId::numeric(0, 0)),
            client_protocol_version: 0,
            request_type,
            security_mode: MessageSecurityMode::None,
            client_nonce: Vec::new(),
            requested_lifetime: 3_600_000,
        };
        let rid = self.next_request_id();
        let open_resp_bytes = transport.request(&self.channel.open_chunk(rid, &open.encode()?)?)?;
        let chunk = parse_chunk(&open_resp_bytes)?;
        self.channel
            .note_incoming_sequence(chunk.sequence_header.sequence_number)?;
        let mut br = UaReader::new(&chunk.body);
        let _type_id = NodeId::decode(&mut br)?;
        let open_resp = OpenSecureChannelResponse::decode_body(&mut br)?;
        // Adopt the server-issued channel + token for symmetric messages. In
        // place: the OPN above already consumed SequenceNumber 1, and Part 6
        // §6.7.2.3 counts per channel, not per chunk kind.
        self.channel.adopt_token(
            open_resp.security_token.channel_id,
            open_resp.security_token.token_id,
        );
        Ok(())
    }

    /// The secured OpenSecureChannel exchange: builds the asymmetric OPN with
    /// the configured SecurityPolicy, opens the server's secured response,
    /// derives the §6.7.6 keys, and installs the symmetric session.
    #[cfg(feature = "crypto")]
    fn open_secure_channel_secured<T: Transport>(
        &mut self,
        transport: &mut T,
        request_type: SecurityTokenRequestType,
    ) -> Result<(), ClientError> {
        let handle = self.next_handle();
        let now = (self.clock)();
        let request_id = self.next_request_id();
        let seq = self.channel.next_send_sequence();
        let channel_id = match request_type {
            SecurityTokenRequestType::Issue => 0,
            SecurityTokenRequestType::Renew => self.channel.channel_id(),
        };

        let policy;
        let mode;
        let client_nonce;
        let opn_bytes;
        {
            let sec = self
                .security
                .as_mut()
                .ok_or(ClientError::Protocol("no SecurityPolicy configured"))?;
            policy = sec.policy;
            mode = sec.mode;
            let msm = match mode {
                SecuredMode::Sign => MessageSecurityMode::Sign,
                SecuredMode::SignAndEncrypt => MessageSecurityMode::SignAndEncrypt,
            };
            let nonce_len = policy.sym_enc_key_len().max(32);
            let mut nonce = alloc::vec![0u8; nonce_len];
            sec.rng.fill_bytes(&mut nonce);
            client_nonce = nonce;
            let open = OpenSecureChannelRequest {
                request_header: RequestHeader::new(NodeId::numeric(0, 0), handle)
                    .with_timestamp(now),
                client_protocol_version: 0,
                request_type,
                security_mode: msm,
                client_nonce: client_nonce.clone(),
                requested_lifetime: 3_600_000,
            };
            let body = open.encode()?;
            let ctx = AsymmetricContext {
                policy,
                sender_certificate: &sec.certificate,
                sender_private_key: &sec.private_key,
                receiver_certificate: &sec.server_certificate,
                receiver_public_key: &sec.server_public_key,
            };
            let mut rng_ref: &mut dyn CryptoRngCore = sec.rng.as_mut();
            // Part 4 §5.6.2: a RENEW names the SecureChannel whose token it
            // replaces; only an ISSUE carries the null id.
            opn_bytes =
                build_asymmetric_chunk(&mut rng_ref, &ctx, channel_id, seq, request_id, &body)
                    .map_err(|_| ClientError::Protocol("secured OPN build failed"))?;
        }

        let resp_bytes = transport.request(&opn_bytes)?;

        let (open_resp, resp_sequence) = {
            let sec = self
                .security
                .as_ref()
                .ok_or(ClientError::Protocol("no SecurityPolicy configured"))?;
            let opened = open_asymmetric_chunk(
                policy,
                &sec.private_key,
                &sec.server_public_key,
                &resp_bytes,
            )
            .map_err(|_| ClientError::Protocol("secured OPN response open/verify failed"))?;
            let sequence_number = opened.sequence_number;
            let mut br = UaReader::new(&opened.body);
            let _type_id = NodeId::decode(&mut br)?;
            (
                OpenSecureChannelResponse::decode_body(&mut br)?,
                sequence_number,
            )
        };
        self.channel.note_incoming_sequence(resp_sequence)?;

        let send_keys = derive_keys(policy, &open_resp.server_nonce, &client_nonce)
            .map_err(|_| ClientError::Protocol("key derivation failed"))?;
        let recv_keys = derive_keys(policy, &client_nonce, &open_resp.server_nonce)
            .map_err(|_| ClientError::Protocol("key derivation failed"))?;
        // In place, for the reason spelled out in `adopt_token`: the secured
        // OPN above already consumed this channel's SequenceNumber 1.
        self.channel.adopt_token(
            open_resp.security_token.channel_id,
            open_resp.security_token.token_id,
        );
        self.channel.install_security(SecuritySession {
            policy,
            mode,
            send_keys,
            recv_keys,
        });
        Ok(())
    }

    /// Reads the `Value` attribute of each node.
    ///
    /// # Errors
    /// [`ClientError`] if not connected or the service fails.
    pub fn read_values<T: Transport>(
        &mut self,
        transport: &mut T,
        node_ids: &[NodeId],
    ) -> Result<Vec<DataValue>, ClientError> {
        if !self.connected {
            return Err(ClientError::Protocol("not connected"));
        }
        let nodes_to_read = node_ids
            .iter()
            .map(|n| ReadValueId {
                node_id: n.clone(),
                attribute_id: ATTRIBUTE_VALUE,
                index_range: String::new(),
                data_encoding: zerodds_opcua_gateway::types::QualifiedName {
                    namespace_index: 0,
                    name: String::new(),
                },
            })
            .collect();
        let req = ServiceRequest::Read(ReadRequest {
            request_header: self.header(),
            max_age: 0.0,
            timestamps_to_return: 0,
            nodes_to_read,
        });
        let ServiceResponse::Read(rr) = self.call_service(transport, req)? else {
            return Err(ClientError::Protocol("expected ReadResponse"));
        };
        Ok(rr.results)
    }

    /// Writes the `Value` attribute of each `(node, value)` pair, returning one
    /// StatusCode per write.
    ///
    /// # Errors
    /// [`ClientError`] if not connected or the service fails.
    pub fn write_values<T: Transport>(
        &mut self,
        transport: &mut T,
        writes: &[(NodeId, DataValue)],
    ) -> Result<Vec<u32>, ClientError> {
        if !self.connected {
            return Err(ClientError::Protocol("not connected"));
        }
        let nodes_to_write = writes
            .iter()
            .map(|(n, v)| WriteValue {
                node_id: n.clone(),
                attribute_id: ATTRIBUTE_VALUE,
                index_range: String::new(),
                value: v.clone(),
            })
            .collect();
        let req = ServiceRequest::Write(WriteRequest {
            request_header: self.header(),
            nodes_to_write,
        });
        let ServiceResponse::Write(wr) = self.call_service(transport, req)? else {
            return Err(ClientError::Protocol("expected WriteResponse"));
        };
        Ok(wr.results)
    }

    /// Reads arbitrary attributes of arbitrary nodes (Part 4 §5.11.2), with the
    /// `TimestampsToReturn` and per-entry `indexRange` the caller chooses.
    ///
    /// # Errors
    /// [`ClientError`] if not connected or the service fails.
    pub fn read_attributes<T: Transport>(
        &mut self,
        transport: &mut T,
        nodes_to_read: Vec<ReadValueId>,
        timestamps_to_return: TimestampsToReturn,
    ) -> Result<Vec<DataValue>, ClientError> {
        if !self.connected {
            return Err(ClientError::Protocol("not connected"));
        }
        let req = ServiceRequest::Read(ReadRequest {
            request_header: self.header(),
            max_age: 0.0,
            timestamps_to_return: match timestamps_to_return {
                TimestampsToReturn::Source => 0,
                TimestampsToReturn::Server => 1,
                TimestampsToReturn::Both => 2,
                TimestampsToReturn::Neither => 3,
            },
            nodes_to_read,
        });
        let ServiceResponse::Read(rr) = self.call_service(transport, req)? else {
            return Err(ClientError::Protocol("expected ReadResponse"));
        };
        Ok(rr.results)
    }

    /// Reads one attribute of one node (Part 4 §5.11.2).
    ///
    /// # Errors
    /// [`ClientError`] if not connected or the service fails.
    pub fn read_attribute<T: Transport>(
        &mut self,
        transport: &mut T,
        node_id: NodeId,
        attribute_id: u32,
    ) -> Result<DataValue, ClientError> {
        let values = self.read_attributes(
            transport,
            alloc::vec![read_value_id(node_id, attribute_id)],
            TimestampsToReturn::Neither,
        )?;
        values
            .into_iter()
            .next()
            .ok_or(ClientError::Protocol("empty ReadResponse"))
    }

    /// HistoryRead (Part 4 §5.11.3) for `nodes`, each starting a fresh read
    /// (no continuation point).
    ///
    /// # Errors
    /// [`ClientError`] if not connected or the service fails.
    pub fn history_read<T: Transport>(
        &mut self,
        transport: &mut T,
        details: HistoryReadDetails,
        nodes: &[NodeId],
        timestamps_to_return: TimestampsToReturn,
    ) -> Result<Vec<HistoryReadResult>, ClientError> {
        self.history_read_continued(
            transport,
            details,
            nodes.iter().map(|n| (n.clone(), Vec::new())).collect(),
            timestamps_to_return,
            false,
        )
    }

    /// As [`Self::history_read`], but each node carries its own
    /// continuation point (empty = fresh read), and `release` sets
    /// `releaseContinuationPoints` (Part 4 §5.11.3.2).
    ///
    /// # Errors
    /// [`ClientError`] if not connected or the service fails.
    pub fn history_read_continued<T: Transport>(
        &mut self,
        transport: &mut T,
        details: HistoryReadDetails,
        nodes: Vec<(NodeId, Vec<u8>)>,
        timestamps_to_return: TimestampsToReturn,
        release: bool,
    ) -> Result<Vec<HistoryReadResult>, ClientError> {
        if !self.connected {
            return Err(ClientError::Protocol("not connected"));
        }
        let nodes_to_read = nodes
            .into_iter()
            .map(|(node_id, continuation_point)| HistoryReadValueId {
                node_id,
                index_range: String::new(),
                data_encoding: zerodds_opcua_gateway::types::QualifiedName {
                    namespace_index: 0,
                    name: String::new(),
                },
                continuation_point,
            })
            .collect();
        let req = ServiceRequest::HistoryRead(HistoryReadRequest {
            request_header: self.header(),
            history_read_details: details,
            timestamps_to_return: match timestamps_to_return {
                TimestampsToReturn::Source => 0,
                TimestampsToReturn::Server => 1,
                TimestampsToReturn::Both => 2,
                TimestampsToReturn::Neither => 3,
            },
            release_continuation_points: release,
            nodes_to_read,
        });
        let ServiceResponse::HistoryRead(hr) = self.call_service(transport, req)? else {
            return Err(ClientError::Protocol("expected HistoryReadResponse"));
        };
        Ok(hr.results)
    }

    /// Writes arbitrary attributes of arbitrary nodes (Part 4 §5.11.4),
    /// returning one StatusCode per write.
    ///
    /// # Errors
    /// [`ClientError`] if not connected or the service fails.
    pub fn write_attributes<T: Transport>(
        &mut self,
        transport: &mut T,
        nodes_to_write: Vec<WriteValue>,
    ) -> Result<Vec<u32>, ClientError> {
        if !self.connected {
            return Err(ClientError::Protocol("not connected"));
        }
        let req = ServiceRequest::Write(WriteRequest {
            request_header: self.header(),
            nodes_to_write,
        });
        let ServiceResponse::Write(wr) = self.call_service(transport, req)? else {
            return Err(ClientError::Protocol("expected WriteResponse"));
        };
        Ok(wr.results)
    }

    /// Calls the session-less Discovery service `GetEndpoints` (Part 4 §5.5.4).
    /// Requires only an open channel ([`open_channel`](Self::open_channel) or a
    /// full [`connect`](Self::connect)).
    ///
    /// # Errors
    /// [`ClientError`] if the channel is not open or the service fails.
    pub fn get_endpoints<T: Transport>(
        &mut self,
        transport: &mut T,
        endpoint_url: &str,
    ) -> Result<Vec<EndpointDescription>, ClientError> {
        if !self.channel_open {
            return Err(ClientError::Protocol("channel not open"));
        }
        let req = ServiceRequest::GetEndpoints(GetEndpointsRequest {
            request_header: self.header(),
            endpoint_url: String::from(endpoint_url),
            locale_ids: Vec::new(),
            profile_uris: Vec::new(),
        });
        let ServiceResponse::GetEndpoints(ge) = self.call_service(transport, req)? else {
            return Err(ClientError::Protocol("expected GetEndpointsResponse"));
        };
        Ok(ge.endpoints)
    }

    /// Calls the session-less Discovery service `FindServers` (Part 4 §5.5.2).
    ///
    /// # Errors
    /// [`ClientError`] if the channel is not open or the service fails.
    pub fn find_servers<T: Transport>(
        &mut self,
        transport: &mut T,
        endpoint_url: &str,
    ) -> Result<Vec<ApplicationDescription>, ClientError> {
        if !self.channel_open {
            return Err(ClientError::Protocol("channel not open"));
        }
        let req = ServiceRequest::FindServers(FindServersRequest {
            request_header: self.header(),
            endpoint_url: String::from(endpoint_url),
            locale_ids: Vec::new(),
            server_uris: Vec::new(),
        });
        let ServiceResponse::FindServers(fs) = self.call_service(transport, req)? else {
            return Err(ClientError::Protocol("expected FindServersResponse"));
        };
        Ok(fs.servers)
    }

    /// Creates a subscription with default parameters and returns its id.
    ///
    /// # Errors
    /// [`ClientError`] if not connected or the service fails.
    pub fn create_subscription<T: Transport>(
        &mut self,
        transport: &mut T,
    ) -> Result<u32, ClientError> {
        if !self.connected {
            return Err(ClientError::Protocol("not connected"));
        }
        let req = ServiceRequest::CreateSubscription(CreateSubscriptionRequest {
            request_header: self.header(),
            requested_publishing_interval: 1000.0,
            requested_lifetime_count: 10_000,
            requested_max_keep_alive_count: 10,
            max_notifications_per_publish: 0,
            publishing_enabled: true,
            priority: 0,
        });
        let ServiceResponse::CreateSubscription(cs) = self.call_service(transport, req)? else {
            return Err(ClientError::Protocol("expected CreateSubscriptionResponse"));
        };
        Ok(cs.subscription_id)
    }

    /// Creates monitored items (one per `(node, client_handle)`) on a
    /// subscription and returns the per-item results.
    ///
    /// # Errors
    /// [`ClientError`] if not connected or the service fails.
    pub fn create_monitored_items<T: Transport>(
        &mut self,
        transport: &mut T,
        subscription_id: u32,
        items: &[(NodeId, u32)],
    ) -> Result<Vec<MonitoredItemCreateResult>, ClientError> {
        if !self.connected {
            return Err(ClientError::Protocol("not connected"));
        }
        let items_to_create = items
            .iter()
            .map(|(node_id, client_handle)| MonitoredItemCreateRequest {
                item_to_monitor: ReadValueId {
                    node_id: node_id.clone(),
                    attribute_id: ATTRIBUTE_VALUE,
                    index_range: String::new(),
                    data_encoding: zerodds_opcua_gateway::types::QualifiedName {
                        namespace_index: 0,
                        name: String::new(),
                    },
                },
                monitoring_mode: 2, // Reporting
                requested_parameters: MonitoringParameters {
                    client_handle: *client_handle,
                    sampling_interval: 1000.0,
                    filter: null_filter(),
                    queue_size: 1,
                    discard_oldest: true,
                },
            })
            .collect();
        let req = ServiceRequest::CreateMonitoredItems(CreateMonitoredItemsRequest {
            request_header: self.header(),
            subscription_id,
            timestamps_to_return: 0,
            items_to_create,
        });
        let ServiceResponse::CreateMonitoredItems(cm) = self.call_service(transport, req)? else {
            return Err(ClientError::Protocol(
                "expected CreateMonitoredItemsResponse",
            ));
        };
        Ok(cm.results)
    }

    /// Sends a Publish and returns `(subscriptionId, changed monitored items)`
    /// decoded from the response's `DataChangeNotification`(s).
    ///
    /// # Errors
    /// [`ClientError`] if not connected or the service fails.
    pub fn publish<T: Transport>(
        &mut self,
        transport: &mut T,
    ) -> Result<(u32, Vec<MonitoredItemNotification>), ClientError> {
        if !self.connected {
            return Err(ClientError::Protocol("not connected"));
        }
        let req = ServiceRequest::Publish(PublishRequest {
            request_header: self.header(),
            subscription_acknowledgements: Vec::new(),
        });
        let ServiceResponse::Publish(pr) = self.call_service(transport, req)? else {
            return Err(ClientError::Protocol("expected PublishResponse"));
        };
        let mut notifications = Vec::new();
        for eo in &pr.notification_message.notification_data {
            if eo.type_id == node_ids::DATA_CHANGE_NOTIFICATION {
                let dcn = DataChangeNotification::from_extension_object(eo)?;
                notifications.extend(dcn.monitored_items);
            }
        }
        Ok((pr.subscription_id, notifications))
    }

    /// Deletes subscriptions and returns the per-subscription StatusCodes.
    ///
    /// # Errors
    /// [`ClientError`] if not connected or the service fails.
    pub fn delete_subscriptions<T: Transport>(
        &mut self,
        transport: &mut T,
        subscription_ids: &[u32],
    ) -> Result<Vec<u32>, ClientError> {
        if !self.connected {
            return Err(ClientError::Protocol("not connected"));
        }
        let req = ServiceRequest::DeleteSubscriptions(DeleteSubscriptionsRequest {
            request_header: self.header(),
            subscription_ids: subscription_ids.to_vec(),
        });
        let ServiceResponse::DeleteSubscriptions(ds) = self.call_service(transport, req)? else {
            return Err(ClientError::Protocol(
                "expected DeleteSubscriptionsResponse",
            ));
        };
        Ok(ds.results)
    }

    /// Browses a node's forward hierarchical references (all reference types,
    /// all node classes, full result mask) and returns the references found.
    ///
    /// # Errors
    /// [`ClientError`] if not connected or the service fails.
    pub fn browse<T: Transport>(
        &mut self,
        transport: &mut T,
        node_id: NodeId,
    ) -> Result<Vec<ReferenceDescription>, ClientError> {
        if !self.connected {
            return Err(ClientError::Protocol("not connected"));
        }
        let req = ServiceRequest::Browse(BrowseRequest {
            request_header: self.header(),
            view: ViewDescription::default(),
            requested_max_references_per_node: 0,
            nodes_to_browse: alloc::vec![BrowseDescription {
                node_id,
                browse_direction: 0,                       // Forward
                reference_type_id: NodeId::numeric(0, 31), // References (all)
                include_subtypes: true,
                node_class_mask: 0,
                result_mask: 0x3F,
            }],
        });
        let ServiceResponse::Browse(br) = self.call_service(transport, req)? else {
            return Err(ClientError::Protocol("expected BrowseResponse"));
        };
        let first = br
            .results
            .into_iter()
            .next()
            .ok_or(ClientError::Protocol("empty BrowseResponse"))?;
        if first.status_code != 0 {
            return Err(ClientError::Protocol("browse returned a bad status"));
        }
        Ok(first.references)
    }

    /// Browses one node with an explicit `requestedMaxReferencesPerNode`,
    /// returning the first batch and the continuation point (empty when the
    /// whole result fit) — Part 4 §5.9.2.
    ///
    /// # Errors
    /// [`ClientError`] if not connected, the service fails, or the browse
    /// returned a bad operation StatusCode.
    pub fn browse_limited<T: Transport>(
        &mut self,
        transport: &mut T,
        node_id: NodeId,
        max_references_per_node: u32,
    ) -> Result<(Vec<ReferenceDescription>, Vec<u8>), ClientError> {
        if !self.connected {
            return Err(ClientError::Protocol("not connected"));
        }
        let req = ServiceRequest::Browse(BrowseRequest {
            request_header: self.header(),
            view: ViewDescription::default(),
            requested_max_references_per_node: max_references_per_node,
            nodes_to_browse: alloc::vec![BrowseDescription {
                node_id,
                browse_direction: 0,                       // Forward
                reference_type_id: NodeId::numeric(0, 31), // References (all)
                include_subtypes: true,
                node_class_mask: 0,
                result_mask: 0x3F,
            }],
        });
        let ServiceResponse::Browse(br) = self.call_service(transport, req)? else {
            return Err(ClientError::Protocol("expected BrowseResponse"));
        };
        let first = br
            .results
            .into_iter()
            .next()
            .ok_or(ClientError::Protocol("empty BrowseResponse"))?;
        if first.status_code != 0 {
            return Err(ClientError::ServiceFault(first.status_code));
        }
        Ok((first.references, first.continuation_point))
    }

    /// Continues a Browse from its continuation point (Part 4 §5.9.3).
    /// With `release` the point is freed instead and the result is empty.
    ///
    /// # Errors
    /// [`ClientError`] if not connected, the service fails, or the
    /// continuation point was rejected.
    pub fn browse_next<T: Transport>(
        &mut self,
        transport: &mut T,
        continuation_point: Vec<u8>,
        release: bool,
    ) -> Result<(Vec<ReferenceDescription>, Vec<u8>), ClientError> {
        if !self.connected {
            return Err(ClientError::Protocol("not connected"));
        }
        let req = ServiceRequest::BrowseNext(BrowseNextRequest {
            request_header: self.header(),
            release_continuation_points: release,
            continuation_points: alloc::vec![continuation_point],
        });
        let ServiceResponse::BrowseNext(br) = self.call_service(transport, req)? else {
            return Err(ClientError::Protocol("expected BrowseNextResponse"));
        };
        let first = br
            .results
            .into_iter()
            .next()
            .ok_or(ClientError::Protocol("empty BrowseNextResponse"))?;
        if first.status_code != 0 {
            return Err(ClientError::ServiceFault(first.status_code));
        }
        Ok((first.references, first.continuation_point))
    }

    /// Browses a node completely, following continuation points until the
    /// server returns none (Part 4 §5.9.2 + §5.9.3).
    ///
    /// # Errors
    /// [`ClientError`] if not connected or a service call fails.
    pub fn browse_all<T: Transport>(
        &mut self,
        transport: &mut T,
        node_id: NodeId,
        max_references_per_node: u32,
    ) -> Result<Vec<ReferenceDescription>, ClientError> {
        let (mut all, mut cp) = self.browse_limited(transport, node_id, max_references_per_node)?;
        while !cp.is_empty() {
            let (more, next) = self.browse_next(transport, cp, false)?;
            all.extend(more);
            cp = next;
        }
        Ok(all)
    }

    /// Modifies a subscription's parameters (Part 4 §5.14.3), returning the
    /// revised `(publishingInterval, lifetimeCount, maxKeepAliveCount)`.
    ///
    /// # Errors
    /// [`ClientError`] if not connected or the service fails.
    pub fn modify_subscription<T: Transport>(
        &mut self,
        transport: &mut T,
        subscription_id: u32,
        publishing_interval: f64,
    ) -> Result<(f64, u32, u32), ClientError> {
        if !self.connected {
            return Err(ClientError::Protocol("not connected"));
        }
        let req = ServiceRequest::ModifySubscription(ModifySubscriptionRequest {
            request_header: self.header(),
            subscription_id,
            requested_publishing_interval: publishing_interval,
            requested_lifetime_count: 10_000,
            requested_max_keep_alive_count: 10,
            max_notifications_per_publish: 0,
            priority: 0,
        });
        let ServiceResponse::ModifySubscription(ms) = self.call_service(transport, req)? else {
            return Err(ClientError::Protocol("expected ModifySubscriptionResponse"));
        };
        Ok((
            ms.revised_publishing_interval,
            ms.revised_lifetime_count,
            ms.revised_max_keep_alive_count,
        ))
    }

    /// Modifies monitored items (Part 4 §5.13.3).
    ///
    /// # Errors
    /// [`ClientError`] if not connected or the service fails.
    pub fn modify_monitored_items<T: Transport>(
        &mut self,
        transport: &mut T,
        subscription_id: u32,
        items: &[(u32, MonitoringParameters)],
    ) -> Result<Vec<MonitoredItemModifyResult>, ClientError> {
        if !self.connected {
            return Err(ClientError::Protocol("not connected"));
        }
        let items_to_modify = items
            .iter()
            .map(|(id, p)| MonitoredItemModifyRequest {
                monitored_item_id: *id,
                requested_parameters: p.clone(),
            })
            .collect();
        let req = ServiceRequest::ModifyMonitoredItems(ModifyMonitoredItemsRequest {
            request_header: self.header(),
            subscription_id,
            timestamps_to_return: 0,
            items_to_modify,
        });
        let ServiceResponse::ModifyMonitoredItems(mm) = self.call_service(transport, req)? else {
            return Err(ClientError::Protocol(
                "expected ModifyMonitoredItemsResponse",
            ));
        };
        Ok(mm.results)
    }

    /// Sets the MonitoringMode of monitored items (Part 4 §5.13.4).
    ///
    /// # Errors
    /// [`ClientError`] if not connected or the service fails.
    pub fn set_monitoring_mode<T: Transport>(
        &mut self,
        transport: &mut T,
        subscription_id: u32,
        mode: MonitoringMode,
        monitored_item_ids: &[u32],
    ) -> Result<Vec<u32>, ClientError> {
        if !self.connected {
            return Err(ClientError::Protocol("not connected"));
        }
        let req = ServiceRequest::SetMonitoringMode(SetMonitoringModeRequest {
            request_header: self.header(),
            subscription_id,
            monitoring_mode: mode.as_i32(),
            monitored_item_ids: monitored_item_ids.to_vec(),
        });
        let ServiceResponse::SetMonitoringMode(sm) = self.call_service(transport, req)? else {
            return Err(ClientError::Protocol("expected SetMonitoringModeResponse"));
        };
        Ok(sm.results)
    }

    /// Adds/removes triggering links (Part 4 §5.13.5), returning
    /// `(addResults, removeResults)`.
    ///
    /// # Errors
    /// [`ClientError`] if not connected or the service fails.
    pub fn set_triggering<T: Transport>(
        &mut self,
        transport: &mut T,
        subscription_id: u32,
        triggering_item_id: u32,
        links_to_add: &[u32],
        links_to_remove: &[u32],
    ) -> Result<(Vec<u32>, Vec<u32>), ClientError> {
        if !self.connected {
            return Err(ClientError::Protocol("not connected"));
        }
        let req = ServiceRequest::SetTriggering(SetTriggeringRequest {
            request_header: self.header(),
            subscription_id,
            triggering_item_id,
            links_to_add: links_to_add.to_vec(),
            links_to_remove: links_to_remove.to_vec(),
        });
        let ServiceResponse::SetTriggering(st) = self.call_service(transport, req)? else {
            return Err(ClientError::Protocol("expected SetTriggeringResponse"));
        };
        Ok((st.add_results, st.remove_results))
    }

    /// Deletes monitored items (Part 4 §5.13.6).
    ///
    /// # Errors
    /// [`ClientError`] if not connected or the service fails.
    pub fn delete_monitored_items<T: Transport>(
        &mut self,
        transport: &mut T,
        subscription_id: u32,
        monitored_item_ids: &[u32],
    ) -> Result<Vec<u32>, ClientError> {
        if !self.connected {
            return Err(ClientError::Protocol("not connected"));
        }
        let req = ServiceRequest::DeleteMonitoredItems(DeleteMonitoredItemsRequest {
            request_header: self.header(),
            subscription_id,
            monitored_item_ids: monitored_item_ids.to_vec(),
        });
        let ServiceResponse::DeleteMonitoredItems(dm) = self.call_service(transport, req)? else {
            return Err(ClientError::Protocol(
                "expected DeleteMonitoredItemsResponse",
            ));
        };
        Ok(dm.results)
    }

    /// Publishes, acknowledging the given `(subscriptionId, sequenceNumber)`
    /// pairs first (Part 4 §5.14.5). Returns the subscription id, the
    /// notifications, the sequence number of the message and the sequence
    /// numbers still available for Republish.
    ///
    /// # Errors
    /// [`ClientError`] if not connected or the service fails.
    pub fn publish_acknowledging<T: Transport>(
        &mut self,
        transport: &mut T,
        acknowledgements: &[(u32, u32)],
    ) -> Result<PublishOutcome, ClientError> {
        if !self.connected {
            return Err(ClientError::Protocol("not connected"));
        }
        let req = ServiceRequest::Publish(PublishRequest {
            request_header: self.header(),
            subscription_acknowledgements: acknowledgements
                .iter()
                .map(|(sid, seq)| SubscriptionAcknowledgement {
                    subscription_id: *sid,
                    sequence_number: *seq,
                })
                .collect(),
        });
        let ServiceResponse::Publish(pr) = self.call_service(transport, req)? else {
            return Err(ClientError::Protocol("expected PublishResponse"));
        };
        let mut notifications = Vec::new();
        let mut status_change = None;
        for eo in &pr.notification_message.notification_data {
            if eo.type_id == node_ids::DATA_CHANGE_NOTIFICATION {
                notifications
                    .extend(DataChangeNotification::from_extension_object(eo)?.monitored_items);
            } else if eo.type_id == node_ids::STATUS_CHANGE_NOTIFICATION {
                status_change = Some(StatusChangeNotification::from_extension_object(eo)?.status);
            }
        }
        Ok(PublishOutcome {
            subscription_id: pr.subscription_id,
            sequence_number: pr.notification_message.sequence_number,
            available_sequence_numbers: pr.available_sequence_numbers,
            more_notifications: pr.more_notifications,
            notifications,
            status_change,
            acknowledge_results: pr.results,
        })
    }

    /// Requests the retransmission of a NotificationMessage (Part 4 §5.14.6).
    ///
    /// # Errors
    /// [`ClientError`] if not connected or the message is no longer available.
    pub fn republish<T: Transport>(
        &mut self,
        transport: &mut T,
        subscription_id: u32,
        retransmit_sequence_number: u32,
    ) -> Result<Vec<MonitoredItemNotification>, ClientError> {
        if !self.connected {
            return Err(ClientError::Protocol("not connected"));
        }
        let req = ServiceRequest::Republish(RepublishRequest {
            request_header: self.header(),
            subscription_id,
            retransmit_sequence_number,
        });
        let ServiceResponse::Republish(rr) = self.call_service(transport, req)? else {
            return Err(ClientError::Protocol("expected RepublishResponse"));
        };
        let mut notifications = Vec::new();
        for eo in &rr.notification_message.notification_data {
            if eo.type_id == node_ids::DATA_CHANGE_NOTIFICATION {
                notifications
                    .extend(DataChangeNotification::from_extension_object(eo)?.monitored_items);
            }
        }
        Ok(notifications)
    }

    /// Transfers subscriptions from another session to this one
    /// (Part 4 §5.14.7).
    ///
    /// # Errors
    /// [`ClientError`] if not connected or the service fails.
    pub fn transfer_subscriptions<T: Transport>(
        &mut self,
        transport: &mut T,
        subscription_ids: &[u32],
        send_initial_values: bool,
    ) -> Result<Vec<TransferResult>, ClientError> {
        if !self.connected {
            return Err(ClientError::Protocol("not connected"));
        }
        let req = ServiceRequest::TransferSubscriptions(TransferSubscriptionsRequest {
            request_header: self.header(),
            subscription_ids: subscription_ids.to_vec(),
            send_initial_values,
        });
        let ServiceResponse::TransferSubscriptions(ts) = self.call_service(transport, req)? else {
            return Err(ClientError::Protocol(
                "expected TransferSubscriptionsResponse",
            ));
        };
        Ok(ts.results)
    }

    /// Closes the session (Part 4 §5.7.4).
    ///
    /// # Errors
    /// [`ClientError`] if the service fails.
    pub fn close_session<T: Transport>(
        &mut self,
        transport: &mut T,
        delete_subscriptions: bool,
    ) -> Result<(), ClientError> {
        let req = ServiceRequest::CloseSession(CloseSessionRequest {
            request_header: self.header(),
            delete_subscriptions,
        });
        let ServiceResponse::CloseSession(_) = self.call_service(transport, req)? else {
            return Err(ClientError::Protocol("expected CloseSessionResponse"));
        };
        self.connected = false;
        Ok(())
    }

    /// Calls a method and returns its result.
    ///
    /// # Errors
    /// [`ClientError`] if not connected or the service fails.
    pub fn call_method<T: Transport>(
        &mut self,
        transport: &mut T,
        object_id: NodeId,
        method_id: NodeId,
        input_arguments: Vec<Variant>,
    ) -> Result<CallMethodResult, ClientError> {
        if !self.connected {
            return Err(ClientError::Protocol("not connected"));
        }
        let req = ServiceRequest::Call(CallRequest {
            request_header: self.header(),
            methods_to_call: alloc::vec![CallMethodRequest {
                object_id,
                method_id,
                input_arguments,
            }],
        });
        let ServiceResponse::Call(cr) = self.call_service(transport, req)? else {
            return Err(ClientError::Protocol("expected CallResponse"));
        };
        cr.results
            .into_iter()
            .next()
            .ok_or(ClientError::Protocol("empty CallResponse"))
    }

    /// Encodes a service request into MSG chunks, sends them, reassembles the
    /// response chunks and decodes the service response.
    fn call_service<T: Transport>(
        &mut self,
        transport: &mut T,
        req: ServiceRequest,
    ) -> Result<ServiceResponse, ClientError> {
        let rid = self.next_request_id();
        let chunk = self.channel.message_chunk(rid, &req.encode()?)?;
        let resp_bytes = transport.request(&chunk)?;
        // Part 6 §6.7.1: the response may span several MessageChunks; feed each
        // to the channel until it hands back the reassembled Message.
        let mut body = None;
        for message in split_messages(&resp_bytes)? {
            match self.channel.open_incoming(message)? {
                ChunkOutcome::Incomplete => {}
                ChunkOutcome::Complete(chunk) => {
                    body = Some(chunk.body);
                    break;
                }
                // Part 6 §6.7.3: the sender aborted this Message; report its
                // error to the application without closing the channel.
                ChunkOutcome::Abort { error, .. } => return Err(ClientError::ServiceFault(error)),
            }
        }
        let body = body.ok_or(ClientError::Protocol(
            "response ended without a final MessageChunk",
        ))?;
        match ServiceResponse::decode(&body)? {
            // Report the server's own reason instead of the caller's
            // "expected a FooResponse".
            ServiceResponse::ServiceFault(f) => {
                Err(ClientError::ServiceFault(f.response_header.service_result))
            }
            other => Ok(other),
        }
    }
}

/// A [`ReadValueId`] for the whole value of one attribute, with the default
/// data encoding (Part 4 §7.28).
#[must_use]
pub fn read_value_id(node_id: NodeId, attribute_id: u32) -> ReadValueId {
    ReadValueId {
        node_id,
        attribute_id,
        index_range: String::new(),
        data_encoding: zerodds_opcua_gateway::types::QualifiedName {
            namespace_index: 0,
            name: String::new(),
        },
    }
}

/// What one Publish returned (Part 4 §5.14.5.2).
#[derive(Debug, Clone, PartialEq)]
pub struct PublishOutcome {
    /// The subscription the notification belongs to.
    pub subscription_id: u32,
    /// The NotificationMessage's sequence number.
    pub sequence_number: u32,
    /// Sequence numbers still available for Republish.
    pub available_sequence_numbers: Vec<u32>,
    /// Whether more notifications were ready than fit in this response.
    pub more_notifications: bool,
    /// The DataChangeNotification entries.
    pub notifications: Vec<MonitoredItemNotification>,
    /// The StatusCode of a StatusChangeNotification, if the message carried one
    /// (Part 4 §7.25.4).
    pub status_change: Option<u32>,
    /// One StatusCode per acknowledgement sent with the request.
    pub acknowledge_results: Vec<u32>,
}

fn client_description() -> ApplicationDescription {
    ApplicationDescription {
        application_uri: String::from("urn:zerodds:opcua-client"),
        product_uri: String::from("urn:zerodds"),
        application_name: zerodds_opcua_gateway::types::LocalizedText {
            locale: None,
            text: Some(String::from("ZeroDDS OPC-UA Client")),
        },
        application_type: ApplicationType::Client,
        gateway_server_uri: String::new(),
        discovery_profile_uri: String::new(),
        discovery_urls: Vec::new(),
    }
}

// ---------------------------------------------------------------------------
// TCP transport + server accept loop (feature `std`).
// ---------------------------------------------------------------------------

#[cfg(feature = "std")]
pub use tcp::{TcpTransport, serve_connection};

#[cfg(feature = "std")]
mod tcp {
    use super::ClientError;
    use crate::server::Server;
    use alloc::vec;
    use alloc::vec::Vec;
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::string::ToString;

    /// Hard cap on a single UACP message read off the wire (16 MiB).
    ///
    /// TCP is a stream transport, so the peer-announced `MessageSize`
    /// (a `u32`, up to ~4 GiB) cannot be checked against "bytes
    /// remaining" the way an in-memory buffer decode can — there is no
    /// bound until the cap below is applied. Mirrors the established
    /// `crates/transport-tcp/src/framing.rs::MAX_FRAME_SIZE` DoS-cap
    /// pattern: reject before allocating, so a malicious/misbehaving
    /// peer cannot force a multi-GB allocation from an 8-byte header.
    const MAX_UACP_MESSAGE_SIZE: usize = 16 * 1024 * 1024;

    fn read_message(stream: &mut TcpStream) -> Result<Vec<u8>, ClientError> {
        let mut header = [0u8; 8];
        stream
            .read_exact(&mut header)
            .map_err(|e| ClientError::Io(e.to_string()))?;
        let size = u32::from_le_bytes([header[4], header[5], header[6], header[7]]) as usize;
        if size < 8 {
            return Err(ClientError::Protocol(
                "UACP MessageSize below header length",
            ));
        }
        if size > MAX_UACP_MESSAGE_SIZE {
            return Err(ClientError::Protocol(
                "UACP MessageSize exceeds MAX_UACP_MESSAGE_SIZE",
            ));
        }
        let mut rest = vec![0u8; size - 8];
        stream
            .read_exact(&mut rest)
            .map_err(|e| ClientError::Io(e.to_string()))?;
        let mut msg = Vec::with_capacity(size);
        msg.extend_from_slice(&header);
        msg.extend_from_slice(&rest);
        Ok(msg)
    }

    /// An `opc.tcp` [`Transport`](super::Transport) over a `TcpStream`.
    #[derive(Debug)]
    pub struct TcpTransport {
        stream: TcpStream,
    }

    impl TcpTransport {
        /// Connects to `addr` (e.g. `127.0.0.1:4840`).
        ///
        /// # Errors
        /// [`ClientError::Io`] if the connection fails.
        pub fn connect(addr: &str) -> Result<Self, ClientError> {
            let stream = TcpStream::connect(addr).map_err(|e| ClientError::Io(e.to_string()))?;
            Ok(Self { stream })
        }
    }

    /// Reads a whole response: every MessageChunk up to and including the one
    /// whose `IsFinal` byte is `F` (final) or `A` (abort) — Part 6 §6.7.1
    /// splits one Message across several chunks, and the caller needs them all.
    fn read_response(stream: &mut TcpStream) -> Result<Vec<u8>, ClientError> {
        let mut out = Vec::new();
        loop {
            let message = read_message(stream)?;
            let is_final = message.get(3).copied().unwrap_or(b'F');
            out.extend_from_slice(&message);
            if is_final != b'C' {
                return Ok(out);
            }
        }
    }

    impl super::Transport for TcpTransport {
        fn request(&mut self, message: &[u8]) -> Result<Vec<u8>, ClientError> {
            self.stream
                .write_all(message)
                .map_err(|e| ClientError::Io(e.to_string()))?;
            read_response(&mut self.stream)
        }

        fn send(&mut self, message: &[u8]) -> Result<(), ClientError> {
            self.stream
                .write_all(message)
                .map_err(|e| ClientError::Io(e.to_string()))
        }
    }

    /// Serves one accepted `TcpStream` connection with `server`: reads framed
    /// UACP messages and writes the server's responses until the peer closes.
    ///
    /// # Errors
    /// [`ClientError`] on an I/O or protocol failure other than a clean close.
    pub fn serve_connection(server: &mut Server, mut stream: TcpStream) -> Result<(), ClientError> {
        loop {
            let msg = match read_message(&mut stream) {
                Ok(m) => m,
                Err(_) => return Ok(()), // peer closed
            };
            match server
                .process(&msg)
                .map_err(|_| ClientError::Protocol("server error"))?
            {
                Some(resp) => stream
                    .write_all(&resp)
                    .map_err(|e| ClientError::Io(e.to_string()))?,
                None => return Ok(()), // CloseSecureChannel
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::net::TcpListener;

        // -------------------------------------------------------------
        // Buffer-cap hardening — a peer-announced UACP `MessageSize`
        // that exceeds `MAX_UACP_MESSAGE_SIZE` must be rejected cleanly
        // (no multi-GB allocation attempt, no hang) right after the
        // 8-byte header is read. Mirrors the established
        // `crates/transport-tcp/src/framing.rs::MAX_FRAME_SIZE` guard.
        // -------------------------------------------------------------

        #[test]
        fn read_message_rejects_oversized_message_size_cleanly() {
            let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
            let addr = listener.local_addr().expect("local_addr");
            let handle = std::thread::spawn(move || {
                let mut stream = TcpStream::connect(addr).expect("connect");
                let mut header = [0u8; 8];
                header[0..4].copy_from_slice(b"MSGF");
                // Announce a message far beyond MAX_UACP_MESSAGE_SIZE; no
                // body bytes follow — the reader must reject before ever
                // attempting to read/allocate the body.
                header[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
                stream.write_all(&header).expect("write header");
            });
            let (mut accepted, _) = listener.accept().expect("accept");
            let res = read_message(&mut accepted);
            assert!(
                matches!(res, Err(ClientError::Protocol(_))),
                "expected clean Protocol rejection, got {res:?}"
            );
            handle.join().expect("writer thread");
        }

        #[test]
        fn read_message_roundtrip_within_bound() {
            let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
            let addr = listener.local_addr().expect("local_addr");
            let body = [1u8, 2, 3, 4, 5, 6, 7, 8];
            let handle = std::thread::spawn(move || {
                let mut stream = TcpStream::connect(addr).expect("connect");
                let mut header = [0u8; 8];
                header[0..4].copy_from_slice(b"MSGF");
                let size = 8 + body.len() as u32;
                header[4..8].copy_from_slice(&size.to_le_bytes());
                stream.write_all(&header).expect("write header");
                stream.write_all(&body).expect("write body");
            });
            let (mut accepted, _) = listener.accept().expect("accept");
            let msg = read_message(&mut accepted).expect("read_message");
            assert_eq!(&msg[0..4], b"MSGF");
            assert_eq!(&msg[8..], &body);
            handle.join().expect("writer thread");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::address_space::{AddressSpace, MethodOutcome};
    use crate::server::Server;
    use crate::services::{ReadAtTimeDetails, ReadRawModifiedDetails};
    use zerodds_opcua_gateway::data_value::VariantValue;

    fn demo_server() -> Server {
        let mut space = AddressSpace::new();
        space.set_value(
            NodeId::numeric(1, 1),
            DataValue::new_value(Variant::scalar(VariantValue::Int32(4242)), 0, 0),
        );
        // A second Variable, so a Write to it is a write to a Node that exists:
        // Part 4 §5.11.4.4 answers a write to an unknown Node with
        // Bad_NodeIdUnknown, not by creating it.
        space.set_value(
            NodeId::numeric(1, 2),
            DataValue::new_value(Variant::scalar(VariantValue::Int32(0)), 0, 0),
        );
        // A method that doubles its Int32 input.
        space.register_method(NodeId::numeric(1, 100), |args| {
            if let Some(v) = args.first() {
                if let Some(VariantValue::Int32(x)) = v.value.first() {
                    return MethodOutcome::good(alloc::vec![Variant::scalar(VariantValue::Int32(
                        x * 2
                    ))]);
                }
            }
            MethodOutcome::fault(0x8000_0000)
        });
        Server::new("opc.tcp://localhost:4840", space)
    }

    #[test]
    fn e2e_loopback_connect_read_call() {
        let mut server = demo_server();
        let mut client = Client::new();
        {
            let mut t = LoopbackTransport::new(&mut server);
            client
                .connect(&mut t, "opc.tcp://localhost:4840")
                .expect("connect");
            assert_ne!(*client.session_id(), NodeId::numeric(0, 0));

            // Read node value.
            let vals = client
                .read_values(&mut t, &[NodeId::numeric(1, 1)])
                .expect("read");
            assert_eq!(
                vals[0].value,
                Some(Variant::scalar(VariantValue::Int32(4242)))
            );

            // Call method (double 21 → 42).
            let res = client
                .call_method(
                    &mut t,
                    NodeId::numeric(0, 0),
                    NodeId::numeric(1, 100),
                    alloc::vec![Variant::scalar(VariantValue::Int32(21))],
                )
                .expect("call");
            assert_eq!(res.status_code, 0);
            assert_eq!(
                res.output_arguments[0],
                Variant::scalar(VariantValue::Int32(42))
            );
        }
    }

    #[test]
    fn e2e_loopback_write_then_read() {
        let mut server = demo_server();
        let mut client = Client::new();
        let mut t = LoopbackTransport::new(&mut server);
        client.connect(&mut t, "opc.tcp://x").expect("connect");

        // Write a new value to node (1,1) and to a fresh node (1,2).
        let results = client
            .write_values(
                &mut t,
                &[
                    (
                        NodeId::numeric(1, 1),
                        DataValue::new_value(Variant::scalar(VariantValue::Int32(7)), 0, 0),
                    ),
                    (
                        NodeId::numeric(1, 2),
                        DataValue::new_value(Variant::scalar(VariantValue::Int32(9)), 0, 0),
                    ),
                ],
            )
            .expect("write");
        assert_eq!(results, alloc::vec![0, 0]);

        // Read both back.
        let vals = client
            .read_values(&mut t, &[NodeId::numeric(1, 1), NodeId::numeric(1, 2)])
            .expect("read");
        assert_eq!(vals[0].value, Some(Variant::scalar(VariantValue::Int32(7))));
        assert_eq!(vals[1].value, Some(Variant::scalar(VariantValue::Int32(9))));
    }

    #[test]
    fn write_hook_can_refuse_a_write_before_it_reaches_the_address_space() {
        let mut server = demo_server();
        server.set_write_hook(Box::new(|_wv, _user| {
            Some(crate::status::BAD_USER_ACCESS_DENIED)
        }));
        let mut client = Client::new();
        let mut t = LoopbackTransport::new(&mut server);
        client.connect(&mut t, "opc.tcp://x").expect("connect");

        let results = client
            .write_values(
                &mut t,
                &[(
                    NodeId::numeric(1, 1),
                    DataValue::new_value(Variant::scalar(VariantValue::Int32(7)), 0, 0),
                )],
            )
            .expect("write service call itself succeeds; the per-value status is Bad");
        assert_eq!(results, alloc::vec![crate::status::BAD_USER_ACCESS_DENIED]);

        // The AddressSpace must never have been touched — the hook refused
        // before `write_attribute` ran, not after.
        let vals = client
            .read_values(&mut t, &[NodeId::numeric(1, 1)])
            .expect("read");
        assert_eq!(
            vals[0].value,
            Some(Variant::scalar(VariantValue::Int32(4242))),
            "demo_server's original value for (1,1) — the refused write must not have landed"
        );
    }

    #[test]
    fn write_hook_observes_the_calling_sessions_identity() {
        use crate::identity::{ClientUserId, UserStore};
        use alloc::sync::Arc;
        use std::sync::Mutex;

        let mut server = demo_server();
        let mut store = UserStore::anonymous_only();
        store.add_user("user", "alice", "s3cret");
        server.set_user_store(store);

        let seen: Arc<Mutex<Vec<ClientUserId>>> = Arc::new(Mutex::new(Vec::new()));
        let seen_in_hook = Arc::clone(&seen);
        server.set_write_hook(Box::new(move |_wv, user| {
            if let Ok(mut log) = seen_in_hook.lock() {
                log.push(user.clone());
            }
            None // let every write proceed — this test only checks identity.
        }));

        let mut client = Client::new();
        client.set_identity(UserIdentityToken::user_name("user", "alice", "s3cret"));
        let mut t = LoopbackTransport::new(&mut server);
        client.connect(&mut t, "opc.tcp://x").expect("connect");
        client
            .write_values(
                &mut t,
                &[(
                    NodeId::numeric(1, 1),
                    DataValue::new_value(Variant::scalar(VariantValue::Int32(1)), 0, 0),
                )],
            )
            .expect("write");

        let log = seen.lock().expect("lock");
        assert_eq!(
            log.as_slice(),
            [ClientUserId::UserName("alice".to_string())],
            "the hook must see the writing session's own identity, not Anonymous"
        );
    }

    #[test]
    fn read_unknown_node_yields_bad_status() {
        let mut server = demo_server();
        let mut client = Client::new();
        let mut t = LoopbackTransport::new(&mut server);
        client.connect(&mut t, "opc.tcp://x").expect("connect");
        let vals = client
            .read_values(&mut t, &[NodeId::numeric(1, 999)])
            .expect("read");
        assert_eq!(vals[0].status, Some(crate::server::BAD_NODE_ID_UNKNOWN));
        assert_eq!(vals[0].value, None);
    }

    #[test]
    fn e2e_loopback_subscription_publish() {
        let mut server = demo_server();
        let mut client = Client::new();
        let mut t = LoopbackTransport::new(&mut server);
        client.connect(&mut t, "opc.tcp://x").expect("connect");

        let sub = client
            .create_subscription(&mut t)
            .expect("create subscription");
        let results = client
            .create_monitored_items(&mut t, sub, &[(NodeId::numeric(1, 1), 77)])
            .expect("create monitored items");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].status_code, 0);

        // First Publish reports the initial value (4242) for client handle 77.
        let (sid, notes) = client.publish(&mut t).expect("publish 1");
        assert_eq!(sid, sub);
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].client_handle, 77);
        assert_eq!(
            notes[0].value.value,
            Some(Variant::scalar(VariantValue::Int32(4242)))
        );

        // No change → next Publish reports nothing.
        let (_, none) = client.publish(&mut t).expect("publish 2");
        assert!(none.is_empty());

        // Change the value, then Publish reports the new value.
        client
            .write_values(
                &mut t,
                &[(
                    NodeId::numeric(1, 1),
                    DataValue::new_value(Variant::scalar(VariantValue::Int32(123)), 0, 0),
                )],
            )
            .expect("write");
        let (_, changed) = client.publish(&mut t).expect("publish 3");
        assert_eq!(changed.len(), 1);
        assert_eq!(
            changed[0].value.value,
            Some(Variant::scalar(VariantValue::Int32(123)))
        );

        // Delete the subscription.
        let del = client.delete_subscriptions(&mut t, &[sub]).expect("delete");
        assert_eq!(del, alloc::vec![0]);
        // Part 4 §5.14.5.3: with no Subscription left for the Session, Publish
        // is answered with the Bad_NoSubscription service result.
        assert_eq!(
            client.publish(&mut t),
            Err(ClientError::ServiceFault(
                crate::status::BAD_NO_SUBSCRIPTION
            ))
        );
    }

    #[test]
    fn e2e_loopback_discovery() {
        let mut server = demo_server();
        let mut client = Client::new();
        let mut t = LoopbackTransport::new(&mut server);
        // Session-less discovery: open the channel only.
        client
            .open_channel(&mut t, "opc.tcp://localhost:4840")
            .expect("open channel");

        let eps = client
            .get_endpoints(&mut t, "opc.tcp://localhost:4840")
            .expect("get endpoints");
        assert_eq!(eps.len(), 1);
        assert_eq!(eps[0].endpoint_url, "opc.tcp://localhost:4840");
        assert_eq!(
            eps[0].security_policy_uri,
            zerodds_opcua_uacp::securechannel::SECURITY_POLICY_NONE
        );

        let servers = client
            .find_servers(&mut t, "opc.tcp://localhost:4840")
            .expect("find servers");
        assert_eq!(servers.len(), 1);
        assert_eq!(
            servers[0].application_name.text.as_deref(),
            Some("ZeroDDS OPC-UA Server")
        );

        // Discovery before an open channel is rejected.
        let mut fresh = Client::new();
        let mut t2 = LoopbackTransport::new(&mut server);
        assert!(fresh.get_endpoints(&mut t2, "opc.tcp://x").is_err());
    }

    #[test]
    fn e2e_loopback_browse() {
        use crate::address_space::{NodeClass, NodeMeta, reference_types};
        use zerodds_opcua_gateway::types::QualifiedName;

        let mut space = AddressSpace::new();
        let objects = NodeId::numeric(0, 85);
        let boiler = NodeId::numeric(1, 1);
        let temp = NodeId::numeric(1, 2);
        for (id, class, name) in [
            (objects.clone(), NodeClass::Object, "Objects"),
            (boiler.clone(), NodeClass::Object, "Boiler"),
            (temp.clone(), NodeClass::Variable, "Temperature"),
        ] {
            space.add_node(
                NodeMeta::new(
                    id,
                    class,
                    QualifiedName {
                        namespace_index: 1,
                        name: String::from(name),
                    },
                )
                .with_type_definition(NodeId::numeric(0, 58)),
            );
        }
        space.add_reference(objects.clone(), reference_types::ORGANIZES, boiler.clone());
        space.add_reference(boiler.clone(), reference_types::HAS_COMPONENT, temp.clone());
        let mut server = Server::new("opc.tcp://localhost:4840", space);

        let mut client = Client::new();
        let mut t = LoopbackTransport::new(&mut server);
        client
            .connect(&mut t, "opc.tcp://localhost:4840")
            .expect("connect");

        // Browse Objects → Boiler (Organizes).
        let refs = client.browse(&mut t, objects).expect("browse objects");
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].node_id.node_id, boiler);
        assert_eq!(refs[0].browse_name.name, "Boiler");
        assert_eq!(refs[0].reference_type_id, reference_types::ORGANIZES);
        assert!(refs[0].is_forward);

        // Browse Boiler → Temperature (HasComponent).
        let children = client.browse(&mut t, boiler).expect("browse boiler");
        assert_eq!(children.len(), 1);
        assert_eq!(children[0].node_id.node_id, temp);
        assert_eq!(children[0].node_class, NodeClass::Variable.as_i32());

        // Browsing an unknown node yields Bad_NodeIdUnknown (browse() maps that
        // to a protocol error).
        assert!(client.browse(&mut t, NodeId::numeric(1, 999)).is_err());
    }

    // -------------------------------------------------------------------
    // Attribute Service Set beyond `Value` (Part 4 §5.11 / Part 3 §5).
    // -------------------------------------------------------------------

    #[test]
    fn e2e_loopback_reads_every_attribute_of_a_variable() {
        use crate::address_space::{NodeClass, NodeMeta};
        use crate::attributes::{
            ClassAttributes, VariableAttributes, access_level, attribute_ids as a, text, write_mask,
        };
        use zerodds_opcua_gateway::types::QualifiedName;

        let mut space = AddressSpace::new();
        let temp = NodeId::numeric(1, 7);
        space.add_node(
            NodeMeta::new(
                temp.clone(),
                NodeClass::Variable,
                QualifiedName {
                    namespace_index: 1,
                    name: String::from("Temperature"),
                },
            )
            .with_description(text("boiler temperature"))
            .with_type_definition(NodeId::numeric(0, 63))
            .with_write_mask(write_mask::DISPLAY_NAME | write_mask::DESCRIPTION)
            .with_class_attributes(ClassAttributes::Variable(VariableAttributes {
                data_type: NodeId::numeric(0, 11), // Double
                value_rank: -1,
                array_dimensions: Vec::new(),
                access_level: access_level::CURRENT_READ | access_level::CURRENT_WRITE,
                user_access_level: access_level::CURRENT_READ,
                minimum_sampling_interval: 250.0,
                historizing: true,
                access_level_ex: 3,
            })),
        );
        space.set_value(
            temp.clone(),
            DataValue::new_value(Variant::scalar(VariantValue::Double(21.5)), 0, 0),
        );
        let mut server = Server::new("opc.tcp://x", space);
        let mut client = Client::new();
        let mut t = LoopbackTransport::new(&mut server);
        client.connect(&mut t, "opc.tcp://x").expect("connect");

        let ids = [
            a::NODE_ID,
            a::NODE_CLASS,
            a::BROWSE_NAME,
            a::DISPLAY_NAME,
            a::DESCRIPTION,
            a::WRITE_MASK,
            a::USER_WRITE_MASK,
            a::VALUE,
            a::DATA_TYPE,
            a::VALUE_RANK,
            a::ARRAY_DIMENSIONS,
            a::ACCESS_LEVEL,
            a::USER_ACCESS_LEVEL,
            a::MINIMUM_SAMPLING_INTERVAL,
            a::HISTORIZING,
            a::ACCESS_LEVEL_EX,
        ];
        let reads = ids
            .iter()
            .map(|id| read_value_id(temp.clone(), *id))
            .collect();
        let values = client
            .read_attributes(&mut t, reads, TimestampsToReturn::Neither)
            .expect("read attributes");
        assert_eq!(values.len(), ids.len());
        for (id, dv) in ids.iter().zip(values.iter()) {
            assert_eq!(dv.status, Some(0), "attribute {id} was not Good");
            assert!(dv.value.is_some(), "attribute {id} carried no value");
        }
        let scalar = |i: usize| values[i].value.as_ref().expect("value").value[0].clone();
        assert_eq!(scalar(0), VariantValue::NodeId(temp.clone()));
        assert_eq!(scalar(1), VariantValue::Int32(NodeClass::Variable.as_i32()));
        assert_eq!(
            scalar(2),
            VariantValue::QualifiedName(QualifiedName {
                namespace_index: 1,
                name: String::from("Temperature"),
            })
        );
        assert_eq!(
            scalar(4),
            VariantValue::LocalizedText(text("boiler temperature"))
        );
        assert_eq!(scalar(7), VariantValue::Double(21.5));
        assert_eq!(scalar(8), VariantValue::NodeId(NodeId::numeric(0, 11)));
        assert_eq!(scalar(9), VariantValue::Int32(-1));
        assert_eq!(scalar(11), VariantValue::Byte(3));
        assert_eq!(scalar(12), VariantValue::Byte(1));
        assert_eq!(scalar(13), VariantValue::Double(250.0));
        assert_eq!(scalar(14), VariantValue::Boolean(true));
        assert_eq!(scalar(15), VariantValue::UInt32(3));

        // Part 4 §5.11.2.4: an attribute the NodeClass does not carry is
        // Bad_AttributeIdInvalid — `Executable` belongs to a Method.
        let bad = client
            .read_attribute(&mut t, temp.clone(), a::EXECUTABLE)
            .expect("read");
        assert_eq!(bad.status, Some(crate::status::BAD_ATTRIBUTE_ID_INVALID));
        // And an id outside Part 6 Table A.1 likewise.
        let out_of_range = client.read_attribute(&mut t, temp, 99).expect("read");
        assert_eq!(
            out_of_range.status,
            Some(crate::status::BAD_ATTRIBUTE_ID_INVALID)
        );
    }

    // -------------------------------------------------------------------
    // HistoryRead Service (Part 4 §5.11.3 / Part 11 Historical Access).
    // -------------------------------------------------------------------

    #[test]
    fn e2e_loopback_history_read_raw_range_returns_retained_samples_in_order() {
        use crate::address_space::AddressSpace;

        let mut space = AddressSpace::new();
        let node = NodeId::numeric(1, 5);
        space.set_historizing(node.clone(), 10);
        for (t, v) in [(100i64, 1), (200, 2), (300, 3)] {
            space.set_value(
                node.clone(),
                DataValue::new_value(Variant::scalar(VariantValue::Int32(v)), 0, t),
            );
        }
        let mut server = Server::new("opc.tcp://x", space);
        let mut client = Client::new();
        let mut t = LoopbackTransport::new(&mut server);
        client.connect(&mut t, "opc.tcp://x").expect("connect");

        let results = client
            .history_read(
                &mut t,
                HistoryReadDetails::RawModified(ReadRawModifiedDetails {
                    is_read_modified: false,
                    start_time: 0,
                    end_time: 1_000,
                    num_values_per_node: 0,
                    return_bounds: false,
                }),
                &[node],
                TimestampsToReturn::Source,
            )
            .expect("history_read");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].status_code, crate::status::GOOD);
        assert!(results[0].continuation_point.is_empty());
        let got: Vec<i32> = results[0]
            .history_data
            .data_values
            .iter()
            .map(|dv| match dv.value {
                Some(Variant { value: ref vs, .. }) => match vs[0] {
                    VariantValue::Int32(x) => x,
                    _ => panic!("expected Int32"),
                },
                None => panic!("expected a value"),
            })
            .collect();
        assert_eq!(got, alloc::vec![1, 2, 3]);
        let ts: Vec<i64> = results[0]
            .history_data
            .data_values
            .iter()
            .map(|dv| dv.source_timestamp.expect("source timestamp"))
            .collect();
        assert_eq!(ts, alloc::vec![100, 200, 300]);
    }

    #[test]
    fn e2e_loopback_history_read_paginates_via_continuation_point() {
        use crate::address_space::AddressSpace;

        let mut space = AddressSpace::new();
        let node = NodeId::numeric(1, 6);
        space.set_historizing(node.clone(), 10);
        for (t, v) in [(100i64, 1), (200, 2), (300, 3)] {
            space.set_value(
                node.clone(),
                DataValue::new_value(Variant::scalar(VariantValue::Int32(v)), 0, t),
            );
        }
        let mut server = Server::new("opc.tcp://x", space);
        let mut client = Client::new();
        let mut t = LoopbackTransport::new(&mut server);
        client.connect(&mut t, "opc.tcp://x").expect("connect");

        let details = HistoryReadDetails::RawModified(ReadRawModifiedDetails {
            is_read_modified: false,
            start_time: 0,
            end_time: 1_000,
            num_values_per_node: 2,
            return_bounds: false,
        });
        let first = client
            .history_read(
                &mut t,
                details.clone(),
                &[node.clone()],
                TimestampsToReturn::Source,
            )
            .expect("first page");
        assert_eq!(first[0].history_data.data_values.len(), 2);
        assert!(
            !first[0].continuation_point.is_empty(),
            "more data remained, so a continuation point must be set"
        );

        let second = client
            .history_read_continued(
                &mut t,
                details,
                alloc::vec![(node, first[0].continuation_point.clone())],
                TimestampsToReturn::Source,
                false,
            )
            .expect("second page");
        assert_eq!(second[0].status_code, crate::status::GOOD);
        assert_eq!(second[0].history_data.data_values.len(), 1);
        assert!(second[0].continuation_point.is_empty());
        let third_val = match second[0].history_data.data_values[0].value {
            Some(Variant { value: ref vs, .. }) => vs[0].clone(),
            None => panic!("expected a value"),
        };
        assert_eq!(third_val, VariantValue::Int32(3));
    }

    #[test]
    fn e2e_loopback_history_read_on_a_non_historizing_node_is_bad_node_id_unknown() {
        use crate::address_space::AddressSpace;

        let mut space = AddressSpace::new();
        let node = NodeId::numeric(1, 7);
        space.set_value(
            node.clone(),
            DataValue::new_value(Variant::scalar(VariantValue::Int32(42)), 0, 0),
        );
        let mut server = Server::new("opc.tcp://x", space);
        let mut client = Client::new();
        let mut t = LoopbackTransport::new(&mut server);
        client.connect(&mut t, "opc.tcp://x").expect("connect");

        let results = client
            .history_read(
                &mut t,
                HistoryReadDetails::RawModified(ReadRawModifiedDetails {
                    is_read_modified: false,
                    start_time: 0,
                    end_time: 1_000,
                    num_values_per_node: 0,
                    return_bounds: false,
                }),
                &[node],
                TimestampsToReturn::Source,
            )
            .expect("history_read");
        assert_eq!(results[0].status_code, crate::status::BAD_NODE_ID_UNKNOWN);
    }

    #[test]
    fn e2e_loopback_history_read_modified_is_refused_not_silently_answered() {
        use crate::address_space::AddressSpace;

        let mut space = AddressSpace::new();
        let node = NodeId::numeric(1, 8);
        space.set_historizing(node.clone(), 10);
        space.set_value(
            node.clone(),
            DataValue::new_value(Variant::scalar(VariantValue::Int32(1)), 100, 100),
        );
        let mut server = Server::new("opc.tcp://x", space);
        let mut client = Client::new();
        let mut t = LoopbackTransport::new(&mut server);
        client.connect(&mut t, "opc.tcp://x").expect("connect");

        let err = client
            .history_read(
                &mut t,
                HistoryReadDetails::RawModified(ReadRawModifiedDetails {
                    is_read_modified: true,
                    start_time: 0,
                    end_time: 1_000,
                    num_values_per_node: 0,
                    return_bounds: false,
                }),
                &[node],
                TimestampsToReturn::Source,
            )
            .expect_err("Bad_HistoryOperationUnsupported must fault the service, not answer data");
        match err {
            ClientError::ServiceFault(code) => {
                assert_eq!(code, crate::status::BAD_HISTORY_OPERATION_UNSUPPORTED);
            }
            other => panic!("expected a ServiceFault, got {other:?}"),
        }
    }

    #[test]
    fn e2e_loopback_history_read_on_a_historizing_node_with_no_samples_is_bad_no_data() {
        use crate::address_space::AddressSpace;

        // Historizing, but never written: distinct from "not historizing at
        // all" ([`crate::status::BAD_NODE_ID_UNKNOWN`]) and must not be
        // confused with it — an empty retained window is Bad_NoData.
        let mut space = AddressSpace::new();
        let node = NodeId::numeric(1, 10);
        space.set_historizing(node.clone(), 10);
        let mut server = Server::new("opc.tcp://x", space);
        let mut client = Client::new();
        let mut t = LoopbackTransport::new(&mut server);
        client.connect(&mut t, "opc.tcp://x").expect("connect");

        let results = client
            .history_read(
                &mut t,
                HistoryReadDetails::RawModified(ReadRawModifiedDetails {
                    is_read_modified: false,
                    start_time: 0,
                    end_time: 1_000,
                    num_values_per_node: 0,
                    return_bounds: false,
                }),
                &[node],
                TimestampsToReturn::Source,
            )
            .expect("history_read");
        assert_eq!(results[0].status_code, crate::status::BAD_NO_DATA);
        assert!(results[0].history_data.data_values.is_empty());
    }

    #[test]
    fn e2e_loopback_history_read_at_time_returns_nearest_prior_sample() {
        use crate::address_space::AddressSpace;

        let mut space = AddressSpace::new();
        let node = NodeId::numeric(1, 9);
        space.set_historizing(node.clone(), 10);
        for (t, v) in [(100i64, 1), (300, 3)] {
            space.set_value(
                node.clone(),
                DataValue::new_value(Variant::scalar(VariantValue::Int32(v)), 0, t),
            );
        }
        let mut server = Server::new("opc.tcp://x", space);
        let mut client = Client::new();
        let mut t = LoopbackTransport::new(&mut server);
        client.connect(&mut t, "opc.tcp://x").expect("connect");

        let results = client
            .history_read(
                &mut t,
                HistoryReadDetails::AtTime(ReadAtTimeDetails {
                    req_times: alloc::vec![250],
                    use_simple_bounds: true,
                }),
                &[node],
                TimestampsToReturn::Source,
            )
            .expect("history_read");
        assert_eq!(results[0].status_code, crate::status::GOOD);
        let got = match results[0].history_data.data_values[0].value {
            Some(Variant { value: ref vs, .. }) => vs[0].clone(),
            None => panic!("expected a value"),
        };
        assert_eq!(got, VariantValue::Int32(1));
    }

    #[test]
    fn e2e_loopback_write_respects_the_write_mask_and_access_level() {
        use crate::address_space::{NodeClass, NodeMeta};
        use crate::attributes::{
            ClassAttributes, VariableAttributes, access_level, attribute_ids as a, text, write_mask,
        };
        use zerodds_opcua_gateway::types::QualifiedName;

        let mut space = AddressSpace::new();
        let node = NodeId::numeric(1, 3);
        space.add_node(
            NodeMeta::new(
                node.clone(),
                NodeClass::Variable,
                QualifiedName {
                    namespace_index: 1,
                    name: String::from("Setpoint"),
                },
            )
            // Only DisplayName is writeable (Part 3 §8.60 Table 44 bit 6).
            .with_write_mask(write_mask::DISPLAY_NAME)
            .with_class_attributes(ClassAttributes::Variable(VariableAttributes {
                // Readable but not writeable (Part 3 §8.57).
                access_level: access_level::CURRENT_READ,
                user_access_level: access_level::CURRENT_READ,
                ..VariableAttributes::default()
            })),
        );
        space.set_value(
            node.clone(),
            DataValue::new_value(Variant::scalar(VariantValue::Int32(1)), 0, 0),
        );
        let mut server = Server::new("opc.tcp://x", space);
        let mut client = Client::new();
        let mut t = LoopbackTransport::new(&mut server);
        client.connect(&mut t, "opc.tcp://x").expect("connect");

        let write = |attribute_id, value| WriteValue {
            node_id: node.clone(),
            attribute_id,
            index_range: String::new(),
            value: DataValue::new_value(Variant::scalar(value), 0, 0),
        };
        let results = client
            .write_attributes(
                &mut t,
                alloc::vec![
                    // Allowed by the WriteMask.
                    write(
                        a::DISPLAY_NAME,
                        VariantValue::LocalizedText(text("Setpoint 1"))
                    ),
                    // Mask bit clear → Bad_NotWritable.
                    write(a::DESCRIPTION, VariantValue::LocalizedText(text("nope"))),
                    // AccessLevel has no CurrentWrite → Bad_NotWritable.
                    write(a::VALUE, VariantValue::Int32(9)),
                    // Right attribute, wrong builtin type → Bad_TypeMismatch.
                    write(a::DISPLAY_NAME, VariantValue::Int32(5)),
                ],
            )
            .expect("write attributes");
        assert_eq!(
            results,
            alloc::vec![
                0,
                crate::status::BAD_NOT_WRITABLE,
                crate::status::BAD_NOT_WRITABLE,
                crate::status::BAD_TYPE_MISMATCH,
            ]
        );
        let back = client
            .read_attribute(&mut t, node.clone(), a::DISPLAY_NAME)
            .expect("read back");
        assert_eq!(
            back.value.expect("value").value[0],
            VariantValue::LocalizedText(text("Setpoint 1"))
        );
        // The refused Value write left the value alone.
        let value = client.read_attribute(&mut t, node, a::VALUE).expect("read");
        assert_eq!(value.value.expect("value").value[0], VariantValue::Int32(1));
    }

    #[test]
    fn e2e_loopback_index_range_read_and_write() {
        use crate::attributes::attribute_ids as a;
        let mut space = AddressSpace::new();
        let node = NodeId::numeric(1, 9);
        space.set_value(
            node.clone(),
            DataValue::new_value(
                Variant {
                    array_dimensions: alloc::vec![4],
                    value: alloc::vec![
                        VariantValue::Int32(10),
                        VariantValue::Int32(11),
                        VariantValue::Int32(12),
                        VariantValue::Int32(13),
                    ],
                },
                0,
                0,
            ),
        );
        let mut server = Server::new("opc.tcp://x", space);
        let mut client = Client::new();
        let mut t = LoopbackTransport::new(&mut server);
        client.connect(&mut t, "opc.tcp://x").expect("connect");

        let ranged = |range: &str| ReadValueId {
            node_id: node.clone(),
            attribute_id: a::VALUE,
            index_range: String::from(range),
            data_encoding: zerodds_opcua_gateway::types::QualifiedName {
                namespace_index: 0,
                name: String::new(),
            },
        };
        let values = client
            .read_attributes(
                &mut t,
                alloc::vec![ranged("1"), ranged("1:2"), ranged("9"), ranged("7:5")],
                TimestampsToReturn::Neither,
            )
            .expect("read ranges");
        assert_eq!(
            values[0].value.as_ref().expect("v").value,
            alloc::vec![VariantValue::Int32(11)]
        );
        assert_eq!(
            values[1].value.as_ref().expect("v").value,
            alloc::vec![VariantValue::Int32(11), VariantValue::Int32(12)]
        );
        // Part 4 §7.27: a lower bound past the end has no data.
        assert_eq!(
            values[2].status,
            Some(crate::status::BAD_INDEX_RANGE_NO_DATA)
        );
        // "7:5" violates "the first integer shall always have a lower value".
        assert_eq!(
            values[3].status,
            Some(crate::status::BAD_INDEX_RANGE_INVALID)
        );

        // A ranged write patches only the selected elements.
        let results = client
            .write_attributes(
                &mut t,
                alloc::vec![WriteValue {
                    node_id: node.clone(),
                    attribute_id: a::VALUE,
                    index_range: String::from("1:2"),
                    value: DataValue::new_value(
                        Variant {
                            array_dimensions: alloc::vec![2],
                            value: alloc::vec![VariantValue::Int32(21), VariantValue::Int32(22)],
                        },
                        0,
                        0,
                    ),
                }],
            )
            .expect("ranged write");
        assert_eq!(results, alloc::vec![0]);
        let whole = client.read_attribute(&mut t, node, a::VALUE).expect("read");
        assert_eq!(
            whole.value.expect("v").value,
            alloc::vec![
                VariantValue::Int32(10),
                VariantValue::Int32(21),
                VariantValue::Int32(22),
                VariantValue::Int32(13),
            ]
        );
    }

    #[test]
    fn e2e_loopback_read_honours_timestamps_to_return_and_max_age() {
        use crate::attributes::attribute_ids as a;
        let mut server = demo_server();
        let mut client = Client::new();
        let mut t = LoopbackTransport::new(&mut server);
        client.connect(&mut t, "opc.tcp://x").expect("connect");
        let node = NodeId::numeric(1, 1);

        let source = client
            .read_attributes(
                &mut t,
                alloc::vec![read_value_id(node.clone(), a::VALUE)],
                TimestampsToReturn::Source,
            )
            .expect("read");
        assert!(source[0].source_timestamp.is_some());
        assert!(source[0].server_timestamp.is_none());

        let neither = client
            .read_attributes(
                &mut t,
                alloc::vec![read_value_id(node.clone(), a::VALUE)],
                TimestampsToReturn::Neither,
            )
            .expect("read");
        assert!(neither[0].source_timestamp.is_none());
        assert!(neither[0].server_timestamp.is_none());

        // Part 4 §5.11.2.2: "Negative values are invalid for maxAge."
        let req = ServiceRequest::Read(ReadRequest {
            request_header: client.header(),
            max_age: -1.0,
            timestamps_to_return: 0,
            nodes_to_read: alloc::vec![read_value_id(node.clone(), a::VALUE)],
        });
        assert_eq!(
            client.call_service(&mut t, req),
            Err(ClientError::ServiceFault(
                crate::status::BAD_MAX_AGE_INVALID
            ))
        );
        // §5.11.2.3 Bad_TimestampsToReturnInvalid for the INVALID (4) value.
        let req = ServiceRequest::Read(ReadRequest {
            request_header: client.header(),
            max_age: 0.0,
            timestamps_to_return: 4,
            nodes_to_read: alloc::vec![read_value_id(node, a::VALUE)],
        });
        assert_eq!(
            client.call_service(&mut t, req),
            Err(ClientError::ServiceFault(
                crate::status::BAD_TIMESTAMPS_TO_RETURN_INVALID
            ))
        );
    }

    // -------------------------------------------------------------------
    // View Service Set: BrowseNext with real continuation points (§5.9.3).
    // -------------------------------------------------------------------

    fn browse_graph_server() -> (Server, NodeId, Vec<NodeId>) {
        use crate::address_space::{NodeClass, NodeMeta, reference_types};
        use zerodds_opcua_gateway::types::QualifiedName;
        let mut space = AddressSpace::new();
        let root = NodeId::numeric(0, 85);
        space.add_node(NodeMeta::new(
            root.clone(),
            NodeClass::Object,
            QualifiedName {
                namespace_index: 0,
                name: String::from("Objects"),
            },
        ));
        let mut children = Vec::new();
        for i in 1..=5u32 {
            let child = NodeId::numeric(1, i);
            space.add_node(NodeMeta::new(
                child.clone(),
                NodeClass::Variable,
                QualifiedName {
                    namespace_index: 1,
                    name: alloc::format!("Child{i}"),
                },
            ));
            space.add_reference(root.clone(), reference_types::ORGANIZES, child.clone());
            children.push(child);
        }
        (Server::new("opc.tcp://x", space), root, children)
    }

    #[test]
    fn e2e_loopback_browse_next_walks_continuation_points() {
        let (mut server, root, children) = browse_graph_server();
        let mut client = Client::new();
        let mut t = LoopbackTransport::new(&mut server);
        client.connect(&mut t, "opc.tcp://x").expect("connect");

        // requestedMaxReferencesPerNode = 2 → the first batch is capped and a
        // continuation point is returned (Part 4 §5.9.2.2).
        let (first, cp) = client
            .browse_limited(&mut t, root.clone(), 2)
            .expect("browse");
        assert_eq!(first.len(), 2);
        assert!(!cp.is_empty());

        // BrowseNext returns the rest (Part 4 §5.9.3).
        let (rest, cp2) = client.browse_next(&mut t, cp, false).expect("browse next");
        assert_eq!(rest.len(), 3);
        assert!(cp2.is_empty());
        let seen: Vec<NodeId> = first
            .iter()
            .chain(rest.iter())
            .map(|r| r.node_id.node_id.clone())
            .collect();
        assert_eq!(seen, children);

        // A continuation point that was consumed is no longer valid.
        let (_, cp3) = client
            .browse_limited(&mut t, root.clone(), 2)
            .expect("browse");
        let released = client
            .browse_next(&mut t, cp3.clone(), true)
            .expect("release");
        assert!(released.0.is_empty() && released.1.is_empty());
        assert_eq!(
            client.browse_next(&mut t, cp3, false),
            Err(ClientError::ServiceFault(
                crate::status::BAD_CONTINUATION_POINT_INVALID
            ))
        );

        // browse_all follows the points to the end.
        let all = client.browse_all(&mut t, root, 1).expect("browse all");
        assert_eq!(all.len(), 5);
    }

    // -------------------------------------------------------------------
    // UserIdentityTokens (Part 4 §5.7.3 / §7.40).
    // -------------------------------------------------------------------

    #[test]
    fn e2e_loopback_user_name_identity_token() {
        use crate::identity::UserStore;
        let mut server = demo_server();
        let mut store = UserStore::anonymous_only();
        store.set_allow_anonymous(false);
        store.add_user("user", "alice", "s3cret");
        server.set_user_store(store);

        // The endpoint publishes the policies the Server accepts (§7.41).
        {
            let mut probe = Client::new();
            let mut t = LoopbackTransport::new(&mut server);
            probe.open_channel(&mut t, "opc.tcp://x").expect("channel");
            let eps = probe
                .get_endpoints(&mut t, "opc.tcp://x")
                .expect("endpoints");
            let types: Vec<i32> = eps[0]
                .user_identity_tokens
                .iter()
                .map(|p| match p.token_type {
                    crate::identity::UserTokenType::Anonymous => 0,
                    crate::identity::UserTokenType::UserName => 1,
                    crate::identity::UserTokenType::Certificate => 2,
                    crate::identity::UserTokenType::IssuedToken => 3,
                })
                .collect();
            assert!(types.contains(&1), "USERNAME policy not published");
        }

        // Anonymous is refused (§5.7.3.3 Bad_IdentityTokenRejected).
        {
            let mut client = Client::new();
            let mut t = LoopbackTransport::new(&mut server);
            assert_eq!(
                client.connect(&mut t, "opc.tcp://x"),
                Err(ClientError::ServiceFault(
                    crate::status::BAD_IDENTITY_TOKEN_REJECTED
                ))
            );
        }
        // The wrong password is Bad_UserAccessDenied.
        {
            let mut client = Client::new();
            client.set_identity(UserIdentityToken::user_name("user", "alice", "wrong"));
            let mut t = LoopbackTransport::new(&mut server);
            assert_eq!(
                client.connect(&mut t, "opc.tcp://x"),
                Err(ClientError::ServiceFault(
                    crate::status::BAD_USER_ACCESS_DENIED
                ))
            );
        }
        // The right one activates and the session works.
        {
            let mut client = Client::new();
            client.set_identity(UserIdentityToken::user_name("user", "alice", "s3cret"));
            let mut t = LoopbackTransport::new(&mut server);
            client.connect(&mut t, "opc.tcp://x").expect("connect");
            let vals = client
                .read_values(&mut t, &[NodeId::numeric(1, 1)])
                .expect("read");
            assert_eq!(
                vals[0].value,
                Some(Variant::scalar(VariantValue::Int32(4242)))
            );
        }
    }

    #[test]
    fn e2e_loopback_x509_and_issued_identity_tokens_and_user_change() {
        use crate::identity::UserStore;
        let mut server = demo_server();
        let mut store = UserStore::anonymous_only();
        store
            .add_certificate("cert", b"user-cert-der".to_vec())
            .add_issued_token("jwt", b"eyJhbGciOiJub25lIn0".to_vec())
            .add_user("user", "bob", "hunter2");
        server.set_user_store(store);

        let mut client = Client::new();
        client.set_identity(UserIdentityToken::x509("cert", b"user-cert-der".to_vec()));
        let mut t = LoopbackTransport::new(&mut server);
        client.connect(&mut t, "opc.tcp://x").expect("x509 connect");

        // §5.7.3.1: the identity of a running Session can be changed.
        client
            .change_user(
                &mut t,
                UserIdentityToken::issued("jwt", b"eyJhbGciOiJub25lIn0".to_vec()),
            )
            .expect("issued token");
        client
            .change_user(
                &mut t,
                UserIdentityToken::user_name("user", "bob", "hunter2"),
            )
            .expect("username");
        // A forged issued token is rejected.
        assert_eq!(
            client.change_user(&mut t, UserIdentityToken::issued("jwt", b"forged".to_vec())),
            Err(ClientError::ServiceFault(
                crate::status::BAD_IDENTITY_TOKEN_REJECTED
            ))
        );
        // A certificate the Server does not trust is Bad_UserAccessDenied.
        assert_eq!(
            client.change_user(
                &mut t,
                UserIdentityToken::x509("cert", b"other-cert".to_vec())
            ),
            Err(ClientError::ServiceFault(
                crate::status::BAD_USER_ACCESS_DENIED
            ))
        );
    }

    // -------------------------------------------------------------------
    // MonitoredItem + Subscription Service Sets in full (§5.13 / §5.14).
    // -------------------------------------------------------------------

    #[test]
    fn e2e_loopback_modify_subscription_and_monitored_items() {
        let mut server = demo_server();
        let mut client = Client::new();
        let mut t = LoopbackTransport::new(&mut server);
        client.connect(&mut t, "opc.tcp://x").expect("connect");

        let sub = client.create_subscription(&mut t).expect("subscription");
        let created = client
            .create_monitored_items(&mut t, sub, &[(NodeId::numeric(1, 1), 77)])
            .expect("create items");
        let item = created[0].monitored_item_id;

        // ModifySubscription (§5.14.3) revises what it was asked for.
        let (interval, lifetime, keep_alive) = client
            .modify_subscription(&mut t, sub, 250.0)
            .expect("modify subscription");
        assert!((interval - 250.0).abs() < f64::EPSILON);
        assert!(lifetime >= keep_alive * 3);

        // ModifyMonitoredItems (§5.13.3) revises the item parameters.
        let modified = client
            .modify_monitored_items(
                &mut t,
                sub,
                &[(
                    item,
                    MonitoringParameters {
                        client_handle: 99,
                        sampling_interval: 500.0,
                        filter: null_filter(),
                        queue_size: 4,
                        discard_oldest: true,
                    },
                )],
            )
            .expect("modify items");
        assert_eq!(modified[0].status_code, 0);
        assert!((modified[0].revised_sampling_interval - 500.0).abs() < f64::EPSILON);
        assert_eq!(modified[0].revised_queue_size, 4);
        // An unknown item id is Bad_MonitoredItemIdInvalid (§5.13.3.4).
        let unknown = client
            .modify_monitored_items(
                &mut t,
                sub,
                &[(
                    9999,
                    MonitoringParameters {
                        client_handle: 1,
                        sampling_interval: 100.0,
                        filter: null_filter(),
                        queue_size: 1,
                        discard_oldest: true,
                    },
                )],
            )
            .expect("modify items");
        assert_eq!(
            unknown[0].status_code,
            crate::status::BAD_MONITORED_ITEM_ID_INVALID
        );

        // The new client handle reaches the Publish response.
        let out = client.publish_acknowledging(&mut t, &[]).expect("publish");
        assert_eq!(out.notifications[0].client_handle, 99);
    }

    #[test]
    fn e2e_loopback_set_monitoring_mode_and_delete_monitored_items() {
        let mut server = demo_server();
        let mut client = Client::new();
        let mut t = LoopbackTransport::new(&mut server);
        client.connect(&mut t, "opc.tcp://x").expect("connect");

        let sub = client.create_subscription(&mut t).expect("subscription");
        let created = client
            .create_monitored_items(&mut t, sub, &[(NodeId::numeric(1, 1), 77)])
            .expect("create items");
        let item = created[0].monitored_item_id;

        // DISABLED stops reporting and drops the queue (§5.13.4.1).
        assert_eq!(
            client
                .set_monitoring_mode(&mut t, sub, MonitoringMode::Disabled, &[item])
                .expect("set mode"),
            alloc::vec![0]
        );
        let out = client.publish_acknowledging(&mut t, &[]).expect("publish");
        assert!(out.notifications.is_empty());

        // REPORTING resumes and the first sample after it is reported again.
        assert_eq!(
            client
                .set_monitoring_mode(&mut t, sub, MonitoringMode::Reporting, &[item])
                .expect("set mode"),
            alloc::vec![0]
        );
        let out = client.publish_acknowledging(&mut t, &[]).expect("publish");
        assert_eq!(out.notifications.len(), 1);

        // An unknown item id is Bad_MonitoredItemIdInvalid (§5.13.4.4).
        assert_eq!(
            client
                .set_monitoring_mode(&mut t, sub, MonitoringMode::Sampling, &[4242])
                .expect("set mode"),
            alloc::vec![crate::status::BAD_MONITORED_ITEM_ID_INVALID]
        );

        // DeleteMonitoredItems (§5.13.6): the item goes, a second delete fails.
        assert_eq!(
            client
                .delete_monitored_items(&mut t, sub, &[item])
                .expect("delete"),
            alloc::vec![0]
        );
        assert_eq!(
            client
                .delete_monitored_items(&mut t, sub, &[item])
                .expect("delete"),
            alloc::vec![crate::status::BAD_MONITORED_ITEM_ID_INVALID]
        );
    }

    #[test]
    fn e2e_loopback_set_triggering_releases_a_sampling_item() {
        let mut server = demo_server();
        let mut client = Client::new();
        let mut t = LoopbackTransport::new(&mut server);
        client.connect(&mut t, "opc.tcp://x").expect("connect");

        let sub = client.create_subscription(&mut t).expect("subscription");
        let created = client
            .create_monitored_items(
                &mut t,
                sub,
                &[(NodeId::numeric(1, 1), 11), (NodeId::numeric(1, 2), 22)],
            )
            .expect("create items");
        let (trigger, reported) = (created[0].monitored_item_id, created[1].monitored_item_id);

        // The second item only samples: on its own it never reports (§7.23).
        assert_eq!(
            client
                .set_monitoring_mode(&mut t, sub, MonitoringMode::Sampling, &[reported])
                .expect("set mode"),
            alloc::vec![0]
        );
        let out = client.publish_acknowledging(&mut t, &[]).expect("publish");
        assert_eq!(out.notifications.len(), 1);
        assert_eq!(out.notifications[0].client_handle, 11);

        // Linked to the triggering item, its queue is released too (§5.13.1.6).
        let (add, remove) = client
            .set_triggering(&mut t, sub, trigger, &[reported], &[])
            .expect("set triggering");
        assert_eq!(add, alloc::vec![0]);
        assert!(remove.is_empty());
        client
            .write_values(
                &mut t,
                &[
                    (
                        NodeId::numeric(1, 1),
                        DataValue::new_value(Variant::scalar(VariantValue::Int32(1)), 0, 0),
                    ),
                    (
                        NodeId::numeric(1, 2),
                        DataValue::new_value(Variant::scalar(VariantValue::Int32(2)), 0, 0),
                    ),
                ],
            )
            .expect("write");
        let out = client.publish_acknowledging(&mut t, &[]).expect("publish");
        let handles: Vec<u32> = out.notifications.iter().map(|n| n.client_handle).collect();
        assert!(
            handles.contains(&11) && handles.contains(&22),
            "{handles:?}"
        );

        // Removing the link is processed and reported (§5.13.5.2).
        let (add, remove) = client
            .set_triggering(&mut t, sub, trigger, &[], &[reported])
            .expect("set triggering");
        assert!(add.is_empty());
        assert_eq!(remove, alloc::vec![0]);
    }

    #[test]
    fn e2e_loopback_republish_and_acknowledgements() {
        let mut server = demo_server();
        let mut client = Client::new();
        let mut t = LoopbackTransport::new(&mut server);
        client.connect(&mut t, "opc.tcp://x").expect("connect");

        let sub = client.create_subscription(&mut t).expect("subscription");
        client
            .create_monitored_items(&mut t, sub, &[(NodeId::numeric(1, 1), 77)])
            .expect("create items");

        let first = client.publish_acknowledging(&mut t, &[]).expect("publish");
        assert_eq!(first.notifications.len(), 1);
        // §5.14.5.2: the message stays available for retransmission until it is
        // acknowledged.
        assert_eq!(
            first.available_sequence_numbers,
            alloc::vec![first.sequence_number]
        );

        // Republish returns exactly that message again (§5.14.6).
        let again = client
            .republish(&mut t, sub, first.sequence_number)
            .expect("republish");
        assert_eq!(again, first.notifications);

        // Acknowledging removes it from the queue; Republish then fails.
        let acked = client
            .publish_acknowledging(&mut t, &[(sub, first.sequence_number)])
            .expect("publish 2");
        assert_eq!(acked.acknowledge_results, alloc::vec![0]);
        assert!(
            !acked
                .available_sequence_numbers
                .contains(&first.sequence_number)
        );
        assert_eq!(
            client.republish(&mut t, sub, first.sequence_number),
            Err(ClientError::ServiceFault(
                crate::status::BAD_MESSAGE_NOT_AVAILABLE
            ))
        );
        // Acknowledging an unknown sequence number is reported per entry.
        let stale = client
            .publish_acknowledging(&mut t, &[(sub, 9999)])
            .expect("publish 3");
        assert_eq!(
            stale.acknowledge_results,
            alloc::vec![crate::status::BAD_SEQUENCE_NUMBER_UNKNOWN]
        );
    }

    #[test]
    fn e2e_loopback_transfer_subscriptions_between_sessions() {
        let mut server = demo_server();
        let mut first = Client::new();
        let mut second = Client::new();
        let mut t = LoopbackTransport::new(&mut server);
        first.connect(&mut t, "opc.tcp://x").expect("connect 1");
        let sub = first.create_subscription(&mut t).expect("subscription");
        first
            .create_monitored_items(&mut t, sub, &[(NodeId::numeric(1, 1), 77)])
            .expect("create items");
        let out = first.publish_acknowledging(&mut t, &[]).expect("publish");
        assert_eq!(out.notifications.len(), 1);

        second.connect(&mut t, "opc.tcp://x").expect("connect 2");
        // §5.14.7.4: a Session cannot transfer a Subscription it already owns.
        let own = first
            .transfer_subscriptions(&mut t, &[sub], true)
            .expect("transfer to self");
        assert_eq!(own[0].status_code, crate::status::BAD_NOTHING_TO_DO);

        let moved = second
            .transfer_subscriptions(&mut t, &[sub], true)
            .expect("transfer");
        assert_eq!(moved[0].status_code, 0);
        assert_eq!(
            moved[0].available_sequence_numbers,
            out.available_sequence_numbers
        );

        // §5.14.7.1: the old Session is told the Subscription was transferred.
        let notice = first
            .publish_acknowledging(&mut t, &[])
            .expect("publish old");
        assert_eq!(
            notice.status_change,
            Some(crate::status::GOOD_SUBSCRIPTION_TRANSFERRED)
        );

        // §5.14.7.2 sendInitialValues: the new Session's first Publish repeats
        // the current value even though nothing changed.
        let taken = second
            .publish_acknowledging(&mut t, &[])
            .expect("publish new");
        assert_eq!(taken.subscription_id, sub);
        assert_eq!(taken.notifications.len(), 1);

        // An unknown subscription is Bad_SubscriptionIdInvalid.
        let unknown = second
            .transfer_subscriptions(&mut t, &[4242], false)
            .expect("transfer");
        assert_eq!(
            unknown[0].status_code,
            crate::status::BAD_SUBSCRIPTION_ID_INVALID
        );
    }

    #[test]
    fn e2e_loopback_session_must_be_activated_and_close_deletes_subscriptions() {
        let mut server = demo_server();
        let mut client = Client::new();
        let mut t = LoopbackTransport::new(&mut server);
        client.connect(&mut t, "opc.tcp://x").expect("connect");
        let sub = client.create_subscription(&mut t).expect("subscription");
        client.close_session(&mut t, true).expect("close");

        // §5.7.3.1: after the Session is gone its token no longer identifies
        // one, so a further Service is Bad_SessionIdInvalid.
        client.connected = true; // keep the client willing to send
        assert_eq!(
            client.delete_subscriptions(&mut t, &[sub]),
            Err(ClientError::ServiceFault(
                crate::status::BAD_SESSION_ID_INVALID
            ))
        );
    }

    // -------------------------------------------------------------------
    // SecureChannel: renewal (§5.6.2) and Message chunking (Part 6 §6.7.1).
    // -------------------------------------------------------------------

    #[test]
    fn e2e_loopback_secure_channel_renew_keeps_the_channel_working() {
        let mut server = demo_server();
        let mut client = Client::new();
        let mut t = LoopbackTransport::new(&mut server);
        client.connect(&mut t, "opc.tcp://x").expect("connect");
        // Part 4 §5.6.2: RENEW creates a new SecurityToken for the existing
        // SecureChannel; the session and its services survive it.
        client.renew_secure_channel(&mut t).expect("renew");
        let vals = client
            .read_values(&mut t, &[NodeId::numeric(1, 1)])
            .expect("read after renew");
        assert_eq!(
            vals[0].value,
            Some(Variant::scalar(VariantValue::Int32(4242)))
        );
    }

    #[test]
    fn e2e_loopback_message_larger_than_the_buffer_is_chunked() {
        // Part 6 §6.7.1: a Message larger than the negotiated buffer is broken
        // into several MessageChunks and reassembled by the receiver.
        let mut space = AddressSpace::new();
        let node = NodeId::numeric(1, 42);
        let big = String::from_utf8(alloc::vec![b'x'; 40_000]).expect("utf8");
        space.set_value(
            node.clone(),
            DataValue::new_value(Variant::scalar(VariantValue::String(big.clone())), 0, 0),
        );
        let mut server = Server::new("opc.tcp://x", space);
        let mut client = Client::new();
        let mut t = LoopbackTransport::new(&mut server);
        client.connect(&mut t, "opc.tcp://x").expect("connect");

        // Reading it back needs a chunked response...
        let vals = client.read_values(&mut t, &[node.clone()]).expect("read");
        assert_eq!(
            vals[0].value,
            Some(Variant::scalar(VariantValue::String(big.clone())))
        );

        // ...and writing an even larger value needs a chunked request.
        let bigger = String::from_utf8(alloc::vec![b'y'; 60_000]).expect("utf8");
        let results = client
            .write_values(
                &mut t,
                &[(
                    node.clone(),
                    DataValue::new_value(
                        Variant::scalar(VariantValue::String(bigger.clone())),
                        0,
                        0,
                    ),
                )],
            )
            .expect("chunked write");
        assert_eq!(results, alloc::vec![0]);
        let back = client.read_values(&mut t, &[node]).expect("read back");
        assert_eq!(
            back[0].value,
            Some(Variant::scalar(VariantValue::String(bigger)))
        );
    }

    #[cfg(feature = "std")]
    #[test]
    fn e2e_tcp_message_larger_than_the_buffer_is_chunked() {
        use std::net::TcpListener;
        use std::thread;

        let big = String::from_utf8(alloc::vec![b'z'; 50_000]).expect("utf8");
        let node = NodeId::numeric(1, 43);
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr").to_string();
        let served = big.clone();
        let served_node = node.clone();
        let server_thread = thread::spawn(move || {
            let mut space = AddressSpace::new();
            space.set_value(
                served_node,
                DataValue::new_value(Variant::scalar(VariantValue::String(served)), 0, 0),
            );
            let mut server = Server::new("opc.tcp://x", space);
            let (stream, _) = listener.accept().expect("accept");
            serve_connection(&mut server, stream).expect("serve");
        });

        let mut transport = TcpTransport::connect(&addr).expect("connect tcp");
        let mut client = Client::new();
        let url = alloc::format!("opc.tcp://{addr}");
        client.connect(&mut transport, &url).expect("connect");
        let vals = client.read_values(&mut transport, &[node]).expect("read");
        assert_eq!(
            vals[0].value,
            Some(Variant::scalar(VariantValue::String(big)))
        );
        drop(transport);
        server_thread.join().expect("join");
    }

    /// The RSA-2048 key pair the secured end-to-end tests share. Generating a
    /// 2048-bit key in a debug build costs minutes; the handshake under test
    /// does not care which key it runs on, so one pair is generated per test
    /// binary and reused.
    #[cfg(all(feature = "crypto", feature = "std"))]
    fn shared_rsa_keys() -> &'static (
        zerodds_opcua_uacp::crypto::RsaPrivateKey,
        zerodds_opcua_uacp::crypto::RsaPrivateKey,
    ) {
        use rand::rngs::OsRng;
        use std::sync::OnceLock;
        use zerodds_opcua_uacp::crypto::RsaPrivateKey;
        static KEYS: OnceLock<(RsaPrivateKey, RsaPrivateKey)> = OnceLock::new();
        KEYS.get_or_init(|| {
            let mut kg = OsRng;
            (
                RsaPrivateKey::new(&mut kg, 2048).expect("client key"),
                RsaPrivateKey::new(&mut kg, 2048).expect("server key"),
            )
        })
    }

    #[cfg(feature = "crypto")]
    #[test]
    fn e2e_secured_loopback_sign_and_encrypt() {
        use crate::server::ServerSecurity;
        use rand::rngs::OsRng;
        use zerodds_opcua_uacp::crypto::{SecuredMode, SecurityPolicy};

        // RSA key pairs, public keys exchanged out of band (trust).
        let (client_key, server_key) = shared_rsa_keys();
        let (client_key, server_key) = (client_key.clone(), server_key.clone());
        let client_cert = b"client-cert-der".to_vec();
        let server_cert = b"server-cert-der".to_vec();
        let client_pub = client_key.to_public_key();
        let server_pub = server_key.to_public_key();
        let policy = SecurityPolicy::Basic256Sha256;
        let mode = SecuredMode::SignAndEncrypt;

        let mut server = demo_server();
        server.set_security(ServerSecurity {
            policy,
            mode,
            private_key: server_key.clone(),
            certificate: server_cert.clone(),
            client_certificate: client_cert.clone(),
            client_public_key: client_pub,
            rng: alloc::boxed::Box::new(OsRng),
        });

        let mut client = Client::new();
        client.set_security(super::ClientSecurity {
            policy,
            mode,
            private_key: client_key,
            certificate: client_cert,
            server_certificate: server_cert,
            server_public_key: server_pub,
            rng: alloc::boxed::Box::new(OsRng),
        });

        let mut t = LoopbackTransport::new(&mut server);
        client
            .connect(&mut t, "opc.tcp://secure")
            .expect("secured connect");
        assert_ne!(*client.session_id(), NodeId::numeric(0, 0));

        // Secured Read.
        let vals = client
            .read_values(&mut t, &[NodeId::numeric(1, 1)])
            .expect("secured read");
        assert_eq!(
            vals[0].value,
            Some(Variant::scalar(VariantValue::Int32(4242)))
        );

        // Secured Call (double 21 → 42).
        let res = client
            .call_method(
                &mut t,
                NodeId::numeric(0, 0),
                NodeId::numeric(1, 100),
                alloc::vec![Variant::scalar(VariantValue::Int32(21))],
            )
            .expect("secured call");
        assert_eq!(
            res.output_arguments[0],
            Variant::scalar(VariantValue::Int32(42))
        );

        // Secured Write then read back.
        let w = client
            .write_values(
                &mut t,
                &[(
                    NodeId::numeric(1, 2),
                    DataValue::new_value(Variant::scalar(VariantValue::Int32(9)), 0, 0),
                )],
            )
            .expect("secured write");
        assert_eq!(w, alloc::vec![0]);
        let back = client
            .read_values(&mut t, &[NodeId::numeric(1, 2)])
            .expect("secured read back");
        assert_eq!(back[0].value, Some(Variant::scalar(VariantValue::Int32(9))));
    }

    #[cfg(all(feature = "crypto", feature = "std"))]
    #[test]
    fn e2e_secured_loopback_sign_only_and_renew() {
        use crate::server::ServerSecurity;
        use rand::rngs::OsRng;
        use zerodds_opcua_uacp::crypto::{SecuredMode, SecurityPolicy};

        // Part 6 §6.7.2.1 Figure 11: in `Sign` mode the chunk stays plaintext
        // and carries an authentication code; only `SignAndEncrypt` encrypts.
        let (client_key, server_key) = shared_rsa_keys();
        let (client_key, server_key) = (client_key.clone(), server_key.clone());
        let client_cert = b"client-cert-der".to_vec();
        let server_cert = b"server-cert-der".to_vec();
        let client_pub = client_key.to_public_key();
        let server_pub = server_key.to_public_key();
        let policy = SecurityPolicy::Aes256Sha256RsaPss;
        let mode = SecuredMode::Sign;

        let mut server = demo_server();
        server.set_security(ServerSecurity {
            policy,
            mode,
            private_key: server_key,
            certificate: server_cert.clone(),
            client_certificate: client_cert.clone(),
            client_public_key: client_pub,
            rng: alloc::boxed::Box::new(OsRng),
        });

        let mut client = Client::new();
        client.set_security(super::ClientSecurity {
            policy,
            mode,
            private_key: client_key,
            certificate: client_cert,
            server_certificate: server_cert,
            server_public_key: server_pub,
            rng: alloc::boxed::Box::new(OsRng),
        });

        let mut t = LoopbackTransport::new(&mut server);
        client
            .connect(&mut t, "opc.tcp://signed")
            .expect("Sign-mode connect");
        let vals = client
            .read_values(&mut t, &[NodeId::numeric(1, 1)])
            .expect("signed read");
        assert_eq!(
            vals[0].value,
            Some(Variant::scalar(VariantValue::Int32(4242)))
        );

        // The endpoint the Server publishes now names the secured policy and
        // MessageSecurityMode it actually runs (Part 4 §7.14).
        let eps = client
            .get_endpoints(&mut t, "opc.tcp://signed")
            .expect("endpoints");
        assert_eq!(eps[0].security_policy_uri, policy.uri());
        assert_eq!(eps[0].security_mode, MessageSecurityMode::Sign);

        // Part 4 §5.6.2 RENEW over a secured channel: new keys, same channel.
        client.renew_secure_channel(&mut t).expect("secured renew");
        let after = client
            .read_values(&mut t, &[NodeId::numeric(1, 1)])
            .expect("read after secured renew");
        assert_eq!(
            after[0].value,
            Some(Variant::scalar(VariantValue::Int32(4242)))
        );
    }

    #[cfg(feature = "std")]
    #[test]
    fn e2e_tcp_connect_read_call() {
        use std::net::TcpListener;
        use std::thread;

        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr").to_string();

        let server_thread = thread::spawn(move || {
            let mut server = demo_server();
            let (stream, _) = listener.accept().expect("accept");
            serve_connection(&mut server, stream).expect("serve");
        });

        let mut transport = TcpTransport::connect(&addr).expect("connect tcp");
        let mut client = Client::new();
        let url = alloc::format!("opc.tcp://{addr}");
        client.connect(&mut transport, &url).expect("connect");
        let vals = client
            .read_values(&mut transport, &[NodeId::numeric(1, 1)])
            .expect("read");
        assert_eq!(
            vals[0].value,
            Some(Variant::scalar(VariantValue::Int32(4242)))
        );
        let res = client
            .call_method(
                &mut transport,
                NodeId::numeric(0, 0),
                NodeId::numeric(1, 100),
                alloc::vec![Variant::scalar(VariantValue::Int32(50))],
            )
            .expect("call");
        assert_eq!(
            res.output_arguments[0],
            Variant::scalar(VariantValue::Int32(100))
        );

        // Trigger a clean close so the server thread's serve loop returns.
        drop(transport);
        server_thread.join().expect("join");
    }
}
