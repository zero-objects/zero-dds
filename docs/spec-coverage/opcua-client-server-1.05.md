# OPC UA Part 4 — Client/Server (UACP + SecureChannel) — Spec-Coverage

**Quelle:** `internal/standards/cache/opcfoundation/opcua-part4-services-1.05.07.pdf`
(Services). Cross-Spec:
`opcua-part6-mappings-1.05.07.pdf` (UACP §7.1, OPC UA Secure Conversation §6.7,
Binär-Codec §5.2),
`opcua-part7-profiles-1.05.02.pdf` (Transport-Profile, SecurityPolicies)
(alle im selben Cache; aus IP-/Copyright-Gründen nicht im Repo getrackt).

**Öffentliche Spec:** OPC-UA-Online-Referenz <https://reference.opcfoundation.org/>
(Part 4 Services, Part 6 Mappings, Part 7 Profiles).

**Kontext:** Nativer pure-Rust `no_std + alloc` OPC-UA-Client/Server-Stack —
das Request/Response-Gegenstück zum UADP-PubSub-Stack
(`zerodds-opcua-pubsub`). Gleiches Vorgehen wie bei den vollen
CORBA-/MQTT-/AMQP-Stacks von ZeroDDS: echter Wire-Stack, nicht nur eine
Gateway-Brücke. Der Part-6-Binär-Codec wird aus `zerodds-opcua-pubsub`
wiederverwendet. `forbid(unsafe_code)`.

Implementation:

- `crates/opcua-uacp/` — UACP (Hello/Ack/Error/ReverseHello), OPC UA Secure
  Conversation (OPN/MSG/CLO-Chunks, Security-Header, SequenceHeader,
  ChannelSecurityToken, Message-Chunking über `C`/`F`/`A` mit Reassembly) und
  die gesicherten SecurityPolicies (Feature `crypto`).
- `crates/opcua-server/` — die Service-Sets Discovery, SecureChannel, Session,
  Attribute, View, Method, MonitoredItem und Subscription, ein in-memory
  AddressSpace mit Part-3-Attributmodell und Referenzgraph, der
  `Server`-Zustandsautomat mit Sessions/UserStore/SecureChannel-Liste, der
  `Client` und der `opc.tcp`-TCP-Transport.

SecurityMode `None` **und** die gesicherten SecurityPolicies (RSA-/AES-Krypto,
`Basic256Sha256`/`Aes128_Sha256_RsaOaep`/`Aes256_Sha256_RsaPss`) sind in beiden
Betriebsarten (`Sign`, `SignAndEncrypt`) end-to-end implementiert. Offene
Punkte: keine.

---

## Part 6 — UACP, SecureChannel und Binär-Codec

### §7.1 OPC UA Connection Protocol (UACP)

**Spec:** Part 6 §7.1, S. 80 — Message-Struktur (8-Byte-Header
`MessageType` + `ChunkType` + `MessageSize`), Hello/Acknowledge/Error sowie
ReverseHello (§7.1.3 Establishing a connection, S. 83).

**Repo:** `crates/opcua-uacp/src/connection.rs` (`MessageType`, `ChunkType`,
`MessageHeader`, `HelloMessage`, `AcknowledgeMessage`, `ErrorMessage`,
`ReverseHelloMessage`).

**Tests:** `crates/opcua-uacp/src/connection.rs::tests::header_round_trips_and_sizes_match`,
`acknowledge_round_trips`, `error_round_trips`, `reverse_hello_round_trips`,
`unknown_message_type_rejected`, `oversized_string_rejected`.

**Status:** done

### §6.7 OPC UA Secure Conversation — Chunk-Rahmung und Message-Chunking

**Spec:** Part 6 §6.7, S. 62 — SecureChannel-Nachrichten als Chunks
(OPN/MSG/CLO), `AsymmetricAlgorithmSecurityHeader` (OPN) /
`SymmetricAlgorithmSecurityHeader` (MSG/CLO), `SequenceHeader`
(§6.7.2 MessageChunk structure). §6.7.1: „OPC UA Secure Conversation will
break OPC UA Messages into several pieces (called 'MessageChunks') that are
smaller than the buffer size allowed by the TransportProtocol"; §6.7.2.2
Table 57 `IsFinal` = `C` (intermediate) / `F` (final) / `A` (abort); §6.7.3:
„The sender indicates that the MessageChunk contains an error by setting the
IsFinal flag to 'A'" und „the receiver shall ignore the Message but shall not
close the SecureChannel".

**Repo:** `crates/opcua-uacp/src/securechannel.rs`
(`SecureChannel::open_chunk` / `message_chunks` / `message_chunk` /
`close_chunk` / `abort_chunk`, `set_send_buffer_size` +
`MIN_SEND_BUFFER_SIZE` (§6.7.1: mindestens 8192 Byte bei RSA),
`open_incoming` mit Reassembly über `C`-Chunks und `ChunkOutcome`
(`Incomplete`/`Complete`/`Abort`), `MAX_REASSEMBLED_MESSAGE`-Deckel,
`split_messages`, `parse_chunk`, `ParsedChunk`, `SECURITY_POLICY_NONE`);
gesicherte Chunk-Typen über
`crates/opcua-uacp/src/crypto.rs::build_symmetric_chunk_typed` +
`OpenedChunk::chunk_type`. Verdrahtung der Puffergröße aus Hello/Acknowledge
in `crates/opcua-server/src/server.rs` (`Server::process`) und
`crates/opcua-server/src/client.rs` (`Client::open_channel`).

**Tests:** `crates/opcua-uacp/src/securechannel.rs::tests::message_chunk_round_trip`,
`open_secure_channel_round_trip_none`, `chunked_message_round_trips_across_c_and_f`,
`abort_chunk_reports_the_error_and_keeps_the_channel`,
`split_messages_splits_by_message_size`;
End-to-End über beide Transporte:
`crates/opcua-server/src/client.rs::tests::e2e_loopback_message_larger_than_the_buffer_is_chunked`,
`e2e_tcp_message_larger_than_the_buffer_is_chunked`.

**Status:** done

### §6.7 SecureChannel-Krypto — gesicherte SecurityPolicies

**Spec:** Part 6 §6.7.2 (Chunk-Security-Layout: SequenceHeader‖Body‖Padding‖
Signature, Signatur über MessageHeader…Padding, danach verschlüsselt),
§6.7.6 Deriving keys (S. 70, P-SHA256); Part 7 SecurityPolicies
`Basic256Sha256`, `Aes128_Sha256_RsaOaep`, `Aes256_Sha256_RsaPss`.

**Repo (Krypto-Kern):** `crates/opcua-uacp/src/crypto.rs` (Feature `crypto`):
`SecurityPolicy` (URI/Algo-Parameter), `p_sha256` + `derive_keys` (§6.7.6:
Signing/Encrypting/IV), symmetrisch `build_symmetric_chunk`/
`open_symmetric_chunk` (AES-128/256-CBC + HMAC-SHA256, Padding, Sign &
SignAndEncrypt), asymmetrisch `build_asymmetric_chunk`/`open_asymmetric_chunk`
(RSA-OAEP-SHA1/SHA256 + RSA-PKCS#1-v1.5/PSS über SHA-256, block-weise OPN-Krypto
mit Padding/ExtraPaddingByte + Zertifikat-Thumbprint via `sha1_thumbprint`).

**Repo (Verdrahtung):** `crates/opcua-uacp/src/securechannel.rs`
(`SecuritySession`, `SecureChannel::install_security`/`open_incoming`,
gesicherte `message_chunk`/`close_chunk`), `crates/opcua-server/src/server.rs`
(`ServerSecurity`, `Server::set_security`, `open_secure_channel_secured`:
OPN öffnen → Nonce → Key-Derivation → gesicherte OPN-Antwort + Session-Install,
ISSUE wie RENEW),
`crates/opcua-server/src/client.rs` (`ClientSecurity`, `Client::set_security`,
`open_secure_channel_secured`). Trust-Store = übergebene Peer-Zertifikate/
-Public-Keys; RNG Caller-bereitgestellt (`Box<dyn CryptoRngCore + Send>`,
OS-RNG unter `std`).

**Tests:** `crates/opcua-uacp/src/crypto.rs::tests::p_sha256_is_deterministic_and_sized`,
`derive_keys_lengths_and_mirror`, `symmetric_sign_and_encrypt_round_trip`
(alle 3 Policies), `symmetric_sign_only_round_trip`, `symmetric_tamper_is_rejected`,
`asymmetric_round_trip_oaep_pkcs15`, `asymmetric_round_trip_oaep_sha256_pss`,
`asymmetric_wrong_signer_is_rejected`, `full_secured_handshake_flow`;
**gesicherter End-to-End je Betriebsart:**
`crates/opcua-server/src/client.rs::tests::e2e_secured_loopback_sign_and_encrypt`
(echte RSA-Keys + OS-RNG: verschlüsselter+signierter Handshake → Read/Call/Write
über `Basic256Sha256`/SignAndEncrypt) und
`e2e_secured_loopback_sign_only_and_renew` (`Aes256_Sha256_RsaPss`/Sign:
Handshake → Read, `EndpointDescription` mit `MessageSecurityMode::Sign` und
Policy-URI, dann RENEW über den gesicherten Kanal).

**Status:** done

### §5.2 OPC UA Binary — Codec-Wiederverwendung

**Spec:** Part 6 §5.2, S. 18 — Binär-Kodierung (Built-in-Typen, NodeId,
Variant, DataValue, ExtensionObject, Arrays).

**Repo:** wiederverwendet aus `crates/opcua-pubsub/src/binary/` über
`UaReader`/`UaWriter`/`UaEncode`/`UaDecode`; `NodeId`/`Variant`/`DataValue`/
`ExtensionObject` aus `zerodds-opcua-gateway`.

**Tests:** `crates/opcua-pubsub/src/binary/builtin.rs::tests` (20 Tests), darunter
`nodeid_two_byte_compact_form`, `guid_encodes_data1_little_endian`,
`variant_scalar_int32`, `variant_multidim_array`, `data_value_full`;
`crates/opcua-pubsub/src/binary/io.rs::tests::primitive_roundtrip_is_little_endian`,
`::read_past_end_reports_eof`, `::floats_roundtrip`;
`crates/opcua-pubsub/src/binary/mod.rs::tests::read_array_rejects_oversized_length`,
`::read_string_array_roundtrip_within_bound`.

**Status:** done

### §7.2 OPC UA TCP — opc.tcp-Transport (End-to-End)

**Spec:** Part 6 §7.2, S. 86 — UA-TCP-Mapping; Transport-Profil
`uatcp-uasc-uabinary` (Part 7).

**Repo:** `crates/opcua-server/src/client.rs`
(`TcpTransport`, `serve_connection`, `Transport`-Trait, `LoopbackTransport`),
`crates/opcua-server/src/server.rs` (`Server::process`).

**Tests:** `crates/opcua-server/src/client.rs::tests::e2e_tcp_connect_read_call`
(echter `opc.tcp`-Socket: Connect/Read/Call über `TcpListener`+Thread),
`e2e_loopback_connect_read_call`, `e2e_loopback_write_then_read`.

**Status:** done

## Part 4 — Service Sets

### §5.6.2 OpenSecureChannel

**Spec:** Part 4 §5.6.2, S. 21 f. — `OpenSecureChannelRequest`/`Response`,
`SecurityTokenRequestType` (Table 11: „ISSUE creates a new SecurityToken for a
new SecureChannel. RENEW creates a new SecurityToken for an existing
SecureChannel"), `secureChannelId` („shall be null when creating a new
SecureChannel"), `ChannelSecurityToken` (RevisedLifetime). §5.6.2.1: „Clients
should request a new SecurityToken after 75 % of its lifetime has elapsed" und
„A Server application should limit the number of SecureChannels … the Server
shall close the oldest unused SecureChannel".

**Repo:** `crates/opcua-uacp/src/securechannel.rs`
(`OpenSecureChannelRequest`, `OpenSecureChannelResponse`,
`SecurityTokenRequestType`, `ChannelSecurityToken`, `SecureChannel::adopt_token`
/ `reset_sequences`), `crates/opcua-server/src/server.rs`
(`Server::open_secure_channel` und `open_secure_channel_secured` mit ISSUE- und
RENEW-Zweig, `Server::channels` als Liste offener SecureChannels mit
`MAX_SECURE_CHANNELS` + Verdrängung des ältesten, `chunk_channel_id` wählt den
Kanal je Chunk aus der `SecureChannelId` des Headers),
`crates/opcua-server/src/client.rs` (`Client::renew_secure_channel`,
`open_secure_channel_plain`/`open_secure_channel_secured` mit
`SecurityTokenRequestType`-Parameter — die RENEW-OPN trägt die eigene
`SecureChannelId`, die ISSUE-OPN die Null-Id).

**Tests:** `crates/opcua-uacp/src/securechannel.rs::tests::open_response_round_trip`,
`crates/opcua-server/src/client.rs::tests::e2e_loopback_connect_read_call`
(durchläuft OPN), `e2e_loopback_secure_channel_renew_keeps_the_channel_working`
(RENEW im Modus `None`: Session und Read überleben den Token-Wechsel),
`e2e_secured_loopback_sign_only_and_renew` (RENEW über den gesicherten Kanal),
`e2e_loopback_transfer_subscriptions_between_sessions` (zwei Clients, zwei
gleichzeitig offene SecureChannels auf demselben Server).

**Status:** done

### §5.7.2 CreateSession

**Spec:** Part 4 §5.7.2, S. 24 — `CreateSessionRequest`/`Response`
(SessionId, AuthenticationToken, RevisedSessionTimeout, ServerEndpoints,
ServerSignature).

**Repo:** `crates/opcua-server/src/services.rs`
(`CreateSessionRequest`, `CreateSessionResponse`),
`crates/opcua-server/src/server.rs` (`Server::create_session`: legt die
`Session` an, zieht die 32-Byte-`serverNonce` (`session_nonce`, unter `crypto`
aus dem konfigurierten CSPRNG) und liefert `serverCertificate` +
`EndpointDescription` mit den `UserTokenPolicy`s des `UserStore`).

**Tests:** `crates/opcua-server/src/client.rs::tests::e2e_loopback_connect_read_call`,
`e2e_tcp_connect_read_call`, `e2e_loopback_user_name_identity_token`.

**Status:** done

### §5.7.3 ActivateSession — UserIdentityTokens

**Spec:** Part 4 §5.7.3, S. 29 ff. — `ActivateSessionRequest`/`Response`
(ClientSignature, LocaleIds, UserIdentityToken, UserTokenSignature,
ServerNonce). §5.7.3.1: „Once used, a serverNonce cannot be used again. For
that reason, the Server returns a new serverNonce each time the
ActivateSession Service is called" und „Clients can change the identity of a
user associated with a Session by calling the ActivateSession Service".
§5.7.3.2: „Null or empty user token shall always be interpreted as anonymous",
serverNonce „shall have a length between 32 and 128 bytes inclusive. The Client
shall check the length". §7.40 Table 181 — die vier UserIdentityTokens
(`AnonymousIdentityToken` §7.40.3, `UserNameIdentityToken` §7.40.4,
`X509IdentityToken` §7.40.5, `IssuedIdentityToken` §7.40.6); §7.41
`UserTokenPolicy`, §7.42 `UserTokenType`. Ergebnis-Codes §5.7.3.3:
`Bad_IdentityTokenInvalid`, `Bad_IdentityTokenRejected`,
`Bad_UserAccessDenied`, `Bad_UserSignatureInvalid`.

**Repo:** `crates/opcua-server/src/identity.rs` (`UserIdentityToken` mit allen
vier Table-181-Typen und ihren `_Encoding_DefaultBinary`-NodeIds in
`token_ids`, `UserStore` mit `add_user`/`add_certificate`/`add_issued_token`/
`set_allow_anonymous` und `validate`, `user_token_policy` für die §7.41-Policy
in der `EndpointDescription`, `ClientUserId`),
`crates/opcua-server/src/services.rs` (`ActivateSessionRequest`,
`ActivateSessionResponse`), `crates/opcua-server/src/server.rs`
(`Server::set_user_store`, `Session`-Liste mit `activated`/`user`/`nonce`,
`activate_session` prüft das Token und zieht je Aufruf eine neue Nonce,
`require_activated_session` weist jeden Session-Dienst vor der Aktivierung mit
`Bad_SessionNotActivated` ab), `crates/opcua-server/src/client.rs`
(`Client::set_identity`, `change_user`, `user_token_signature` — die
§6.1.8-Signatur über `serverCertificate‖serverNonce` via
`crates/opcua-uacp/src/crypto.rs::sign_with_policy`/`signature_data`).

**Tests:** `crates/opcua-server/src/identity.rs::tests::all_four_table_181_tokens_round_trip_through_the_extension_object`,
`null_token_is_anonymous`, `user_token_policy_round_trips`,
`store_accepts_each_token_type_and_rejects_bad_credentials`,
`anonymous_is_rejected_when_the_server_does_not_allow_it`;
`crates/opcua-server/src/services.rs::tests::session_lifecycle_round_trips`;
End-to-End `crates/opcua-server/src/client.rs::tests::e2e_loopback_user_name_identity_token`
(publizierte USERNAME-Policy, Anonymous abgelehnt, falsches Passwort
`Bad_UserAccessDenied`, richtiges aktiviert) und
`e2e_loopback_x509_and_issued_identity_tokens_and_user_change`
(X.509- und IssuedToken-Aktivierung, Identitätswechsel auf der laufenden
Session, gefälschtes Token und unbekanntes Zertifikat abgelehnt),
`e2e_loopback_session_must_be_activated_and_close_deletes_subscriptions`.

**Status:** done

### §5.7.4 CloseSession

**Spec:** Part 4 §5.7.4, S. 31 — `CloseSessionRequest` (DeleteSubscriptions)
/`Response`.

**Repo:** `crates/opcua-server/src/services.rs`
(`CloseSessionRequest`, `CloseSessionResponse`).

**Tests:** `crates/opcua-server/src/services.rs::tests::session_lifecycle_round_trips`.

**Status:** done

### §5.11.2 Read — alle Attribute des Adressraums

**Spec:** Part 4 §5.11.2, S. 45 ff. — „This Service is used to read one or more
**Attributes** of one or more Nodes"; `ReadRequest` (MaxAge, TimestampsToReturn,
`ReadValueId[]`) / `ReadResponse` (`DataValue[]`). §5.11.2.2: „Negative values
are invalid for maxAge"; `indexRange` nach §7.27 / Annex A.3; §7.39
`TimestampsToReturn`. Ergebnis-Codes §5.11.2.3/.4: `Bad_MaxAgeInvalid`,
`Bad_TimestampsToReturnInvalid`, `Bad_NothingToDo`, `Bad_NodeIdUnknown`,
`Bad_AttributeIdInvalid`, `Bad_IndexRangeInvalid`, `Bad_IndexRangeNoData`,
`Bad_NotReadable`, `Bad_DataEncodingInvalid`. Die Attribut-Ids stehen in
Part 6 Annex A.1 Table A.1 (1 `NodeId` … 27 `AccessLevelEx`), die
NodeClass-Zuordnung in Part 3 §5.

**Repo:** `crates/opcua-server/src/attributes.rs` (`attribute_ids` = Table A.1,
`ClassAttributes` mit den Part-3-Attributsätzen je NodeClass, `access_level`-
Bits nach Part 3 §8.57, `NumericRange` (Parser + `apply`) nach §7.27/Annex A.3,
`TimestampsToReturn`), `crates/opcua-server/src/address_space.rs`
(`NodeMeta` mit Description/WriteMask/UserWriteMask/ClassAttributes,
`AddressSpace::read_attribute` liefert jedes Attribut der NodeClass und
`Bad_AttributeIdInvalid` für jedes andere), `crates/opcua-server/src/status.rs`
(die StatusCodes), `crates/opcua-server/src/services.rs`
(`ReadRequest`, `ReadResponse`, `ReadValueId`),
`crates/opcua-server/src/server.rs` (`Server::read` prüft maxAge und
TimestampsToReturn, `read_one` wendet `indexRange` und `dataEncoding` an),
`crates/opcua-server/src/client.rs` (`Client::read_attributes`,
`read_attribute`, `read_values`, `read_value_id`).

**Tests:** `crates/opcua-server/src/attributes.rs::tests::numeric_range_parses_the_annex_a3_grammar`,
`numeric_range_rejects_the_forms_the_spec_calls_invalid`,
`numeric_range_apply_selects_and_clamps`,
`timestamps_to_return_strips_what_was_not_asked_for`;
`crates/opcua-server/src/services.rs::tests::read_request_round_trips`,
`read_response_round_trips`;
End-to-End `crates/opcua-server/src/client.rs::tests::e2e_loopback_reads_every_attribute_of_a_variable`
(16 Attribute einer Variable, jedes `Good` mit geprüftem Wert; `Executable`
und eine Id außerhalb Table A.1 als `Bad_AttributeIdInvalid`),
`e2e_loopback_index_range_read_and_write`,
`e2e_loopback_read_honours_timestamps_to_return_and_max_age`,
`read_unknown_node_yields_bad_status`.

**Status:** done

### §5.11.4 Write — alle beschreibbaren Attribute

**Spec:** Part 4 §5.11.4, S. 50 ff. — „This Service is used to write values to
one or more **Attributes** of one or more Nodes"; `attributeId` „shall be a
valid Attribute id … The IntegerIds for the Attributes are defined in
OPC 10000-6"; `indexRange` nach §7.27; „If the SourceTimestamp or the
ServerTimestamp is specified, the Server shall use these values. The Server
returns a Bad_WriteNotSupported error if it does not support writing of
timestamps." Ergebnis-Codes §5.11.4.4: `Bad_NodeIdUnknown`,
`Bad_AttributeIdInvalid`, `Bad_IndexRangeInvalid`, `Bad_IndexRangeNoData`,
`Bad_WriteNotSupported`, `Bad_NotWritable`, `Bad_TypeMismatch`. Welches
Attribut beschreibbar ist, steht in Part 3 §8.60 Table 44 (`AttributeWriteMask`,
Bits 0–25) — mit dem ausdrücklichen Sonderfall, dass das `Value`-Attribut einer
Variable nicht in der Maske steht, sondern von `AccessLevel`/`UserAccessLevel`
(Part 3 §8.57) gesteuert wird.

**Repo:** `crates/opcua-server/src/attributes.rs` (`write_mask`-Bits = Table 44,
`write_mask_bit` bildet Attribut-Id auf Bit ab und gibt für `Value` bewusst
`None` zurück), `crates/opcua-server/src/address_space.rs`
(`AddressSpace::write_attribute`: WriteMask ∧ UserWriteMask, dann
`write_value_attribute` mit `CurrentWrite`/`TimestampWrite`/`StatusWrite`
bzw. `ValueForVariableType`, `write_class_attribute` je NodeClass;
`Bad_TypeMismatch` bei falschem Builtin-Typ, `Bad_WriteNotSupported` für
`NodeId`/`NodeClass`), `crates/opcua-server/src/services.rs`
(`WriteRequest`, `WriteResponse`, `WriteValue`),
`crates/opcua-server/src/server.rs` (`Server::write_one` löst `indexRange`
gegen den aktuellen Wert auf), `crates/opcua-server/src/client.rs`
(`Client::write_attributes`, `write_values`).

**Tests:** `crates/opcua-server/src/attributes.rs::tests::write_mask_bit_matches_table_44`,
`numeric_range_replace_writes_in_place`;
`crates/opcua-server/src/services.rs::tests::write_round_trips`;
End-to-End `crates/opcua-server/src/client.rs::tests::e2e_loopback_write_respects_the_write_mask_and_access_level`
(gesetztes Maskenbit schreibt, gelöschtes gibt `Bad_NotWritable`, fehlendes
`CurrentWrite` gibt `Bad_NotWritable`, falscher Typ gibt `Bad_TypeMismatch`,
der abgelehnte Schreibvorgang lässt den Wert unverändert),
`e2e_loopback_index_range_read_and_write`, `e2e_loopback_write_then_read`.

**Status:** done

### §5.12.2 Call

**Spec:** Part 4 §5.12.2, S. 54 — `CallRequest` (`CallMethodRequest[]`) /
`CallResponse` (`CallMethodResult[]`).

**Repo:** `crates/opcua-server/src/services.rs`
(`CallMethodRequest`, `CallMethodResult`, `CallRequest`, `CallResponse`),
`crates/opcua-server/src/address_space.rs` (Method-Handler-Registry),
`crates/opcua-server/src/client.rs` (`Client::call_method`).

**Tests:** `crates/opcua-server/src/services.rs::tests::call_round_trips`,
`crates/opcua-server/src/address_space.rs::tests::method_call_dispatch`,
`crates/opcua-server/src/client.rs::tests::e2e_loopback_connect_read_call`,
`e2e_tcp_connect_read_call`.

**Status:** done

### §5.9.2 / §5.9.3 Browse + BrowseNext (View Service Set)

**Spec:** Part 4 §5.9.2 Browse (S. 38 ff.), §5.9.3 BrowseNext (S. 40 ff.),
§7.29 ReferenceDescription (S. 148), §7.6 BrowseResult (S. 118), §7.9
ContinuationPoint (S. 129), §7.44 ViewDescription (S. 169); ExpandedNodeId
Part 6 §5.2.2.10. §5.9.2.2 `requestedMaxReferencesPerNode`: „Indicates the
maximum number of references to return for each starting Node … The value 0
indicates that the Client is imposing no limitation." §5.9.3.1: „This Service
is used to request the next set of Browse or BrowseNext response information
that is too large to be sent in a single response." §5.9.3.2
`releaseContinuationPoints`: „TRUE — passed continuationPoints shall be reset
to free resources in the Server. The continuation points are released and the
results and diagnosticInfos arrays are empty." Ergebnis-Codes §5.9.2.4/§5.9.3.4:
`Bad_NodeIdUnknown`, `Bad_BrowseDirectionInvalid`, `Bad_ReferenceTypeIdInvalid`,
`Bad_NoContinuationPoints`, `Bad_ContinuationPointInvalid`, `Bad_NothingToDo`.

**Repo:** `crates/opcua-server/src/address_space.rs` (Knoten-/Referenz-Modell:
`NodeMeta`, `ReferenceRecord`, `NodeClass`, `reference_types`-Konstanten ns0,
`add_node`/`add_reference`/`node_meta`/`browse` mit Richtung +
Reference-Subtype-Filter (`includeSubtypes`) + NodeClass-Maske),
`crates/opcua-server/src/services.rs` (`ViewDescription`, `BrowseDescription`,
`ReferenceDescription`, `BrowseResult`, `BrowseRequest`/`BrowseResponse`,
`BrowseNextRequest`/`BrowseNextResponse` (NodeIds 533/536),
ExpandedNodeId-Wire); `crates/opcua-server/src/server.rs`
(`browse_one`: Reference-Filter + `resultMask`-Projektion via
`project_reference`, `Bad_NodeIdUnknown` für unbekannte Knoten,
`Bad_BrowseDirectionInvalid` und `Bad_ReferenceTypeIdInvalid` per
`is_known_reference_type`, Kappung auf `requestedMaxReferencesPerNode` mit
`allocate_continuation_point`; `browse_next_one` setzt fort oder gibt frei;
`ContinuationPoint`-Liste mit `MAX_CONTINUATION_POINTS` →
`Bad_NoContinuationPoints`), `crates/opcua-server/src/client.rs`
(`Client::browse`, `browse_limited`, `browse_next`, `browse_all`).

**Tests:** `crates/opcua-server/src/address_space.rs::tests::browse_forward_and_inverse`,
`browse_subtype_filter_excludes_non_hierarchical`, `browse_node_class_mask_filters_targets`,
`crates/opcua-server/src/services.rs::tests::browse_round_trips`,
`crates/opcua-server/src/client.rs::tests::e2e_loopback_browse`
(Objects→Boiler→Temperature über Organizes/HasComponent) und
`e2e_loopback_browse_next_walks_continuation_points` (fünf Referenzen,
`requestedMaxReferencesPerNode` = 2 → erster Batch + Continuation-Point,
BrowseNext liefert die restlichen drei in Reihenfolge, `release` gibt den Punkt
frei und ein zweiter Zugriff darauf ist `Bad_ContinuationPointInvalid`,
`browse_all` läuft die Kette bei Batchgröße 1 bis zum Ende ab).

**Status:** done

### §5.5.4 / §5.5.2 Discovery — GetEndpoints + FindServers

**Spec:** Part 4 §5.5.4 GetEndpoints (S. 14), §5.5.2 FindServers (S. 12) —
session-lose Discovery-Services (nur SecureChannel, keine Session nötig).

**Repo:** `crates/opcua-server/src/services.rs`
(`GetEndpointsRequest`/`GetEndpointsResponse` (NodeIds 426/429),
`FindServersRequest`/`FindServersResponse` (NodeIds 422/425)),
`crates/opcua-server/src/server.rs` (Dispatch-Arme: GetEndpoints liefert die
`EndpointDescription` des Servers — `Server::endpoint` mit den
`UserTokenPolicy`s aus dem `UserStore` und, unter `crypto`, dem tatsächlich
konfigurierten `MessageSecurityMode` samt Policy-URI (`endpoint_security`) —,
FindServers seine `ApplicationDescription`),
`crates/opcua-server/src/client.rs` (`Client::open_channel` — Hello+OPN ohne
Session — plus `Client::get_endpoints`/`find_servers`).

**Tests:** `crates/opcua-server/src/services.rs::tests::discovery_requests_round_trip`,
`crates/opcua-server/src/client.rs::tests::e2e_loopback_discovery`
(session-loser Channel → GetEndpoints liefert Endpoint mit
SecurityPolicy `None`, FindServers liefert den Server);
`e2e_secured_loopback_sign_only_and_renew` prüft denselben Endpoint mit
`Aes256_Sha256_RsaPss`/`Sign`, `e2e_loopback_user_name_identity_token` die
publizierte USERNAME-`UserTokenPolicy`.

**Status:** done

### §5.13 / §5.14 Subscription + MonitoredItem Service Sets

**Spec:** Part 4 §5.13.2 CreateMonitoredItems (S. 63), §5.13.3
ModifyMonitoredItems (S. 65), §5.13.4 SetMonitoringMode (S. 67), §5.13.5
SetTriggering (S. 68), §5.13.6 DeleteMonitoredItems (S. 69), §5.14.2
CreateSubscription (S. 76), §5.14.3 ModifySubscription (S. 78), §5.14.4
SetPublishingMode (S. 79), §5.14.5 Publish (S. 81), §5.14.6 Republish (S. 83),
§5.14.7 TransferSubscriptions (S. 83), §5.14.8 DeleteSubscriptions (S. 85);
§7.21 MonitoringParameters, §7.23 MonitoringMode, §7.25.2
DataChangeNotification (S. 145), §7.25.4 StatusChangeNotification (S. 146),
§7.26 NotificationMessage (S. 147), §7.39 TimestampsToReturn.

Die Sampling-Regeln, gegen die dieser Server misst, stehen in §5.14.1: die
Subscription „attempts to send a NotificationMessage" pro Zyklus (b), ein
Zyklus ohne Notifications liefert eine Keep-Alive-Nachricht (f), die Session
hält eine „retransmission queue of sent NotificationMessages … until they are
acknowledged" (i). §5.13.4.1: „Setting the mode to DISABLED causes all queued
Notifications to be deleted." §5.13.6.1: „When a MonitoredItem is deleted, its
triggered item links are also deleted." §5.14.3.2: „If the requested value is 0
or negative, the Server shall revise with the fastest supported publishing
interval", „The lifetime count shall be a minimum of three times the keep-alive
count". §5.14.7.1: „the Server shall issue a StatusChangeNotification
notificationMessage with the status code Good_SubscriptionTransferred to the
old Session".

**Repo:** `crates/opcua-server/src/services.rs` — die zwölf Dienste beider
Service-Sets als Wire-Typen: `CreateSubscriptionRequest`/`Response`,
`ModifySubscriptionRequest`/`Response` (793/796), `SetPublishingModeRequest`/
`Response`, `PublishRequest`/`Response`, `RepublishRequest`/`Response`
(832/835), `TransferSubscriptionsRequest`/`Response` + `TransferResult`
(841/844/838), `DeleteSubscriptionsRequest`/`Response`,
`CreateMonitoredItemsRequest`/`Response`, `ModifyMonitoredItemsRequest`/
`Response` + `MonitoredItemModifyRequest`/`MonitoredItemModifyResult`
(763/766/757/760), `SetMonitoringModeRequest`/`Response` (769/772),
`SetTriggeringRequest`/`Response` (775/778), `DeleteMonitoredItemsRequest`/
`Response` (781/784), dazu `MonitoringParameters`, `SubscriptionAcknowledgement`,
`MonitoredItemNotification`, `DataChangeNotification` (811) und
`StatusChangeNotification` (820).

`crates/opcua-server/src/server.rs` — der Subscription-Zustand: `Subscription`
mit revidierten Parametern (`revise_publishing_interval`/`revise_keep_alive`/
`revise_lifetime` nach §5.14.3.2), `retransmission_queue` der Tiefe
`RETRANSMISSION_QUEUE_DEPTH`, `MonitoredItem` mit `MonitoringMode`,
`queue`/`queue_size`/`discard_oldest` (§5.13.1.5, `trim_queue`),
`index_range`, `timestamps_to_return` und `triggered_items` (§5.13.5);
`Server::publish` verarbeitet erst die Acknowledgements, meldet einer Session,
deren Subscription übernommen wurde, die §7.25.4-StatusChangeNotification,
sampelt dann über `sample_subscription`/`sample_item` und stellt die Queues der
Reporting-Items samt der von ihnen getriggerten Sampling-Items zu, begrenzt
durch `max_notifications_per_publish` (`more_notifications`); ohne Notification
geht eine Keep-Alive-Nachricht hinaus, ohne Subscription der Service-Fehler
`Bad_NoSubscription`. `crates/opcua-server/src/attributes.rs`
(`MonitoringMode`, `TimestampsToReturn`, `NumericRange`) und
`crates/opcua-server/src/status.rs` (die StatusCodes der beiden Service-Sets).

`crates/opcua-server/src/client.rs` — `create_subscription`,
`modify_subscription`, `create_monitored_items`, `modify_monitored_items`,
`set_monitoring_mode`, `set_triggering`, `delete_monitored_items`, `publish`,
`publish_acknowledging` (→ `PublishOutcome`), `republish`,
`transfer_subscriptions`, `delete_subscriptions`.

**Tests:** `crates/opcua-server/src/services.rs::tests::subscription_round_trips`,
`publish_data_change_notification_round_trips`;
End-to-End in `crates/opcua-server/src/client.rs::tests`:
`e2e_loopback_subscription_publish` (Erstwert, unveränderter Zyklus,
Wertwechsel, Löschen, danach `Bad_NoSubscription`),
`e2e_loopback_modify_subscription_and_monitored_items` (revidierte Intervalle,
neuer Client-Handle wirkt im nächsten Publish, unbekannte Item-Id ist
`Bad_MonitoredItemIdInvalid`),
`e2e_loopback_set_monitoring_mode_and_delete_monitored_items`
(DISABLED verwirft die Queue, REPORTING nimmt sie wieder auf, doppeltes
Löschen ist `Bad_MonitoredItemIdInvalid`),
`e2e_loopback_set_triggering_releases_a_sampling_item`
(ein Sampling-Item meldet erst, nachdem es an das triggernde Item gebunden
wurde), `e2e_loopback_republish_and_acknowledgements`
(Republish liefert die Nachricht erneut, ein Acknowledgement entfernt sie aus
der Queue, danach `Bad_MessageNotAvailable`, eine unbekannte Sequenznummer
ist `Bad_SequenceNumberUnknown`),
`e2e_loopback_transfer_subscriptions_between_sessions`
(Übernahme in eine zweite Session, `Good_SubscriptionTransferred` an die alte,
`sendInitialValues` im ersten Publish der neuen, `Bad_NothingToDo` für die
eigene und `Bad_SubscriptionIdInvalid` für eine unbekannte Subscription).

**Status:** done

### §7.32/§7.33 Gemeinsame Service-Header + Common-Types

**Spec:** Part 4 §7.32 RequestHeader (S. 150), §7.33 ResponseHeader (S. 151),
§7.12 DiagnosticInfo (S. 131), §7.36 SignatureData (S. 153), §7.28 ReadValueId
(S. 148), §7.14 EndpointDescription (S. 133), §7.20 MessageSecurityMode
(S. 135).

**Repo:** `crates/opcua-uacp/src/securechannel.rs`
(`RequestHeader`, `ResponseHeader`, `null_extension_object`, minimaler
DiagnosticInfo-Skip), `crates/opcua-server/src/services.rs`
(`SignatureData`, `ReadValueId`, Header-Encode/Decode gegen die Public-Structs),
`crates/opcua-server/src/wire.rs` (String/ByteString/Array/DiagnosticInfo-Hilfen),
`MessageSecurityMode`/`EndpointDescription` aus `zerodds-opcua-pubsub`.

**Tests:** `crates/opcua-uacp/src/securechannel.rs::tests::response_header_with_string_table_round_trips`,
`crates/opcua-server/src/services.rs::tests::read_response_round_trips`.

**Status:** done

---

## Audit-Status

16 done / 0 partial / 0 open / 0 n/a (informative) / 0 n/a (rejected).

Test-Lauf: `cargo test -p zerodds-opcua-uacp -p zerodds-opcua-server --features zerodds-opcua-server/crypto`
— 81 Tests grün (23 UACP inkl. Krypto und Message-Chunking + 58 Server), 0
failed; inkl. echtem `opc.tcp`-End-to-End, gechunkter Nachricht über beide
Transporte, gesicherten Loopbacks in `Sign` und `SignAndEncrypt`,
SecureChannel-Renew, allen vier UserIdentityTokens, dem vollen Attributsatz,
BrowseNext mit Continuation-Points sowie beiden Service-Sets für
MonitoredItems und Subscriptions.

Offene Punkte: keine. Decision-Records: keine.
