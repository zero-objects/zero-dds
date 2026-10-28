# OPC UA Part 4 — Client/Server (UACP + SecureChannel) — Spec Coverage

**Source:** `internal/standards/cache/opcfoundation/opcua-part4-services-1.05.07.pdf`
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

- `crates/opcua-uacp/` — UACP (Hello/Ack/Error/ReverseHello), OPC UA Secure
  Conversation (OPN/MSG/CLO chunks, security headers, SequenceHeader,
  ChannelSecurityToken, message chunking across `C`/`F`/`A` with reassembly)
  and the secured SecurityPolicies (feature `crypto`).
- `crates/opcua-server/` — the Discovery, SecureChannel, Session, Attribute,
  View, Method, MonitoredItem and Subscription service sets, an in-memory
  AddressSpace with the Part 3 attribute model and a reference graph, the
  `Server` state machine with sessions/user store/SecureChannel list, the
  `Client` and the `opc.tcp` TCP transport.

SecurityMode `None` **and** the secured SecurityPolicies (RSA/AES crypto,
`Basic256Sha256`/`Aes128_Sha256_RsaOaep`/`Aes256_Sha256_RsaPss`) are implemented
end to end in both modes (`Sign`, `SignAndEncrypt`). Open items: none.

---

## Part 6 — UACP, SecureChannel and binary codec

### §7.1 OPC UA Connection Protocol (UACP)

**Spec:** Part 6 §7.1, p. 80 — message structure (8-byte header
`MessageType` + `ChunkType` + `MessageSize`), Hello/Acknowledge/Error and
ReverseHello (§7.1.3 Establishing a connection, p. 83).

**Repo:** `crates/opcua-uacp/src/connection.rs` (`MessageType`, `ChunkType`,
`MessageHeader`, `HelloMessage`, `AcknowledgeMessage`, `ErrorMessage`,
`ReverseHelloMessage`).

**Tests:** `crates/opcua-uacp/src/connection.rs::tests::header_round_trips_and_sizes_match`,
`acknowledge_round_trips`, `error_round_trips`, `reverse_hello_round_trips`,
`unknown_message_type_rejected`, `oversized_string_rejected`.

**Status:** done

### §6.7 OPC UA Secure Conversation — chunk framing and message chunking

**Spec:** Part 6 §6.7, p. 62 — SecureChannel messages as chunks (OPN/MSG/CLO),
`AsymmetricAlgorithmSecurityHeader` (OPN) /
`SymmetricAlgorithmSecurityHeader` (MSG/CLO), `SequenceHeader`
(§6.7.2 MessageChunk structure). §6.7.1: "OPC UA Secure Conversation will
break OPC UA Messages into several pieces (called 'MessageChunks') that are
smaller than the buffer size allowed by the TransportProtocol"; §6.7.2.2
Table 57 `IsFinal` = `C` (intermediate) / `F` (final) / `A` (abort); §6.7.3:
"The sender indicates that the MessageChunk contains an error by setting the
IsFinal flag to 'A'" and "the receiver shall ignore the Message but shall not
close the SecureChannel".

**Repo:** `crates/opcua-uacp/src/securechannel.rs`
(`SecureChannel::open_chunk` / `message_chunks` / `message_chunk` /
`close_chunk` / `abort_chunk`, `set_send_buffer_size` +
`MIN_SEND_BUFFER_SIZE` (§6.7.1: at least 8192 bytes with RSA),
`open_incoming` reassembling across `C` chunks into `ChunkOutcome`
(`Incomplete`/`Complete`/`Abort`), the `MAX_REASSEMBLED_MESSAGE` cap,
`split_messages`, `parse_chunk`, `ParsedChunk`, `SECURITY_POLICY_NONE`);
secured chunk types via
`crates/opcua-uacp/src/crypto.rs::build_symmetric_chunk_typed` +
`OpenedChunk::chunk_type`. The buffer size negotiated in Hello/Acknowledge is
applied in `crates/opcua-server/src/server.rs` (`Server::process`) and
`crates/opcua-server/src/client.rs` (`Client::open_channel`).

**Tests:** `crates/opcua-uacp/src/securechannel.rs::tests::message_chunk_round_trip`,
`open_secure_channel_round_trip_none`, `chunked_message_round_trips_across_c_and_f`,
`abort_chunk_reports_the_error_and_keeps_the_channel`,
`split_messages_splits_by_message_size`;
end-to-end over both transports:
`crates/opcua-server/src/client.rs::tests::e2e_loopback_message_larger_than_the_buffer_is_chunked`,
`e2e_tcp_message_larger_than_the_buffer_is_chunked`.

**Status:** done

### §6.7 SecureChannel crypto — secured SecurityPolicies

**Spec:** Part 6 §6.7.2 (chunk security layout: SequenceHeader‖Body‖Padding‖
Signature, the signature covering MessageHeader…Padding, then encrypted),
§6.7.6 Deriving keys (p. 70, P-SHA256); Part 7 SecurityPolicies
`Basic256Sha256`, `Aes128_Sha256_RsaOaep`, `Aes256_Sha256_RsaPss`.

**Repo (crypto core):** `crates/opcua-uacp/src/crypto.rs` (feature `crypto`):
`SecurityPolicy` (URI/algorithm parameters), `p_sha256` + `derive_keys`
(§6.7.6: signing/encrypting/IV), symmetric `build_symmetric_chunk`/
`open_symmetric_chunk` (AES-128/256-CBC + HMAC-SHA256, padding, Sign and
SignAndEncrypt), asymmetric `build_asymmetric_chunk`/`open_asymmetric_chunk`
(RSA-OAEP-SHA1/SHA256 + RSA-PKCS#1-v1.5/PSS over SHA-256, block-wise OPN crypto
with padding/ExtraPaddingByte + certificate thumbprint via `sha1_thumbprint`).

**Repo (wiring):** `crates/opcua-uacp/src/securechannel.rs`
(`SecuritySession`, `SecureChannel::install_security`/`open_incoming`,
secured `message_chunk`/`close_chunk`), `crates/opcua-server/src/server.rs`
(`ServerSecurity`, `Server::set_security`, `open_secure_channel_secured`:
open the OPN → nonce → key derivation → secured OPN response + session install,
for ISSUE as well as RENEW),
`crates/opcua-server/src/client.rs` (`ClientSecurity`, `Client::set_security`,
`open_secure_channel_secured`). The trust store is the peer certificates /
public keys handed in; the RNG is caller-supplied
(`Box<dyn CryptoRngCore + Send>`, OS RNG under `std`).

**Tests:** `crates/opcua-uacp/src/crypto.rs::tests::p_sha256_is_deterministic_and_sized`,
`derive_keys_lengths_and_mirror`, `symmetric_sign_and_encrypt_round_trip`
(all 3 policies), `symmetric_sign_only_round_trip`, `symmetric_tamper_is_rejected`,
`asymmetric_round_trip_oaep_pkcs15`, `asymmetric_round_trip_oaep_sha256_pss`,
`asymmetric_wrong_signer_is_rejected`, `full_secured_handshake_flow`;
**a secured end-to-end per mode:**
`crates/opcua-server/src/client.rs::tests::e2e_secured_loopback_sign_and_encrypt`
(real RSA keys + OS RNG: encrypted+signed handshake → Read/Call/Write over
`Basic256Sha256`/SignAndEncrypt) and
`e2e_secured_loopback_sign_only_and_renew` (`Aes256_Sha256_RsaPss`/Sign:
handshake → Read, an `EndpointDescription` carrying
`MessageSecurityMode::Sign` and the policy URI, then RENEW over the secured
channel).

**Status:** done

### §5.2 OPC UA Binary — codec reuse

**Spec:** Part 6 §5.2, p. 18 — binary encoding (built-in types, NodeId,
Variant, DataValue, ExtensionObject, arrays).

**Repo:** reused from `crates/opcua-pubsub/src/binary/` via
`UaReader`/`UaWriter`/`UaEncode`/`UaDecode`; `NodeId`/`Variant`/`DataValue`/
`ExtensionObject` from `zerodds-opcua-gateway`.

**Tests:** `crates/opcua-pubsub/src/binary/builtin.rs::tests` (20 tests), among them
`nodeid_two_byte_compact_form`, `guid_encodes_data1_little_endian`,
`variant_scalar_int32`, `variant_multidim_array`, `data_value_full`;
`crates/opcua-pubsub/src/binary/io.rs::tests::primitive_roundtrip_is_little_endian`,
`::read_past_end_reports_eof`, `::floats_roundtrip`;
`crates/opcua-pubsub/src/binary/mod.rs::tests::read_array_rejects_oversized_length`,
`::read_string_array_roundtrip_within_bound`.

**Status:** done

### §7.2 OPC UA TCP — opc.tcp transport (end-to-end)

**Spec:** Part 6 §7.2, p. 86 — the UA-TCP mapping; transport profile
`uatcp-uasc-uabinary` (Part 7).

**Repo:** `crates/opcua-server/src/client.rs`
(`TcpTransport`, `serve_connection`, the `Transport` trait,
`LoopbackTransport`), `crates/opcua-server/src/server.rs` (`Server::process`).

**Tests:** `crates/opcua-server/src/client.rs::tests::e2e_tcp_connect_read_call`
(a real `opc.tcp` socket: connect/read/call over `TcpListener` + thread),
`e2e_loopback_connect_read_call`, `e2e_loopback_write_then_read`.

**Status:** done

## Part 4 — Service Sets

### §5.6.2 OpenSecureChannel

**Spec:** Part 4 §5.6.2, p. 21 f. — `OpenSecureChannelRequest`/`Response`,
`SecurityTokenRequestType` (Table 11: "ISSUE creates a new SecurityToken for a
new SecureChannel. RENEW creates a new SecurityToken for an existing
SecureChannel"), `secureChannelId` ("shall be null when creating a new
SecureChannel"), `ChannelSecurityToken` (RevisedLifetime). §5.6.2.1: "Clients
should request a new SecurityToken after 75 % of its lifetime has elapsed" and
"A Server application should limit the number of SecureChannels … the Server
shall close the oldest unused SecureChannel".

**Repo:** `crates/opcua-uacp/src/securechannel.rs`
(`OpenSecureChannelRequest`, `OpenSecureChannelResponse`,
`SecurityTokenRequestType`, `ChannelSecurityToken`, `SecureChannel::adopt_token`
/ `reset_sequences`), `crates/opcua-server/src/server.rs`
(`Server::open_secure_channel` and `open_secure_channel_secured` with an ISSUE
and a RENEW branch, `Server::channels` as the list of open SecureChannels with
`MAX_SECURE_CHANNELS` and eviction of the oldest, `chunk_channel_id` selecting
the channel per chunk from the header's `SecureChannelId`),
`crates/opcua-server/src/client.rs` (`Client::renew_secure_channel`,
`open_secure_channel_plain`/`open_secure_channel_secured` taking a
`SecurityTokenRequestType` — the RENEW OPN carries its own `SecureChannelId`,
the ISSUE OPN the null id).

**Tests:** `crates/opcua-uacp/src/securechannel.rs::tests::open_response_round_trip`,
`crates/opcua-server/src/client.rs::tests::e2e_loopback_connect_read_call`
(runs through OPN), `e2e_loopback_secure_channel_renew_keeps_the_channel_working`
(RENEW in mode `None`: the session and a Read survive the token change),
`e2e_secured_loopback_sign_only_and_renew` (RENEW over the secured channel),
`e2e_loopback_transfer_subscriptions_between_sessions` (two clients, two
SecureChannels open at once on the same server).

**Status:** done

### §5.7.2 CreateSession

**Spec:** Part 4 §5.7.2, p. 24 — `CreateSessionRequest`/`Response`
(SessionId, AuthenticationToken, RevisedSessionTimeout, ServerEndpoints,
ServerSignature).

**Repo:** `crates/opcua-server/src/services.rs`
(`CreateSessionRequest`, `CreateSessionResponse`),
`crates/opcua-server/src/server.rs` (`Server::create_session`: creates the
`Session`, draws the 32-byte `serverNonce` (`session_nonce`, from the
configured CSPRNG under `crypto`) and returns the `serverCertificate` plus an
`EndpointDescription` carrying the `UserTokenPolicy`s of the `UserStore`).

**Tests:** `crates/opcua-server/src/client.rs::tests::e2e_loopback_connect_read_call`,
`e2e_tcp_connect_read_call`, `e2e_loopback_user_name_identity_token`.

**Status:** done

### §5.7.3 ActivateSession — UserIdentityTokens

**Spec:** Part 4 §5.7.3, p. 29 ff. — `ActivateSessionRequest`/`Response`
(ClientSignature, LocaleIds, UserIdentityToken, UserTokenSignature,
ServerNonce). §5.7.3.1: "Once used, a serverNonce cannot be used again. For
that reason, the Server returns a new serverNonce each time the
ActivateSession Service is called" and "Clients can change the identity of a
user associated with a Session by calling the ActivateSession Service".
§5.7.3.2: "Null or empty user token shall always be interpreted as anonymous",
and the serverNonce "shall have a length between 32 and 128 bytes inclusive.
The Client shall check the length". §7.40 Table 181 — the four
UserIdentityTokens (`AnonymousIdentityToken` §7.40.3, `UserNameIdentityToken`
§7.40.4, `X509IdentityToken` §7.40.5, `IssuedIdentityToken` §7.40.6); §7.41
`UserTokenPolicy`, §7.42 `UserTokenType`. Result codes §5.7.3.3:
`Bad_IdentityTokenInvalid`, `Bad_IdentityTokenRejected`,
`Bad_UserAccessDenied`, `Bad_UserSignatureInvalid`.

**Repo:** `crates/opcua-server/src/identity.rs` (`UserIdentityToken` with all
four Table 181 types and their `_Encoding_DefaultBinary` NodeIds in
`token_ids`, `UserStore` with `add_user`/`add_certificate`/`add_issued_token`/
`set_allow_anonymous` and `validate`, `user_token_policy` for the §7.41 policy
in the `EndpointDescription`, `ClientUserId`),
`crates/opcua-server/src/services.rs` (`ActivateSessionRequest`,
`ActivateSessionResponse`), `crates/opcua-server/src/server.rs`
(`Server::set_user_store`, the `Session` list with `activated`/`user`/`nonce`,
`activate_session` validating the token and drawing a fresh nonce per call,
`require_activated_session` rejecting every session service before activation
with `Bad_SessionNotActivated`), `crates/opcua-server/src/client.rs`
(`Client::set_identity`, `change_user`, `user_token_signature` — the §6.1.8
signature over `serverCertificate‖serverNonce` via
`crates/opcua-uacp/src/crypto.rs::sign_with_policy`/`signature_data`).

**Tests:** `crates/opcua-server/src/identity.rs::tests::all_four_table_181_tokens_round_trip_through_the_extension_object`,
`null_token_is_anonymous`, `user_token_policy_round_trips`,
`store_accepts_each_token_type_and_rejects_bad_credentials`,
`anonymous_is_rejected_when_the_server_does_not_allow_it`;
`crates/opcua-server/src/services.rs::tests::session_lifecycle_round_trips`;
end-to-end `crates/opcua-server/src/client.rs::tests::e2e_loopback_user_name_identity_token`
(the published USERNAME policy, anonymous refused, a wrong password giving
`Bad_UserAccessDenied`, the right one activating) and
`e2e_loopback_x509_and_issued_identity_tokens_and_user_change`
(X.509 and IssuedToken activation, an identity change on the running session,
a forged token and an untrusted certificate refused),
`e2e_loopback_session_must_be_activated_and_close_deletes_subscriptions`.

**Status:** done

### §5.7.4 CloseSession

**Spec:** Part 4 §5.7.4, p. 31 — `CloseSessionRequest` (DeleteSubscriptions)
/`Response`.

**Repo:** `crates/opcua-server/src/services.rs`
(`CloseSessionRequest`, `CloseSessionResponse`).

**Tests:** `crates/opcua-server/src/services.rs::tests::session_lifecycle_round_trips`.

**Status:** done

### §5.11.2 Read — every attribute of the AddressSpace

**Spec:** Part 4 §5.11.2, p. 45 ff. — "This Service is used to read one or more
**Attributes** of one or more Nodes"; `ReadRequest` (MaxAge, TimestampsToReturn,
`ReadValueId[]`) / `ReadResponse` (`DataValue[]`). §5.11.2.2: "Negative values
are invalid for maxAge"; `indexRange` per §7.27 / Annex A.3; §7.39
`TimestampsToReturn`. Result codes §5.11.2.3/.4: `Bad_MaxAgeInvalid`,
`Bad_TimestampsToReturnInvalid`, `Bad_NothingToDo`, `Bad_NodeIdUnknown`,
`Bad_AttributeIdInvalid`, `Bad_IndexRangeInvalid`, `Bad_IndexRangeNoData`,
`Bad_NotReadable`, `Bad_DataEncodingInvalid`. The attribute ids are listed in
Part 6 Annex A.1 Table A.1 (1 `NodeId` … 27 `AccessLevelEx`), the NodeClass
each belongs to in Part 3 §5.

**Repo:** `crates/opcua-server/src/attributes.rs` (`attribute_ids` = Table A.1,
`ClassAttributes` holding the Part 3 attribute set per NodeClass, the
`access_level` bits per Part 3 §8.57, `NumericRange` (parser + `apply`) per
§7.27/Annex A.3, `TimestampsToReturn`),
`crates/opcua-server/src/address_space.rs` (`NodeMeta` with
Description/WriteMask/UserWriteMask/ClassAttributes,
`AddressSpace::read_attribute` returning every attribute of the NodeClass and
`Bad_AttributeIdInvalid` for every other), `crates/opcua-server/src/status.rs`
(the StatusCodes), `crates/opcua-server/src/services.rs`
(`ReadRequest`, `ReadResponse`, `ReadValueId`),
`crates/opcua-server/src/server.rs` (`Server::read` validating maxAge and
TimestampsToReturn, `read_one` applying `indexRange` and `dataEncoding`),
`crates/opcua-server/src/client.rs` (`Client::read_attributes`,
`read_attribute`, `read_values`, `read_value_id`).

**Tests:** `crates/opcua-server/src/attributes.rs::tests::numeric_range_parses_the_annex_a3_grammar`,
`numeric_range_rejects_the_forms_the_spec_calls_invalid`,
`numeric_range_apply_selects_and_clamps`,
`timestamps_to_return_strips_what_was_not_asked_for`;
`crates/opcua-server/src/services.rs::tests::read_request_round_trips`,
`read_response_round_trips`;
end-to-end `crates/opcua-server/src/client.rs::tests::e2e_loopback_reads_every_attribute_of_a_variable`
(16 attributes of one Variable, each `Good` with its value checked;
`Executable` and an id outside Table A.1 as `Bad_AttributeIdInvalid`),
`e2e_loopback_index_range_read_and_write`,
`e2e_loopback_read_honours_timestamps_to_return_and_max_age`,
`read_unknown_node_yields_bad_status`.

**Status:** done

### §5.11.4 Write — every writeable attribute

**Spec:** Part 4 §5.11.4, p. 50 ff. — "This Service is used to write values to
one or more **Attributes** of one or more Nodes"; `attributeId` "shall be a
valid Attribute id … The IntegerIds for the Attributes are defined in
OPC 10000-6"; `indexRange` per §7.27; "If the SourceTimestamp or the
ServerTimestamp is specified, the Server shall use these values. The Server
returns a Bad_WriteNotSupported error if it does not support writing of
timestamps." Result codes §5.11.4.4: `Bad_NodeIdUnknown`,
`Bad_AttributeIdInvalid`, `Bad_IndexRangeInvalid`, `Bad_IndexRangeNoData`,
`Bad_WriteNotSupported`, `Bad_NotWritable`, `Bad_TypeMismatch`. Which attribute
is writeable is defined in Part 3 §8.60 Table 44 (`AttributeWriteMask`,
bits 0–25) — with the explicit special case that the `Value` attribute of a
Variable is not in the mask but governed by `AccessLevel`/`UserAccessLevel`
(Part 3 §8.57).

**Repo:** `crates/opcua-server/src/attributes.rs` (the `write_mask` bits =
Table 44, `write_mask_bit` mapping attribute id to bit and deliberately
returning `None` for `Value`), `crates/opcua-server/src/address_space.rs`
(`AddressSpace::write_attribute`: WriteMask ∧ UserWriteMask, then
`write_value_attribute` with `CurrentWrite`/`TimestampWrite`/`StatusWrite`
or `ValueForVariableType`, and `write_class_attribute` per NodeClass;
`Bad_TypeMismatch` on a wrong builtin type, `Bad_WriteNotSupported` for
`NodeId`/`NodeClass`), `crates/opcua-server/src/services.rs`
(`WriteRequest`, `WriteResponse`, `WriteValue`),
`crates/opcua-server/src/server.rs` (`Server::write_one` resolving
`indexRange` against the current value), `crates/opcua-server/src/client.rs`
(`Client::write_attributes`, `write_values`).

**Tests:** `crates/opcua-server/src/attributes.rs::tests::write_mask_bit_matches_table_44`,
`numeric_range_replace_writes_in_place`;
`crates/opcua-server/src/services.rs::tests::write_round_trips`;
end-to-end `crates/opcua-server/src/client.rs::tests::e2e_loopback_write_respects_the_write_mask_and_access_level`
(a set mask bit writes, a cleared one gives `Bad_NotWritable`, a missing
`CurrentWrite` gives `Bad_NotWritable`, a wrong type gives `Bad_TypeMismatch`,
and the refused write leaves the value unchanged),
`e2e_loopback_index_range_read_and_write`, `e2e_loopback_write_then_read`.

**Status:** done

### §5.12.2 Call

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

### §5.9.2 / §5.9.3 Browse + BrowseNext (View Service Set)

**Spec:** Part 4 §5.9.2 Browse (p. 38 ff.), §5.9.3 BrowseNext (p. 40 ff.),
§7.29 ReferenceDescription (p. 148), §7.6 BrowseResult (p. 118), §7.9
ContinuationPoint (p. 129), §7.44 ViewDescription (p. 169); ExpandedNodeId
Part 6 §5.2.2.10. §5.9.2.2 `requestedMaxReferencesPerNode`: "Indicates the
maximum number of references to return for each starting Node … The value 0
indicates that the Client is imposing no limitation." §5.9.3.1: "This Service
is used to request the next set of Browse or BrowseNext response information
that is too large to be sent in a single response." §5.9.3.2
`releaseContinuationPoints`: "TRUE — passed continuationPoints shall be reset
to free resources in the Server. The continuation points are released and the
results and diagnosticInfos arrays are empty." Result codes §5.9.2.4/§5.9.3.4:
`Bad_NodeIdUnknown`, `Bad_BrowseDirectionInvalid`, `Bad_ReferenceTypeIdInvalid`,
`Bad_NoContinuationPoints`, `Bad_ContinuationPointInvalid`, `Bad_NothingToDo`.

**Repo:** `crates/opcua-server/src/address_space.rs` (node/reference model:
`NodeMeta`, `ReferenceRecord`, `NodeClass`, the ns0 `reference_types`
constants, `add_node`/`add_reference`/`node_meta`/`browse` with direction +
reference-subtype filter (`includeSubtypes`) + NodeClass mask),
`crates/opcua-server/src/services.rs` (`ViewDescription`, `BrowseDescription`,
`ReferenceDescription`, `BrowseResult`, `BrowseRequest`/`BrowseResponse`,
`BrowseNextRequest`/`BrowseNextResponse` (NodeIds 533/536), the ExpandedNodeId
wire form); `crates/opcua-server/src/server.rs` (`browse_one`: reference filter
+ `resultMask` projection via `project_reference`, `Bad_NodeIdUnknown` for
unknown nodes, `Bad_BrowseDirectionInvalid` and `Bad_ReferenceTypeIdInvalid`
via `is_known_reference_type`, capping at `requestedMaxReferencesPerNode` with
`allocate_continuation_point`; `browse_next_one` continuing or releasing; the
`ContinuationPoint` list bounded by `MAX_CONTINUATION_POINTS` →
`Bad_NoContinuationPoints`), `crates/opcua-server/src/client.rs`
(`Client::browse`, `browse_limited`, `browse_next`, `browse_all`).

**Tests:** `crates/opcua-server/src/address_space.rs::tests::browse_forward_and_inverse`,
`browse_subtype_filter_excludes_non_hierarchical`, `browse_node_class_mask_filters_targets`,
`crates/opcua-server/src/services.rs::tests::browse_round_trips`,
`crates/opcua-server/src/client.rs::tests::e2e_loopback_browse`
(Objects→Boiler→Temperature over Organizes/HasComponent) and
`e2e_loopback_browse_next_walks_continuation_points` (five references,
`requestedMaxReferencesPerNode` = 2 → a first batch plus a continuation point,
BrowseNext returning the remaining three in order, `release` freeing the point
so a second use of it is `Bad_ContinuationPointInvalid`, and `browse_all`
walking the whole chain at batch size 1).

**Status:** done

### §5.5.4 / §5.5.2 Discovery — GetEndpoints + FindServers

**Spec:** Part 4 §5.5.4 GetEndpoints (p. 14), §5.5.2 FindServers (p. 12) —
session-less discovery services (SecureChannel only, no session required).

**Repo:** `crates/opcua-server/src/services.rs`
(`GetEndpointsRequest`/`GetEndpointsResponse` (NodeIds 426/429),
`FindServersRequest`/`FindServersResponse` (NodeIds 422/425)),
`crates/opcua-server/src/server.rs` (dispatch arms: GetEndpoints returns the
server's `EndpointDescription` — `Server::endpoint` with the `UserTokenPolicy`s
from the `UserStore` and, under `crypto`, the `MessageSecurityMode` and policy
URI actually configured (`endpoint_security`) —, FindServers its
`ApplicationDescription`), `crates/opcua-server/src/client.rs`
(`Client::open_channel` — Hello+OPN without a session — plus
`Client::get_endpoints`/`find_servers`).

**Tests:** `crates/opcua-server/src/services.rs::tests::discovery_requests_round_trip`,
`crates/opcua-server/src/client.rs::tests::e2e_loopback_discovery`
(session-less channel → GetEndpoints returns an endpoint with SecurityPolicy
`None`, FindServers returns the server);
`e2e_secured_loopback_sign_only_and_renew` checks the same endpoint under
`Aes256_Sha256_RsaPss`/`Sign`, `e2e_loopback_user_name_identity_token` the
published USERNAME `UserTokenPolicy`.

**Status:** done

### §5.13 / §5.14 Subscription + MonitoredItem Service Sets

**Spec:** Part 4 §5.13.2 CreateMonitoredItems (p. 63), §5.13.3
ModifyMonitoredItems (p. 65), §5.13.4 SetMonitoringMode (p. 67), §5.13.5
SetTriggering (p. 68), §5.13.6 DeleteMonitoredItems (p. 69), §5.14.2
CreateSubscription (p. 76), §5.14.3 ModifySubscription (p. 78), §5.14.4
SetPublishingMode (p. 79), §5.14.5 Publish (p. 81), §5.14.6 Republish (p. 83),
§5.14.7 TransferSubscriptions (p. 83), §5.14.8 DeleteSubscriptions (p. 85);
§7.21 MonitoringParameters, §7.23 MonitoringMode, §7.25.2
DataChangeNotification (p. 145), §7.25.4 StatusChangeNotification (p. 146),
§7.26 NotificationMessage (p. 147), §7.39 TimestampsToReturn.

The sampling rules this server is measured against are in §5.14.1: the
subscription "attempts to send a NotificationMessage" per cycle (b), a cycle
without notifications produces a keep-alive message (f), and the session keeps
a "retransmission queue of sent NotificationMessages … until they are
acknowledged" (i). §5.13.4.1: "Setting the mode to DISABLED causes all queued
Notifications to be deleted." §5.13.6.1: "When a MonitoredItem is deleted, its
triggered item links are also deleted." §5.14.3.2: "If the requested value is 0
or negative, the Server shall revise with the fastest supported publishing
interval", "The lifetime count shall be a minimum of three times the keep-alive
count". §5.14.7.1: "the Server shall issue a StatusChangeNotification
notificationMessage with the status code Good_SubscriptionTransferred to the
old Session".

**Repo:** `crates/opcua-server/src/services.rs` — the twelve services of both
service sets as wire types: `CreateSubscriptionRequest`/`Response`,
`ModifySubscriptionRequest`/`Response` (793/796), `SetPublishingModeRequest`/
`Response`, `PublishRequest`/`Response`, `RepublishRequest`/`Response`
(832/835), `TransferSubscriptionsRequest`/`Response` + `TransferResult`
(841/844/838), `DeleteSubscriptionsRequest`/`Response`,
`CreateMonitoredItemsRequest`/`Response`, `ModifyMonitoredItemsRequest`/
`Response` + `MonitoredItemModifyRequest`/`MonitoredItemModifyResult`
(763/766/757/760), `SetMonitoringModeRequest`/`Response` (769/772),
`SetTriggeringRequest`/`Response` (775/778), `DeleteMonitoredItemsRequest`/
`Response` (781/784), plus `MonitoringParameters`,
`SubscriptionAcknowledgement`, `MonitoredItemNotification`,
`DataChangeNotification` (811) and `StatusChangeNotification` (820).

`crates/opcua-server/src/server.rs` — the subscription state: `Subscription`
with its revised parameters (`revise_publishing_interval`/`revise_keep_alive`/
`revise_lifetime` per §5.14.3.2), the `retransmission_queue` of depth
`RETRANSMISSION_QUEUE_DEPTH`, `MonitoredItem` with `MonitoringMode`,
`queue`/`queue_size`/`discard_oldest` (§5.13.1.5, `trim_queue`),
`index_range`, `timestamps_to_return` and `triggered_items` (§5.13.5);
`Server::publish` processes the acknowledgements first, reports the §7.25.4
StatusChangeNotification to a session whose subscription was taken over,
then samples via `sample_subscription`/`sample_item` and delivers the queues of
the reporting items together with the sampling items they trigger, bounded by
`max_notifications_per_publish` (`more_notifications`); with nothing to report
a keep-alive message goes out, and with no subscription at all the service
result `Bad_NoSubscription`. `crates/opcua-server/src/attributes.rs`
(`MonitoringMode`, `TimestampsToReturn`, `NumericRange`) and
`crates/opcua-server/src/status.rs` (the StatusCodes of both service sets).

`crates/opcua-server/src/client.rs` — `create_subscription`,
`modify_subscription`, `create_monitored_items`, `modify_monitored_items`,
`set_monitoring_mode`, `set_triggering`, `delete_monitored_items`, `publish`,
`publish_acknowledging` (→ `PublishOutcome`), `republish`,
`transfer_subscriptions`, `delete_subscriptions`.

**Tests:** `crates/opcua-server/src/services.rs::tests::subscription_round_trips`,
`publish_data_change_notification_round_trips`;
end-to-end in `crates/opcua-server/src/client.rs::tests`:
`e2e_loopback_subscription_publish` (initial value, unchanged cycle, value
change, deletion, then `Bad_NoSubscription`),
`e2e_loopback_modify_subscription_and_monitored_items` (revised intervals, the
new client handle reaching the next Publish, an unknown item id giving
`Bad_MonitoredItemIdInvalid`),
`e2e_loopback_set_monitoring_mode_and_delete_monitored_items`
(DISABLED discarding the queue, REPORTING resuming it, a second delete giving
`Bad_MonitoredItemIdInvalid`),
`e2e_loopback_set_triggering_releases_a_sampling_item`
(a sampling item only reports once linked to the triggering item),
`e2e_loopback_republish_and_acknowledgements`
(Republish returning the message again, an acknowledgement removing it from the
queue, then `Bad_MessageNotAvailable`, and an unknown sequence number giving
`Bad_SequenceNumberUnknown`),
`e2e_loopback_transfer_subscriptions_between_sessions`
(takeover into a second session, `Good_SubscriptionTransferred` to the old one,
`sendInitialValues` in the new session's first Publish, `Bad_NothingToDo` for
the session's own subscription and `Bad_SubscriptionIdInvalid` for an unknown
one).

**Status:** done

### §7.32/§7.33 Common service headers + common types

**Spec:** Part 4 §7.32 RequestHeader (p. 150), §7.33 ResponseHeader (p. 151),
§7.12 DiagnosticInfo (p. 131), §7.36 SignatureData (p. 153), §7.28 ReadValueId
(p. 148), §7.14 EndpointDescription (p. 133), §7.20 MessageSecurityMode
(p. 135).

**Repo:** `crates/opcua-uacp/src/securechannel.rs`
(`RequestHeader`, `ResponseHeader`, `null_extension_object`, a minimal
DiagnosticInfo skip), `crates/opcua-server/src/services.rs`
(`SignatureData`, `ReadValueId`, header encode/decode against the public
structs), `crates/opcua-server/src/wire.rs` (String/ByteString/Array/
DiagnosticInfo helpers), `MessageSecurityMode`/`EndpointDescription` from
`zerodds-opcua-pubsub`.

**Tests:** `crates/opcua-uacp/src/securechannel.rs::tests::response_header_with_string_table_round_trips`,
`crates/opcua-server/src/services.rs::tests::read_response_round_trips`.

**Status:** done

---

## Audit status

16 done / 0 partial / 0 open / 0 n/a (informative) / 0 n/a (rejected).

Test run: `cargo test -p zerodds-opcua-uacp -p zerodds-opcua-server --features zerodds-opcua-server/crypto`
— 81 tests green (23 UACP incl. crypto and message chunking + 58 server), 0
failed; including the real `opc.tcp` end-to-end, a chunked Message over both
transports, secured loopbacks in `Sign` and `SignAndEncrypt`, SecureChannel
renewal, all four UserIdentityTokens, the full attribute set, BrowseNext with
continuation points and both the MonitoredItem and Subscription service sets.

Open items: none. Decision records: none.
