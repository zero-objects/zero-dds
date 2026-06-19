# OPC UA Part 4 — Client/Server (UACP + SecureChannel) — Spec-Coverage

**Quelle:** `docs/standards/cache/opcfoundation/opcua-part4-services-1.05.07.pdf`
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

- `crates/opcua-uacp/` — UACP (Hello/Ack/Error/ReverseHello, Message-Chunking),
  OPC UA Secure Conversation (OPN/MSG/CLO-Chunks, Security-Header,
  SequenceHeader, ChannelSecurityToken) und die gesicherten SecurityPolicies
  (Feature `crypto`); 20 Tests grün (`--features crypto`).
- `crates/opcua-server/` — voller Service-Satz (Session/Read/Write/Call/Browse/
  Discovery/Subscription), in-memory AddressSpace mit Referenzgraph,
  `Server`-Zustandsautomat, `Client` und `opc.tcp`-TCP-Transport; 22 Tests grün,
  inkl. echtem TCP-End-to-End und gesichertem Loopback.

SecurityMode `None` **und** die gesicherten SecurityPolicies (RSA-/AES-Krypto,
`Basic256Sha256`/`Aes128_Sha256_RsaOaep`/`Aes256_Sha256_RsaPss`) sind end-to-end
implementiert. Offene Punkte: keine.

---

## §7.1 OPC UA Connection Protocol (UACP)

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

## §6.7 OPC UA Secure Conversation — Chunk-Rahmung

**Spec:** Part 6 §6.7, S. 62 — SecureChannel-Nachrichten als Chunks
(OPN/MSG/CLO), `AsymmetricAlgorithmSecurityHeader` (OPN) /
`SymmetricAlgorithmSecurityHeader` (MSG/CLO), `SequenceHeader`
(§6.7.2 MessageChunk structure, S. 62).

**Repo:** `crates/opcua-uacp/src/securechannel.rs`
(`SecureChannel::open_chunk` / `message_chunk` / `close_chunk`, `parse_chunk`,
`ParsedChunk`, `SECURITY_POLICY_NONE`).

**Tests:** `crates/opcua-uacp/src/securechannel.rs::tests::message_chunk_round_trip`,
`open_secure_channel_round_trip_none`.

**Status:** done (SecurityMode `None`; gesicherte Krypto siehe §6.7-Krypto)

## §6.7 SecureChannel-Krypto — gesicherte SecurityPolicies

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
OPN öffnen → Nonce → Key-Derivation → gesicherte OPN-Antwort + Session-Install),
`crates/opcua-server/src/client.rs` (`ClientSecurity`, `Client::set_security`,
`open_secure_channel_secured`). Trust-Store = übergebene Peer-Zertifikate/
-Public-Keys; RNG Caller-bereitgestellt (`Box<dyn CryptoRngCore + Send>`,
OS-RNG unter `std`).

**Tests:** `crates/opcua-uacp/src/crypto.rs::tests::p_sha256_is_deterministic_and_sized`,
`derive_keys_lengths_and_mirror`, `symmetric_sign_and_encrypt_round_trip`
(alle 3 Policies), `symmetric_sign_only_round_trip`, `symmetric_tamper_is_rejected`,
`asymmetric_round_trip_oaep_pkcs15`, `asymmetric_round_trip_oaep_sha256_pss`,
`asymmetric_wrong_signer_is_rejected`, `full_secured_handshake_flow`;
**gesicherter End-to-End:** `crates/opcua-server/src/client.rs::tests::e2e_secured_loopback_sign_and_encrypt`
(echte RSA-Keys + OS-RNG: verschlüsselter+signierter Handshake → Read/Call/Write
über `Basic256Sha256`/SignAndEncrypt).

**Status:** done (Krypto-Kern + Chunk-Security + Server/Client-Handshake
vollständig verdrahtet und mit echtem gesichertem E2E getestet)

## §5.6.2 OpenSecureChannel

**Spec:** Part 4 §5.6.2, S. 21 — `OpenSecureChannelRequest`/`Response`,
`SecurityTokenRequestType`, `ChannelSecurityToken` (RevisedLifetime).

**Repo:** `crates/opcua-uacp/src/securechannel.rs`
(`OpenSecureChannelRequest`, `OpenSecureChannelResponse`,
`SecurityTokenRequestType`, `ChannelSecurityToken`),
`crates/opcua-server/src/server.rs` (`Server::open_secure_channel`).

**Tests:** `crates/opcua-uacp/src/securechannel.rs::tests::open_response_round_trip`,
`crates/opcua-server/src/client.rs::tests::e2e_loopback_connect_read_call`
(durchläuft OPN).

**Status:** done (SecurityMode `None`)

## §5.7.2 CreateSession

**Spec:** Part 4 §5.7.2, S. 24 — `CreateSessionRequest`/`Response`
(SessionId, AuthenticationToken, RevisedSessionTimeout, ServerEndpoints,
ServerSignature).

**Repo:** `crates/opcua-server/src/services.rs`
(`CreateSessionRequest`, `CreateSessionResponse`),
`crates/opcua-server/src/server.rs` (`handle_service` CreateSession-Arm).

**Tests:** `crates/opcua-server/src/client.rs::tests::e2e_loopback_connect_read_call`,
`e2e_tcp_connect_read_call`.

**Status:** done

## §5.7.3 ActivateSession

**Spec:** Part 4 §5.7.3, S. 29 — `ActivateSessionRequest`/`Response`
(ClientSignature, LocaleIds, UserIdentityToken, ServerNonce).

**Repo:** `crates/opcua-server/src/services.rs`
(`ActivateSessionRequest`, `ActivateSessionResponse`).

**Tests:** `crates/opcua-server/src/services.rs::tests::session_lifecycle_round_trips`,
`crates/opcua-server/src/client.rs::tests::e2e_loopback_connect_read_call`.

**Status:** done (Anonymous-UserIdentityToken; SecurityMode `None`)

## §5.7.4 CloseSession

**Spec:** Part 4 §5.7.4, S. 31 — `CloseSessionRequest` (DeleteSubscriptions)
/`Response`.

**Repo:** `crates/opcua-server/src/services.rs`
(`CloseSessionRequest`, `CloseSessionResponse`).

**Tests:** `crates/opcua-server/src/services.rs::tests::session_lifecycle_round_trips`.

**Status:** done

## §5.11.2 Read

**Spec:** Part 4 §5.11.2, S. 45 — `ReadRequest` (MaxAge, TimestampsToReturn,
`ReadValueId[]`) / `ReadResponse` (`DataValue[]`).

**Repo:** `crates/opcua-server/src/services.rs`
(`ReadRequest`, `ReadResponse`, `ReadValueId`),
`crates/opcua-server/src/server.rs` (Read-Arm gegen AddressSpace),
`crates/opcua-server/src/client.rs` (`Client::read_values`).

**Tests:** `crates/opcua-server/src/services.rs::tests::read_request_round_trips`,
`read_response_round_trips`,
`crates/opcua-server/src/client.rs::tests::read_unknown_node_yields_bad_status`.

**Status:** done (Value-Attribut)

## §5.11.4 Write

**Spec:** Part 4 §5.11.4, S. 50 — `WriteRequest` (`WriteValue[]`) /
`WriteResponse` (`StatusCode[]`).

**Repo:** `crates/opcua-server/src/services.rs`
(`WriteRequest`, `WriteResponse`, `WriteValue`),
`crates/opcua-server/src/server.rs` (Write-Arm: `set_value`,
`Bad_AttributeIdInvalid` für Nicht-Value-Attribute),
`crates/opcua-server/src/client.rs` (`Client::write_values`).

**Tests:** `crates/opcua-server/src/services.rs::tests::write_round_trips`,
`crates/opcua-server/src/client.rs::tests::e2e_loopback_write_then_read`.

**Status:** done (Value-Attribut)

## §5.12.2 Call

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

## §5.9.2 Browse (View Service Set)

**Spec:** Part 4 §5.9.2 Browse (S. 38), §7.29 ReferenceDescription (S. 148),
§7.6 BrowseResult (S. 118), §7.44 ViewDescription (S. 169);
ExpandedNodeId Part 6 §5.2.2.10.

**Repo:** `crates/opcua-server/src/address_space.rs` (Knoten-/Referenz-Modell:
`NodeMeta`, `ReferenceRecord`, `NodeClass`, `reference_types`-Konstanten ns0,
`add_node`/`add_reference`/`node_meta`/`browse` mit Richtung +
Reference-Subtype-Filter (`includeSubtypes`) + NodeClass-Maske),
`crates/opcua-server/src/services.rs` (`ViewDescription`, `BrowseDescription`,
`ReferenceDescription`, `BrowseResult`, `BrowseRequest`/`BrowseResponse`,
ExpandedNodeId-Wire), `crates/opcua-server/src/server.rs` (`browse_one`:
Reference-Filter + `resultMask`-Projektion, `Bad_NodeIdUnknown` für unbekannte
Knoten), `crates/opcua-server/src/client.rs` (`Client::browse`).

**Tests:** `crates/opcua-server/src/address_space.rs::tests::browse_forward_and_inverse`,
`browse_subtype_filter_excludes_non_hierarchical`, `browse_node_class_mask_filters_targets`,
`crates/opcua-server/src/services.rs::tests::browse_round_trips`,
`crates/opcua-server/src/client.rs::tests::e2e_loopback_browse`
(Objects→Boiler→Temperature über Organizes/HasComponent).

**Status:** done (Continuation-Points: einzelner Batch, kein BrowseNext —
AddressSpaces dieser Größe liefern alles in einem `BrowseResult`)

## §5.5.4 / §5.5.2 Discovery — GetEndpoints + FindServers

**Spec:** Part 4 §5.5.4 GetEndpoints (S. 14), §5.5.2 FindServers (S. 12) —
session-lose Discovery-Services (nur SecureChannel, keine Session nötig).

**Repo:** `crates/opcua-server/src/services.rs`
(`GetEndpointsRequest`/`GetEndpointsResponse` (NodeIds 426/429),
`FindServersRequest`/`FindServersResponse` (NodeIds 422/425)),
`crates/opcua-server/src/server.rs` (Dispatch-Arme: GetEndpoints liefert die
`EndpointDescription` des Servers, FindServers seine `ApplicationDescription`),
`crates/opcua-server/src/client.rs` (`Client::open_channel` — Hello+OPN ohne
Session — plus `Client::get_endpoints`/`find_servers`).

**Tests:** `crates/opcua-server/src/services.rs::tests::discovery_requests_round_trip`,
`crates/opcua-server/src/client.rs::tests::e2e_loopback_discovery`
(session-loser Channel → GetEndpoints liefert Endpoint mit
SecurityPolicy `None`, FindServers liefert den Server).

**Status:** done

## §5.13 / §5.14 Subscription + MonitoredItem Service Sets

**Spec:** Part 4 §5.14.2 CreateSubscription (S. 76), §5.14.4 SetPublishingMode
(S. 79), §5.13.2 CreateMonitoredItems (S. 63), §5.14.5 Publish (S. 81),
§5.14.8 DeleteSubscriptions (S. 85); §7.25.2 DataChangeNotification (S. 145),
§7.26 NotificationMessage (S. 147).

**Repo:** `crates/opcua-server/src/services.rs`
(`CreateSubscriptionRequest`/`Response`, `SetPublishingModeRequest`/`Response`,
`MonitoringParameters`, `MonitoredItemCreateRequest`/`Result`,
`CreateMonitoredItemsRequest`/`Response`, `SubscriptionAcknowledgement`,
`MonitoredItemNotification`, `DataChangeNotification`
(ExtensionObject-Wrapping TypeId 811), `NotificationMessage`,
`PublishRequest`/`Response`, `DeleteSubscriptionsRequest`/`Response`),
`crates/opcua-server/src/server.rs` (Subscription-/MonitoredItem-State,
`publish_one`: **Sample-on-Publish** — bei jedem Publish die MonitoredItems
gegen den AddressSpace abgleichen, geänderte Werte als DataChangeNotification),
`crates/opcua-server/src/client.rs`
(`create_subscription`/`create_monitored_items`/`publish`/`delete_subscriptions`).

**Tests:** `crates/opcua-server/src/services.rs::tests::subscription_round_trips`,
`publish_data_change_notification_round_trips`,
`crates/opcua-server/src/client.rs::tests::e2e_loopback_subscription_publish`
(Subscription + MonitoredItem auf einem Knoten → erster Publish liefert
Initialwert, kein Change → leer, nach Write neuer Wert, dann
DeleteSubscriptions).

**Status:** done (Sample-on-Publish-Modell, passend zum synchronen Transport;
Modify/Republish und server-getaktetes Sampling sind nicht nötig, da der
Push-Use-Case auch von Part-14-PubSub abgedeckt ist)

## §7.32/§7.33 Gemeinsame Service-Header + Common-Types

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

## §5.2 OPC UA Binary — Codec-Wiederverwendung

**Spec:** Part 6 §5.2, S. 18 — Binär-Kodierung (Built-in-Typen, NodeId,
Variant, DataValue, ExtensionObject, Arrays).

**Repo:** wiederverwendet aus `crates/opcua-pubsub/src/binary/` über
`UaReader`/`UaWriter`/`UaEncode`/`UaDecode`; `NodeId`/`Variant`/`DataValue`/
`ExtensionObject` aus `zerodds-opcua-gateway`.

**Tests:** abgedeckt durch die PubSub-Codec-Tests
(`crates/opcua-pubsub/src/binary/`) + alle Round-Trip-Tests hier.

**Status:** done

## §7.2 OPC UA TCP — opc.tcp-Transport (End-to-End)

**Spec:** Part 6 §7.2, S. 86 — UA-TCP-Mapping; Transport-Profil
`uatcp-uasc-uabinary` (Part 7).

**Repo:** `crates/opcua-server/src/client.rs`
(`TcpTransport`, `serve_connection`, `Transport`-Trait, `LoopbackTransport`),
`crates/opcua-server/src/server.rs` (`Server::process`).

**Tests:** `crates/opcua-server/src/client.rs::tests::e2e_tcp_connect_read_call`
(echter `opc.tcp`-Socket: Connect/Read/Call über `TcpListener`+Thread),
`e2e_loopback_connect_read_call`, `e2e_loopback_write_then_read`.

**Status:** done

---

## Audit-Status

16 done / 0 partial / 0 open / 0 n/a (informative) / 0 n/a (rejected).

Test-Lauf: `cargo test -p zerodds-opcua-uacp -p zerodds-opcua-server --features crypto`
— 42 Tests grün (20 UACP inkl. 10 Krypto + 22 Server inkl. gesichertem E2E +
Browse + Discovery + Subscription/Publish), 0 failed; inkl. echtem `opc.tcp`-E2E
und gesichertem `Basic256Sha256`-Loopback.

Offene Punkte: keine. Decision-Records: keine.
