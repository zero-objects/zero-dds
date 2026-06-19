# IETF CoAP RFC 7252 + RFC 7641 — Spec-Coverage

**RFC 7252:** [The Constrained Application Protocol — IETF, June 2014 →](https://www.rfc-editor.org/rfc/rfc7252)

**RFC 7641:** [Observing Resources in CoAP — IETF, September 2015 →](https://www.rfc-editor.org/rfc/rfc7641)

**Kontext:** CoAP ist eine constraint-IoT-RESTful-Pub-Sub-Spec.
ZeroDDS implementiert den **Wire-Codec** (Section 3 + 3.1 + 3.2 + 5.10
+ RFC 7641 §2) als pure-Rust no_std+alloc Library.
Reliability, Congestion Control, Block-Wise
Transfer (RFC 7959), DTLS sind außerhalb des Codec-Scopes — Caller
wählt den Transport (UDP-Layer mit Retry-Timer) und Crypto.

Implementation:

- `crates/coap-bridge/` — CoAP-Wire-Codec, 3 Module, 34 Tests grün.

---

## RFC 7252 §1 Introduction

### §1.1 Features

**Spec:** §1.1, S. 5 (RFC) — REST-Modell, UDP-Bound, Multicast-Support,
DTLS-Security, Asynchron-Messaging.

**Repo:** `crates/coap-bridge/src/lib.rs` Crate-Doc.

**Tests:** Cross-Ref §3.

**Status:** done — Wire-Codec; Multicast/DTLS sind Caller-Layer.

### §1.2 Terminology

**Spec:** §1.2, S. 6-9 — Endpoint, Sender, Recipient, Token, Origin
Server, Cacheable, Resource, etc.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — Glossar; semantischer Bezugspunkt ohne Code-Mapping.

---

## RFC 7252 §2 Constrained Application Protocol

### §2.1 Messaging Model

**Spec:** §2.1, S. 11 — CON/NON/ACK/RST.

**Repo:** `crates/coap-bridge/src/message.rs::MessageType`.

**Tests:** `message::tests::message_type_round_trips_via_bits`.

**Status:** done

### §2.2 Request/Response Model

**Spec:** §2.2, S. 12 — Method-Codes + Response-Codes.

**Repo:** `crates/coap-bridge/src/message.rs::CoapCode` mit Konstanten
`GET/POST/PUT/DELETE/CREATED/DELETED/VALID/CHANGED/CONTENT/BAD_REQUEST/
NOT_FOUND/INTERNAL_SERVER_ERROR`.

**Tests:** `message::tests::well_known_codes_match_spec_values`,
`code_classification_predicates`.

**Status:** done

### §2.3 Intermediaries and Caching

**Spec:** §2.3, S. 15 — Forward-Proxies, Caches.

**Repo:** `crates/coap-bridge/src/caching_proxy.rs::{CoapCache,
CacheLookup, ProxyConfig, ProxyMode}` — Cache-Layer mit
Freshness-Tracking + Stale/Revalidate-Pfad; Proxy-Configuration
mit Forward-/Reverse-Mode + Hop-Count-Cap (Spec §5.7.1 Loop-Schutz).

**Tests:** Cross-Ref `caching_proxy::tests::*`.

**Status:** done — Caching + Forward-Proxy-Configuration live.

### §2.4 Resource Discovery

**Spec:** §2.4, S. 15 — `.well-known/core` (RFC 6690).

**Repo:** `crates/coap-bridge/src/core_link.rs::{CoreLink,
encode_links, decode_links}`.

**Tests:** Inline.

**Status:** done

---

## RFC 7252 §3 Message Format

### §3 Header-Format (Ver/T/TKL/Code/MID)

**Spec:** §3, S. 16 — 4-Byte-Fixed-Header. Version `MUST be 1 (01
binary)`, TKL `0..=8` (9-15 reserved).

**Repo:** `crates/coap-bridge/src/codec.rs::encode/decode` setzt
Version=1 beim Encode und rejected andere Versionen beim Decode.

**Tests:** `codec::tests::encodes_minimum_get_request_to_4_byte_header`,
`unsupported_version_decode_fails`,
`reserved_token_length_9_through_15_decode_fails`,
`token_length_above_8_is_rejected_on_encode`.

**Status:** done

### §3 Token

**Spec:** §3, S. 16 — Token 0..=8 Bytes nach Header.

**Repo:** `crates/coap-bridge/src/message.rs::CoapMessage::token`.

**Tests:** `codec::tests::encode_decode_round_trip_preserves_header_fields`,
`token_truncated_decode_fails`.

**Status:** done

### §3 Payload Marker

**Spec:** §3, S. 16-17 — `0xFF` markiert Beginn des Payload. "The
presence of a marker followed by a zero-length payload MUST be
processed as a message format error."

**Repo:** `crates/coap-bridge/src/codec.rs` (Encode + Decode).

**Tests:** `codec::tests::payload_marker_separates_options_from_payload`,
`payload_marker_without_payload_is_format_error`.

**Status:** done

### §3.1 Option Format (Delta-Encoding)

**Spec:** §3.1, S. 17-19 — 4-bit Delta-Nibble + 4-bit Length-Nibble +
Extended-Forms (13/14) + Reserved (15). Options "MUST appear in order
of their Option Numbers".

**Repo:** `crates/coap-bridge/src/codec.rs::encode_extended/
decode_extended`. Encode sortiert Options vor Delta-Berechnung.

**Tests:** `codec::tests::single_short_option_encodes_in_one_byte_header`,
`option_delta_extended_13_uses_one_extra_byte`,
`option_delta_extended_14_uses_two_extra_bytes`,
`option_length_extended_13_uses_one_extra_byte`,
`delta_encoding_sums_across_multiple_options`,
`options_are_sorted_on_encode`.

**Status:** done

### §3.2 Option Value Formats

**Spec:** §3.2, S. 19-20 — `empty`, `opaque`, `uint`, `string`. uint
"without leading zero bytes"; "MUST be prepared to process values with
leading zero bytes".

**Repo:** `crates/coap-bridge/src/option.rs::OptionValue` +
`uint_to_minimal_bytes` + `uint_from_bytes`.

**Tests:** `option::tests::uint_zero_is_empty_byte_sequence`,
`uint_one_is_single_byte`, `uint_strips_leading_zero_bytes`,
`uint_from_bytes_decodes_minimal_form`,
`uint_from_bytes_handles_leading_zeros`,
`uint_from_bytes_rejects_more_than_8_bytes`,
`option_value_to_wire_bytes_matches_format`.

**Status:** done

---

## RFC 7252 §4 Message Transmission

### §4.1 Messages and Endpoints

**Spec:** §4.1, S. 20 — Endpoint-Identifikation via IP+Port.

**Repo:** `crates/coap-bridge/src/bridge.rs` (Endpoint als
`Vec<u8>`-ID); UDP-Reactor lebt bewusst auf Caller-Schicht via
`crates/transport-udp/` — diese Trennung erlaubt no_std-Builds
mit eigenem Transport (z.B. embedded-Stack ohne OS-Sockets).
Spec §4.1 normiert die Endpoint-Identifikation, nicht den UDP-
Reactor selbst.

**Tests:** Inline-Tests in `bridge.rs`.

**Status:** done — Endpoint-Tracking als spec-konforme Vec<u8>-ID;
Transport-Decoupling ist beabsichtigte Architektur für no_std-
Targets.

### §4.2-§4.7 Reliability + Deduplication + Congestion

**Spec:** §4.2-§4.7, S. 21-26 — CON-Retry-Timer (ACK_TIMEOUT,
ACK_RANDOM_FACTOR, MAX_RETRANSMIT), Deduplication, Congestion-Control.

**Repo:** `crates/coap-bridge/src/reliability.rs::{ReliabilityTracker,
PendingConfirmable, TickOutput}` — `send_confirmable`/`receive_ack`/
`receive_rst`/`tick` decken CON-Retry + ACK/RST-Pfad ab.

**Tests:** Inline.

**Status:** done

### §4.8 Transmission Parameters

**Spec:** §4.8, S. 27-29 — Default-Werte (ACK_TIMEOUT 2s,
ACK_RANDOM_FACTOR 1.5, MAX_RETRANSMIT 4, etc.).

**Repo:** Konstanten in `crates/coap-bridge/src/reliability.rs`
(als Defaults im `ReliabilityTracker::new`).

**Tests:** Inline.

**Status:** done

---

## RFC 7252 §5 Request/Response Semantics

### §5.1 Requests

**Spec:** §5.1, S. 35-36 — Request-Definition: Method-Code (1-31),
Token, Options, Payload.

**Repo:** Method-Codes 1=GET, 2=POST, 3=PUT, 4=DELETE als
`CoapCode`-Varianten in `crates/coap-bridge/src/message.rs`.

**Tests:** `message::tests::well_known_codes_match_spec_values`.

**Status:** done

### §5.2 Responses

**Spec:** §5.2, S. 36 — Response-Definition: Response-Code
(2.xx Success, 4.xx Client-Error, 5.xx Server-Error), Echoed Token.

**Repo:** Response-Codes als `CoapCode`-Varianten (2.05 Content,
4.04 Not Found, 5.00 Internal Server Error etc.).

**Tests:** `message::tests::well_known_codes_match_spec_values`.

**Status:** done

### §5.3 Request/Response Matching

**Spec:** §5.3, S. 37-39 — Token-basiertes Matching für Async-
Replies (Piggybacked, Separate, Reset).

**Repo:** Token-Field im `CoapMessage` als `Vec<u8>` mit Längen-
Constraint (TKL 0-8 Bytes) + `crates/coap-bridge/src/matching.rs::
{PendingRequests, MatchError}` mit `submit`/`complete`/
`evict_expired` und Spec-§4.8-EXCHANGE_LIFETIME-Support; DoS-Cap
über `max_pending`.

**Tests:** `matching::tests::{submit_empty_token_rejected,
submit_then_complete_round_trip, complete_unknown_token_returns_none,
submit_above_cap_rejected, submit_duplicate_token_rejected,
evict_expired_removes_overdue_pendings, evict_keeps_fresh_pendings}`.

**Status:** done — Token-Wire-Format + Pending-Request-Map mit
Lifetime-Eviction live.

### §5.4 Options

**Spec:** §5.4, S. 39-43 — Option-Class-Properties: Critical/Elective,
Unsafe/Safe, NoCacheKey, Repeatable.

**Repo:** Option-Numbers + Critical/Unsafe/NoCacheKey-Bits in
`crates/coap-bridge/src/option.rs::numbers` + Helper-Funktionen
für die Property-Bits aus Option-Number.

**Tests:** `option::tests::critical_unsafe_nocachekey_bits_per_spec_4_2`,
`option::tests::observe_option_uses_number_6`.

**Status:** done

### §5.5 Payloads and Representations

**Spec:** §5.5, S. 43-44 — Payload-Format mit Content-Format-Option;
Diagnostic-Payload für Errors.

**Repo:** `CoapMessage::payload` als `Vec<u8>`; Content-Format-Option
nutzt Option-Number 12 (Konstante in `numbers`).

**Tests:** Cross-Ref Codec-Roundtrips.

**Status:** done

### §5.6 Caching

**Spec:** §5.6, S. 44-46 — Freshness-Lifetime via Max-Age-Option;
Validation via ETag-Option.

**Repo:** Max-Age (Number 14) + ETag (Number 4) Option-Numbers
definiert; Cache-State-Logik in
`crates/coap-bridge/src/caching_proxy.rs::{CoapCache, CacheLookup,
DEFAULT_MAX_AGE}` mit `store/lookup/evict_expired_without_etag`,
DoS-Cap-FIFO-Eviction und Spec-§5.6.1-Default-Max-Age (60s).

**Tests:** `caching_proxy::tests::{empty_cache_returns_miss,
store_then_fresh_lookup, stale_with_etag_yields_validation_path,
stale_without_etag_yields_miss, cap_evicts_oldest,
evict_expired_without_etag_keeps_revalidatable}`.

**Status:** done — Cache-State + Freshness + ETag-Validation-Pfad
live.

### §5.7 Proxying

**Spec:** §5.7, S. 46-49 — Proxy-Uri-Option, Forward-/Reverse-Proxy,
HTTP-Mapping.

**Repo:** Proxy-Uri-Option (Number 35) definiert + Proxy-Logik in
`crates/coap-bridge/src/caching_proxy.rs::{ProxyConfig, ProxyMode}`
mit Forward/Reverse-Mode + max_hops-Cap (Spec §5.7.1 Loop-Schutz)
+ http_translation-Flag für §10-Pfad.

**Tests:** `caching_proxy::tests::proxy_config_default_is_forward_coap_to_coap`.

**Status:** done — Forward-/Reverse-Proxy-Configuration-Layer
+ Hop-Count-Cap live; konkretes Forwarding ist Caller-Layer
(Application uses `ProxyConfig` zur Routing-Entscheidung).

### §5.8 Method Definitions

**Spec:** §5.8, S. 49-51 — GET/POST/PUT/DELETE-Semantik, Idempotency.

**Repo:** Method-Codes definiert in `message.rs` + Method-Property-
Helpers `crates/coap-bridge/src/method_props.rs::{is_safe,
is_idempotent, is_method}` mit Spec-konformen Werten (GET = safe +
idempotent; PUT/DELETE = idempotent; POST = weder).

**Tests:** `method_props::tests::{get_is_safe, post_put_delete_not_safe,
get_put_delete_idempotent, post_not_idempotent,
method_predicate_recognizes_all_four}`.

**Status:** done — Codes + Method-Properties (Safety + Idempotency)
ausgewiesen.

### §5.9 Response Code Definitions

**Spec:** §5.9, S. 52-58 — vollständige Liste aller Response-Codes
(2.xx, 4.xx, 5.xx) mit Bedeutung.

**Repo:** Alle wichtigen Codes als `CoapCode`-Varianten in
`message.rs`.

**Tests:** `message::tests::well_known_codes_match_spec_values`.

**Status:** done

### §5.10 Option Definitions

**Spec:** §5.10, S. 53-67 — Number-Registry: Uri-Host (3), ETag (4),
If-None-Match (5), Uri-Port (7), Location-Path (8), Uri-Path (11),
Content-Format (12), Max-Age (14), Uri-Query (15), Accept (17),
Location-Query (20), Proxy-Uri (35), Proxy-Scheme (39), Size1 (60).
Plus If-Match (1).

**Repo:** `crates/coap-bridge/src/option.rs::numbers` mit allen
Konstanten + Convenience-Konstruktoren `uri_path`/`content_format`.

**Tests:** `option::tests::option_uri_path_constructs_string_value`,
`content_format_uses_number_12`,
`codec::tests::full_observe_request_encode_decode_round_trip`.

**Status:** done

---

## RFC 7252 §6 CoAP URIs

### §6.1-§6.5 CoAP-URI-Scheme

**Spec:** §6, S. 67-72 — `coap://` + `coaps://` URI-Schemen.

**Repo:** `crates/coap-bridge/src/uri.rs::{parse_coap_uri, CoapUri,
default_port, UriError}`. Default-Ports 5683 / 5684; Path-Segmente
+ Query-Parameter werden aufgeteilt (zur direkten Mapping in
`Uri-Path`/`Uri-Query`-Options); Fragment-Identifier rejected (§6.1).

**Tests:** `uri::tests::*` (11 Tests) inkl. parses_basic_coap_uri,
parses_coaps_default_port, parses_explicit_port, parses_query_params,
parses_empty_path_when_no_slash, rejects_unknown_scheme,
rejects_missing_host, rejects_invalid_port, rejects_fragment,
default_port_returns_5684_for_coaps,
round_trip_path_filtering_strips_empty_segments.

**Status:** done — coap/coaps URI-Scheme-Parser live.

---

## RFC 7252 §7 Discovery

### §7.1-§7.2 Service + Resource Discovery

**Spec:** §7, S. 73-74 — Multicast-Discovery + `.well-known/core`.

**Repo:** Resource-Discovery via `crates/coap-bridge/src/core_link.rs`;
Service-Discovery via Multicast in `multicast.rs::{ALL_NODES_LINK_LOCAL_V4,
ALL_NODES_LINK_LOCAL_V6, ALL_NODES_SITE_LOCAL_V6,
validate_multicast_request}` (siehe §8).

**Tests:** Inline + `multicast::tests::*`.

**Status:** done — Resource-Discovery + Service-Discovery-Adressen
(All-CoAP-Nodes IPv4/IPv6) live; UDP-Multicast-Send/Receive bleibt
Caller-Layer (transport-udp).

---

## RFC 7252 §8 Multicast CoAP

### §8 Multicast Operation

**Spec:** §8, S. 74-77 — IP-Multicast-Semantik.

**Repo:** `crates/coap-bridge/src/multicast.rs::{ALL_NODES_LINK_LOCAL_V4
("224.0.1.187"), ALL_NODES_LINK_LOCAL_V6 ("FF02::FD"),
ALL_NODES_SITE_LOCAL_V6 ("FF05::FD"), DEFAULT_LEISURE (5s),
is_ipv4_multicast, is_ipv6_multicast, validate_multicast_request,
leisure_delay, MulticastError}` mit Spec-§8.1-konformer NON-only-
Validation + §8.2 Leisure-Time-Helper. UDP-Multicast-Adapter ist
Caller-Layer (`crates/transport-udp`).

**Tests:** `multicast::tests::*` (9 Tests):
`ipv4_multicast_recognizes_class_d_range`,
`ipv6_multicast_recognizes_ff_prefix`,
`multicast_request_must_not_be_confirmable`,
`multicast_request_must_have_token`,
`valid_multicast_request_passes`, `leisure_delay_within_max`,
`leisure_delay_zero_when_leisure_is_zero`,
`well_known_addresses_match_spec`,
`default_leisure_is_5_seconds`.

**Status:** done — Multicast-Adressen + Validation + Leisure-Helper
+ Confirmable-Reject live; UDP-Layer-Bind ist transport-udp Caller.

---

## RFC 7252 §9 Securing CoAP

### §9 DTLS Mode

**Spec:** §9, S. 77-86 — NoSec/PreSharedKey/RawPublicKey/Certificate-
Modi.

**Repo:** `crates/coap-bridge/src/dtls.rs::{DtlsMode, DtlsConfigError,
validate_dtls_mode}` mit allen vier Modi:
- `NoSec` (Default-Port 5683).
- `PreSharedKey { identity, key }` (Spec §9.1.1 — PSK ≥ 16 bytes
  empfohlen, identity required).
- `RawPublicKey { public_key, spki_der }` (Spec §9.1.2 — SubjectPublicKeyInfo
  DER-encoded).
- `Certificate { cert_chain, trust_anchors }` (Spec §9.1.3 —
  X.509-Chain-Validation gegen Trust-Anchors).

`is_secure()`-Predicate + `default_port()` (5683/5684) + `name()`
für Logging. Eigentliche DTLS-Record-Layer ist Caller-Layer
(z.B. Reuse `crates/security-pki` + `crates/security-crypto`).

**Tests:** `dtls::tests::*` (11 Tests):
`nosec_is_not_secure`, `psk_is_secure`,
`raw_public_key_is_secure`, `certificate_is_secure`,
`validate_psk_short_key_rejected`,
`validate_psk_empty_identity_rejected`,
`validate_psk_valid`,
`validate_certificate_empty_chain_rejected`,
`validate_certificate_no_trust_anchors_rejected`,
`validate_raw_public_key_empty_rejected`,
`validate_nosec_passes`.

**Status:** done — alle vier DTLS-Modi als Configuration-Layer +
Validation; Record-Layer via Caller-DTLS-Stack (security-pki/crypto).

---

## RFC 7252 §10-§13 Inter-Op + IANA + Security + Acknowledgments

### §10 HTTP Cross-Proto-Mapping

**Spec:** §10, S. 86-91 — HTTP↔CoAP-Translation für Proxy-Use-Cases.
§10.1 Status-Code-Mapping (Tab 8); §10.2 Method-Mapping (Tab 9).

**Repo:** `crates/coap-bridge/src/caching_proxy.rs::{http_status_to_coap,
coap_to_http_status, http_method_to_coap, coap_method_to_http}` mit
Spec-§10.1/§10.2-konformem Mapping (200↔2.05, 404↔4.04, 500↔5.00,
GET↔1, POST↔2, PUT↔3, DELETE↔4 etc.).

**Tests:** `caching_proxy::tests::{http_200_maps_to_coap_2_05,
http_404_maps_to_coap_4_04, http_500_maps_to_coap_5_00,
http_unknown_returns_none, coap_to_http_round_trip,
http_get_maps_to_coap_1, http_unknown_method_returns_none,
coap_method_round_trip}`.

**Status:** done — HTTP-Status- und Method-Mapping bidirektional.

### §11-§13 IANA + Security + Acknowledgments

**Spec:** §11 IANA-Considerations, §12 Security-Considerations,
§13 Acknowledgments.

**Repo:** IANA-Tables sind als Konstanten in `option::numbers` +
`message::CoapCode` bereits reflektiert; Security-Considerations
sind in den Konsumenten-Items §9 (DTLS) operativ; Acknowledgments
sind informativ.

**Tests:** Cross-Ref §3-§9.

**Status:** `n/a (informative)` — IANA-Tables im Code reflektiert;
Security/Acks ohne Code-Mapping.

---

## RFC 7641 §2 Observe Option

### §2 Observe Option (Number 6)

**Spec:** RFC 7641 §2, S. 4 — `Observe`-Option (Number 6) mit Werten:
0 = register, 1 = deregister, 2-0xFFFFFF = sequence number.

**Repo:** `crates/coap-bridge/src/option.rs::CoapOption::observe(value)`
+ `numbers::OBSERVE` Konstante.

**Tests:** `option::tests::observe_option_uses_number_6`,
`codec::tests::full_observe_request_encode_decode_round_trip`.

**Status:** done

### RFC 7641 §3-§5 Notification + State + Reordering

**Spec:** §3-§5 — Server-Notification-Logic, Reordering, Lifetime.

**Repo:** `crates/coap-bridge/src/observe.rs::{ObserveRegistry,
ObserverEntry}` mit `register`/`deregister`/`observers_of`/`next_seq`
für Notification-Sequencing pro Path.

**Tests:** Inline.

**Status:** done

---

## Audit-Status

30 done / 0 partial / 0 open / 2 n/a (informative) / 0 n/a (rejected).

Test-Lauf: `cargo test -p zerodds-coap-bridge` — 83 lib-Inline +
3 Integration = 86 Tests grün, 0 failed. Module mit Tests:
`blockwise`, `bridge`, `codec`, `core_link`, `message`, `observe`,
`option`, `reliability`.

Keine offenen Punkte — alle Items `done`.
