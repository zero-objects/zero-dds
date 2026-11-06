# IETF CoAP RFC 7252 + RFC 7641 — Spec Coverage

**RFC 7252:** [The Constrained Application Protocol — IETF, June 2014 →](https://www.rfc-editor.org/rfc/rfc7252)

**RFC 7641:** [Observing Resources in CoAP — IETF, September 2015 →](https://www.rfc-editor.org/rfc/rfc7641)

**Context:** CoAP is a RESTful spec for constrained devices. ZeroDDS
implements it in `crates/coap-bridge/`: the wire codec (Section 3, 5.10,
RFC 7641 §2) as a no_std+alloc library, plus reliability, block-wise
transfer (RFC 7959), observe, a DTLS transport (feature `dtls`), OSCORE and
the daemon.

Implementation:

- `crates/coap-bridge/` — codec, transport building blocks and the daemon `zerodds-coap-bridged`.

---

## RFC 7252 §1 Introduction

### §1.1 Features

**Spec:** §1.1, p. 5 (RFC) — REST model, UDP-bound, multicast support, DTLS
security, asynchronous messaging.

**Repo:** `crates/coap-bridge/src/lib.rs` crate doc.

**Tests:** REST methods + response classes:
`crates/coap-bridge/src/message.rs::tests::coap_1_1_well_known_codes_match_spec_values`;
asynchronous messaging (CON/NON/ACK/RST):
`crates/coap-bridge/src/message.rs::tests::coap_1_1_the_message_types_are_the_numbers_of_section_3`;
multicast support:
`crates/coap-bridge/src/multicast.rs::tests::coap_1_1_well_known_addresses_match_spec`;
DTLS security:
`crates/coap-bridge/src/dtls.rs::tests::coap_1_1_psk_is_secure`;
UDP-bound (default ports 5683/5684):
`crates/coap-bridge/src/uri.rs::tests::coap_1_1_default_port_returns_5684_for_coaps`.

**Status:** done — overview; multicast and DTLS are items of their own (§8, §9, §11).

### §1.2 Terminology

**Spec:** §1.2, p. 6-9 — endpoint, sender, recipient, token, origin server,
cacheable, resource, etc.

**Repo:** —

**Tests:** —

**Status:** `n/a (informative)` — glossary; a semantic reference point
without a code mapping.

---

## RFC 7252 §2 Constrained Application Protocol

### §2.1 Messaging model

**Spec:** §2.1, p. 11 — CON/NON/ACK/RST.

**Repo:** `crates/coap-bridge/src/message.rs::MessageType`.

**Tests:** `message::tests::coap_1_1_the_message_types_are_the_numbers_of_section_3`.

**Status:** done

### §2.2 Request/response model

**Spec:** §2.2, p. 12 — method codes + response codes.

**Repo:** `crates/coap-bridge/src/message.rs::CoapCode` with the constants
`GET/POST/PUT/DELETE/CREATED/DELETED/VALID/CHANGED/CONTENT/BAD_REQUEST/NOT_FOUND/INTERNAL_SERVER_ERROR`.

**Tests:** `crates/coap-bridge/src/message.rs::tests::coap_1_1_well_known_codes_match_spec_values`,
`coap_2_2_code_classification_predicates`.

**Status:** done

### §2.3 Intermediaries and caching

**Spec:** §2.3, p. 15 — forward proxies, caches.

**Repo:** `crates/coap-bridge/src/caching_proxy.rs::{CoapCache, CacheLookup,
ProxyConfig, ProxyMode}` — a cache layer with freshness tracking +
stale/revalidate path; proxy configuration with forward/reverse mode +
hop-count cap (spec §5.7.1 loop protection).

**Tests:** `crates/coap-bridge/src/caching_proxy.rs::tests::{coap_2_3_empty_cache_returns_miss,
coap_2_3_store_then_fresh_lookup, coap_2_3_stale_with_etag_yields_validation_path,
coap_2_3_stale_without_etag_yields_miss, coap_2_3_cap_evicts_oldest,
coap_2_3_evict_expired_without_etag_keeps_revalidatable,
coap_2_3_proxy_config_default_is_forward_coap_to_coap}`.

**Status:** done — caching + forward-proxy configuration live.

### §2.4 Resource discovery

**Spec:** §2.4, p. 15 — `.well-known/core` (RFC 6690).

**Repo:** `crates/coap-bridge/src/core_link.rs::{CoreLink, encode_links,
decode_links}`.

**Tests:** `crates/coap-bridge/src/core_link.rs::tests::{coap_2_4_encode_simple_link_format,
coap_2_4_encode_multiple_links_comma_separated, coap_2_4_a_link_with_attributes_is_the_rfc_6690_form,
coap_2_4_decode_handles_numeric_attr, coap_2_4_decode_handles_quoted_string_with_comma,
coap_2_4_decode_rejects_uri_without_brackets, coap_2_4_decode_attr_without_value_yields_empty,
coap_2_4_encode_escapes_inner_quotes}`.

**Status:** done

---

## RFC 7252 §3 Message format

### §3 Header format (Ver/T/TKL/Code/MID)

**Spec:** §3, p. 16 — a 4-byte fixed header. Version `MUST be 1 (01 binary)`,
TKL `0..=8` (9-15 reserved).

**Repo:** `crates/coap-bridge/src/codec.rs::encode/decode` sets Version=1 on
encode and rejects other versions on decode.

**Tests:** `codec::tests::coap_3_encodes_minimum_get_request_to_4_byte_header`,
`coap_3_unsupported_version_decode_fails`,
`coap_3_reserved_token_length_9_through_15_decode_fails`,
`coap_3_token_length_above_8_is_rejected_on_encode`.

**Status:** done

### §3 Token

**Spec:** §3, p. 16 — a token 0..=8 bytes after the header.

**Repo:** `crates/coap-bridge/src/message.rs::CoapMessage::token`.

**Tests:** `codec::tests::coap_3_the_header_of_each_message_type_is_the_octets_of_the_layout`,
`coap_3_token_truncated_decode_fails`.

**Status:** done

### §3 Payload marker

**Spec:** §3, p. 16-17 — `0xFF` marks the start of the payload. "The presence
of a marker followed by a zero-length payload MUST be processed as a message
format error."

**Repo:** `crates/coap-bridge/src/codec.rs` (encode + decode).

**Tests:** `codec::tests::coap_3_payload_marker_separates_options_from_payload`,
`coap_3_payload_marker_without_payload_is_format_error`.

**Status:** done

### §3.1 Option format (delta encoding)

**Spec:** §3.1, p. 17-19 — a 4-bit delta nibble + a 4-bit length nibble +
extended forms (13/14) + reserved (15). Options "MUST appear in order of
their Option Numbers".

**Repo:** `crates/coap-bridge/src/codec.rs::encode_extended/decode_extended`.
Encode sorts options before the delta computation.

**Tests:** `codec::tests::coap_3_1_single_short_option_encodes_in_one_byte_header`,
`coap_3_1_option_delta_extended_13_uses_one_extra_byte`,
`coap_3_1_option_delta_extended_14_uses_two_extra_bytes`,
`coap_3_1_option_length_extended_13_uses_one_extra_byte`,
`coap_3_1_delta_encoding_sums_across_multiple_options`,
`coap_3_1_options_are_sorted_on_encode`.

**Status:** done

### §3.2 Option value formats

**Spec:** §3.2, p. 19-20 — `empty`, `opaque`, `uint`, `string`. uint "without
leading zero bytes"; "MUST be prepared to process values with leading zero
bytes".

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

## RFC 7252 §4 Message transmission

### §4.1 Messages and endpoints

**Spec:** §4.1, p. 20 — endpoint identification via IP+port.

**Repo:** `crates/coap-bridge/src/bridge.rs` (endpoint as a `Vec<u8>` ID); the
UDP reactor deliberately lives at the caller layer via
`crates/transport-udp/` — this separation allows no_std builds with their own
transport (e.g. an embedded stack without OS sockets). Spec §4.1 standardizes
the endpoint identification, not the UDP reactor itself.

**Tests:** `crates/coap-bridge/src/bridge.rs::tests::{coap_4_1_parses_topic_only,
coap_4_1_parses_topic_with_key, coap_4_1_rejects_non_dds_path,
coap_4_1_bridge_apply_write_then_read_round_trip,
coap_4_1_bridge_observe_register_increments_observer_count}`.

**Status:** done — endpoint tracking as a spec-conformant Vec<u8> ID; the
transport decoupling is an intentional architecture for no_std targets.

### §4.2-§4.7 Reliability + deduplication + congestion

**Spec:** §4.2-§4.7, p. 21-26 — CON retry with exponential backoff up to
`MAX_RETRANSMIT`; §4.5 "The recipient [...] SHOULD acknowledge each duplicate
copy of a Confirmable message [...] but SHOULD process any request or response
in the message only once" — Message ID duplicates within `EXCHANGE_LIFETIME`
(CON) resp. `NON_LIFETIME` (NON); §4.7 `NSTART` as the cap on simultaneously
outstanding interactions towards one server.

**Repo:** `crates/coap-bridge/src/reliability.rs::{ReliabilityTracker,
PendingConfirmable, TickOutput}` — `send_confirmable`/`receive_ack`/`receive_rst`/`tick`
cover the CON retry + ACK/RST path;
`crates/coap-bridge/src/reliability.rs::{DuplicateDetector, DedupKind,
Reception}` with `receive`/`remember_ack`/`evict_expired` for the §4.5
deduplication (cached ACK instead of a second processing);
`crates/coap-bridge/src/reliability.rs::OutstandingInteractions` with
`try_begin`/`end` for the §4.7 `NSTART` cap per server.

**Tests:** `crates/coap-bridge/src/reliability.rs::tests::{coap_4_2_tick_before_timeout_does_nothing,
coap_4_2_tick_after_timeout_retransmits, coap_4_2_exhausting_retransmits_times_out,
coap_4_2_interval_doubles_per_retransmit, coap_4_2_send_then_ack_clears_pending,
coap_4_2_rst_clears_pending, coap_4_2_duplicate_message_id_within_exchange_lifetime_is_detected,
coap_4_2_duplicate_entry_expires_after_its_lifetime, coap_4_2_duplicate_detection_is_per_endpoint,
coap_4_2_duplicate_table_cap_bounds_memory, coap_4_2_nstart_caps_outstanding_interactions_per_server}`.

**Status:** done

### §4.8 Transmission parameters

**Spec:** §4.8, p. 27-29 — Table 2: `ACK_TIMEOUT` 2 s, `ACK_RANDOM_FACTOR` 1.5,
`MAX_RETRANSMIT` 4, `NSTART` 1, `DEFAULT_LEISURE` 5 s, `PROBING_RATE`
1 byte/s. §4.8.2 derives from them:
`MAX_TRANSMIT_SPAN = ACK_TIMEOUT * ((2 ** MAX_RETRANSMIT) - 1) *
ACK_RANDOM_FACTOR` (45 s), `MAX_TRANSMIT_WAIT` (93 s),
`EXCHANGE_LIFETIME = MAX_TRANSMIT_SPAN + (2 * MAX_LATENCY) +
PROCESSING_DELAY` (247 s), `NON_LIFETIME` (145 s).

**Repo:** `crates/coap-bridge/src/reliability.rs::{ACK_TIMEOUT_MS,
ACK_RANDOM_FACTOR_NUM, ACK_RANDOM_FACTOR_DEN, MAX_RETRANSMIT, NSTART,
DEFAULT_LEISURE_MS, PROBING_RATE_BYTES_PER_SEC, MAX_LATENCY_MS,
PROCESSING_DELAY_MS, MAX_TRANSMIT_SPAN_MS, MAX_TRANSMIT_WAIT_MS,
EXCHANGE_LIFETIME_MS, NON_LIFETIME_MS}`; `initial_interval` uses
`ACK_TIMEOUT * ACK_RANDOM_FACTOR` as the first retry interval.

**Tests:** `crates/coap-bridge/src/reliability.rs::tests::{coap_4_8_transmission_parameters_match_spec_4_8_table_2,
coap_4_8_derived_transmission_parameters_follow_spec_4_8_2_formulas,
coap_4_8_backoff_schedule_follows_ack_timeout_and_random_factor}` — the last one checks
the resulting retry sequence 3000/6000/12000/24000 ms and the give-up after
`MAX_RETRANSMIT` at `MAX_TRANSMIT_WAIT`.

**Status:** done

---

## RFC 7252 §5 Request/response semantics

### §5.1 Requests

**Spec:** §5.1, p. 35-36 — request definition: method code (1-31), token,
options, payload.

**Repo:** method codes 1=GET, 2=POST, 3=PUT, 4=DELETE as `CoapCode` variants
in `crates/coap-bridge/src/message.rs`.

**Tests:** `crates/coap-bridge/src/message.rs::tests::coap_1_1_well_known_codes_match_spec_values`.

**Status:** done

### §5.2 Responses

**Spec:** §5.2, p. 36 — response definition: response code (2.xx success, 4.xx
client error, 5.xx server error), echoed token.

**Repo:** response codes as `CoapCode` variants (2.05 Content, 4.04 Not Found,
5.00 Internal Server Error, etc.).

**Tests:** `crates/coap-bridge/src/message.rs::tests::coap_1_1_well_known_codes_match_spec_values`.

**Status:** done

### §5.3 Request/response matching

**Spec:** §5.3, p. 37-39 — token-based matching for async replies
(piggybacked, separate, reset).

**Repo:** the token field in `CoapMessage` as a `Vec<u8>` with a length
constraint (TKL 0-8 bytes) + `crates/coap-bridge/src/matching.rs::{PendingRequests,
MatchError}` with `submit`/`complete`/`evict_expired` and spec-§4.8
EXCHANGE_LIFETIME support; a DoS cap via `max_pending`.

**Tests:** `matching::tests::{coap_5_3_submit_empty_token_rejected,
coap_5_3_submit_then_complete_round_trip, coap_5_3_complete_unknown_token_returns_none,
coap_5_3_submit_above_cap_rejected, coap_5_3_submit_duplicate_token_rejected,
coap_5_3_evict_expired_removes_overdue_pendings, coap_5_3_evict_keeps_fresh_pendings}`.

**Status:** done — token wire format + pending-request map with lifetime
eviction live.

### §5.4 Options

**Spec:** §5.4, p. 39-43 — option-class properties: Critical/Elective,
Unsafe/Safe, NoCacheKey, Repeatable.

**Repo:** option numbers in `crates/coap-bridge/src/option.rs::numbers`;
the property bits derived from the option number in
`crates/coap-bridge/src/option.rs::{is_critical, is_unsafe, is_nocachekey,
is_repeatable}` following the §5.4.6 formula (`Critical = (onum & 1)`,
`UnSafe = (onum & 2)`, `NoCacheKey = ((onum & 0x1e) == 0x1c)`) plus the
same-named methods on `CoapOption`.

**Tests:** `crates/coap-bridge/src/option.rs::tests::{coap_5_4_critical_unsafe_nocachekey_bits_per_spec_5_4_6,
coap_5_4_repeatable_options_match_spec_table_4, coap_5_4_observe_option_uses_number_6}`.

**Status:** done

### §5.5 Payloads and representations

**Spec:** §5.5.1/§5.5.3, p. 43-44 — the payload is a representation whose
media type the Content-Format option names; §5.5.2 "If no Content-Format
option is given, the payload of responses indicating a client or server error
is a brief human-readable diagnostic message" in UTF-8.

**Repo:** `crates/coap-bridge/src/message.rs::CoapMessage::payload` as a
`Vec<u8>`; the Content-Format option (number 12) via
`crates/coap-bridge/src/option.rs::CoapOption::content_format`; the
payload-marker split in `crates/coap-bridge/src/codec.rs`.

**Tests:** `crates/coap-bridge/src/codec.rs::tests::{coap_5_5_representation_carries_content_format_per_spec_5_5,
coap_5_5_diagnostic_payload_of_error_response_carries_no_content_format,
coap_3_payload_marker_separates_options_from_payload}`.

**Status:** done

### §5.6 Caching

**Spec:** §5.6, p. 44-46 — freshness lifetime via the Max-Age option;
validation via the ETag option.

**Repo:** Max-Age (number 14) + ETag (number 4) option numbers defined; cache
state logic in `crates/coap-bridge/src/caching_proxy.rs::{CoapCache,
CacheLookup, DEFAULT_MAX_AGE}` with `store/lookup/evict_expired_without_etag`,
a DoS-cap FIFO eviction, and the spec-§5.6.1 default Max-Age (60s).

**Tests:** `caching_proxy::tests::{coap_2_3_empty_cache_returns_miss,
coap_2_3_store_then_fresh_lookup, coap_2_3_stale_with_etag_yields_validation_path,
coap_2_3_stale_without_etag_yields_miss, coap_2_3_cap_evicts_oldest,
coap_2_3_evict_expired_without_etag_keeps_revalidatable}`.

**Status:** done — cache state + freshness + ETag-validation path live.

### §5.7 Proxying

**Spec:** §5.7, p. 46-49 — the Proxy-Uri option, forward/reverse proxy, HTTP
mapping.

**Repo:** the Proxy-Uri option (number 35) defined + proxy logic in
`crates/coap-bridge/src/caching_proxy.rs::{ProxyConfig, ProxyMode}` with
forward/reverse mode + a max_hops cap (spec §5.7.1 loop protection) + an
http_translation flag for the §10 path.

**Tests:** `caching_proxy::tests::coap_2_3_proxy_config_default_is_forward_coap_to_coap`.

**Status:** partial — the Proxy-Uri option, forward/reverse mode and the hop limit are built as configuration; forwarding a request is missing (§5.7a).

### §5.7a Forwarding through a proxy

**Spec:** §5.7, pp. 46-49 — a forward proxy performs the request against the target URI from `Proxy-Uri`, a reverse proxy maps it to an origin server, and both return the response.

**Repo:** —

**Tests:** —

**Status:** open — `crates/coap-bridge/src/caching_proxy.rs` holds only `ProxyConfig`/`ProxyMode`; no path forwards a request to the target.

### §5.8 Method definitions

**Spec:** §5.8, p. 49-51 — GET/POST/PUT/DELETE semantics, idempotency.

**Repo:** method codes defined in `crates/coap-bridge/src/message.rs` + the method-property helpers
`crates/coap-bridge/src/method_props.rs::{is_safe, is_idempotent, is_method}`
with spec-conformant values (GET = safe + idempotent; PUT/DELETE = idempotent;
POST = neither).

**Tests:** `method_props::tests::{coap_5_8_get_is_safe, coap_5_8_post_put_delete_not_safe,
coap_5_8_get_put_delete_idempotent, coap_5_8_post_not_idempotent,
coap_5_8_method_predicate_recognizes_all_four}`.

**Status:** done — codes + method properties (safety + idempotency) declared.

### §5.9 Response code definitions

**Spec:** §5.9, p. 52-58 — the complete list of all response codes (2.xx,
4.xx, 5.xx) with meaning.

**Repo:** all important codes as `CoapCode` variants in `crates/coap-bridge/src/message.rs`.

**Tests:** `crates/coap-bridge/src/message.rs::tests::coap_1_1_well_known_codes_match_spec_values`.

**Status:** done

### §5.10 Option definitions

**Spec:** §5.10, p. 53-67 — the number registry: Uri-Host (3), ETag (4),
If-None-Match (5), Uri-Port (7), Location-Path (8), Uri-Path (11),
Content-Format (12), Max-Age (14), Uri-Query (15), Accept (17),
Location-Query (20), Proxy-Uri (35), Proxy-Scheme (39), Size1 (60). Plus
If-Match (1).

**Repo:** `crates/coap-bridge/src/option.rs::numbers` with all constants +
the convenience constructors `uri_path`/`content_format`.

**Tests:** `option::tests::coap_5_10_option_uri_path_constructs_string_value`,
`coap_5_10_content_format_uses_number_12`,
`codec::tests::coap_5_10_an_observe_registration_is_the_octets_of_the_layout`.

**Status:** done

---

## RFC 7252 §6 CoAP URIs

### §6.1-§6.5 CoAP URI scheme

**Spec:** §6, p. 67-72 — `coap://` + `coaps://` URI schemes.

**Repo:** `crates/coap-bridge/src/uri.rs::{parse_coap_uri, CoapUri,
default_port, UriError}`. Default ports 5683 / 5684; path segments + query
parameters are split out (for direct mapping into the Uri-Path/Uri-Query
options); a fragment identifier is rejected (§6.1).

**Tests:** `crates/coap-bridge/src/uri.rs::tests::{coap_6_1_parses_basic_coap_uri,
coap_6_1_parses_coaps_default_port, coap_6_1_parses_explicit_port, coap_6_1_parses_query_params,
coap_6_1_parses_empty_path_when_no_slash, coap_6_1_rejects_unknown_scheme, coap_6_1_rejects_missing_host,
coap_6_1_rejects_invalid_port, coap_6_1_rejects_fragment, coap_1_1_default_port_returns_5684_for_coaps,
coap_6_1_round_trip_path_filtering_strips_empty_segments}` (11).

**Status:** done — the coap/coaps URI-scheme parser live.

---

## RFC 7252 §7 Discovery

### §7.1-§7.2 Service + resource discovery

**Spec:** §7, p. 73-74 — multicast discovery + `.well-known/core`.

**Repo:** resource discovery via `crates/coap-bridge/src/core_link.rs`;
service discovery via multicast in `multicast.rs::{ALL_NODES_LINK_LOCAL_V4,
ALL_NODES_LINK_LOCAL_V6, ALL_NODES_SITE_LOCAL_V6, validate_multicast_request}`
(see §8).

**Tests:** resource discovery:
`crates/coap-bridge/src/core_link.rs::tests::{coap_2_4_encode_simple_link_format,
coap_2_4_a_link_with_attributes_is_the_rfc_6690_form}`; service discovery:
`crates/coap-bridge/src/multicast.rs::tests::{coap_1_1_well_known_addresses_match_spec,
coap_7_1_valid_multicast_request_passes, coap_7_1_multicast_request_must_not_be_confirmable}`.

**Status:** partial — resource discovery via `/.well-known/core` and the multicast addresses are built and checked; service discovery by multicast needs the multicast reception of §8a.

---

## RFC 7252 §8 Multicast CoAP

### §8 Multicast operation

**Spec:** §8, p. 74-77 — IP-multicast semantics.

**Repo:** `crates/coap-bridge/src/multicast.rs::{ALL_NODES_LINK_LOCAL_V4
("224.0.1.187"), ALL_NODES_LINK_LOCAL_V6 ("FF02::FD"), ALL_NODES_SITE_LOCAL_V6
("FF05::FD"), DEFAULT_LEISURE (5s), is_ipv4_multicast, is_ipv6_multicast,
validate_multicast_request, leisure_delay, MulticastError}` with spec-§8.1
NON-only validation + the §8.2 leisure-time helper.

**Tests:** `multicast::tests::*` (9 tests):
`coap_8_ipv4_multicast_recognizes_class_d_range`,
`coap_8_ipv6_multicast_recognizes_ff_prefix`,
`coap_7_1_multicast_request_must_not_be_confirmable`,
`coap_8_multicast_request_must_have_token`, `coap_7_1_valid_multicast_request_passes`,
`coap_8_leisure_delay_within_max`, `coap_8_leisure_delay_zero_when_leisure_is_zero`,
`coap_1_1_well_known_addresses_match_spec`, `coap_8_default_leisure_is_5_seconds`.

**Status:** partial — multicast addresses, the NON-only check, the token requirement and the leisure time are built and checked; group join, sending and receiving in the daemon are missing (§8a).

### §8a Multicast sending and receiving in the daemon

**Spec:** §8, pp. 74-77 — send and receive requests to an IP multicast group; a server answers after the leisure time.

**Repo:** —

**Tests:** —

**Status:** open — `crates/coap-bridge/src/` joins no multicast group (no `join_multicast_v4`/`join_multicast_v6`); no socket path uses the helpers of `multicast.rs`.

---

## RFC 7252 §9 Securing CoAP

### §9 DTLS mode

**Spec:** §9, p. 77-86 — NoSec/PreSharedKey/RawPublicKey/Certificate modes.

**Repo:** `crates/coap-bridge/src/dtls.rs::{DtlsMode, DtlsConfigError,
validate_dtls_mode}` with all four modes:
- `NoSec` (default port 5683).
- `PreSharedKey { identity, key }` (spec §9.1.1 — PSK ≥ 16 bytes recommended,
  identity required).
- `RawPublicKey { public_key, spki_der }` (spec §9.1.2 — SubjectPublicKeyInfo
  DER-encoded).
- `Certificate { cert_chain, trust_anchors }` (spec §9.1.3 — X.509 chain
  validation against trust anchors).

An `is_secure()` predicate + `default_port()` (5683/5684) + `name()` for
logging. DTLS 1.2 transport in `crates/coap-bridge/src/dtls_transport.rs`
(feature `dtls`, `webrtc-dtls`) with PSK and certificate.

**Tests:** `crates/coap-bridge/src/dtls.rs::tests::{coap_9_nosec_is_not_secure, coap_1_1_psk_is_secure, coap_9_raw_public_key_is_secure, coap_9_certificate_is_secure, coap_9_validate_psk_short_key_rejected, coap_9_validate_psk_empty_identity_rejected, coap_9_validate_psk_valid, coap_9_validate_certificate_empty_chain_rejected, coap_9_validate_certificate_no_trust_anchors_rejected, coap_9_validate_raw_public_key_empty_rejected, coap_9_validate_nosec_passes}`; `crates/coap-bridge/tests/dtls_coap_e2e.rs::{coap_9_dtls_coap_get_content_roundtrip, coap_9_dtls_psk_handshake_carries_coap, coap_9_dtls_psk_unknown_identity_is_refused, coap_9_dtls_certificate_files_and_rotation_keep_sessions_alive}`; `crates/coap-bridge/tests/daemon_dtls_e2e.rs::coap_9_daemon_serves_coaps_over_dtls_request_response_and_observe`.

**Status:** partial — NoSec, PreSharedKey and Certificate run over the DTLS transport; RawPublicKey exists only as configuration (§9a).

### §9a RawPublicKey mode in the DTLS transport

**Spec:** §9, p. 68 (text version) — "The NoSec and RawPublicKey modes are mandatory to implement for this specification."; §9.1.3.2 RawPublicKey (RFC 7250).

**Repo:** —

**Tests:** —

**Status:** open — `crates/coap-bridge/src/dtls.rs` knows `DtlsMode::RawPublicKey` as configuration; `crates/coap-bridge/src/dtls_transport.rs` negotiates only PSK and certificate (`DtlsPeerIdentity::{Psk, Certificate}`), a handshake with a raw public key (RFC 7250) is missing.

---

## RFC 7252 §10-§13 Inter-op + security + IANA + acknowledgments

### §10 HTTP cross-protocol mapping

**Spec:** §10, p. 86-91 — HTTP↔CoAP translation for proxy use cases. §10.1
status-code mapping (Tab 8); §10.2 method mapping (Tab 9).

**Repo:** `crates/coap-bridge/src/caching_proxy.rs::{http_status_to_coap,
coap_to_http_status, http_method_to_coap, coap_method_to_http}` with
spec-§10.1/§10.2-conformant mapping (200↔2.05, 404↔4.04, 500↔5.00, GET↔1,
POST↔2, PUT↔3, DELETE↔4, etc.).

**Tests:** `caching_proxy::tests::{coap_10_http_200_maps_to_coap_2_05,
coap_10_http_404_maps_to_coap_4_04, coap_10_http_500_maps_to_coap_5_00,
coap_10_http_unknown_returns_none, coap_10_the_status_codes_map_both_ways, coap_10_http_get_maps_to_coap_1,
coap_10_http_unknown_method_returns_none, coap_10_the_methods_map_both_ways}`.

**Status:** done — HTTP status and method mapping bidirectional.

### §11.1 Parsing and URI processing

**Spec:** §11.1, p. 80 (RFC) — "the URI processing code in CoAP
implementations [...] should be implemented with special care. CoAP
access control implementations need to ensure they don't introduce
vulnerabilities through discrepancies between the code deriving access
control decisions from a URI and the code finally serving up the
resource addressed by the URI."

**Repo:** `crates/coap-bridge/src/codec.rs` (`decode`),
`crates/coap-bridge/src/uri.rs` (`parse_coap_uri`).

**Tests:** `crates/coap-bridge/tests/fuzz_smoke.rs::coap_11_1_fuzz_coap_decode_no_panic`,
`crates/coap-bridge/src/uri.rs::tests::coap_6_1_rejects_fragment`.

**Status:** partial — decoder robustness and URI decomposition are
tested; no test checks that the access decision (topic ACL) and serving
use the same decomposition of the URI.

### §11.2 Cache: no delivery to requests with lesser transport security

**Spec:** §11.2, p. 81 (RFC) — "a caching proxy MUST NOT make cached
values available to requests that have lesser transport-security
properties than those the proxy would require to perform request
forwarding in the first place."

**Repo:** —

**Tests:** —

**Status:** open — `CoapCache` (`crates/coap-bridge/src/caching_proxy.rs`)
stores and serves entries without regard to the transport security of
the request.

### §11.2 coaps responses not in a shared cache

**Spec:** §11.2, p. 81 (RFC) — "responses to "coaps" identified requests
are never "public" and thus MUST NOT be reused for shared caching,
unless the cache is able to make equivalent access control decisions to
the ones that led to the cached entry."

**Repo:** —

**Tests:** —

**Status:** open — `CoapCache` does not distinguish responses to `coap`
and `coaps` requests.

### §11.3 No large amplification without authentication

**Spec:** §11.3, p. 82 (RFC) — "large amplification factors SHOULD NOT
be provided in the response if the request is not authenticated."

**Repo:** —

**Tests:** —

**Status:** open — no code found that limits the response size for
unauthenticated requests.

### §11.3 Multicast requests only authenticated and only for specific resources

**Spec:** §11.3, p. 82 (RFC) — "CoAP servers SHOULD NOT accept multicast
requests that can not be authenticated in some way [...]. If possible, a
CoAP server SHOULD limit the support for multicast requests to the
specific resources where the feature is required."

**Repo:** —

**Tests:** —

**Status:** open — the daemon (`crates/coap-bridge/src/daemon/`) does
not distinguish multicast from unicast requests and restricts multicast
neither to authenticated requests nor to resources.

### §11.3 Detect the multicast destination via IPV6_RECVPKTINFO

**Spec:** §11.3, p. 82 (RFC) — "Implementations SHOULD make use of
modern APIs such as IPV6_RECVPKTINFO [RFC3542], if available, to make
this determination."

**Repo:** —

**Tests:** —

**Status:** open — no code in `crates/coap-bridge` reads the destination
address of a received packet.

### §11.4, §11.5 IP spoofing and cross-protocol attacks

**Spec:** §11.4–§11.5, p. 83–86 (RFC) — threat analysis (forged
Reset/ACK/responses, multicast spoofing, DNS replies read as CoAP
messages).

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — description of attacks without a
requirement of its own; the countermeasure named (non-trivial,
randomized token) is §5.3.1.

### §11.6 Constrained-node considerations

**Spec:** §11.6, p. 86 (RFC) — "the node MUST NOT be used for processes
that require good entropy, such as key generation. [...] Special care
must be taken in implementation of cryptographic primitives."

**Repo:** —

**Tests:** —

**Status:** open — the OSCORE cryptography uses `aes`, `ccm`, `hmac` and
`sha2` from RustCrypto; no test checks constant-time behaviour. The
bridge generates no keys.

### §12.1, §12.2, §12.4–§12.7 IANA registrations

**Spec:** §12.1–§12.2, §12.4–§12.7, p. 86–97 (RFC) — instructions to
IANA: code registries, option numbers registry, URI schemes `coap` and
`coaps`, ports 5683 and 5684.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — registration instructions to IANA; the
values themselves are items §5.8, §5.9, §5.10 and §6.

### §12.3 CoAP Content-Formats registry

**Spec:** §12.3, p. 91–93 (RFC) — initial entries: `text/plain;
charset=utf-8` = 0, `application/link-format` = 40, `application/xml` =
41, `application/octet-stream` = 42, `application/exi` = 47,
`application/json` = 50.

**Repo:** —

**Tests:** —

**Status:** open — `crates/coap-bridge/src/daemon/server.rs` sets 40 for
`application/link-format`, without a test; there are no constants for
the other entries.

### §12.8 Multicast address registration

**Spec:** §12.8, p. 97 (RFC) — "All CoAP Nodes": IPv4 `224.0.1.187`,
IPv6 `FF0X::FD`; "CoAP needs the Link-Local and Site-Local scopes only."

**Repo:** `crates/coap-bridge/src/multicast.rs` (`ALL_NODES_LINK_LOCAL_V4`,
`ALL_NODES_LINK_LOCAL_V6`, `ALL_NODES_SITE_LOCAL_V6`).

**Tests:** `crates/coap-bridge/src/multicast.rs::tests::coap_1_1_well_known_addresses_match_spec`.

**Status:** done

### §13 Acknowledgements

**Spec:** §13, p. 97 (RFC) — acknowledgements.

**Repo:** —

**Tests:** —

**Status:** n/a (informative) — acknowledgements.

---

## RFC 7641 §2 Observe option

### §2 Observe option (number 6)

**Spec:** RFC 7641 §2, p. 4 — the `Observe` option (number 6) with values: 0 =
register, 1 = deregister, 2-0xFFFFFF = sequence number.

**Repo:** `crates/coap-bridge/src/option.rs::CoapOption::observe(value)` +
the `numbers::OBSERVE` constant.

**Tests:** `option::tests::coap_5_4_observe_option_uses_number_6`,
`codec::tests::coap_5_10_an_observe_registration_is_the_octets_of_the_layout`.

**Status:** done

### RFC 7641 §3-§5 Notification + state + reordering

**Spec:** RFC 7641 §3.1/§3.4/§3.6, p. 7-14 — registration via `Observe = 0`,
a monotonically increasing sequence number modulo 2^24, and the reordering
rule: a new notification (V2, T2) is fresher than the stored one (V1, T1)
exactly when `(V1 < V2 and V2 - V1 < 2^23)` or `(V1 > V2 and V1 - V2 > 2^23)`
or `T2 > T1 + 128 seconds`; §3.6 deregistration ends the observe
relationship.

**Repo:** `crates/coap-bridge/src/observe.rs::{ObserveRegistry, ObserverEntry}`
with `register`/`deregister`/`observers_of`/`next_seq` for notification
sequencing per path; `crates/coap-bridge/src/observe.rs::{notification_is_fresher,
FRESHNESS_WINDOW_MS}` for the §3.4 reordering rule.

**Tests:** `crates/coap-bridge/src/observe.rs::tests::{coap_3_register_then_observers_lists_one,
coap_3_next_seq_increments_per_observer, coap_3_next_seq_wraps_at_24_bits,
coap_3_notifications_are_ordered_by_sequence_number_per_spec_3_4,
coap_3_wrapped_sequence_number_is_still_recognised_as_fresher,
coap_3_notification_older_than_the_freshness_window_is_always_accepted,
coap_3_deregister_ends_the_observe_relationship_per_spec_3_6}`.

**Status:** done

---

## Audit status

27 done / 5 partial / 10 open / 0 blocked / 4 n/a (informative).

Test run: in CI, crate tier and pillar jobs; the cited tests with the latest result of each job in the pipelines 8501 to 9053 of `main` (`python3 ci/coverage_results.py --document coap-rfc-7252.md`):

* `cargo nextest run --profile merge -p zerodds-coap-bridge --all-targets` (job `merge-rust-coap-bridge`) — 125 tests green.
* `cargo nextest run --profile merge -p zerodds-coap-bridge --all-targets --features dtls,oscore` (job `merge-rust-coap-bridge-features-dtls-oscore`) — 5 tests green.

130 cited tests green, 0 not green.

Open items: `coap-rfc-7252.open.en.md`.
