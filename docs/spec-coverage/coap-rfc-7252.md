# IETF CoAP RFC 7252 + RFC 7641 — Spec-Coverage

**RFC 7252:** [The Constrained Application Protocol — IETF, June 2014 →](https://www.rfc-editor.org/rfc/rfc7252)

**RFC 7641:** [Observing Resources in CoAP — IETF, September 2015 →](https://www.rfc-editor.org/rfc/rfc7641)

**Kontext:** CoAP ist eine RESTful-Spec für eingeschränkte Geräte.
ZeroDDS implementiert sie in `crates/coap-bridge/`: Wire-Codec (Section 3,
5.10, RFC 7641 §2) als no_std+alloc-Library, dazu Reliability,
Block-Wise Transfer (RFC 7959), Observe, einen DTLS-Transport (Feature
`dtls`), OSCORE und den Daemon.

Implementation:

- `crates/coap-bridge/` — Codec, Transport-Bausteine und Daemon `zerodds-coap-bridged`.

---

## RFC 7252 §1 Introduction

### §1.1 Features

**Spec:** §1.1, S. 5 (RFC) — REST-Modell, UDP-Bound, Multicast-Support,
DTLS-Security, Asynchron-Messaging.

**Repo:** `crates/coap-bridge/src/lib.rs` Crate-Doc.

**Tests:** REST-Methoden + Response-Klassen:
`crates/coap-bridge/src/message.rs::tests::coap_1_1_well_known_codes_match_spec_values`;
Asynchron-Messaging (CON/NON/ACK/RST):
`crates/coap-bridge/src/message.rs::tests::coap_1_1_the_message_types_are_the_numbers_of_section_3`;
Multicast-Support:
`crates/coap-bridge/src/multicast.rs::tests::coap_1_1_well_known_addresses_match_spec`;
DTLS-Security:
`crates/coap-bridge/src/dtls.rs::tests::coap_1_1_psk_is_secure`;
UDP-Bound (Default-Ports 5683/5684):
`crates/coap-bridge/src/uri.rs::tests::coap_1_1_default_port_returns_5684_for_coaps`.

**Status:** done — Übersicht; Multicast und DTLS stehen als eigene Items (§8, §9, §11).

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

**Tests:** `message::tests::coap_1_1_the_message_types_are_the_numbers_of_section_3`.

**Status:** done

### §2.2 Request/Response Model

**Spec:** §2.2, S. 12 — Method-Codes + Response-Codes.

**Repo:** `crates/coap-bridge/src/message.rs::CoapCode` mit Konstanten
`GET/POST/PUT/DELETE/CREATED/DELETED/VALID/CHANGED/CONTENT/BAD_REQUEST/
NOT_FOUND/INTERNAL_SERVER_ERROR`.

**Tests:** `crates/coap-bridge/src/message.rs::tests::coap_1_1_well_known_codes_match_spec_values`,
`coap_2_2_code_classification_predicates`.

**Status:** done

### §2.3 Intermediaries and Caching

**Spec:** §2.3, S. 15 — Forward-Proxies, Caches.

**Repo:** `crates/coap-bridge/src/caching_proxy.rs::{CoapCache,
CacheLookup, ProxyConfig, ProxyMode}` — Cache-Layer mit
Freshness-Tracking + Stale/Revalidate-Pfad; Proxy-Configuration
mit Forward-/Reverse-Mode + Hop-Count-Cap (Spec §5.7.1 Loop-Schutz).

**Tests:** `crates/coap-bridge/src/caching_proxy.rs::tests::{coap_2_3_empty_cache_returns_miss,
coap_2_3_store_then_fresh_lookup, coap_2_3_stale_with_etag_yields_validation_path,
coap_2_3_stale_without_etag_yields_miss, coap_2_3_cap_evicts_oldest,
coap_2_3_evict_expired_without_etag_keeps_revalidatable,
coap_2_3_proxy_config_default_is_forward_coap_to_coap}`.

**Status:** done — Caching + Forward-Proxy-Configuration live.

### §2.4 Resource Discovery

**Spec:** §2.4, S. 15 — `.well-known/core` (RFC 6690).

**Repo:** `crates/coap-bridge/src/core_link.rs::{CoreLink,
encode_links, decode_links}`.

**Tests:** `crates/coap-bridge/src/core_link.rs::tests::{coap_2_4_encode_simple_link_format,
coap_2_4_encode_multiple_links_comma_separated, coap_2_4_a_link_with_attributes_is_the_rfc_6690_form,
coap_2_4_decode_handles_numeric_attr, coap_2_4_decode_handles_quoted_string_with_comma,
coap_2_4_decode_rejects_uri_without_brackets, coap_2_4_decode_attr_without_value_yields_empty,
coap_2_4_encode_escapes_inner_quotes}`.

**Status:** done

---

## RFC 7252 §3 Message Format

### §3 Header-Format (Ver/T/TKL/Code/MID)

**Spec:** §3, S. 16 — 4-Byte-Fixed-Header. Version `MUST be 1 (01
binary)`, TKL `0..=8` (9-15 reserved).

**Repo:** `crates/coap-bridge/src/codec.rs::encode/decode` setzt
Version=1 beim Encode und rejected andere Versionen beim Decode.

**Tests:** `codec::tests::coap_3_encodes_minimum_get_request_to_4_byte_header`,
`coap_3_unsupported_version_decode_fails`,
`coap_3_reserved_token_length_9_through_15_decode_fails`,
`coap_3_token_length_above_8_is_rejected_on_encode`.

**Status:** done

### §3 Token

**Spec:** §3, S. 16 — Token 0..=8 Bytes nach Header.

**Repo:** `crates/coap-bridge/src/message.rs::CoapMessage::token`.

**Tests:** `codec::tests::coap_3_the_header_of_each_message_type_is_the_octets_of_the_layout`,
`coap_3_token_truncated_decode_fails`.

**Status:** done

### §3 Payload Marker

**Spec:** §3, S. 16-17 — `0xFF` markiert Beginn des Payload. "The
presence of a marker followed by a zero-length payload MUST be
processed as a message format error."

**Repo:** `crates/coap-bridge/src/codec.rs` (Encode + Decode).

**Tests:** `codec::tests::coap_3_payload_marker_separates_options_from_payload`,
`coap_3_payload_marker_without_payload_is_format_error`.

**Status:** done

### §3.1 Option Format (Delta-Encoding)

**Spec:** §3.1, S. 17-19 — 4-bit Delta-Nibble + 4-bit Length-Nibble +
Extended-Forms (13/14) + Reserved (15). Options "MUST appear in order
of their Option Numbers".

**Repo:** `crates/coap-bridge/src/codec.rs::encode_extended/
decode_extended`. Encode sortiert Options vor Delta-Berechnung.

**Tests:** `codec::tests::coap_3_1_single_short_option_encodes_in_one_byte_header`,
`coap_3_1_option_delta_extended_13_uses_one_extra_byte`,
`coap_3_1_option_delta_extended_14_uses_two_extra_bytes`,
`coap_3_1_option_length_extended_13_uses_one_extra_byte`,
`coap_3_1_delta_encoding_sums_across_multiple_options`,
`coap_3_1_options_are_sorted_on_encode`.

**Status:** done

### §3.2 Option Value Formats

**Spec:** §3.2, S. 19-20 — `empty`, `opaque`, `uint`, `string`. uint
"without leading zero bytes"; "MUST be prepared to process values with
leading zero bytes".

**Repo:** `crates/coap-bridge/src/option.rs::OptionValue` +
`uint_to_minimal_bytes` + `uint_from_bytes`.

**Tests:** `option::tests::coap_3_2_uint_zero_is_empty_byte_sequence`,
`coap_3_2_uint_one_is_single_byte`, `coap_3_2_uint_strips_leading_zero_bytes`,
`coap_3_2_uint_from_bytes_decodes_minimal_form`,
`coap_3_2_uint_from_bytes_handles_leading_zeros`,
`coap_3_2_uint_from_bytes_rejects_more_than_8_bytes`,
`coap_3_2_option_value_to_wire_bytes_matches_format`.

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

**Tests:** `crates/coap-bridge/src/bridge.rs::tests::{coap_4_1_parses_topic_only,
coap_4_1_parses_topic_with_key, coap_4_1_rejects_non_dds_path,
coap_4_1_bridge_apply_write_then_read_round_trip,
coap_4_1_bridge_observe_register_increments_observer_count}`.

**Status:** done — Endpoint-Tracking als spec-konforme Vec<u8>-ID;
Transport-Decoupling ist beabsichtigte Architektur für no_std-
Targets.

### §4.2-§4.7 Reliability + Deduplication + Congestion

**Spec:** §4.2-§4.7, S. 21-26 — CON-Retry mit exponentiellem Backoff bis
`MAX_RETRANSMIT`; §4.5 „The recipient […] SHOULD acknowledge each
duplicate copy of a Confirmable message […] but SHOULD process any
request or response in the message only once" — Message-ID-Duplikate
innerhalb `EXCHANGE_LIFETIME` (CON) bzw. `NON_LIFETIME` (NON); §4.7
`NSTART` als Obergrenze gleichzeitig ausstehender Interaktionen
gegenüber einem Server.

**Repo:** `crates/coap-bridge/src/reliability.rs::{ReliabilityTracker,
PendingConfirmable, TickOutput}` — `send_confirmable`/`receive_ack`/
`receive_rst`/`tick` decken CON-Retry + ACK/RST-Pfad ab;
`crates/coap-bridge/src/reliability.rs::{DuplicateDetector, DedupKind,
Reception}` mit `receive`/`remember_ack`/`evict_expired` für die §4.5-
Deduplikation (ACK aus dem Cache statt zweiter Verarbeitung);
`crates/coap-bridge/src/reliability.rs::OutstandingInteractions` mit
`try_begin`/`end` für die §4.7-`NSTART`-Schranke pro Server.

**Tests:** `crates/coap-bridge/src/reliability.rs::tests::{coap_4_2_tick_before_timeout_does_nothing,
coap_4_2_tick_after_timeout_retransmits, coap_4_2_exhausting_retransmits_times_out,
coap_4_2_interval_doubles_per_retransmit, coap_4_2_send_then_ack_clears_pending,
coap_4_2_rst_clears_pending, coap_4_2_duplicate_message_id_within_exchange_lifetime_is_detected,
coap_4_2_duplicate_entry_expires_after_its_lifetime, coap_4_2_duplicate_detection_is_per_endpoint,
coap_4_2_duplicate_table_cap_bounds_memory, coap_4_2_nstart_caps_outstanding_interactions_per_server}`.

**Status:** done

### §4.8 Transmission Parameters

**Spec:** §4.8, S. 27-29 — Tab. 2: `ACK_TIMEOUT` 2 s, `ACK_RANDOM_FACTOR`
1.5, `MAX_RETRANSMIT` 4, `NSTART` 1, `DEFAULT_LEISURE` 5 s,
`PROBING_RATE` 1 Byte/s. §4.8.2 leitet daraus ab:
`MAX_TRANSMIT_SPAN = ACK_TIMEOUT * ((2 ** MAX_RETRANSMIT) - 1) *
ACK_RANDOM_FACTOR` (45 s), `MAX_TRANSMIT_WAIT` (93 s),
`EXCHANGE_LIFETIME = MAX_TRANSMIT_SPAN + (2 * MAX_LATENCY) +
PROCESSING_DELAY` (247 s), `NON_LIFETIME` (145 s).

**Repo:** `crates/coap-bridge/src/reliability.rs::{ACK_TIMEOUT_MS,
ACK_RANDOM_FACTOR_NUM, ACK_RANDOM_FACTOR_DEN, MAX_RETRANSMIT, NSTART,
DEFAULT_LEISURE_MS, PROBING_RATE_BYTES_PER_SEC, MAX_LATENCY_MS,
PROCESSING_DELAY_MS, MAX_TRANSMIT_SPAN_MS, MAX_TRANSMIT_WAIT_MS,
EXCHANGE_LIFETIME_MS, NON_LIFETIME_MS}`; `initial_interval` setzt
`ACK_TIMEOUT * ACK_RANDOM_FACTOR` als erstes Retry-Intervall.

**Tests:** `crates/coap-bridge/src/reliability.rs::tests::{coap_4_8_transmission_parameters_match_spec_4_8_table_2,
coap_4_8_derived_transmission_parameters_follow_spec_4_8_2_formulas,
coap_4_8_backoff_schedule_follows_ack_timeout_and_random_factor}` — der letzte
prüft die daraus folgende Retry-Folge 3000/6000/12000/24000 ms und den
Abbruch nach `MAX_RETRANSMIT` bei `MAX_TRANSMIT_WAIT`.

**Status:** done

---

## RFC 7252 §5 Request/Response Semantics

### §5.1 Requests

**Spec:** §5.1, S. 35-36 — Request-Definition: Method-Code (1-31),
Token, Options, Payload.

**Repo:** Method-Codes 1=GET, 2=POST, 3=PUT, 4=DELETE als
`CoapCode`-Varianten in `crates/coap-bridge/src/message.rs`.

**Tests:** `crates/coap-bridge/src/message.rs::tests::coap_1_1_well_known_codes_match_spec_values`.

**Status:** done

### §5.2 Responses

**Spec:** §5.2, S. 36 — Response-Definition: Response-Code
(2.xx Success, 4.xx Client-Error, 5.xx Server-Error), Echoed Token.

**Repo:** Response-Codes als `CoapCode`-Varianten (2.05 Content,
4.04 Not Found, 5.00 Internal Server Error etc.).

**Tests:** `crates/coap-bridge/src/message.rs::tests::coap_1_1_well_known_codes_match_spec_values`.

**Status:** done

### §5.3 Request/Response Matching

**Spec:** §5.3, S. 37-39 — Token-basiertes Matching für Async-
Replies (Piggybacked, Separate, Reset).

**Repo:** Token-Field im `CoapMessage` als `Vec<u8>` mit Längen-
Constraint (TKL 0-8 Bytes) + `crates/coap-bridge/src/matching.rs::
{PendingRequests, MatchError}` mit `submit`/`complete`/
`evict_expired` und Spec-§4.8-EXCHANGE_LIFETIME-Support; DoS-Cap
über `max_pending`.

**Tests:** `matching::tests::{coap_5_3_submit_empty_token_rejected,
coap_5_3_submit_then_complete_round_trip, coap_5_3_complete_unknown_token_returns_none,
coap_5_3_submit_above_cap_rejected, coap_5_3_submit_duplicate_token_rejected,
coap_5_3_evict_expired_removes_overdue_pendings, coap_5_3_evict_keeps_fresh_pendings}`.

**Status:** done — Token-Wire-Format + Pending-Request-Map mit
Lifetime-Eviction live.

### §5.4 Options

**Spec:** §5.4, S. 39-43 — Option-Class-Properties: Critical/Elective,
Unsafe/Safe, NoCacheKey, Repeatable.

**Repo:** Option-Numbers in `crates/coap-bridge/src/option.rs::numbers`;
Property-Bits aus der Option-Number in
`crates/coap-bridge/src/option.rs::{is_critical, is_unsafe,
is_nocachekey, is_repeatable}` nach der §5.4.6-Formel
(`Critical = (onum & 1)`, `UnSafe = (onum & 2)`,
`NoCacheKey = ((onum & 0x1e) == 0x1c)`) plus die gleichnamigen
Methoden auf `CoapOption`.

**Tests:** `crates/coap-bridge/src/option.rs::tests::{coap_5_4_critical_unsafe_nocachekey_bits_per_spec_5_4_6,
coap_5_4_repeatable_options_match_spec_table_4, coap_5_4_observe_option_uses_number_6}`.

**Status:** done

### §5.5 Payloads and Representations

**Spec:** §5.5.1/§5.5.3, S. 43-44 — Payload als Repräsentation, deren
Medientyp die Content-Format-Option benennt; §5.5.2 „If no
Content-Format option is given, the payload of responses indicating a
client or server error is a brief human-readable diagnostic message"
in UTF-8.

**Repo:** `crates/coap-bridge/src/message.rs::CoapMessage::payload` als
`Vec<u8>`; Content-Format-Option (Number 12) über
`crates/coap-bridge/src/option.rs::CoapOption::content_format`;
Payload-Marker-Trennung in `crates/coap-bridge/src/codec.rs`.

**Tests:** `crates/coap-bridge/src/codec.rs::tests::{coap_5_5_representation_carries_content_format_per_spec_5_5,
coap_5_5_diagnostic_payload_of_error_response_carries_no_content_format,
coap_3_payload_marker_separates_options_from_payload}`.

**Status:** done

### §5.6 Caching

**Spec:** §5.6, S. 44-46 — Freshness-Lifetime via Max-Age-Option;
Validation via ETag-Option.

**Repo:** Max-Age (Number 14) + ETag (Number 4) Option-Numbers
definiert; Cache-State-Logik in
`crates/coap-bridge/src/caching_proxy.rs::{CoapCache, CacheLookup,
DEFAULT_MAX_AGE}` mit `store/lookup/evict_expired_without_etag`,
DoS-Cap-FIFO-Eviction und Spec-§5.6.1-Default-Max-Age (60s).

**Tests:** `caching_proxy::tests::{coap_2_3_empty_cache_returns_miss,
coap_2_3_store_then_fresh_lookup, coap_2_3_stale_with_etag_yields_validation_path,
coap_2_3_stale_without_etag_yields_miss, coap_2_3_cap_evicts_oldest,
coap_2_3_evict_expired_without_etag_keeps_revalidatable}`.

**Status:** done — Cache-State + Freshness + ETag-Validation-Pfad
live.

### §5.7 Proxying

**Spec:** §5.7, S. 46-49 — Proxy-Uri-Option, Forward-/Reverse-Proxy,
HTTP-Mapping.

**Repo:** Proxy-Uri-Option (Number 35) definiert + Proxy-Logik in
`crates/coap-bridge/src/caching_proxy.rs::{ProxyConfig, ProxyMode}`
mit Forward/Reverse-Mode + max_hops-Cap (Spec §5.7.1 Loop-Schutz)
+ http_translation-Flag für §10-Pfad.

**Tests:** `caching_proxy::tests::coap_2_3_proxy_config_default_is_forward_coap_to_coap`.

**Status:** partial — Proxy-Uri-Option, Forward-/Reverse-Modus und Hop-Grenze sind als Konfiguration gebaut; das Weiterleiten eines Requests fehlt (§5.7a).

### §5.7a Weiterleiten durch einen Proxy

**Spec:** §5.7, S. 46-49 — ein Forward-Proxy führt den Request mit der Ziel-URI aus `Proxy-Uri` aus, ein Reverse-Proxy bildet ihn auf einen Ursprungs-Server ab, und beide liefern die Response zurück.

**Repo:** —

**Tests:** —

**Status:** open — `crates/coap-bridge/src/caching_proxy.rs` hält nur `ProxyConfig`/`ProxyMode`; kein Pfad sendet einen Request an das Ziel weiter.

### §5.8 Method Definitions

**Spec:** §5.8, S. 49-51 — GET/POST/PUT/DELETE-Semantik, Idempotency.

**Repo:** Method-Codes definiert in `crates/coap-bridge/src/message.rs` + Method-Property-
Helpers `crates/coap-bridge/src/method_props.rs::{is_safe,
is_idempotent, is_method}` mit Spec-konformen Werten (GET = safe +
idempotent; PUT/DELETE = idempotent; POST = weder).

**Tests:** `method_props::tests::{coap_5_8_get_is_safe, coap_5_8_post_put_delete_not_safe,
coap_5_8_get_put_delete_idempotent, coap_5_8_post_not_idempotent,
coap_5_8_method_predicate_recognizes_all_four}`.

**Status:** done — Codes + Method-Properties (Safety + Idempotency)
ausgewiesen.

### §5.9 Response Code Definitions

**Spec:** §5.9, S. 52-58 — vollständige Liste aller Response-Codes
(2.xx, 4.xx, 5.xx) mit Bedeutung.

**Repo:** Alle wichtigen Codes als `CoapCode`-Varianten in
`crates/coap-bridge/src/message.rs`.

**Tests:** `crates/coap-bridge/src/message.rs::tests::coap_1_1_well_known_codes_match_spec_values`.

**Status:** done

### §5.10 Option Definitions

**Spec:** §5.10, S. 53-67 — Number-Registry: Uri-Host (3), ETag (4),
If-None-Match (5), Uri-Port (7), Location-Path (8), Uri-Path (11),
Content-Format (12), Max-Age (14), Uri-Query (15), Accept (17),
Location-Query (20), Proxy-Uri (35), Proxy-Scheme (39), Size1 (60).
Plus If-Match (1).

**Repo:** `crates/coap-bridge/src/option.rs::numbers` mit allen
Konstanten + Convenience-Konstruktoren `uri_path`/`content_format`.

**Tests:** `option::tests::coap_5_10_option_uri_path_constructs_string_value`,
`coap_5_10_content_format_uses_number_12`,
`codec::tests::coap_5_10_an_observe_registration_is_the_octets_of_the_layout`.

**Status:** done

---

## RFC 7252 §6 CoAP URIs

### §6.1-§6.5 CoAP-URI-Scheme

**Spec:** §6, S. 67-72 — `coap://` + `coaps://` URI-Schemen.

**Repo:** `crates/coap-bridge/src/uri.rs::{parse_coap_uri, CoapUri,
default_port, UriError}`. Default-Ports 5683 / 5684; Path-Segmente
+ Query-Parameter werden aufgeteilt (zur direkten Mapping in
`Uri-Path`/`Uri-Query`-Options); Fragment-Identifier rejected (§6.1).

**Tests:** `crates/coap-bridge/src/uri.rs::tests::{coap_6_1_parses_basic_coap_uri,
coap_6_1_parses_coaps_default_port, coap_6_1_parses_explicit_port, coap_6_1_parses_query_params,
coap_6_1_parses_empty_path_when_no_slash, coap_6_1_rejects_unknown_scheme,
coap_6_1_rejects_missing_host, coap_6_1_rejects_invalid_port, coap_6_1_rejects_fragment,
coap_1_1_default_port_returns_5684_for_coaps,
coap_6_1_round_trip_path_filtering_strips_empty_segments}` (11).

**Status:** done — coap/coaps URI-Scheme-Parser live.

---

## RFC 7252 §7 Discovery

### §7.1-§7.2 Service + Resource Discovery

**Spec:** §7, S. 73-74 — Multicast-Discovery + `.well-known/core`.

**Repo:** Resource-Discovery via `crates/coap-bridge/src/core_link.rs`;
Service-Discovery via Multicast in `multicast.rs::{ALL_NODES_LINK_LOCAL_V4,
ALL_NODES_LINK_LOCAL_V6, ALL_NODES_SITE_LOCAL_V6,
validate_multicast_request}` (siehe §8).

**Tests:** Resource-Discovery:
`crates/coap-bridge/src/core_link.rs::tests::{coap_2_4_encode_simple_link_format,
coap_2_4_a_link_with_attributes_is_the_rfc_6690_form}`; Service-Discovery:
`crates/coap-bridge/src/multicast.rs::tests::{coap_1_1_well_known_addresses_match_spec,
coap_7_1_valid_multicast_request_passes, coap_7_1_multicast_request_must_not_be_confirmable}`.

**Status:** partial — Resource-Discovery über `/.well-known/core` und die Multicast-Adressen sind gebaut und geprüft; Service-Discovery per Multicast braucht den Multicast-Empfang aus §8a.

---

## RFC 7252 §8 Multicast CoAP

### §8 Multicast Operation

**Spec:** §8, S. 74-77 — IP-Multicast-Semantik.

**Repo:** `crates/coap-bridge/src/multicast.rs::{ALL_NODES_LINK_LOCAL_V4
("224.0.1.187"), ALL_NODES_LINK_LOCAL_V6 ("FF02::FD"),
ALL_NODES_SITE_LOCAL_V6 ("FF05::FD"), DEFAULT_LEISURE (5s),
is_ipv4_multicast, is_ipv6_multicast, validate_multicast_request,
leisure_delay, MulticastError}` mit Spec-§8.1-konformer NON-only-
Validation + §8.2 Leisure-Time-Helper.

**Tests:** `multicast::tests::*` (9 Tests):
`coap_8_ipv4_multicast_recognizes_class_d_range`,
`coap_8_ipv6_multicast_recognizes_ff_prefix`,
`coap_7_1_multicast_request_must_not_be_confirmable`,
`coap_8_multicast_request_must_have_token`,
`coap_7_1_valid_multicast_request_passes`, `coap_8_leisure_delay_within_max`,
`coap_8_leisure_delay_zero_when_leisure_is_zero`,
`coap_1_1_well_known_addresses_match_spec`,
`coap_8_default_leisure_is_5_seconds`.

**Status:** partial — Multicast-Adressen, NON-only-Prüfung, Token-Pflicht und Leisure-Zeit sind gebaut und geprüft; Gruppenbeitritt, Senden und Empfangen im Daemon fehlen (§8a).

### §8a Multicast-Versand und -Empfang im Daemon

**Spec:** §8, S. 74-77 — Requests an eine IP-Multicast-Gruppe senden und empfangen; ein Server antwortet nach der Leisure-Zeit.

**Repo:** —

**Tests:** —

**Status:** open — `crates/coap-bridge/src/` tritt keiner Multicast-Gruppe bei (kein `join_multicast_v4`/`join_multicast_v6`); die Helfer aus `multicast.rs` werden von keinem Socket-Pfad benutzt.

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
für Logging. DTLS-1.2-Transport in `crates/coap-bridge/src/dtls_transport.rs`
(Feature `dtls`, `webrtc-dtls`) mit PSK und Zertifikat.

**Tests:** `crates/coap-bridge/src/dtls.rs::tests::{coap_9_nosec_is_not_secure, coap_1_1_psk_is_secure, coap_9_raw_public_key_is_secure, coap_9_certificate_is_secure, coap_9_validate_psk_short_key_rejected, coap_9_validate_psk_empty_identity_rejected, coap_9_validate_psk_valid, coap_9_validate_certificate_empty_chain_rejected, coap_9_validate_certificate_no_trust_anchors_rejected, coap_9_validate_raw_public_key_empty_rejected, coap_9_validate_nosec_passes}`; `crates/coap-bridge/tests/dtls_coap_e2e.rs::{coap_9_dtls_coap_get_content_roundtrip, coap_9_dtls_psk_handshake_carries_coap, coap_9_dtls_psk_unknown_identity_is_refused, coap_9_dtls_certificate_files_and_rotation_keep_sessions_alive}`; `crates/coap-bridge/tests/daemon_dtls_e2e.rs::coap_9_daemon_serves_coaps_over_dtls_request_response_and_observe`.

**Status:** partial — NoSec, PreSharedKey und Certificate laufen über den DTLS-Transport; RawPublicKey ist nur als Konfiguration vorhanden (§9a).

### §9a RawPublicKey-Modus im DTLS-Transport

**Spec:** §9, S. 68 (Textfassung) — „The NoSec and RawPublicKey modes are mandatory to implement for this specification.“; §9.1.3.2 RawPublicKey (RFC 7250).

**Repo:** —

**Tests:** —

**Status:** open — `crates/coap-bridge/src/dtls.rs` kennt `DtlsMode::RawPublicKey` als Konfiguration; `crates/coap-bridge/src/dtls_transport.rs` handelt nur PSK und Zertifikat aus (`DtlsPeerIdentity::{Psk, Certificate}`), ein Handshake mit Raw Public Key (RFC 7250) fehlt.

---

## RFC 7252 §10-§13 Inter-Op + Security + IANA + Acknowledgments

### §10 HTTP Cross-Proto-Mapping

**Spec:** §10, S. 86-91 — HTTP↔CoAP-Translation für Proxy-Use-Cases.
§10.1 Status-Code-Mapping (Tab 8); §10.2 Method-Mapping (Tab 9).

**Repo:** `crates/coap-bridge/src/caching_proxy.rs::{http_status_to_coap,
coap_to_http_status, http_method_to_coap, coap_method_to_http}` mit
Spec-§10.1/§10.2-konformem Mapping (200↔2.05, 404↔4.04, 500↔5.00,
GET↔1, POST↔2, PUT↔3, DELETE↔4 etc.).

**Tests:** `caching_proxy::tests::{coap_10_http_200_maps_to_coap_2_05,
coap_10_http_404_maps_to_coap_4_04, coap_10_http_500_maps_to_coap_5_00,
coap_10_http_unknown_returns_none, coap_10_the_status_codes_map_both_ways,
coap_10_http_get_maps_to_coap_1, coap_10_http_unknown_method_returns_none,
coap_10_the_methods_map_both_ways}`.

**Status:** done — HTTP-Status- und Method-Mapping bidirektional.

### §11.1 Parsing und URI-Verarbeitung

**Spec:** §11.1, S. 80 (RFC) — "the URI processing code in CoAP
implementations [...] should be implemented with special care. CoAP
access control implementations need to ensure they don't introduce
vulnerabilities through discrepancies between the code deriving access
control decisions from a URI and the code finally serving up the
resource addressed by the URI."

**Repo:** `crates/coap-bridge/src/codec.rs` (`decode`),
`crates/coap-bridge/src/uri.rs` (`parse_coap_uri`).

**Tests:** `crates/coap-bridge/tests/fuzz_smoke.rs::coap_11_1_fuzz_coap_decode_no_panic`,
`crates/coap-bridge/src/uri.rs::tests::coap_6_1_rejects_fragment`.

**Status:** partial — Robustheit des Decoders und die URI-Zerlegung
sind getestet; kein Test prüft, dass Zugriffsentscheidung (Topic-ACL)
und Auslieferung dieselbe Zerlegung der URI verwenden.

### §11.2 Cache: keine Auslieferung an Anfragen mit geringerer Transportsicherheit

**Spec:** §11.2, S. 81 (RFC) — "a caching proxy MUST NOT make cached
values available to requests that have lesser transport-security
properties than those the proxy would require to perform request
forwarding in the first place."

**Repo:** —

**Tests:** —

**Status:** open — `CoapCache` (`crates/coap-bridge/src/caching_proxy.rs`)
speichert und liefert Einträge ohne Bezug auf die Transportsicherheit
der Anfrage.

### §11.2 coaps-Antworten nicht im geteilten Cache

**Spec:** §11.2, S. 81 (RFC) — "responses to "coaps" identified requests
are never "public" and thus MUST NOT be reused for shared caching,
unless the cache is able to make equivalent access control decisions to
the ones that led to the cached entry."

**Repo:** —

**Tests:** —

**Status:** open — `CoapCache` unterscheidet nicht zwischen Antworten auf
`coap`- und `coaps`-Anfragen.

### §11.3 Keine große Verstärkung ohne Authentifizierung

**Spec:** §11.3, S. 82 (RFC) — "large amplification factors SHOULD NOT
be provided in the response if the request is not authenticated."

**Repo:** —

**Tests:** —

**Status:** open — kein Code gefunden, der die Antwortgröße für nicht
authentifizierte Anfragen begrenzt.

### §11.3 Multicast-Anfragen nur authentifiziert und nur für bestimmte Ressourcen

**Spec:** §11.3, S. 82 (RFC) — "CoAP servers SHOULD NOT accept multicast
requests that can not be authenticated in some way [...]. If possible, a
CoAP server SHOULD limit the support for multicast requests to the
specific resources where the feature is required."

**Repo:** —

**Tests:** —

**Status:** open — der Daemon (`crates/coap-bridge/src/daemon/`)
unterscheidet Multicast- und Unicast-Anfragen nicht und schränkt
Multicast weder auf authentifizierte Anfragen noch auf Ressourcen ein.

### §11.3 Multicast-Ziel über IPV6_RECVPKTINFO erkennen

**Spec:** §11.3, S. 82 (RFC) — "Implementations SHOULD make use of
modern APIs such as IPV6_RECVPKTINFO [RFC3542], if available, to make
this determination."

**Repo:** —

**Tests:** —

**Status:** open — kein Code in `crates/coap-bridge` liest die
Zieladresse eines empfangenen Pakets.

### §11.4, §11.5 IP-Spoofing und Cross-Protocol-Angriffe

**Spec:** §11.4–§11.5, S. 83–86 (RFC) — Bedrohungsanalyse
(gefälschte Reset/ACK/Antworten, Multicast-Spoofing, DNS-Antworten als
CoAP-Nachricht).

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — Beschreibung von Angriffen ohne eigene
Anforderung; die genannte Gegenmaßnahme (nicht triviales, zufälliges
Token) ist §5.3.1.

### §11.6 Constrained-Node Considerations

**Spec:** §11.6, S. 86 (RFC) — "the node MUST NOT be used for processes
that require good entropy, such as key generation. [...] Special care
must be taken in implementation of cryptographic primitives."

**Repo:** —

**Tests:** —

**Status:** open — die OSCORE-Kryptografie verwendet `aes`, `ccm`,
`hmac` und `sha2` aus RustCrypto; kein Test prüft die Laufzeitkonstanz.
Die Bridge erzeugt keine Schlüssel.

### §12.1, §12.2, §12.4–§12.7 IANA-Registrierungen

**Spec:** §12.1–§12.2, §12.4–§12.7, S. 86–97 (RFC) — Anweisungen an die
IANA: Code-Registries, Option-Numbers-Registry, URI-Schemes `coap` und
`coaps`, Ports 5683 und 5684.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — Registrierungsanweisungen an die IANA;
die Werte selbst sind als Items §5.8, §5.9, §5.10 und §6 geführt.

### §12.3 CoAP Content-Formats Registry

**Spec:** §12.3, S. 91–93 (RFC) — Anfangseinträge: `text/plain;
charset=utf-8` = 0, `application/link-format` = 40, `application/xml` =
41, `application/octet-stream` = 42, `application/exi` = 47,
`application/json` = 50.

**Repo:** —

**Tests:** —

**Status:** open — `crates/coap-bridge/src/daemon/server.rs` setzt 40 für
`application/link-format`, ohne Test; für die übrigen Einträge gibt es
keine Konstanten.

### §12.8 Multicast Address Registration

**Spec:** §12.8, S. 97 (RFC) — "All CoAP Nodes": IPv4 `224.0.1.187`,
IPv6 `FF0X::FD`; "CoAP needs the Link-Local and Site-Local scopes only."

**Repo:** `crates/coap-bridge/src/multicast.rs` (`ALL_NODES_LINK_LOCAL_V4`,
`ALL_NODES_LINK_LOCAL_V6`, `ALL_NODES_SITE_LOCAL_V6`).

**Tests:** `crates/coap-bridge/src/multicast.rs::tests::coap_1_1_well_known_addresses_match_spec`.

**Status:** done

### §13 Acknowledgements

**Spec:** §13, S. 97 (RFC) — Danksagung.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — Danksagung.

---

## RFC 7641 §2 Observe Option

### §2 Observe Option (Number 6)

**Spec:** RFC 7641 §2, S. 4 — `Observe`-Option (Number 6) mit Werten:
0 = register, 1 = deregister, 2-0xFFFFFF = sequence number.

**Repo:** `crates/coap-bridge/src/option.rs::CoapOption::observe(value)`
+ `numbers::OBSERVE` Konstante.

**Tests:** `option::tests::coap_5_4_observe_option_uses_number_6`,
`codec::tests::coap_5_10_an_observe_registration_is_the_octets_of_the_layout`.

**Status:** done

### RFC 7641 §3-§5 Notification + State + Reordering

**Spec:** RFC 7641 §3.1/§3.4/§3.6, S. 7-14 — Registrierung über
`Observe = 0`, monoton steigende Sequenznummer modulo 2^24, und die
Reordering-Regel: eine neue Notification (V2, T2) ist frischer als die
gespeicherte (V1, T1) genau dann, wenn `(V1 < V2 and V2 - V1 < 2^23)`
oder `(V1 > V2 and V1 - V2 > 2^23)` oder `T2 > T1 + 128 seconds`;
§3.6 Deregistrierung beendet die Observe-Beziehung.

**Repo:** `crates/coap-bridge/src/observe.rs::{ObserveRegistry,
ObserverEntry}` mit `register`/`deregister`/`observers_of`/`next_seq`
für Notification-Sequencing pro Path;
`crates/coap-bridge/src/observe.rs::{notification_is_fresher,
FRESHNESS_WINDOW_MS}` für die §3.4-Reordering-Regel.

**Tests:** `crates/coap-bridge/src/observe.rs::tests::{coap_3_register_then_observers_lists_one,
coap_3_next_seq_increments_per_observer, coap_3_next_seq_wraps_at_24_bits,
coap_3_notifications_are_ordered_by_sequence_number_per_spec_3_4,
coap_3_wrapped_sequence_number_is_still_recognised_as_fresher,
coap_3_notification_older_than_the_freshness_window_is_always_accepted,
coap_3_deregister_ends_the_observe_relationship_per_spec_3_6}`.

**Status:** done

---

## Audit-Status

27 done / 5 partial / 10 open / 0 blocked / 4 n/a (informative).

Test-Lauf: in CI, Krate-Ebene und Säulen-Jobs; die zitierten Tests mit dem letzten Ergebnis jedes Jobs in den Pipelines 8501 bis 9053 von `main` (`python3 ci/coverage_results.py --document coap-rfc-7252.md`):

* `cargo nextest run --profile merge -p zerodds-coap-bridge --all-targets` (Job `merge-rust-coap-bridge`) — 125 Tests grün.
* `cargo nextest run --profile merge -p zerodds-coap-bridge --all-targets --features dtls,oscore` (Job `merge-rust-coap-bridge-features-dtls-oscore`) — 5 Tests grün.

130 zitierte Tests grün, 0 nicht grün.

Offene Punkte: `coap-rfc-7252.open.md`.
