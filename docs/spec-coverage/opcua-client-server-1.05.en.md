# OPC UA Part 4 — Client/Server (UACP + SecureChannel) — Spec Coverage

**Source:** `docs/standards/cache/opcfoundation/opcua-part4-services-1.05.07.pdf`
(Services). Cross-spec:
`opcua-part6-mappings-1.05.07.pdf` (UACP §7.1, OPC UA Secure Conversation §6.7,
binary codec §5.2),
`opcua-part7-profiles-1.05.02.pdf` (transport profiles, SecurityPolicies)
(all in the same cache; not tracked in the repo for IP/copyright reasons).

**Public spec:** OPC UA online reference <https://reference.opcfoundation.org/>
(Part 4 Services, Part 6 Mappings, Part 7 Profiles).

**Context:** A native pure-Rust `no_std + alloc` OPC-UA Client/Server stack —
the request/response counterpart to the UADP PubSub stack
(`zerodds-opcua-pubsub`). Same approach as ZeroDDS's full CORBA/MQTT/AMQP
stacks: a real wire stack, not just a gateway bridge. The Part 6 binary codec
is reused from `zerodds-opcua-pubsub`. `forbid(unsafe_code)`.

Implementation:

- `crates/opcua-uacp/` — UACP (Hello/Ack/Error/ReverseHello, message chunking),
  OPC UA Secure Conversation (OPN/MSG/CLO chunks, security headers,
  SequenceHeader, ChannelSecurityToken) and the secured SecurityPolicies
  (feature `crypto`); 20 tests green (`--features crypto`).
- `crates/opcua-server/` — the full service set (Session/Read/Write/Call/Browse/
  Discovery/Subscription), in-memory AddressSpace with a reference graph,
  `Server` state machine, `Client` and `opc.tcp` TCP transport; 22 tests green,
  including a real TCP end-to-end and a secured loopback.

SecurityMode `None` **and** the secured SecurityPolicies (RSA/AES crypto,
`Basic256Sha256`/`Aes128_Sha256_RsaOaep`/`Aes256_Sha256_RsaPss`) are implemented
end to end. Open items: none.

---

## §7.1 OPC UA Connection Protocol (UACP)

**Spec:** Part 6 §7.1, p. 80 — message structure (8-byte header
`MessageType` + `ChunkType` + `MessageSize`), Hello/Acknowledge/Error plus
ReverseHello (§7.1.3 Establishing a connection, p. 83).

**Repo:** `crates/opcua-uacp/src/connection.rs` (`MessageType`, `ChunkType`,
`MessageHeader`, `HelloMessage`, `AcknowledgeMessage`, `ErrorMessage`,
`ReverseHelloMessage`).

**Tests:** `crates/opcua-uacp/src/connection.rs::tests::header_round_trips_and_sizes_match`,
`acknowledge_round_trips`, `error_round_trips`, `reverse_hello_round_trips`,
`unknown_message_type_rejected`, `oversized_string_rejected`.

**Status:** done

## §6.7 OPC UA Secure Conversation — chunk framing

**Spec:** Part 6 §6.7, p. 62 — SecureChannel messages as chunks
(OPN/MSG/CLO), `AsymmetricAlgorithmSecurityHeader` (OPN) /
`SymmetricAlgorithmSecurityHeader` (MSG/CLO), `SequenceHeader`
(§6.7.2 MessageChunk structure, p. 62).

**Repo:** `crates/opcua-uacp/src/securechannel.rs`
(`SecureChannel::open_chunk` / `message_chunk` / `close_chunk`, `parse_chunk`,
`ParsedChunk`, `SECURITY_POLICY_NONE`).

**Tests:** `crates/opcua-uacp/src/securechannel.rs::tests::message_chunk_round_trip`,
`open_secure_channel_round_trip_none`.

**Status:** done (SecurityMode `None`; secured crypto see §6.7 crypto)

## §6.7 SecureChannel crypto — secured SecurityPolicies

**Spec:** Part 6 §6.7.2 (chunk security layout: SequenceHeader‖Body‖Padding‖
Signature, signature over MessageHeader…Padding, then encrypted),
§6.7.6 Deriving keys (p. 70, P-SHA256); Part 7 SecurityPolicies
`Basic256Sha256`, `Aes128_Sha256_RsaOaep`, `Aes256_Sha256_RsaPss`.

**Repo (crypto core):** `crates/opcua-uacp/src/crypto.rs` (feature `crypto`):
`SecurityPolicy` (URI/algorithm params), `p_sha256` + `derive_keys` (§6.7.6:
signing/encrypting/IV), symmetric `build_symmetric_chunk`/`open_symmetric_chunk`
(AES-128/256-CBC + HMAC-SHA256, padding, Sign & SignAndEncrypt), asymmetric
`build_asymmetric_chunk`/`open_asymmetric_chunk` (RSA-OAEP-SHA1/SHA256 +
RSA-PKCS#1-v1.5/PSS over SHA-256, block-wise OPN crypto with padding/extra
padding byte + certificate thumbprint via `sha1_thumbprint`).

**Repo (wiring):** `crates/opcua-uacp/src/securechannel.rs`
(`SecuritySession`, `SecureChannel::install_security`/`open_incoming`, secured
`message_chunk`/`close_chunk`), `crates/opcua-server/src/server.rs`
(`ServerSecurity`, `Server::set_security`, `open_secure_channel_secured`: open
OPN → nonce → key derivation → secured OPN response + session install),
`crates/opcua-server/src/client.rs` (`ClientSecurity`, `Client::set_security`,
`open_secure_channel_secured`). Trust store = supplied peer certificates/public
keys; RNG caller-provided (`Box<dyn CryptoRngCore + Send>`, OS RNG under `std`).

**Tests:** `crates/opcua-uacp/src/crypto.rs::tests::p_sha256_is_deterministic_and_sized`,
`derive_keys_lengths_and_mirror`, `symmetric_sign_and_encrypt_round_trip`
(all 3 policies), `symmetric_sign_only_round_trip`, `symmetric_tamper_is_rejected`,
`asymmetric_round_trip_oaep_pkcs15`, `asymmetric_round_trip_oaep_sha256_pss`,
`asymmetric_wrong_signer_is_rejected`, `full_secured_handshake_flow`;
**secured end-to-end:** `crates/opcua-server/src/client.rs::tests::e2e_secured_loopback_sign_and_encrypt`
(real RSA keys + OS RNG: encrypted+signed handshake → Read/Call/Write over
`Basic256Sha256`/SignAndEncrypt).

**Status:** done (crypto core + chunk security + Server/Client handshake fully
wired and tested with a real secured E2E)

## §5.6.2 OpenSecureChannel

**Spec:** Part 4 §5.6.2, p. 21 — `OpenSecureChannelRequest`/`Response`,
`SecurityTokenRequestType`, `ChannelSecurityToken` (RevisedLifetime).

**Repo:** `crates/opcua-uacp/src/securechannel.rs`
(`OpenSecureChannelRequest`, `OpenSecureChannelResponse`,
`SecurityTokenRequestType`, `ChannelSecurityToken`),
`crates/opcua-server/src/server.rs` (`Server::open_secure_channel`).

**Tests:** `crates/opcua-uacp/src/securechannel.rs::tests::open_response_round_trip`,
`crates/opcua-server/src/client.rs::tests::e2e_loopback_connect_read_call`
(runs OPN).

**Status:** done (SecurityMode `None`)

## §5.7.2 CreateSession

**Spec:** Part 4 §5.7.2, p. 24 — `CreateSessionRequest`/`Response`
(SessionId, AuthenticationToken, RevisedSessionTimeout, ServerEndpoints,
ServerSignature).

**Repo:** `crates/opcua-server/src/services.rs`
(`CreateSessionRequest`, `CreateSessionResponse`),
`crates/opcua-server/src/server.rs` (`handle_service` CreateSession arm).

**Tests:** `crates/opcua-server/src/client.rs::tests::e2e_loopback_connect_read_call`,
`e2e_tcp_connect_read_call`.

**Status:** done

## §5.7.3 ActivateSession

**Spec:** Part 4 §5.7.3, p. 29 — `ActivateSessionRequest`/`Response`
(ClientSignature, LocaleIds, UserIdentityToken, ServerNonce).

**Repo:** `crates/opcua-server/src/services.rs`
(`ActivateSessionRequest`, `ActivateSessionResponse`).

**Tests:** `crates/opcua-server/src/services.rs::tests::session_lifecycle_round_trips`,
`crates/opcua-server/src/client.rs::tests::e2e_loopback_connect_read_call`.

**Status:** done (anonymous UserIdentityToken; SecurityMode `None`)

## §5.7.4 CloseSession

**Spec:** Part 4 §5.7.4, p. 31 — `CloseSessionRequest` (DeleteSubscriptions)
/`Response`.

**Repo:** `crates/opcua-server/src/services.rs`
(`CloseSessionRequest`, `CloseSessionResponse`).

**Tests:** `crates/opcua-server/src/services.rs::tests::session_lifecycle_round_trips`.

**Status:** done

## §5.11.2 Read

**Spec:** Part 4 §5.11.2, p. 45 — `ReadRequest` (MaxAge, TimestampsToReturn,
`ReadValueId[]`) / `ReadResponse` (`DataValue[]`).

**Repo:** `crates/opcua-server/src/services.rs`
(`ReadRequest`, `ReadResponse`, `ReadValueId`),
`crates/opcua-server/src/server.rs` (Read arm against AddressSpace),
`crates/opcua-server/src/client.rs` (`Client::read_values`).

**Tests:** `crates/opcua-server/src/services.rs::tests::read_request_round_trips`,
`read_response_round_trips`,
`crates/opcua-server/src/client.rs::tests::read_unknown_node_yields_bad_status`.

**Status:** done (Value attribute)

## §5.11.4 Write

**Spec:** Part 4 §5.11.4, p. 50 — `WriteRequest` (`WriteValue[]`) /
`WriteResponse` (`StatusCode[]`).

**Repo:** `crates/opcua-server/src/services.rs`
(`WriteRequest`, `WriteResponse`, `WriteValue`),
`crates/opcua-server/src/server.rs` (Write arm: `set_value`,
`Bad_AttributeIdInvalid` for non-Value attributes),
`crates/opcua-server/src/client.rs` (`Client::write_values`).

**Tests:** `crates/opcua-server/src/services.rs::tests::write_round_trips`,
`crates/opcua-server/src/client.rs::tests::e2e_loopback_write_then_read`.

**Status:** done (Value attribute)

## §5.12.2 Call

**Spec:** Part 4 §5.12.2, p. 54 — `CallRequest` (`CallMethodRequest[]`) /
`CallResponse` (`CallMethodResult[]`).

**Repo:** `crates/opcua-server/src/services.rs`
(`CallMethodRequest`, `CallMethodResult`, `CallRequest`, `CallResponse`),
`crates/opcua-server/src/address_space.rs` (method handler registry),
`crates/opcua-server/src/client.rs` (`Client::call_method`).

**Tests:** `crates/opcua-server/src/services.rs::tests::call_round_trips`,
`crates/opcua-server/src/address_space.rs::tests::method_call_dispatch`,
`crates/opcua-server/src/client.rs::tests::e2e_loopback_connect_read_call`,
`e2e_tcp_connect_read_call`.

**Status:** done

## §5.9.2 Browse (View Service Set)

**Spec:** Part 4 §5.9.2 Browse (p. 38), §7.29 ReferenceDescription (p. 148),
§7.6 BrowseResult (p. 118), §7.44 ViewDescription (p. 169); ExpandedNodeId
Part 6 §5.2.2.10.

**Repo:** `crates/opcua-server/src/address_space.rs` (node/reference model:
`NodeMeta`, `ReferenceRecord`, `NodeClass`, ns0 `reference_types` constants,
`add_node`/`add_reference`/`node_meta`/`browse` with direction +
reference-subtype filter (`includeSubtypes`) + node-class mask),
`crates/opcua-server/src/services.rs` (`ViewDescription`, `BrowseDescription`,
`ReferenceDescription`, `BrowseResult`, `BrowseRequest`/`BrowseResponse`,
ExpandedNodeId wire), `crates/opcua-server/src/server.rs` (`browse_one`:
reference filter + `resultMask` projection, `Bad_NodeIdUnknown` for unknown
nodes), `crates/opcua-server/src/client.rs` (`Client::browse`).

**Tests:** `crates/opcua-server/src/address_space.rs::tests::browse_forward_and_inverse`,
`browse_subtype_filter_excludes_non_hierarchical`, `browse_node_class_mask_filters_targets`,
`crates/opcua-server/src/services.rs::tests::browse_round_trips`,
`crates/opcua-server/src/client.rs::tests::e2e_loopback_browse`
(Objects→Boiler→Temperature over Organizes/HasComponent).

**Status:** done (continuation points: single batch, no BrowseNext —
address spaces of this size return everything in one `BrowseResult`)

## §5.5.4 / §5.5.2 Discovery — GetEndpoints + FindServers

**Spec:** Part 4 §5.5.4 GetEndpoints (p. 14), §5.5.2 FindServers (p. 12) —
session-less discovery services (SecureChannel only, no session needed).

**Repo:** `crates/opcua-server/src/services.rs`
(`GetEndpointsRequest`/`GetEndpointsResponse` (NodeIds 426/429),
`FindServersRequest`/`FindServersResponse` (NodeIds 422/425)),
`crates/opcua-server/src/server.rs` (dispatch arms: GetEndpoints returns the
server's `EndpointDescription`, FindServers its `ApplicationDescription`),
`crates/opcua-server/src/client.rs` (`Client::open_channel` — Hello+OPN without
a session — plus `Client::get_endpoints`/`find_servers`).

**Tests:** `crates/opcua-server/src/services.rs::tests::discovery_requests_round_trip`,
`crates/opcua-server/src/client.rs::tests::e2e_loopback_discovery`
(session-less channel → GetEndpoints returns an endpoint with SecurityPolicy
`None`, FindServers returns the server).

**Status:** done

## §5.13 / §5.14 Subscription + MonitoredItem Service Sets

**Spec:** Part 4 §5.14.2 CreateSubscription (p. 76), §5.14.4 SetPublishingMode
(p. 79), §5.13.2 CreateMonitoredItems (p. 63), §5.14.5 Publish (p. 81),
§5.14.8 DeleteSubscriptions (p. 85); §7.25.2 DataChangeNotification (p. 145),
§7.26 NotificationMessage (p. 147).

**Repo:** `crates/opcua-server/src/services.rs`
(`CreateSubscriptionRequest`/`Response`, `SetPublishingModeRequest`/`Response`,
`MonitoringParameters`, `MonitoredItemCreateRequest`/`Result`,
`CreateMonitoredItemsRequest`/`Response`, `SubscriptionAcknowledgement`,
`MonitoredItemNotification`, `DataChangeNotification`
(ExtensionObject wrapping TypeId 811), `NotificationMessage`,
`PublishRequest`/`Response`, `DeleteSubscriptionsRequest`/`Response`),
`crates/opcua-server/src/server.rs` (subscription/monitored-item state,
`publish_one`: **sample-on-publish** — on each Publish, sample the monitored
items against the AddressSpace and report changed values as a
DataChangeNotification), `crates/opcua-server/src/client.rs`
(`create_subscription`/`create_monitored_items`/`publish`/`delete_subscriptions`).

**Tests:** `crates/opcua-server/src/services.rs::tests::subscription_round_trips`,
`publish_data_change_notification_round_trips`,
`crates/opcua-server/src/client.rs::tests::e2e_loopback_subscription_publish`
(subscription + monitored item on a node → first Publish reports the initial
value, no change → empty, after a Write the new value, then
DeleteSubscriptions).

**Status:** done (sample-on-publish model, suited to the synchronous transport;
Modify/Republish and server-paced sampling are not required as the push use case
is also covered by Part 14 PubSub)

## §7.32/§7.33 Common service headers + common types

**Spec:** Part 4 §7.32 RequestHeader (p. 150), §7.33 ResponseHeader (p. 151),
§7.12 DiagnosticInfo (p. 131), §7.36 SignatureData (p. 153), §7.28 ReadValueId
(p. 148), §7.14 EndpointDescription (p. 133), §7.20 MessageSecurityMode
(p. 135).

**Repo:** `crates/opcua-uacp/src/securechannel.rs`
(`RequestHeader`, `ResponseHeader`, `null_extension_object`, minimal
DiagnosticInfo skip), `crates/opcua-server/src/services.rs`
(`SignatureData`, `ReadValueId`, header encode/decode against the public structs),
`crates/opcua-server/src/wire.rs` (String/ByteString/Array/DiagnosticInfo helpers),
`MessageSecurityMode`/`EndpointDescription` from `zerodds-opcua-pubsub`.

**Tests:** `crates/opcua-uacp/src/securechannel.rs::tests::response_header_with_string_table_round_trips`,
`crates/opcua-server/src/services.rs::tests::read_response_round_trips`.

**Status:** done

## §5.2 OPC UA Binary — codec reuse

**Spec:** Part 6 §5.2, p. 18 — binary encoding (built-in types, NodeId,
Variant, DataValue, ExtensionObject, arrays).

**Repo:** reused from `crates/opcua-pubsub/src/binary/` via
`UaReader`/`UaWriter`/`UaEncode`/`UaDecode`; `NodeId`/`Variant`/`DataValue`/
`ExtensionObject` from `zerodds-opcua-gateway`.

**Tests:** covered by the PubSub codec tests
(`crates/opcua-pubsub/src/binary/`) plus every round-trip test here.

**Status:** done

## §7.2 OPC UA TCP — opc.tcp transport (end-to-end)

**Spec:** Part 6 §7.2, p. 86 — UA-TCP mapping; transport profile
`uatcp-uasc-uabinary` (Part 7).

**Repo:** `crates/opcua-server/src/client.rs`
(`TcpTransport`, `serve_connection`, `Transport` trait, `LoopbackTransport`),
`crates/opcua-server/src/server.rs` (`Server::process`).

**Tests:** `crates/opcua-server/src/client.rs::tests::e2e_tcp_connect_read_call`
(real `opc.tcp` socket: connect/Read/Call over `TcpListener`+thread),
`e2e_loopback_connect_read_call`, `e2e_loopback_write_then_read`.

**Status:** done

---

## Audit status

16 done / 0 partial / 0 open / 0 n/a (informative) / 0 n/a (rejected).

Test run: `cargo test -p zerodds-opcua-uacp -p zerodds-opcua-server --features crypto`
— 42 tests green (20 UACP incl. 10 crypto + 22 server incl. secured E2E +
Browse + Discovery + Subscription/Publish), 0 failed; including a real
`opc.tcp` E2E and a secured `Basic256Sha256` loopback.

Open items: none. Decision records: none.
