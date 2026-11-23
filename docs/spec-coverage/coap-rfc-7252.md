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

**Repo:** `crates/coap-bridge/src/caching_proxy.rs::{CoapCache, CacheKey,
CacheScope, CacheLookup}` — Cache mit Freshness, Max-Age-Anpassung,
ETag-Validierung und Geltungsbereich; der Proxy des Daemons in
`crates/coap-bridge/src/daemon/proxy.rs::Proxy` (Forward und Reverse,
§5.7).

**Tests:** `crates/coap-bridge/src/caching_proxy.rs::tests::{coap_2_3_empty_cache_returns_miss,
coap_2_3_store_then_fresh_lookup, coap_2_3_stale_with_etag_yields_validation_path,
coap_2_3_stale_without_etag_yields_miss, coap_2_3_cap_evicts_oldest,
coap_2_3_evict_expired_without_etag_keeps_revalidatable}`,
`crates/coap-bridge/tests/proxy_e2e.rs::coap_5_7_2_a_forward_proxy_carries_out_the_request_and_serves_the_cache`.

**Status:** done

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
`MAX_RETRANSMIT`; §4.2/§4.3 Reset für eine nicht verarbeitbare Nachricht
und für eine leere Confirmable-Nachricht („CoAP ping“); §4.5 „The
recipient […] SHOULD acknowledge each duplicate copy of a Confirmable
message […] but SHOULD process any request or response in the message
only once" — Message-ID-Duplikate innerhalb `EXCHANGE_LIFETIME` (CON) bzw.
`NON_LIFETIME` (NON); §4.7 `NSTART` als Obergrenze gleichzeitig
ausstehender Interaktionen gegenüber einem Server.

**Repo:** `crates/coap-bridge/src/reliability.rs::{ReliabilityTracker,
PendingConfirmable, TickOutput, DuplicateDetector, DedupKind, Reception,
OutstandingInteractions}`. Im Daemon (`crates/coap-bridge/src/daemon/server.rs`):
`Dedup` über `DuplicateDetector` für UDP, DTLS und Multicast (ein
Duplikat einer CON bekommt die erste Bestätigung, eines einer NON nichts);
`message_layer`/`reject_undecodable` (Reset für Ping, für eine CON, die
keine Anfrage ist, und für eine nicht dekodierbare CON; ACK und RST ohne
Antwort). Der Proxy (`crates/coap-bridge/src/daemon/proxy.rs`) sendet
CON mit Wiederholung nach ACK_TIMEOUT·[1, 1,5] und Verdopplung, eine
getrennte Antwort als CON bis zur Bestätigung und führt eine wiederholte
Anfrage einmal aus.

**Tests:** `crates/coap-bridge/src/reliability.rs::tests::{coap_4_2_tick_before_timeout_does_nothing,
coap_4_2_tick_after_timeout_retransmits, coap_4_2_exhausting_retransmits_times_out,
coap_4_2_interval_doubles_per_retransmit, coap_4_2_send_then_ack_clears_pending,
coap_4_2_rst_clears_pending, coap_4_2_duplicate_message_id_within_exchange_lifetime_is_detected,
coap_4_2_duplicate_entry_expires_after_its_lifetime, coap_4_2_duplicate_detection_is_per_endpoint,
coap_4_2_duplicate_table_cap_bounds_memory, coap_4_2_nstart_caps_outstanding_interactions_per_server}`;
der Daemon: `crates/coap-bridge/tests/daemon_e2e.rs::{coap_4_5_a_retransmitted_post_is_written_once,
coap_4_3_a_ping_is_reset_and_an_acknowledgement_is_not_answered}`,
`crates/coap-bridge/tests/proxy_e2e.rs::{coap_5_7_1_a_silent_destination_is_5_04_after_an_empty_ack,
coap_5_7_2_a_forward_proxy_carries_out_the_request_and_serves_the_cache}`.

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
Unsafe/Safe, NoCacheKey, Repeatable; §5.4.1 „Unrecognized options of class
"critical" that occur in a Confirmable request MUST cause the return of a
4.02 (Bad Option) response […] in a Non-confirmable message MUST cause
the message to be rejected"; §5.4.5 eine überzählige Wiederholung gilt als
unbekannte Option.

**Repo:** `crates/coap-bridge/src/option.rs::{is_critical, is_unsafe,
is_nocachekey, is_repeatable}` nach der §5.4.6-Formel; im Daemon
`crates/coap-bridge/src/daemon/server.rs::{unrecognized_critical,
LOCAL_OPTIONS}` (4.02 für CON, Reset für NON), im Proxy
`crates/coap-bridge/src/daemon/proxy.rs::Proxy::target` (Unsafe unbekannt
4.02, Safe-to-Forward weitergeleitet).

**Tests:** `crates/coap-bridge/src/option.rs::tests::{coap_5_4_critical_unsafe_nocachekey_bits_per_spec_5_4_6,
coap_5_4_repeatable_options_match_spec_table_4, coap_5_4_observe_option_uses_number_6}`,
`crates/coap-bridge/tests/daemon_e2e.rs::coap_5_10_the_daemon_checks_options_methods_and_preconditions`,
`crates/coap-bridge/src/daemon/proxy.rs::tests::coap_5_7_1_an_unrecognized_unsafe_option_is_4_02_and_a_safe_one_is_forwarded`.

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

**Repo:** `crates/coap-bridge/src/caching_proxy.rs::{CoapCache, CacheKey,
CacheScope, CacheLookup, DEFAULT_MAX_AGE, max_age_of}`: `store_response`
(2.05 mit Max-Age, Default 60 s, oder ETag), `serve` (frisch mit um die
Verweildauer verringertem Max-Age, sonst veraltet mit ETag),
`revalidated` (2.03 setzt die Frische mit dem neuen Max-Age, §5.9.1.3),
`invalidate` (2.01/2.02/2.04), Verdrängung des am längsten gespeicherten
Eintrags. Der Proxy validiert einen veralteten Eintrag mit seiner ETag.

**Tests:** `crates/coap-bridge/src/caching_proxy.rs::tests::{coap_2_3_empty_cache_returns_miss,
coap_2_3_store_then_fresh_lookup, coap_2_3_stale_with_etag_yields_validation_path,
coap_2_3_stale_without_etag_yields_miss, coap_2_3_cap_evicts_oldest,
coap_2_3_evict_expired_without_etag_keeps_revalidatable,
coap_5_7_1_a_cached_response_never_extends_the_max_age,
coap_5_9_1_3_a_valid_response_refreshes_the_entry,
coap_5_9_1_a_change_marks_the_stored_responses_not_fresh}`,
`crates/coap-bridge/tests/proxy_e2e.rs::coap_5_7_2_a_forward_proxy_carries_out_the_request_and_serves_the_cache`.

**Status:** done

### §5.7 Proxying

**Spec:** §5.7, S. 46-49 — Forward-Proxy (Proxy-Uri, Proxy-Scheme),
Reverse-Proxy, Optionen, 5.02/5.04/5.05, Max-Age aus dem Cache; „When a
client uses a proxy to make a request that will use a secure URI scheme
[…], the request towards the proxy SHOULD be sent using DTLS".

**Repo:** `crates/coap-bridge/src/daemon/proxy.rs::Proxy` mit der
Konfiguration `proxy.*` (`crates/coap-bridge/src/daemon/config.rs::ProxySettings`);
`crates/coap-bridge/src/caching_proxy.rs` für den Cache;
`crates/coap-bridge/src/dtls_transport.rs::DtlsCoapClient::connect` (OpenSSL,
`crates/dtls-openssl`) für `coaps`-Ziele;
`proxy.require_dtls_for_coaps` (Default an) verlangt DTLS auf der Seite
des Clients.

**Tests:** `crates/coap-bridge/src/daemon/proxy.rs::tests::{coap_5_7_2_without_forwarding_a_proxy_request_is_5_05,
coap_5_7_2_a_uri_naming_the_proxy_itself_is_a_local_request,
coap_5_7_1_an_unrecognized_unsafe_option_is_4_02_and_a_safe_one_is_forwarded,
rfc8768_3_the_hop_limit_is_decremented_inserted_and_reached,
coap_5_7_3_a_reverse_route_maps_the_path_to_the_origin}`,
`crates/coap-bridge/tests/proxy_e2e.rs::{coap_5_7_2_a_forward_proxy_carries_out_the_request_and_serves_the_cache,
coap_5_7_1_a_silent_destination_is_5_04_after_an_empty_ack,
coap_5_7_1_an_unsafe_option_in_the_response_is_5_02,
coap_5_7_3_a_reverse_route_reaches_its_origin_and_maps_the_location,
rfc7959_2_10_the_proxy_reassembles_and_cuts_blocks,
coap_5_7_a_coaps_destination_is_reached_over_dtls}`.

**Status:** done

### §5.7a Weiterleiten durch einen Proxy

**Spec:** §5.7, S. 46-49 — ein Forward-Proxy führt den Request mit der Ziel-URI aus `Proxy-Uri` aus, ein Reverse-Proxy bildet ihn auf einen Ursprungs-Server ab, und beide liefern die Response zurück.

**Repo:** `crates/coap-bridge/src/daemon/proxy.rs` — `route` (Proxy-Uri,
Proxy-Scheme mit Uri-Host/Uri-Port nach §6.5, die eigene Autorität als
lokale Anfrage, `proxy.reverse`), `target` (Optionen nach §5.7.1,
Hop-Limit nach RFC 8768), `exchange_all` mit `UdpUpstream`/`DtlsUpstream`
(eigenes Token, CON mit Wiederholung, Block1/Block2), `fetch_multicast`
(§8.2.2), `answer_from_destination` (5.02 für unbekannte Unsafe-Optionen
der Antwort, Location-Path zurück, Cache), `deliver` (Huckepack, sonst
leere ACK und getrennte CON). Gegenprobe mit libcoap 4.3.4: `coap-client
-P` über den Daemon an `coap-server` (GET, `.well-known/core`, getrennte
Antwort des Ursprungs, 4.04, Block2 mit 32-Octet-Blöcken, Reverse-Route,
5.02 für einen geschlossenen Port) und an `coaps`-Ursprünge mit OpenSSL
und GnuTLS, PSK und Zertifikat (eine fremde CA ergibt 5.02).

**Tests:** `crates/coap-bridge/src/daemon/proxy.rs::tests::{coap_5_7_2_without_forwarding_a_proxy_request_is_5_05,
coap_5_7_2_a_uri_naming_the_proxy_itself_is_a_local_request,
coap_5_7_1_an_unrecognized_unsafe_option_is_4_02_and_a_safe_one_is_forwarded,
rfc8768_3_the_hop_limit_is_decremented_inserted_and_reached,
coap_5_7_3_a_reverse_route_maps_the_path_to_the_origin}`,
`crates/coap-bridge/tests/proxy_e2e.rs::{coap_5_7_2_a_forward_proxy_carries_out_the_request_and_serves_the_cache,
coap_5_7_1_a_silent_destination_is_5_04_after_an_empty_ack,
coap_5_7_1_an_unsafe_option_in_the_response_is_5_02,
coap_5_7_3_a_reverse_route_reaches_its_origin_and_maps_the_location,
rfc7959_2_10_the_proxy_reassembles_and_cuts_blocks,
coap_5_7_a_coaps_destination_is_reached_over_dtls}`.

**Status:** done

### §5.8 Method Definitions

**Spec:** §5.8, S. 49-51 — GET/POST/PUT/DELETE-Semantik, Idempotency; „A
request with an unrecognized or unsupported Method Code MUST generate a
4.05 (Method Not Allowed) piggybacked response."

**Repo:** `crates/coap-bridge/src/method_props.rs::{is_safe,
is_idempotent, is_method}`; im Daemon
`crates/coap-bridge/src/daemon/server.rs::dispatch_core` (4.05 für eine
andere Methode als die vier, für `/.well-known/core` alles außer GET);
§5.8.1 GET liefert die aktuelle Repräsentation, das letzte Sample.

**Tests:** `crates/coap-bridge/src/method_props.rs::tests::{coap_5_8_get_is_safe, coap_5_8_post_put_delete_not_safe,
coap_5_8_get_put_delete_idempotent, coap_5_8_post_not_idempotent,
coap_5_8_method_predicate_recognizes_all_four}`, `crates/coap-bridge/tests/daemon_e2e.rs::coap_5_10_the_daemon_checks_options_methods_and_preconditions`,
`crates/coap-bridge/tests/daemon_e2e.rs::coap_5_8_1_a_get_returns_the_last_sample`.

**Status:** done

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
Location-Query (20), Proxy-Uri (35), Proxy-Scheme (39), Size1 (60), If-Match
(1); dazu die Regeln: §5.10.3 Content-Format, §5.10.4 Accept (4.06),
§5.10.5 Max-Age (Default 60), §5.10.6 ETag (2.03), §5.10.8 If-Match und
If-None-Match (4.12), §5.10.2 Proxy-Uri hat Vorrang vor den Uri-Optionen.

**Repo:** `crates/coap-bridge/src/option.rs::numbers`; im Daemon
`crates/coap-bridge/src/daemon/server.rs::{dispatch_core, serve_get, serve_write,
preconditions_hold, etag_of, content_response}` (4.06 für ein Accept einer Form,
die die Ressource nicht gibt, 4.15 für ein Content-Format ohne Form, 4.12, ETag
des Samples und 2.03; die Formen in `zerodds-coap-bridge-1.0` §4.5); Max-Age in
`crates/coap-bridge/src/caching_proxy.rs::max_age_of`; Proxy-Uri und
Proxy-Scheme in `crates/coap-bridge/src/daemon/proxy.rs::Proxy::route`.

**Tests:** `crates/coap-bridge/src/option.rs::tests::{coap_5_10_option_uri_path_constructs_string_value,
coap_5_10_content_format_uses_number_12}`,
`crates/coap-bridge/src/codec.rs::tests::coap_5_10_an_observe_registration_is_the_octets_of_the_layout`,
`crates/coap-bridge/tests/daemon_e2e.rs::coap_5_10_the_daemon_checks_options_methods_and_preconditions`, `crates/coap-bridge/src/daemon/proxy.rs::tests::coap_5_7_2_a_uri_naming_the_proxy_itself_is_a_local_request`.

**Status:** done

---

## RFC 7252 §6 CoAP URIs

### §6.1-§6.5 CoAP-URI-Scheme

**Spec:** §6, S. 67-72 — `coap://` + `coaps://` URI-Schemen; §6.3
Normalisierung; §6.4 Zerlegung in Uri-Host, Uri-Port, Uri-Path (jedes
Segment, auch ein leeres), Uri-Query, prozentdekodiert; §6.5
Zusammensetzung.

**Repo:** `crates/coap-bridge/src/uri.rs::{parse_coap_uri, CoapUri,
CoapUri::to_uri, percent_decode, percent_encode, default_port, UriError}`
(IPv6-Literal in Klammern, Schema und Host ohne Groß-/Kleinschreibung,
Benutzerinformation abgewiesen, Fragment abgewiesen); der Ressourcenname
nach §6.5 Schritt 6 in `crates/coap-bridge/src/daemon/server.rs::resource_name`.

**Tests:** `crates/coap-bridge/src/uri.rs::tests::{coap_6_1_parses_basic_coap_uri,
coap_6_1_parses_coaps_default_port, coap_6_1_parses_explicit_port, coap_6_1_parses_query_params,
coap_6_1_parses_empty_path_when_no_slash, coap_6_1_rejects_unknown_scheme,
coap_6_1_rejects_missing_host, coap_6_1_rejects_invalid_port, coap_6_1_rejects_fragment,
coap_1_1_default_port_returns_5684_for_coaps, coap_6_4_an_empty_segment_is_a_uri_path_option,
coap_6_4_the_segments_and_arguments_are_percent_decoded, coap_6_1_an_ipv6_literal_is_in_brackets,
coap_6_3_scheme_and_host_ignore_case_and_the_default_port_is_implied}`,
`crates/coap-bridge/src/daemon/server.rs::tests::coap_6_5_the_resource_name_joins_the_segments`.

**Status:** done

---

## RFC 7252 §7 Discovery

### §7.1-§7.2 Service + Resource Discovery

**Spec:** §7, S. 73-74 — Service-Discovery per Multicast an die
All-CoAP-Nodes-Adressen; Resource-Discovery über `/.well-known/core` mit
den Abfragefiltern aus RFC 6690 §4.1.

**Repo:** `crates/coap-bridge/src/core_link.rs::{CoreLink, filter_links}`;
der Daemon tritt mit `coap.multicast.enabled` den Gruppen bei
(`crates/coap-bridge/src/daemon/multicast_io.rs::{groups_of, join}`,
Default 224.0.1.187 bzw. ff02::fd, ff05::fd) und beantwortet
`/.well-known/core` per Multicast (`crates/coap-bridge/src/daemon/server.rs::handle_multicast`).

**Tests:** `crates/coap-bridge/src/core_link.rs::tests::{coap_2_4_encode_simple_link_format,
coap_2_4_a_link_with_attributes_is_the_rfc_6690_form, coap_7_2_rfc_6690_query_filtering}`,
`crates/coap-bridge/src/multicast.rs::tests::{coap_1_1_well_known_addresses_match_spec, coap_7_1_valid_multicast_request_passes,
coap_7_1_multicast_request_must_not_be_confirmable}`, `crates/coap-bridge/tests/daemon_e2e.rs::coap_8_a_multicast_request_is_answered_by_unicast_within_the_leisure` (nur Linux).

**Status:** done

---

## RFC 7252 §8 Multicast CoAP

### §8 Multicast Operation

**Spec:** §8, S. 74-77 — §8.1 nur Non-confirmable, mit Token, keine
Reset-Antwort; §8.2 Antworten per Unicast zu einem zufälligen Zeitpunkt
der Leisure-Zeit, keine Fehlerantwort, keine leere Antwort; §8.2.1 Caching;
§8.2.2 Proxying.

**Repo:** `crates/coap-bridge/src/multicast.rs::{ALL_NODES_LINK_LOCAL_V4,
ALL_NODES_LINK_LOCAL_V6, ALL_NODES_SITE_LOCAL_V6, DEFAULT_LEISURE,
validate_multicast_request, leisure_delay}`; im Daemon
`crates/coap-bridge/src/daemon/server.rs::handle_multicast` und
`crates/coap-bridge/src/daemon/multicast_io.rs::Leisure` (Zeitpunkt in der
Leisure-Zeit, eine weitere Antwort für dieselbe Gruppe in der nächsten);
§8.2.2 in `crates/coap-bridge/src/daemon/proxy.rs::Proxy::fetch_multicast`
(alle Antworten innerhalb `proxy.multicast_wait_ms`, nicht gecacht).

**Tests:** `crates/coap-bridge/src/multicast.rs::tests::{coap_8_ipv4_multicast_recognizes_class_d_range,
coap_8_ipv6_multicast_recognizes_ff_prefix, coap_7_1_multicast_request_must_not_be_confirmable,
coap_8_multicast_request_must_have_token, coap_7_1_valid_multicast_request_passes,
coap_8_leisure_delay_within_max, coap_8_leisure_delay_zero_when_leisure_is_zero,
coap_1_1_well_known_addresses_match_spec, coap_8_default_leisure_is_5_seconds}`, `crates/coap-bridge/tests/daemon_e2e.rs::coap_8_a_multicast_request_is_answered_by_unicast_within_the_leisure` (nur Linux).

**Status:** done

### §8a Multicast-Versand und -Empfang im Daemon

**Spec:** §8, S. 74-77 — Requests an eine IP-Multicast-Gruppe senden und empfangen; ein Server antwortet nach der Leisure-Zeit.

**Repo:** `crates/coap-bridge/src/daemon/multicast_io.rs::{join,
enable_destination_info, recv_with_destination, Leisure}` (Beitritt auf
dem Socket des Daemons, eine IPv6-Gruppe bei IPv4-Bind auf einem zweiten,
nur-IPv6-Socket; die Zieladresse jedes Datagramms); Konfiguration
`coap.multicast.*` (`crates/coap-bridge/src/daemon/config.rs::MulticastSettings`);
Senden an eine Gruppe als Proxy-Ziel (§8.2.2).

**Tests:** `crates/coap-bridge/tests/daemon_e2e.rs::coap_8_a_multicast_request_is_answered_by_unicast_within_the_leisure` (nur Linux).

**Status:** done

---

## RFC 7252 §9 Securing CoAP

### §9 DTLS Mode

**Spec:** §9, S. 77-86 — NoSec/PreSharedKey/RawPublicKey/Certificate-
Modi; §9.1.3.1 PSK mit TLS_PSK_WITH_AES_128_CCM_8; §9.1.3.2 RawPublicKey mit
TLS_ECDHE_ECDSA_WITH_AES_128_CCM_8, secp256r1; §9.1.3.3 X.509, das
Zertifikat geprüft.

**Repo:** `crates/coap-bridge/src/dtls.rs::{DtlsMode, DtlsConfigError,
validate_dtls_mode, DtlsCredentials}`; der DTLS-1.2-Transport
`crates/coap-bridge/src/dtls_transport.rs` (Feature `dtls`) über
`crates/dtls-openssl` (OpenSSL ab 3.2, ADR 0014): PSK, Zertifikat mit Prüfung
gegen Vertrauensanker und Namen, Rohschlüssel nach RFC 7250. Die beiden
CCM-8-Suiten werden zuerst angeboten; OpenSSL 3 führt sie unter seiner
Standard-Sicherheitsstufe, ein Security-Callback
(`crates/dtls-openssl/src/ffi.rs::allow_ccm8`) lässt sie zu; P-256 steht in
der Gruppenliste. Gegenprobe mit libcoap 4.3.4 (GnuTLS und OpenSSL).

**Tests:** `crates/coap-bridge/src/dtls.rs::tests::{coap_9_nosec_is_not_secure, coap_1_1_psk_is_secure,
coap_9_raw_public_key_is_secure, coap_9_certificate_is_secure,
coap_9_validate_psk_short_key_rejected, coap_9_validate_psk_empty_identity_rejected,
coap_9_validate_psk_valid, coap_9_validate_certificate_empty_chain_rejected,
coap_9_validate_certificate_no_trust_anchors_rejected,
coap_9_validate_raw_public_key_empty_rejected, coap_9_validate_nosec_passes}`;
`crates/dtls-openssl/tests/handshake.rs::{coap_9_1_3_1_a_pre_shared_key_with_tls_psk_with_aes_128_ccm_8,
coap_9_1_3_1_an_unknown_identity_or_another_key_fails,
coap_9_1_3_2_raw_public_keys_named_after_rfc_6920,
coap_9_1_3_3_a_certificate_is_checked_against_the_anchors_and_the_name}`;
`crates/coap-bridge/tests/dtls_coap_e2e.rs::{coap_9_dtls_coap_get_content_roundtrip, coap_9_dtls_psk_handshake_carries_coap,
coap_9_dtls_psk_unknown_identity_is_refused, coap_9_1_3_2_raw_public_keys_carry_coap,
coap_9_dtls_certificate_files_and_rotation_keep_sessions_alive}`;
`crates/coap-bridge/tests/daemon_dtls_e2e.rs::{coap_9_daemon_serves_coaps_over_dtls_request_response_and_observe,
coap_9_1_3_2_the_daemon_takes_raw_public_keys_and_names_them_in_the_acl}`.

**Status:** done

### §9a RawPublicKey-Modus im DTLS-Transport

**Spec:** §9, S. 68 (Textfassung) — „The NoSec and RawPublicKey modes are mandatory to implement for this specification.“; §9.1.3.2 RawPublicKey (RFC 7250); §9.1.3.2.1 Kennungen nach RFC 6920, mindestens `sha-256-120`, Binärform, Sprechform bei einer Benutzerschnittstelle, eine Zugriffsliste der Kennungen.

**Repo:** `crates/dtls-openssl/src/ffi.rs` (`set_cert_types`, `peer_rpk_spki`,
`store_ctx_rpk_spki` über OpenSSLs `SSL_CTX_set1_client_cert_type`,
`SSL_CTX_set1_server_cert_type`, `SSL_get0_peer_rpk`,
`X509_STORE_CTX_get0_rpk`); `crates/dtls-openssl/src/config.rs`
(`ServerConfig::raw_public_key`/`trusted_client_keys`,
`ClientConfig::raw_public_key`/`trusted_server_keys`, der Verify-Callback
gegen die Kennungen); `crates/dtls-openssl/src/ni.rs::KeyName` (RFC 6920:
`ni:`, `nih:` mit Luhn-mod-16-Prüfziffer, Binärform; `sha-256` bis
`sha-256-32`); im Daemon `coap.dtls.rpk_key_file`/`rpk_trusted` und
`proxy.dtls.rpk_key_file`/`rpk_trusted`, das ACL-Subjekt `rpk:` mit der
`ni`-URI. Gegenprobe: libcoap 4.3.4 mit GnuTLS als Client gegen den Daemon und
als Ursprung hinter dem Proxy; ein nicht gelisteter Schlüssel erhält Alert 46.

**Tests:** `crates/dtls-openssl/src/ni.rs::tests::{rfc6920_8_2_the_names_of_the_example_key,
rfc6920_2_the_encodings_of_one_name_are_equal, rfc6920_8_1_hello_world}`,
`crates/dtls-openssl/tests/handshake.rs::coap_9_1_3_2_raw_public_keys_named_after_rfc_6920`,
`crates/coap-bridge/tests/dtls_coap_e2e.rs::coap_9_1_3_2_raw_public_keys_carry_coap`,
`crates/coap-bridge/tests/daemon_dtls_e2e.rs::coap_9_1_3_2_the_daemon_takes_raw_public_keys_and_names_them_in_the_acl`.

**Status:** done

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

**Repo:** `crates/coap-bridge/src/codec.rs::decode`;
`crates/coap-bridge/src/daemon/server.rs::resource_name` bildet den einen
Ressourcennamen (§6.5 Schritt 6, ein `/` in einem Segment
prozentkodiert, `.`/`..` abgewiesen), den die Topic-Zuordnung und die ACL
gleichermaßen verwenden; `crates/coap-bridge/src/uri.rs::parse_coap_uri`.

**Tests:** `crates/coap-bridge/tests/fuzz_smoke.rs::coap_11_1_fuzz_coap_decode_no_panic`,
`crates/coap-bridge/src/daemon/server.rs::tests::{coap_11_1_a_slash_in_a_segment_does_not_name_another_resource,
coap_5_10_1_a_dot_segment_is_no_resource_name, coap_6_5_the_resource_name_joins_the_segments}`,
`crates/coap-bridge/tests/daemon_e2e.rs::coap_5_9_2_5_an_unknown_resource_is_4_04_and_a_dot_segment_4_00`,
`crates/coap-bridge/src/uri.rs::tests::{coap_6_1_rejects_fragment, coap_6_4_the_segments_and_arguments_are_percent_decoded}`.

**Status:** done

### §11.2 Cache: keine Auslieferung an Anfragen mit geringerer Transportsicherheit

**Spec:** §11.2, S. 81 (RFC) — "a caching proxy MUST NOT make cached
values available to requests that have lesser transport-security
properties than those the proxy would require to perform request
forwarding in the first place."

**Repo:** `crates/coap-bridge/src/caching_proxy.rs::{CacheKey, CacheScope}`:
die Antwort auf eine `coaps`-Anfrage liegt unter `CacheScope::Private` der
DTLS-Identität des Clients und wird nur einer Anfrage mit derselben
Identität geliefert; ohne DTLS-Identität wird sie nicht gespeichert
(`crates/coap-bridge/src/daemon/proxy.rs::Proxy::cache_key`). Der Proxy
leitet eine `coaps`-Anfrage nur über DTLS auf der Seite des Clients weiter
(`proxy.require_dtls_for_coaps`, sonst 4.01). Die geteilten Einträge für
`coap` liefert er erst nach der Authentifizierung der Bridge (§7.2 der
Bridge-Spezifikation), die auch für das Weiterleiten gilt.

**Tests:** `crates/coap-bridge/src/caching_proxy.rs::tests::coap_11_2_a_coaps_response_is_served_only_to_the_same_identity`,
`crates/coap-bridge/src/daemon/proxy.rs::tests::coap_11_2_a_coaps_response_is_cached_only_under_the_dtls_identity`,
`crates/coap-bridge/tests/proxy_e2e.rs::coap_5_7_a_coaps_destination_is_reached_over_dtls`.

**Status:** done

### §11.2 coaps-Antworten nicht im geteilten Cache

**Spec:** §11.2, S. 81 (RFC) — "responses to "coaps" identified requests
are never "public" and thus MUST NOT be reused for shared caching,
unless the cache is able to make equivalent access control decisions to
the ones that led to the cached entry."

**Repo:** `crates/coap-bridge/src/caching_proxy.rs::CacheScope`
(`Shared` für `coap`, `Private(identity)` für `coaps`), gesetzt in
`crates/coap-bridge/src/daemon/proxy.rs::Proxy::cache_key`.

**Tests:** `crates/coap-bridge/src/caching_proxy.rs::tests::coap_11_2_a_coaps_response_is_served_only_to_the_same_identity`,
`crates/coap-bridge/src/daemon/proxy.rs::tests::coap_11_2_a_coaps_response_is_cached_only_under_the_dtls_identity`,
`crates/coap-bridge/tests/proxy_e2e.rs::coap_5_7_a_coaps_destination_is_reached_over_dtls`.

**Status:** done

### §11.3 Keine große Verstärkung ohne Authentifizierung

**Spec:** §11.3, S. 82 (RFC) — "large amplification factors SHOULD NOT
be provided in the response if the request is not authenticated."

**Repo:** `crates/coap-bridge/src/daemon/server.rs::{block_limit,
representation, UNAUTHENTICATED_AMPLIFICATION}`: eine Anfrage ohne DTLS,
OSCORE und Auth-Token bekommt höchstens das Achtfache ihrer Größe zurück,
eine größere Repräsentation in Block2-Blöcken (RFC 7959) mit ETag und Size2;
ebenso die Benachrichtigungen an einen so registrierten Beobachter und die
Antworten des Proxys (`crates/coap-bridge/src/daemon/proxy.rs::Proxy::for_client`).

**Tests:** `crates/coap-bridge/src/daemon/server.rs::tests::coap_11_3_an_unauthenticated_response_is_sliced_into_small_blocks`,
`crates/coap-bridge/tests/proxy_e2e.rs::rfc7959_2_10_the_proxy_reassembles_and_cuts_blocks`.

**Status:** done

### §11.3 Multicast-Anfragen nur authentifiziert und nur für bestimmte Ressourcen

**Spec:** §11.3, S. 82 (RFC) — "CoAP servers SHOULD NOT accept multicast
requests that can not be authenticated in some way [...]. If possible, a
CoAP server SHOULD limit the support for multicast requests to the
specific resources where the feature is required."

**Repo:** `crates/coap-bridge/src/daemon/server.rs::handle_multicast` mit
`coap.multicast.resources` (die Ressourcen, die eine Multicast-Anfrage
beantworten) und `coap.multicast.unauthenticated_resources` (die, die es
ohne OSCORE tun); beide Default `.well-known/core`.

**Tests:** `crates/coap-bridge/tests/daemon_e2e.rs::coap_8_a_multicast_request_is_answered_by_unicast_within_the_leisure` (nur Linux).

**Status:** done

### §11.3 Multicast-Ziel über IPV6_RECVPKTINFO erkennen

**Spec:** §11.3, S. 82 (RFC) — "Implementations SHOULD make use of
modern APIs such as IPV6_RECVPKTINFO [RFC3542], if available, to make
this determination."

**Repo:** `crates/coap-bridge/src/daemon/multicast_io.rs::{enable_destination_info,
recv_with_destination}` (`IP_PKTINFO`/`IPV6_RECVPKTINFO` über `nix`,
ohne `unsafe`); die Empfangsschleife
(`crates/coap-bridge/src/daemon/server.rs::accept_loop`) gibt eine Anfrage
an eine Multicast-Zieladresse an `handle_multicast`.

**Tests:** `crates/coap-bridge/tests/daemon_e2e.rs::coap_8_a_multicast_request_is_answered_by_unicast_within_the_leisure` (nur Linux).

**Status:** done

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

**Repo:** Zufall stammt vom Betriebssystem: die Tokens des Proxys über
`getrandom` (`crates/coap-bridge/src/daemon/proxy.rs::random_token`), das
selbstsignierte DTLS-Zertifikat mit dem Zufall von OpenSSL (`crates/dtls-openssl/src/config.rs`); OSCORE erzeugt
keine Schlüssel, sondern leitet sie mit HKDF aus dem konfigurierten
Master Secret ab. Die AEAD-Prüfung ist die von RustCrypto `ccm` 0.5.0 (der
Tag-Vergleich mit `subtle::ConstantTimeEq`, `ccm-0.5.0/src/lib.rs`, Zeile
281), AES die von `aes` 0.8.4. Die Laufzeitkonstanz ist dem Quelltext der
Abhängigkeiten entnommen, nicht gemessen.

**Tests:** `crates/coap-bridge/src/daemon/proxy.rs::tests::coap_5_3_1_the_proxy_tokens_come_from_the_system_randomness`,
`crates/coap-bridge/src/oscore/aead.rs::tests::rfc3610_packet_vector_1`.

**Status:** done

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

**Repo:** `crates/coap-bridge/src/content_format.rs` mit den
Anfangseinträgen, CBOR (60) und den Formaten der Bridge im
experimentellen Bereich (65000–65002), `media_type`; der Daemon setzt 40
für `/.well-known/core` und 65000 für ein Topic.

**Tests:** `crates/coap-bridge/src/content_format.rs::tests::{coap_12_3_the_initial_registry_entries,
coap_12_3_the_bridges_formats_are_in_the_experimental_range}`.

**Status:** done

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

42 done / 0 partial / 0 open / 0 blocked / 4 n/a (informative).

Test-Lauf: in CI, Krate-Ebene und Säulen-Jobs; die zitierten Tests mit dem letzten Ergebnis jedes Jobs in den Pipelines 8501 bis 9860 von `main` (`python3 ci/coverage_results.py --document coap-rfc-7252.md`):

* `cargo nextest run --profile merge -p zerodds-coap-bridge --all-targets` (Job `merge-rust-coap-bridge`) — 156 Tests grün.
* `cargo nextest run --profile merge -p zerodds-dtls-openssl --all-targets` (Job `merge-rust-dtls-openssl`) — 7 Tests grün.
* `cargo nextest run --profile merge -p zerodds-coap-bridge --all-targets --features dtls,oscore` (Job `merge-rust-coap-bridge-features-dtls-oscore`) — 9 Tests grün.

172 zitierte Tests grün, 0 nicht grün.
