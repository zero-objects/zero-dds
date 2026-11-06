# AMQP 0.9.1 — Spec Coverage

**Spec:** [AMQP 0-9-1 — Protocol Specification →](https://www.rabbitmq.com/resources/specs/amqp0-9-1.pdf) (AMQP WG, RabbitMQ-hosted) + the AMQP 0-9-1 Complete Reference Guide.

**Context:** `crates/amqp-0-9-1` implements AMQP **0.9.1** as a standalone
protocol (class/method framing, typed field tables, broker model) — separate
from AMQP 1.0 (`crates/amqp-bridge` + `crates/amqp-endpoint`). It covers every
class a complete producer/consumer needs: connection, channel, exchange, queue,
basic, confirm (publisher confirms) and tx (transactions). Each item is backed
by a repo path and a test: the octets of the methods against the protocol's
XML definitions (`amqp0-9-1.xml`, RabbitMQ's `amqp0-9-1.extended.xml` for
`basic.nack` and `confirm`), the client against RabbitMQ with pika and
qpid-proton (AMQP 1.0) on the other side.

Implementation:

- `crates/amqp-0-9-1/` — class/method framing, typed field tables,
  content properties, broker model, synchronous client.

---

## §4.2 Wire types

### §4.2.5 Base types + typed field tables

**Spec:** §4.2.5 — integers big-endian; `shortstr` = 1-byte length + bytes
(max 255); `longstr` = 4-byte length + bytes; `bit` packed into octets;
`field-table` (§4.2.5.5) = a longstr-framed list of `(shortstr name, typed
value)` pairs with value types `t/b/B/s/u/I/i/l/f/d/D/S/T/F/A/x/V`.

**Repo:** `crates/amqp-0-9-1/src/types.rs::{Reader, Writer}` — u8/u16/u32/u64
+ signed (`i8`/`i16`/`i32`/`i64`) + float (`f32`/`f64`) big-endian, `short_str`
(255 enforcement), `long_str`, `pack_bits`. Full field-table codec: the
`FieldValue` enum (all value types incl. nested `Table` + `Array` + `Decimal`
+ `Timestamp`), `Writer::{field_value, field_table}` (encode),
`Reader::{field_value, field_table}` (decode). `field_table_strs` remains an
S-only fast path for the `client-properties` advertised in start-ok.

**Tests:** `crates/amqp-0-9-1/src/types.rs::tests::{amqp091_4_2_5_1_integers_are_in_network_byte_order, amqp091_4_2_5_2_bits_pack_from_the_low_bit, amqp091_4_2_5_3_a_short_string_has_one_length_octet_a_long_string_four, amqp091_4_2_5_3_a_short_string_over_255_octets_is_refused, amqp091_4_2_5_5_a_field_table_is_its_length_then_name_type_value, amqp091_4_2_5_5_the_field_types_are_the_tags_rabbitmq_reads, amqp091_4_2_5_5_an_unknown_field_type_is_refused}` (expected octets from §4.2.5 and RabbitMQ's errata, "Field types").

**Status:** done

## §4.2.2 Frame format

### §4.2.2 Frame (type/channel/size/payload/0xCE) + protocol header

**Spec:** §4.2.2 — protocol header `AMQP\x00\x00\x09\x01`; each frame is
`[type:octet][channel:short][size:long][payload][frame-end:0xCE]`; frame types
METHOD(1)/HEADER(2)/BODY(3)/HEARTBEAT(8).

**Repo:** `crates/amqp-0-9-1/src/frame.rs::{Frame, FrameType,
PROTOCOL_HEADER, FRAME_END}` — `encode`/`decode` (with truncation detection +
frame-end validation).

**Tests:** `crates/amqp-0-9-1/src/frame.rs::tests::{amqp091_4_2_2_the_protocol_header_is_amqp_0_0_9_1, amqp091_4_2_3_a_method_frame_is_type_channel_size_payload_and_frame_end, amqp091_4_2_3_the_frame_types_are_one_two_three_and_eight, amqp091_4_2_3_a_frame_cut_short_or_without_frame_end_is_refused}`.

**Status:** done

## §1.4 connection class

### §1.4 connection (start/start-ok/tune/tune-ok/open/open-ok/close)

**Spec:** §1.4 + §4.2.7 — handshake: server `connection.start` → client
`start-ok` (client-properties, mechanism, response, locale) → server `tune` →
client `tune-ok` (channel-max/frame-max/heartbeat negotiated) → client `open`
(vhost) → server `open-ok`; `close`/`close-ok`.

**Repo:** `crates/amqp-0-9-1/src/method.rs::{connection_start_ok,
connection_tune_ok, connection_open, connection_tune_params}` +
`crates/amqp-0-9-1/src/client.rs::Amqp091Client::{connect, close}` (drives the
full handshake with PLAIN auth).

**Tests:** `crates/amqp-0-9-1/src/method.rs::tests::{amqp091_1_4_start_ok_is_properties_mechanism_plain_response_and_locale, amqp091_1_4_tune_and_tune_ok_carry_channel_max_frame_max_heartbeat, amqp091_1_4_open_is_the_virtual_host_and_two_reserved_fields, amqp091_1_4_a_method_payload_begins_with_class_and_method}` (expected octets from `amqp0-9-1.xml`); RabbitMQ: `crates/amqp-0-9-1/tests/rabbitmq_amqp091_e2e.rs::amqp091_1_4_the_client_handshakes_declares_publishes_and_gets`.

**Status:** done

> **Heartbeat negotiation** (§4.2.7): `tune-ok` negotiates the values and the
> synchronous client selects `heartbeat=0` (disabled). This is spec-compliant —
> §4.2.7 explicitly allows 0, and a blocking client correctly advertises no
> heartbeat interval it would not service.

## §1.5 channel class

### §1.5 channel.open/open-ok + flow/flow-ok + close/close-ok

**Spec:** §1.5 — `channel.open`/`open-ok`; `channel.flow`/`flow-ok` (producer
throttling); `channel.close`/`close-ok` (close the channel, the connection
stays open).

**Repo:** `crates/amqp-0-9-1/src/method.rs::{channel_open, channel_flow,
channel_flow_ok, channel_close, channel_close_ok}` + `crates/amqp-0-9-1/src/client.rs::Amqp091Client::
{channel_flow, channel_close}` (connect opens channel 1).

**Tests:** `crates/amqp-0-9-1/src/method.rs::tests::amqp091_1_5_channel_open_flow_and_close_are_their_fields`; RabbitMQ: `crates/amqp-0-9-1/tests/rabbitmq_amqp091_e2e.rs::amqp091_1_5_flow_and_close_are_answered_and_the_connection_stays`.

**Status:** done

## §1.6 exchange class

### §1.6 exchange.declare/declare-ok + delete/delete-ok

**Spec:** §1.6 — `exchange.declare` (type direct/fanout/topic/headers, durable,
…); `exchange.delete`.

**Repo:** `crates/amqp-0-9-1/src/method.rs::{exchange_declare, exchange_delete}`
+ `crates/amqp-0-9-1/src/client.rs::Amqp091Client::{exchange_declare, exchange_delete}`.

**Tests:** `crates/amqp-0-9-1/src/method.rs::tests::amqp091_1_6_exchange_declare_and_delete_are_their_fields`; RabbitMQ: `crates/amqp-0-9-1/tests/rabbitmq_amqp091_e2e.rs::amqp091_1_6_a_topic_exchange_routes_to_the_bound_queue`.

**Status:** done

## §1.7 queue class

### §1.7 queue.declare/bind/unbind/purge/delete

**Spec:** §1.7 — `queue.declare`/`declare-ok`; `queue.bind`/`unbind` (exchange
bindings); `queue.purge` (+message-count); `queue.delete` (+message-count).

**Repo:** `crates/amqp-0-9-1/src/method.rs::{queue_declare, queue_bind,
queue_unbind, queue_purge, queue_delete, queue_declare_ok_name,
queue_op_ok_message_count}` + `crates/amqp-0-9-1/src/client.rs::Amqp091Client::{queue_declare,
queue_bind, queue_unbind, queue_purge, queue_delete}`.

**Tests:** `crates/amqp-0-9-1/src/method.rs::tests::{amqp091_1_7_queue_declare_bind_purge_delete_unbind_are_their_fields, amqp091_1_7_declare_ok_names_the_queue_purge_ok_and_delete_ok_count}`; RabbitMQ: `crates/amqp-0-9-1/tests/rabbitmq_amqp091_e2e.rs::amqp091_1_6_a_topic_exchange_routes_to_the_bound_queue`.

**Status:** done

## §1.8 basic class

### §1.8 basic.publish + content (header/body) + content properties

**Spec:** §1.8 + §4.2.6 + §1.8.1 — `basic.publish` followed by a content header
frame (class-id, weight, body-size, property-flags + properties) + body frame(s).
Properties: content-type/-encoding, headers (field table), delivery-mode,
priority, correlation-id, reply-to, expiration, message-id, timestamp, type,
user-id, app-id.

**Repo:** `crates/amqp-0-9-1/src/method.rs::{basic_publish, content_header,
content_header_with_props, content_header_body_size, parse_content_header,
ContentProperties}` + `crates/amqp-0-9-1/src/client.rs::Amqp091Client::{publish, publish_with_props}`.

**Tests:** `crates/amqp-0-9-1/src/method.rs::tests::{amqp091_1_8_publish_is_its_fields, amqp091_4_2_6_1_a_content_header_without_properties_has_flags_zero, amqp091_4_2_6_1_the_properties_follow_their_flag_bits_in_order}`; RabbitMQ: `crates/amqp-0-9-1/tests/rabbitmq_amqp091_e2e.rs::{amqp091_1_4_the_client_handshakes_declares_publishes_and_gets, amqp091_1_8_a_published_message_is_read_by_an_amqp_1_0_client, amqp091_4_2_6_1_pika_and_the_client_read_each_others_properties}`.

**Status:** done

### §1.8 basic.get/get-ok/get-empty + basic.ack + basic.reject/nack

**Spec:** §1.8 — synchronous pull: `basic.get` → `get-ok` (delivery-tag, …) +
content, or `get-empty`; `basic.ack`; `basic.reject` (+requeue); `basic.nack`
(RabbitMQ, +multiple/requeue).

**Repo:** `crates/amqp-0-9-1/src/method.rs::{basic_get,
basic_get_ok_delivery_tag, basic_ack, basic_ack_delivery_tag, basic_reject,
basic_nack}` + `crates/amqp-0-9-1/src/client.rs::Amqp091Client::{get, get_with_props, get_reject}`.

**Tests:** `crates/amqp-0-9-1/src/method.rs::tests::{amqp091_1_8_get_ack_reject_nack_are_their_fields, amqp091_1_8_get_ok_and_ack_begin_with_the_delivery_tag}`; RabbitMQ: `crates/amqp-0-9-1/tests/rabbitmq_amqp091_e2e.rs::{amqp091_1_8_get_reads_what_pika_published, amqp091_1_8_a_message_rejected_with_requeue_is_delivered_again}`.

**Status:** done

### §1.8 basic.qos + async consume/deliver/cancel

**Spec:** §1.8 — `basic.qos` (prefetch window); `basic.consume`/`consume-ok`
(subscription) → server-pushed `basic.deliver` + content → `basic.ack`;
`basic.cancel`/`cancel-ok`.

**Repo:** `crates/amqp-0-9-1/src/method.rs::{basic_qos, basic_consume,
basic_cancel, basic_consume_ok_tag, basic_deliver_delivery_tag}` +
`crates/amqp-0-9-1/src/client.rs::Amqp091Client::{qos, consume_one}`.

**Tests:** `crates/amqp-0-9-1/src/method.rs::tests::{amqp091_1_8_qos_consume_and_cancel_are_their_fields, amqp091_1_8_consume_ok_and_deliver_carry_their_tags}`; RabbitMQ: `crates/amqp-0-9-1/tests/rabbitmq_amqp091_e2e.rs::amqp091_1_8_consume_delivers_asynchronously`.

**Status:** done

## confirm class (RabbitMQ extension, class 85)

### confirm.select/select-ok + publisher confirms

**Spec:** RabbitMQ `confirm.select` — put the channel into confirm mode; each
published message is confirmed by the broker with a server-pushed `basic.ack`
(delivery-tag).

**Repo:** `crates/amqp-0-9-1/src/method.rs::{confirm_select,
basic_ack_delivery_tag}` + `crates/amqp-0-9-1/src/client.rs::Amqp091Client::{confirm_select,
publish_confirmed}`.

**Tests:** `crates/amqp-0-9-1/src/method.rs::tests::amqp091_confirm_select_is_class_85_method_10_with_no_wait` (`amqp0-9-1.extended.xml`); RabbitMQ: `crates/amqp-0-9-1/tests/rabbitmq_amqp091_e2e.rs::amqp091_confirm_the_broker_acks_a_publish_after_select`.

**Status:** done

## §1.9 tx class

### §1.9 tx.select/commit/rollback

**Spec:** §1.9 — `tx.select` (start a transaction), `tx.commit`, `tx.rollback`.

**Repo:** `crates/amqp-0-9-1/src/method.rs::{tx_select, tx_commit, tx_rollback}`
+ `crates/amqp-0-9-1/src/client.rs::Amqp091Client::{tx_select, tx_commit, tx_rollback}`.

**Tests:** `crates/amqp-0-9-1/src/method.rs::tests::amqp091_1_9_tx_select_commit_and_rollback_have_no_fields`; RabbitMQ: `crates/amqp-0-9-1/tests/rabbitmq_amqp091_e2e.rs::amqp091_1_9_a_rolled_back_publish_is_not_queued_a_committed_one_is`.

**Status:** done

---

## Audit status

11 done / 0 partial / 0 open / 0 blocked / 0 n/a (informative).

Test run: in CI, crate tier and pillar jobs; the cited tests with the latest result of each job in the pipelines 8501 to 9053 of `main` (`python3 ci/coverage_results.py --document amqp-0-9-1.md`):

* `cargo nextest run --profile merge -p zerodds-amqp-0-9-1 --all-targets` (job `merge-rust-amqp-0-9-1`) — 28 tests green.
* `python3 ci/run_integration.py integration-bridges-vendor-amqp` (job `integration-bridges-vendor-amqp`) — 10 tests green.

38 cited tests green, 0 not green.

