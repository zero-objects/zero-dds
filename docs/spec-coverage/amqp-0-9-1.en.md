# AMQP 0.9.1 — Spec Coverage

**Spec:** [AMQP 0-9-1 — Protocol Specification →](https://www.rabbitmq.com/resources/specs/amqp0-9-1.pdf) (AMQP WG, RabbitMQ-hosted) + the AMQP 0-9-1 Complete Reference Guide.

**Context:** `crates/amqp-0-9-1` implements AMQP **0.9.1** as a standalone
protocol (class/method framing, typed field tables, broker model) — separate
from AMQP 1.0 (`crates/amqp-bridge` + `crates/amqp-endpoint`). It covers every
class a complete producer/consumer needs: connection, channel, exchange, queue,
basic, confirm (publisher confirms) and tx (transactions). Each item is backed
by a repo path and a test; interop is proven live against RabbitMQ 4.0 plus
cross-protocol against qpid-proton (AMQP 1.0).

Implementation:

- `crates/amqp-0-9-1/` — class/method framing, typed field tables,
  content properties, broker model; 25 tests green (live against RabbitMQ 4.0).

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

**Tests:** `types::tests::{integer_roundtrip_big_endian,
shortstr_longstr_roundtrip, shortstr_rejects_over_255, field_table_strs_then_skip,
bit_packing, field_table_typed_roundtrip, unknown_field_type_rejected}`.

**Status:** done

## §4.2.2 Frame format

### §4.2.2 Frame (type/channel/size/payload/0xCE) + protocol header

**Spec:** §4.2.2 — protocol header `AMQP\x00\x00\x09\x01`; each frame is
`[type:octet][channel:short][size:long][payload][frame-end:0xCE]`; frame types
METHOD(1)/HEADER(2)/BODY(3)/HEARTBEAT(8).

**Repo:** `crates/amqp-0-9-1/src/frame.rs::{Frame, FrameType,
PROTOCOL_HEADER, FRAME_END}` — `encode`/`decode` (with truncation detection +
frame-end validation).

**Tests:** `frame::tests::{frame_roundtrips, decode_reports_truncation,
protocol_header_is_0_0_9_1}`.

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

**Tests:** `method::tests::{start_ok_carries_plain_response,
tune_ok_roundtrips_params}`; live `tests/rabbitmq_amqp091_e2e.rs::
zerodds_amqp091_self_roundtrip`.

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
channel_flow_ok, channel_close, channel_close_ok}` + `client.rs::Amqp091Client::
{channel_flow, channel_close}` (connect opens channel 1).

**Tests:** `method::tests::channel_close_and_flow_ids`; live
`tests/rabbitmq_amqp091_e2e.rs::zerodds_amqp091_channel_flow_and_close`.

**Status:** done

## §1.6 exchange class

### §1.6 exchange.declare/declare-ok + delete/delete-ok

**Spec:** §1.6 — `exchange.declare` (type direct/fanout/topic/headers, durable,
…); `exchange.delete`.

**Repo:** `crates/amqp-0-9-1/src/method.rs::{exchange_declare, exchange_delete}`
+ `client.rs::Amqp091Client::{exchange_declare, exchange_delete}`.

**Tests:** `method::tests::exchange_declare_carries_type`; live
`tests/rabbitmq_amqp091_e2e.rs::zerodds_amqp091_exchange_routing` (topic
exchange + routing into a bound queue).

**Status:** done

## §1.7 queue class

### §1.7 queue.declare/bind/unbind/purge/delete

**Spec:** §1.7 — `queue.declare`/`declare-ok`; `queue.bind`/`unbind` (exchange
bindings); `queue.purge` (+message-count); `queue.delete` (+message-count).

**Repo:** `crates/amqp-0-9-1/src/method.rs::{queue_declare, queue_bind,
queue_unbind, queue_purge, queue_delete, queue_declare_ok_name,
queue_op_ok_message_count}` + `client.rs::Amqp091Client::{queue_declare,
queue_bind, queue_unbind, queue_purge, queue_delete}`.

**Tests:** `method::tests::{queue_declare_and_publish_ids,
queue_bind_unbind_purge_delete_ids, queue_op_ok_count_parsed}`; live
`tests/rabbitmq_amqp091_e2e.rs::zerodds_amqp091_exchange_routing`.

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
ContentProperties}` + `client.rs::Amqp091Client::{publish, publish_with_props}`.

**Tests:** `method::tests::{content_header_roundtrip, content_properties_roundtrip,
content_header_no_props_matches_legacy}`; live `tests/rabbitmq_amqp091_e2e.rs::
{zerodds_amqp091_self_roundtrip, zerodds_amqp091_publish_consumed_by_proton_10,
zerodds_amqp091_content_properties}` (persistent delivery-mode + typed headers
round-trip through RabbitMQ).

**Status:** done

### §1.8 basic.get/get-ok/get-empty + basic.ack + basic.reject/nack

**Spec:** §1.8 — synchronous pull: `basic.get` → `get-ok` (delivery-tag, …) +
content, or `get-empty`; `basic.ack`; `basic.reject` (+requeue); `basic.nack`
(RabbitMQ, +multiple/requeue).

**Repo:** `crates/amqp-0-9-1/src/method.rs::{basic_get,
basic_get_ok_delivery_tag, basic_ack, basic_ack_delivery_tag, basic_reject,
basic_nack}` + `client.rs::Amqp091Client::{get, get_with_props, get_reject}`.

**Tests:** `method::tests::{basic_consume_cancel_qos_ids, ack_delivery_tag_parsed}`;
live `tests/rabbitmq_amqp091_e2e.rs::{zerodds_amqp091_consumes_pika_published,
zerodds_amqp091_reject_requeue}` + cross-stack
`amqp-endpoint/tests/rabbitmq_cross_stack_e2e.rs`.

**Status:** done

### §1.8 basic.qos + async consume/deliver/cancel

**Spec:** §1.8 — `basic.qos` (prefetch window); `basic.consume`/`consume-ok`
(subscription) → server-pushed `basic.deliver` + content → `basic.ack`;
`basic.cancel`/`cancel-ok`.

**Repo:** `crates/amqp-0-9-1/src/method.rs::{basic_qos, basic_consume,
basic_cancel, basic_consume_ok_tag, basic_deliver_delivery_tag}` +
`client.rs::Amqp091Client::{qos, consume_one}`.

**Tests:** `method::tests::{basic_consume_cancel_qos_ids,
basic_deliver_tag_skips_consumer_tag, consume_ok_tag_parsed}`; live
`tests/rabbitmq_amqp091_e2e.rs::zerodds_amqp091_async_consume`.

**Status:** done

## confirm class (RabbitMQ extension, class 85)

### confirm.select/select-ok + publisher confirms

**Spec:** RabbitMQ `confirm.select` — put the channel into confirm mode; each
published message is confirmed by the broker with a server-pushed `basic.ack`
(delivery-tag).

**Repo:** `crates/amqp-0-9-1/src/method.rs::{confirm_select,
basic_ack_delivery_tag}` + `client.rs::Amqp091Client::{confirm_select,
publish_confirmed}`.

**Tests:** `method::tests::confirm_and_tx_ids`; live
`tests/rabbitmq_amqp091_e2e.rs::zerodds_amqp091_publisher_confirms`.

**Status:** done

## §1.9 tx class

### §1.9 tx.select/commit/rollback

**Spec:** §1.9 — `tx.select` (start a transaction), `tx.commit`, `tx.rollback`.

**Repo:** `crates/amqp-0-9-1/src/method.rs::{tx_select, tx_commit, tx_rollback}`
+ `client.rs::Amqp091Client::{tx_select, tx_commit, tx_rollback}`.

**Tests:** `method::tests::confirm_and_tx_ids`; live
`tests/rabbitmq_amqp091_e2e.rs::zerodds_amqp091_transactions` (rollback does not
deliver, commit does).

**Status:** done

---

## Audit status

11 done / 0 partial / 0 open / 0 n/a (informative) / 0 n/a (rejected).

Test run: `cargo test -p zerodds-amqp-0-9-1 --lib` — 25 tests green, 0 failed.
RabbitMQ interop additionally live (Linux bench host, `AMQP_RABBITMQ=1`):
`cargo test -p zerodds-amqp-0-9-1 --test rabbitmq_amqp091_e2e -- --ignored` —
10 tests green; cross-stack `cargo test -p zerodds-amqp-endpoint --test
rabbitmq_cross_stack_e2e -- --ignored` — 2 tests green.
