// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The OPC-UA [`Server`]: a request-driven state machine that runs the
//! Hello → OpenSecureChannel → Session handshake and dispatches the Read and
//! Call services against an [`AddressSpace`] (SecurityMode `None`).

use alloc::vec::Vec;

use zerodds_opcua_gateway::data_value::DataValue;
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
    ChannelSecurityToken, OpenSecureChannelRequest, OpenSecureChannelResponse, ResponseHeader,
    SECURITY_POLICY_NONE, SecureChannel, parse_chunk,
};

use crate::address_space::AddressSpace;
use crate::services::{
    ATTRIBUTE_VALUE, ActivateSessionResponse, BrowseDescription, BrowseResponse, BrowseResult,
    CallMethodResult, CallResponse, CloseSessionResponse, CreateMonitoredItemsResponse,
    CreateSessionResponse, CreateSubscriptionResponse, DataChangeNotification,
    DeleteSubscriptionsResponse, FindServersResponse, GetEndpointsResponse,
    MonitoredItemCreateResult, MonitoredItemNotification, NotificationMessage, PublishResponse,
    ReadResponse, ReferenceDescription, ServiceRequest, ServiceResponse, SetPublishingModeResponse,
    SignatureData, WriteResponse, null_filter,
};

#[cfg(feature = "crypto")]
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
    pub certificate: alloc::vec::Vec<u8>,
    /// The trusted client's DER certificate.
    pub client_certificate: alloc::vec::Vec<u8>,
    /// The trusted client's RSA public key (verifies the client's OPN).
    pub client_public_key: RsaPublicKey,
    /// A cryptographically secure RNG (OS RNG under `std`).
    pub rng: Box<dyn CryptoRngCore + Send>,
}

/// StatusCode `Good`.
pub const GOOD: u32 = 0;
/// StatusCode `Bad_NodeIdUnknown`.
pub const BAD_NODE_ID_UNKNOWN: u32 = 0x8034_0000;
/// StatusCode `Bad_MethodInvalid`.
pub const BAD_METHOD_INVALID: u32 = 0x8025_0000;
/// StatusCode `Bad_AttributeIdInvalid`.
pub const BAD_ATTRIBUTE_ID_INVALID: u32 = 0x8035_0000;
/// StatusCode `Bad_SubscriptionIdInvalid`.
pub const BAD_SUBSCRIPTION_ID_INVALID: u32 = 0x8028_0000;

/// A monitored item the server samples on Publish. (The `monitored_item_id`
/// returned to the client is the per-subscription counter; we don't yet expose
/// Modify/DeleteMonitoredItems, so it isn't retained per item.)
#[derive(Debug, Clone)]
struct MonitoredItem {
    client_handle: u32,
    node_id: NodeId,
    last_value: Option<DataValue>,
}

/// A server-side subscription: a set of monitored items + a sequence counter.
#[derive(Debug, Clone)]
struct Subscription {
    id: u32,
    publishing_enabled: bool,
    seq: u32,
    next_item_id: u32,
    items: Vec<MonitoredItem>,
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
    endpoint_url: alloc::string::String,
    channel: Option<SecureChannel>,
    next_channel_id: u32,
    next_token_id: u32,
    next_session: u32,
    subscriptions: Vec<Subscription>,
    next_subscription_id: u32,
    #[cfg(feature = "crypto")]
    security: Option<ServerSecurity>,
}

impl core::fmt::Debug for Server {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Server")
            .field("endpoint_url", &self.endpoint_url)
            .field("channel", &self.channel)
            .finish_non_exhaustive()
    }
}

impl Server {
    /// Creates a server serving `address_space` at `endpoint_url`.
    #[must_use]
    pub fn new(
        endpoint_url: impl Into<alloc::string::String>,
        address_space: AddressSpace,
    ) -> Self {
        Self {
            address_space,
            endpoint_url: endpoint_url.into(),
            channel: None,
            next_channel_id: 1,
            next_token_id: 1,
            next_session: 1,
            subscriptions: Vec::new(),
            next_subscription_id: 1,
            #[cfg(feature = "crypto")]
            security: None,
        }
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
    /// (or `None` for messages that need no reply, e.g. CloseSecureChannel).
    ///
    /// # Errors
    /// [`ServerError`] on a malformed message or protocol-state violation.
    pub fn process(&mut self, incoming: &[u8]) -> Result<Option<Vec<u8>>, ServerError> {
        let mut r = UaReader::new(incoming);
        let header = MessageHeader::decode(&mut r)?;
        match header.message_type {
            MessageType::Hello => {
                let hello = HelloMessage::decode_body(&mut r)?;
                let ack = AcknowledgeMessage {
                    protocol_version: 0,
                    receive_buffer_size: hello.receive_buffer_size.max(8192),
                    send_buffer_size: hello.send_buffer_size.max(8192),
                    max_message_size: hello.max_message_size,
                    max_chunk_count: hello.max_chunk_count,
                };
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
                    chunk.sequence_header.request_id,
                )?))
            }
            MessageType::Message => {
                let chunk = self
                    .channel
                    .as_ref()
                    .ok_or(ServerError::Protocol("MSG before OpenSecureChannel"))?
                    .open_incoming(incoming)?;
                let req = ServiceRequest::decode(&chunk.body)?;
                let resp = self.handle_service(req);
                let body = resp.encode()?;
                let channel = self
                    .channel
                    .as_mut()
                    .ok_or(ServerError::Protocol("MSG before OpenSecureChannel"))?;
                Ok(Some(
                    channel.message_chunk(chunk.sequence_header.request_id, &body)?,
                ))
            }
            MessageType::CloseSecureChannel => {
                self.channel = None;
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
        request_id: u32,
    ) -> Result<Vec<u8>, ServerError> {
        let channel_id = self.next_channel_id;
        self.next_channel_id += 1;
        let token_id = self.next_token_id;
        self.next_token_id += 1;
        let mut channel = SecureChannel::new(channel_id, token_id);

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
        self.channel = Some(channel);
        Ok(out)
    }

    /// The secured OpenSecureChannel handshake: opens the client's asymmetric
    /// OPN, derives the §6.7.6 keys, and returns the secured OPN response while
    /// installing the symmetric [`SecuritySession`] on the channel.
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
        let channel_id = self.next_channel_id;
        self.next_channel_id += 1;
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
        let mut channel = SecureChannel::new(channel_id, token_id);
        let seq = channel.next_send_sequence();
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
        self.channel = Some(channel);
        Ok(out)
    }

    fn handle_service(&mut self, req: ServiceRequest) -> ServiceResponse {
        match req {
            ServiceRequest::CreateSession(r) => {
                let session = self.next_session;
                self.next_session += 1;
                ServiceResponse::CreateSession(CreateSessionResponse {
                    response_header: ResponseHeader::new(r.request_header.request_handle, GOOD),
                    session_id: NodeId::numeric(1, session),
                    authentication_token: NodeId::numeric(0, 0x1000_0000 + session),
                    revised_session_timeout: if r.requested_session_timeout > 0.0 {
                        r.requested_session_timeout
                    } else {
                        1_200_000.0
                    },
                    server_nonce: Vec::new(),
                    server_certificate: Vec::new(),
                    server_endpoints: alloc::vec![self.endpoint_none()],
                    server_signature: SignatureData::default(),
                    max_request_message_size: 0,
                })
            }
            ServiceRequest::ActivateSession(r) => {
                ServiceResponse::ActivateSession(ActivateSessionResponse {
                    response_header: ResponseHeader::new(r.request_header.request_handle, GOOD),
                    server_nonce: Vec::new(),
                    results: Vec::new(),
                })
            }
            ServiceRequest::CloseSession(r) => {
                ServiceResponse::CloseSession(CloseSessionResponse {
                    response_header: ResponseHeader::new(r.request_header.request_handle, GOOD),
                })
            }
            ServiceRequest::Read(r) => {
                let results = r
                    .nodes_to_read
                    .iter()
                    .map(|rv| match self.address_space.value(&rv.node_id) {
                        Some(dv) => dv.clone(),
                        None => DataValue {
                            value: None,
                            status: Some(BAD_NODE_ID_UNKNOWN),
                            source_timestamp: None,
                            server_timestamp: None,
                            source_pico_sec: None,
                            server_pico_sec: None,
                        },
                    })
                    .collect();
                ServiceResponse::Read(ReadResponse {
                    response_header: ResponseHeader::new(r.request_header.request_handle, GOOD),
                    results,
                })
            }
            ServiceRequest::Write(r) => {
                let results = r
                    .nodes_to_write
                    .iter()
                    .map(|wv| {
                        if wv.attribute_id == ATTRIBUTE_VALUE {
                            self.address_space
                                .set_value(wv.node_id.clone(), wv.value.clone());
                            GOOD
                        } else {
                            BAD_ATTRIBUTE_ID_INVALID
                        }
                    })
                    .collect();
                ServiceResponse::Write(WriteResponse {
                    response_header: ResponseHeader::new(r.request_header.request_handle, GOOD),
                    results,
                })
            }
            ServiceRequest::Browse(r) => {
                let results = r
                    .nodes_to_browse
                    .iter()
                    .map(|bd| self.browse_one(bd))
                    .collect();
                ServiceResponse::Browse(BrowseResponse {
                    response_header: ResponseHeader::new(r.request_header.request_handle, GOOD),
                    results,
                })
            }
            ServiceRequest::GetEndpoints(r) => {
                ServiceResponse::GetEndpoints(GetEndpointsResponse {
                    response_header: ResponseHeader::new(r.request_header.request_handle, GOOD),
                    endpoints: alloc::vec![self.endpoint_none()],
                })
            }
            ServiceRequest::FindServers(r) => ServiceResponse::FindServers(FindServersResponse {
                response_header: ResponseHeader::new(r.request_header.request_handle, GOOD),
                servers: alloc::vec![self.endpoint_none().server],
            }),
            ServiceRequest::CreateSubscription(r) => {
                let id = self.next_subscription_id;
                self.next_subscription_id += 1;
                self.subscriptions.push(Subscription {
                    id,
                    publishing_enabled: r.publishing_enabled,
                    seq: 0,
                    next_item_id: 1,
                    items: Vec::new(),
                });
                let interval = if r.requested_publishing_interval > 0.0 {
                    r.requested_publishing_interval
                } else {
                    1000.0
                };
                ServiceResponse::CreateSubscription(CreateSubscriptionResponse {
                    response_header: ResponseHeader::new(r.request_header.request_handle, GOOD),
                    subscription_id: id,
                    revised_publishing_interval: interval,
                    revised_lifetime_count: r.requested_lifetime_count.max(10_000),
                    revised_max_keep_alive_count: r.requested_max_keep_alive_count.max(10),
                })
            }
            ServiceRequest::SetPublishingMode(r) => {
                let enabled = r.publishing_enabled;
                let results = r
                    .subscription_ids
                    .iter()
                    .map(|sid| {
                        if let Some(s) = self.subscriptions.iter_mut().find(|s| s.id == *sid) {
                            s.publishing_enabled = enabled;
                            GOOD
                        } else {
                            BAD_SUBSCRIPTION_ID_INVALID
                        }
                    })
                    .collect();
                ServiceResponse::SetPublishingMode(SetPublishingModeResponse {
                    response_header: ResponseHeader::new(r.request_header.request_handle, GOOD),
                    results,
                })
            }
            ServiceRequest::CreateMonitoredItems(r) => {
                let handle = r.request_header.request_handle;
                match self
                    .subscriptions
                    .iter_mut()
                    .find(|s| s.id == r.subscription_id)
                {
                    None => ServiceResponse::CreateMonitoredItems(CreateMonitoredItemsResponse {
                        response_header: ResponseHeader::new(handle, BAD_SUBSCRIPTION_ID_INVALID),
                        results: Vec::new(),
                    }),
                    Some(sub) => {
                        let results = r
                            .items_to_create
                            .iter()
                            .map(|it| {
                                let mid = sub.next_item_id;
                                sub.next_item_id += 1;
                                let p = &it.requested_parameters;
                                sub.items.push(MonitoredItem {
                                    client_handle: p.client_handle,
                                    node_id: it.item_to_monitor.node_id.clone(),
                                    last_value: None,
                                });
                                MonitoredItemCreateResult {
                                    status_code: GOOD,
                                    monitored_item_id: mid,
                                    revised_sampling_interval: if p.sampling_interval > 0.0 {
                                        p.sampling_interval
                                    } else {
                                        1000.0
                                    },
                                    revised_queue_size: p.queue_size.max(1),
                                    filter_result: null_filter(),
                                }
                            })
                            .collect();
                        ServiceResponse::CreateMonitoredItems(CreateMonitoredItemsResponse {
                            response_header: ResponseHeader::new(handle, GOOD),
                            results,
                        })
                    }
                }
            }
            ServiceRequest::Publish(r) => {
                ServiceResponse::Publish(self.publish_one(r.request_header.request_handle))
            }
            ServiceRequest::DeleteSubscriptions(r) => {
                let results = r
                    .subscription_ids
                    .iter()
                    .map(|sid| {
                        if let Some(pos) = self.subscriptions.iter().position(|s| s.id == *sid) {
                            self.subscriptions.remove(pos);
                            GOOD
                        } else {
                            BAD_SUBSCRIPTION_ID_INVALID
                        }
                    })
                    .collect();
                ServiceResponse::DeleteSubscriptions(DeleteSubscriptionsResponse {
                    response_header: ResponseHeader::new(r.request_header.request_handle, GOOD),
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
                    response_header: ResponseHeader::new(r.request_header.request_handle, GOOD),
                    results,
                })
            }
        }
    }

    /// Browses one [`BrowseDescription`] against the AddressSpace, honouring the
    /// reference-type filter, node-class mask and result mask.
    fn browse_one(&self, bd: &BrowseDescription) -> BrowseResult {
        // An unknown source node is reported per Part 4 §5.9.2.
        if self.address_space.node_meta(&bd.node_id).is_none() {
            return BrowseResult {
                status_code: BAD_NODE_ID_UNKNOWN,
                continuation_point: Vec::new(),
                references: Vec::new(),
            };
        }
        let null_id = NodeId::numeric(0, 0);
        let filter = if bd.reference_type_id == null_id {
            None
        } else {
            Some(&bd.reference_type_id)
        };
        let matches = self.address_space.browse(
            &bd.node_id,
            bd.browse_direction,
            filter,
            bd.include_subtypes,
            bd.node_class_mask,
        );
        let mask = bd.result_mask;
        let empty_qn = QualifiedName {
            namespace_index: 0,
            name: alloc::string::String::new(),
        };
        let empty_lt = LocalizedText {
            locale: None,
            text: None,
        };
        let expanded = |id: NodeId| ExpandedNodeId {
            node_id: id,
            namespace_uri: alloc::string::String::new(),
            server_index: 0,
        };
        let references = matches
            .into_iter()
            .map(|m| ReferenceDescription {
                reference_type_id: if mask & 0x01 != 0 {
                    m.reference_type
                } else {
                    null_id.clone()
                },
                is_forward: mask & 0x02 != 0 && m.is_forward,
                node_id: expanded(m.target.node_id),
                browse_name: if mask & 0x08 != 0 {
                    m.target.browse_name
                } else {
                    empty_qn.clone()
                },
                display_name: if mask & 0x10 != 0 {
                    m.target.display_name
                } else {
                    empty_lt.clone()
                },
                node_class: if mask & 0x04 != 0 {
                    m.target.node_class.as_i32()
                } else {
                    0
                },
                type_definition: expanded(if mask & 0x20 != 0 {
                    m.target.type_definition
                } else {
                    null_id.clone()
                }),
            })
            .collect();
        BrowseResult {
            status_code: GOOD,
            continuation_point: Vec::new(),
            references,
        }
    }

    /// Handles a Publish by sampling the first publishing-enabled
    /// subscription's monitored items against the AddressSpace and returning a
    /// `DataChangeNotification` for the values that changed since last reported
    /// (sample-on-publish, suited to a synchronous transport).
    fn publish_one(&mut self, request_handle: u32) -> PublishResponse {
        let empty = NotificationMessage {
            sequence_number: 0,
            publish_time: 0,
            notification_data: Vec::new(),
        };
        let addr = &self.address_space;
        let Some(sub) = self.subscriptions.iter_mut().find(|s| s.publishing_enabled) else {
            return PublishResponse {
                response_header: ResponseHeader::new(request_handle, GOOD),
                subscription_id: 0,
                available_sequence_numbers: Vec::new(),
                more_notifications: false,
                notification_message: empty,
                results: Vec::new(),
            };
        };
        let mut notes = Vec::new();
        for item in &mut sub.items {
            let current = addr.value(&item.node_id).cloned();
            if current != item.last_value {
                item.last_value.clone_from(&current);
                if let Some(v) = current {
                    notes.push(MonitoredItemNotification {
                        client_handle: item.client_handle,
                        value: v,
                    });
                }
            }
        }
        sub.seq += 1;
        let seq = sub.seq;
        let sub_id = sub.id;
        let notification_data = if notes.is_empty() {
            Vec::new()
        } else {
            match (DataChangeNotification {
                monitored_items: notes,
            })
            .to_extension_object()
            {
                Ok(eo) => alloc::vec![eo],
                Err(_) => Vec::new(),
            }
        };
        PublishResponse {
            response_header: ResponseHeader::new(request_handle, GOOD),
            subscription_id: sub_id,
            available_sequence_numbers: alloc::vec![seq],
            more_notifications: false,
            notification_message: NotificationMessage {
                sequence_number: seq,
                publish_time: 0,
                notification_data,
            },
            results: Vec::new(),
        }
    }

    fn endpoint_none(&self) -> EndpointDescription {
        use zerodds_opcua_pubsub::uadp::datatypes::ApplicationDescription;
        EndpointDescription {
            endpoint_url: self.endpoint_url.clone(),
            server: ApplicationDescription {
                application_uri: alloc::string::String::from("urn:zerodds:opcua-server"),
                product_uri: alloc::string::String::from("urn:zerodds"),
                application_name: zerodds_opcua_gateway::types::LocalizedText {
                    locale: None,
                    text: Some(alloc::string::String::from("ZeroDDS OPC-UA Server")),
                },
                application_type: ApplicationType::Server,
                gateway_server_uri: alloc::string::String::new(),
                discovery_profile_uri: alloc::string::String::new(),
                discovery_urls: alloc::vec![self.endpoint_url.clone()],
            },
            server_certificate: Vec::new(),
            security_mode: MessageSecurityMode::None,
            security_policy_uri: alloc::string::String::from(SECURITY_POLICY_NONE),
            user_identity_tokens: Vec::new(),
            transport_profile_uri: alloc::string::String::from(
                "http://opcfoundation.org/UA-Profile/Transport/uatcp-uasc-uabinary",
            ),
            security_level: 0,
        }
    }
}
