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
use zerodds_opcua_uacp::connection::{HelloMessage, MessageHeader, MessageType};
use zerodds_opcua_uacp::securechannel::{
    OpenSecureChannelRequest, OpenSecureChannelResponse, RequestHeader, SecureChannel,
    SecurityTokenRequestType, null_extension_object, parse_chunk,
};

use crate::services::{
    ATTRIBUTE_VALUE, ActivateSessionRequest, BrowseDescription, BrowseRequest, CallMethodRequest,
    CallMethodResult, CallRequest, CreateMonitoredItemsRequest, CreateSessionRequest,
    CreateSubscriptionRequest, DataChangeNotification, DeleteSubscriptionsRequest,
    FindServersRequest, GetEndpointsRequest, MonitoredItemCreateRequest, MonitoredItemCreateResult,
    MonitoredItemNotification, MonitoringParameters, PublishRequest, ReadRequest, ReadValueId,
    ReferenceDescription, ServiceRequest, ServiceResponse, SignatureData, ViewDescription,
    WriteRequest, WriteValue, null_filter,
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
/// [`set_security`](Self::set_security) (feature `crypto`) for a secured
/// SecurityPolicy.
pub struct Client {
    channel: SecureChannel,
    auth_token: NodeId,
    session_id: NodeId,
    request_handle: u32,
    next_request_id: u32,
    channel_open: bool,
    connected: bool,
    #[cfg(feature = "crypto")]
    security: Option<ClientSecurity>,
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
            #[cfg(feature = "crypto")]
            security: None,
        }
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
        let handle = self.next_handle();
        RequestHeader::new(token, handle)
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

        // 2. OpenSecureChannel — secured if a SecurityPolicy is configured.
        let secured = {
            #[cfg(feature = "crypto")]
            {
                if self.security.is_some() {
                    self.open_secure_channel_secured(transport)?;
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
            self.open_secure_channel_plain(transport)?;
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

        // 4. ActivateSession.
        let activate = ServiceRequest::ActivateSession(ActivateSessionRequest {
            request_header: self.header(),
            client_signature: SignatureData::default(),
            locale_ids: Vec::new(),
            user_identity_token: null_extension_object(),
            user_token_signature: SignatureData::default(),
        });
        let resp = self.call_service(transport, activate)?;
        if !matches!(resp, ServiceResponse::ActivateSession(_)) {
            return Err(ClientError::Protocol("expected ActivateSessionResponse"));
        }
        self.connected = true;
        Ok(())
    }

    /// The plaintext (SecurityMode `None`) OpenSecureChannel exchange.
    fn open_secure_channel_plain<T: Transport>(
        &mut self,
        transport: &mut T,
    ) -> Result<(), ClientError> {
        let open = OpenSecureChannelRequest {
            request_header: RequestHeader::new(NodeId::numeric(0, 0), self.next_handle()),
            client_protocol_version: 0,
            request_type: SecurityTokenRequestType::Issue,
            security_mode: MessageSecurityMode::None,
            client_nonce: Vec::new(),
            requested_lifetime: 3_600_000,
        };
        let rid = self.next_request_id();
        let open_resp_bytes = transport.request(&self.channel.open_chunk(rid, &open.encode()?)?)?;
        let chunk = parse_chunk(&open_resp_bytes)?;
        let mut br = UaReader::new(&chunk.body);
        let _type_id = NodeId::decode(&mut br)?;
        let open_resp = OpenSecureChannelResponse::decode_body(&mut br)?;
        // Adopt the server-issued channel + token for symmetric messages.
        self.channel = SecureChannel::new(
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
    ) -> Result<(), ClientError> {
        let handle = self.next_handle();
        let request_id = self.next_request_id();
        let seq = self.channel.next_send_sequence();

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
                request_header: RequestHeader::new(NodeId::numeric(0, 0), handle),
                client_protocol_version: 0,
                request_type: SecurityTokenRequestType::Issue,
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
            opn_bytes = build_asymmetric_chunk(&mut rng_ref, &ctx, 0, seq, request_id, &body)
                .map_err(|_| ClientError::Protocol("secured OPN build failed"))?;
        }

        let resp_bytes = transport.request(&opn_bytes)?;

        let open_resp = {
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
            let mut br = UaReader::new(&opened.body);
            let _type_id = NodeId::decode(&mut br)?;
            OpenSecureChannelResponse::decode_body(&mut br)?
        };

        let send_keys = derive_keys(policy, &open_resp.server_nonce, &client_nonce)
            .map_err(|_| ClientError::Protocol("key derivation failed"))?;
        let recv_keys = derive_keys(policy, &client_nonce, &open_resp.server_nonce)
            .map_err(|_| ClientError::Protocol("key derivation failed"))?;
        let mut channel = SecureChannel::new(
            open_resp.security_token.channel_id,
            open_resp.security_token.token_id,
        );
        channel.install_security(SecuritySession {
            policy,
            mode,
            send_keys,
            recv_keys,
        });
        self.channel = channel;
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

    /// Encodes a service request into a MSG chunk, sends it, and decodes the
    /// service response from the MSG response.
    fn call_service<T: Transport>(
        &mut self,
        transport: &mut T,
        req: ServiceRequest,
    ) -> Result<ServiceResponse, ClientError> {
        let rid = self.next_request_id();
        let chunk = self.channel.message_chunk(rid, &req.encode()?)?;
        let resp_bytes = transport.request(&chunk)?;
        let parsed = self.channel.open_incoming(&resp_bytes)?;
        ServiceResponse::decode(&parsed.body).map_err(ClientError::from)
    }
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

    impl super::Transport for TcpTransport {
        fn request(&mut self, message: &[u8]) -> Result<Vec<u8>, ClientError> {
            self.stream
                .write_all(message)
                .map_err(|e| ClientError::Io(e.to_string()))?;
            read_message(&mut self.stream)
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
    use zerodds_opcua_gateway::data_value::VariantValue;

    fn demo_server() -> Server {
        let mut space = AddressSpace::new();
        space.set_value(
            NodeId::numeric(1, 1),
            DataValue::new_value(Variant::scalar(VariantValue::Int32(4242)), 0, 0),
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
        // After deletion, a Publish has no enabled subscription.
        let (sid2, _) = client.publish(&mut t).expect("publish 4");
        assert_eq!(sid2, 0);
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
        use zerodds_opcua_gateway::types::{LocalizedText, QualifiedName};

        let mut space = AddressSpace::new();
        let objects = NodeId::numeric(0, 85);
        let boiler = NodeId::numeric(1, 1);
        let temp = NodeId::numeric(1, 2);
        for (id, class, name) in [
            (objects.clone(), NodeClass::Object, "Objects"),
            (boiler.clone(), NodeClass::Object, "Boiler"),
            (temp.clone(), NodeClass::Variable, "Temperature"),
        ] {
            space.add_node(NodeMeta {
                node_id: id,
                node_class: class,
                browse_name: QualifiedName {
                    namespace_index: 1,
                    name: String::from(name),
                },
                display_name: LocalizedText {
                    locale: None,
                    text: Some(String::from(name)),
                },
                type_definition: NodeId::numeric(0, 58),
            });
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

    #[cfg(feature = "crypto")]
    #[test]
    fn e2e_secured_loopback_sign_and_encrypt() {
        use crate::server::ServerSecurity;
        use rand::rngs::OsRng;
        use zerodds_opcua_uacp::crypto::{RsaPrivateKey, SecuredMode, SecurityPolicy};

        // Generate RSA key pairs and exchange public keys out of band (trust).
        let mut kg = OsRng;
        let client_key = RsaPrivateKey::new(&mut kg, 2048).expect("client key");
        let server_key = RsaPrivateKey::new(&mut kg, 2048).expect("server key");
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
