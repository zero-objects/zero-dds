# AMQP 0.9.1 — Spec-Coverage

**Spec:** [AMQP 0-9-1 — Protocol Specification →](https://www.rabbitmq.com/resources/specs/amqp0-9-1.pdf) (AMQP WG, RabbitMQ-gehostet) + AMQP 0-9-1 Complete Reference Guide.

**Scope-Hinweis:** `crates/amqp-0-9-1` implementiert AMQP **0.9.1** als
eigenständiges Protokoll (class/method-Framing, typisierte Field-Tables,
Broker-Modell) — getrennt von AMQP 1.0 (`crates/amqp-bridge` +
`crates/amqp-endpoint`). Abgedeckt sind alle Klassen, die ein vollständiger
Producer/Consumer braucht: connection, channel, exchange, queue, basic, confirm
(Publisher-Confirms) und tx (Transaktionen). Jedes Item ist mit Repo-Pfad und
Test belegt; die Interop ist live gegen RabbitMQ 4.0 + Cross-Protocol gegen
qpid-proton (AMQP 1.0) bewiesen.

Implementation:

- `crates/amqp-0-9-1/` — class/method-Framing, typisierte Field-Tables,
  Content-Properties, Broker-Modell; 25 Tests grün (live gegen RabbitMQ 4.0).

---

## §4.2 Wire-Typen

### §4.2.5 Basis-Typen + typisierte Field-Tables

**Spec:** §4.2.5 — Integer big-endian; `shortstr` = 1-Byte-Länge + Bytes
(max 255); `longstr` = 4-Byte-Länge + Bytes; `bit` gepackt in Octets;
`field-table` (§4.2.5.5) = longstr-gerahmte `(shortstr name, typed value)`-Liste
mit Werttypen `t/b/B/s/u/I/i/l/f/d/D/S/T/F/A/x/V`.

**Repo:** `crates/amqp-0-9-1/src/types.rs::{Reader, Writer}` — u8/u16/u32/u64
+ signed (`i8`/`i16`/`i32`/`i64`) + float (`f32`/`f64`) big-endian, `short_str`
(255-Enforcement), `long_str`, `pack_bits`. Voller Field-Table-Codec:
`FieldValue`-Enum (alle Werttypen inkl. nested `Table` + `Array` + `Decimal` +
`Timestamp`), `Writer::{field_value, field_table}` (encode),
`Reader::{field_value, field_table}` (decode). `field_table_strs` bleibt als
S-only-Schnellpfad für `client-properties` in start-ok.

**Tests:** `types::tests::{integer_roundtrip_big_endian,
shortstr_longstr_roundtrip, shortstr_rejects_over_255, field_table_strs_then_skip,
bit_packing, field_table_typed_roundtrip, unknown_field_type_rejected}`.

**Status:** done

## §4.2.2 Frame-Format

### §4.2.2 Frame (type/channel/size/payload/0xCE) + Protokoll-Header

**Spec:** §4.2.2 — Protokoll-Header `AMQP\x00\x00\x09\x01`; jedes Frame
`[type:octet][channel:short][size:long][payload][frame-end:0xCE]`; Frame-Typen
METHOD(1)/HEADER(2)/BODY(3)/HEARTBEAT(8).

**Repo:** `crates/amqp-0-9-1/src/frame.rs::{Frame, FrameType,
PROTOCOL_HEADER, FRAME_END}` — `encode`/`decode` (mit Truncation-Erkennung +
frame-end-Validierung).

**Tests:** `frame::tests::{frame_roundtrips, decode_reports_truncation,
protocol_header_is_0_0_9_1}`.

**Status:** done

## §1.4 connection-Klasse

### §1.4 connection (start/start-ok/tune/tune-ok/open/open-ok/close)

**Spec:** §1.4 + §4.2.7 — Handshake: Server `connection.start` → Client
`start-ok` (client-properties, Mechanismus, Response, Locale) → Server `tune`
→ Client `tune-ok` (channel-max/frame-max/heartbeat ausgehandelt) → Client
`open` (vhost) → Server `open-ok`; `close`/`close-ok`.

**Repo:** `crates/amqp-0-9-1/src/method.rs::{connection_start_ok,
connection_tune_ok, connection_open, connection_tune_params}` +
`crates/amqp-0-9-1/src/client.rs::Amqp091Client::{connect, close}` (treibt den
vollen Handshake mit PLAIN-Auth).

**Tests:** `method::tests::{start_ok_carries_plain_response,
tune_ok_roundtrips_params}`; Live `tests/rabbitmq_amqp091_e2e.rs::
zerodds_amqp091_self_roundtrip`.

**Status:** done

> **Heartbeat-Aushandlung** (§4.2.7): `tune-ok` handelt die Werte aus und der
> synchrone Client wählt `heartbeat=0` (deaktiviert). Das ist spec-konform —
> §4.2.7 erlaubt 0 explizit, und ein blockierender Client annonciert korrekt
> kein Heartbeat-Intervall, das er nicht bedienen würde.

## §1.5 channel-Klasse

### §1.5 channel.open/open-ok + flow/flow-ok + close/close-ok

**Spec:** §1.5 — `channel.open`/`open-ok`; `channel.flow`/`flow-ok`
(Producer-Drosselung); `channel.close`/`close-ok` (Kanal schließen, Connection
bleibt offen).

**Repo:** `crates/amqp-0-9-1/src/method.rs::{channel_open, channel_flow,
channel_flow_ok, channel_close, channel_close_ok}` + `client.rs::Amqp091Client::
{channel_flow, channel_close}` (connect öffnet Kanal 1).

**Tests:** `method::tests::channel_close_and_flow_ids`; Live
`tests/rabbitmq_amqp091_e2e.rs::zerodds_amqp091_channel_flow_and_close`.

**Status:** done

## §1.6 exchange-Klasse

### §1.6 exchange.declare/declare-ok + delete/delete-ok

**Spec:** §1.6 — `exchange.declare` (Typ direct/fanout/topic/headers, durable,
…); `exchange.delete`.

**Repo:** `crates/amqp-0-9-1/src/method.rs::{exchange_declare, exchange_delete}`
+ `client.rs::Amqp091Client::{exchange_declare, exchange_delete}`.

**Tests:** `method::tests::exchange_declare_carries_type`; Live
`tests/rabbitmq_amqp091_e2e.rs::zerodds_amqp091_exchange_routing` (topic-
Exchange + Routing in eine gebundene Queue).

**Status:** done

## §1.7 queue-Klasse

### §1.7 queue.declare/bind/unbind/purge/delete

**Spec:** §1.7 — `queue.declare`/`declare-ok`; `queue.bind`/`unbind`
(Exchange-Bindings); `queue.purge` (+message-count); `queue.delete`
(+message-count).

**Repo:** `crates/amqp-0-9-1/src/method.rs::{queue_declare, queue_bind,
queue_unbind, queue_purge, queue_delete, queue_declare_ok_name,
queue_op_ok_message_count}` + `client.rs::Amqp091Client::{queue_declare,
queue_bind, queue_unbind, queue_purge, queue_delete}`.

**Tests:** `method::tests::{queue_declare_and_publish_ids,
queue_bind_unbind_purge_delete_ids, queue_op_ok_count_parsed}`; Live
`tests/rabbitmq_amqp091_e2e.rs::zerodds_amqp091_exchange_routing`.

**Status:** done

## §1.8 basic-Klasse

### §1.8 basic.publish + Content (Header/Body) + Content-Properties

**Spec:** §1.8 + §4.2.6 + §1.8.1 — `basic.publish` gefolgt von Content-Header-
Frame (class-id, weight, body-size, property-flags + Properties) + Body-Frame(s).
Properties: content-type/-encoding, headers (Field-Table), delivery-mode,
priority, correlation-id, reply-to, expiration, message-id, timestamp, type,
user-id, app-id.

**Repo:** `crates/amqp-0-9-1/src/method.rs::{basic_publish, content_header,
content_header_with_props, content_header_body_size, parse_content_header,
ContentProperties}` + `client.rs::Amqp091Client::{publish, publish_with_props}`.

**Tests:** `method::tests::{content_header_roundtrip, content_properties_roundtrip,
content_header_no_props_matches_legacy}`; Live `tests/rabbitmq_amqp091_e2e.rs::
{zerodds_amqp091_self_roundtrip, zerodds_amqp091_publish_consumed_by_proton_10,
zerodds_amqp091_content_properties}` (persistent delivery-mode + typed headers
round-trip durch RabbitMQ).

**Status:** done

### §1.8 basic.get/get-ok/get-empty + basic.ack + basic.reject/nack

**Spec:** §1.8 — synchroner Pull: `basic.get` → `get-ok` (delivery-tag, …) +
Content, oder `get-empty`; `basic.ack`; `basic.reject` (+requeue); `basic.nack`
(RabbitMQ, +multiple/requeue).

**Repo:** `crates/amqp-0-9-1/src/method.rs::{basic_get,
basic_get_ok_delivery_tag, basic_ack, basic_ack_delivery_tag, basic_reject,
basic_nack}` + `client.rs::Amqp091Client::{get, get_with_props, get_reject}`.

**Tests:** `method::tests::{basic_consume_cancel_qos_ids, ack_delivery_tag_parsed}`;
Live `tests/rabbitmq_amqp091_e2e.rs::{zerodds_amqp091_consumes_pika_published,
zerodds_amqp091_reject_requeue}` + Cross-Stack
`amqp-endpoint/tests/rabbitmq_cross_stack_e2e.rs`.

**Status:** done

### §1.8 basic.qos + asynchroner consume/deliver/cancel

**Spec:** §1.8 — `basic.qos` (Prefetch-Fenster); `basic.consume`/`consume-ok`
(Subscription) → server-pushed `basic.deliver` + Content → `basic.ack`;
`basic.cancel`/`cancel-ok`.

**Repo:** `crates/amqp-0-9-1/src/method.rs::{basic_qos, basic_consume,
basic_cancel, basic_consume_ok_tag, basic_deliver_delivery_tag}` +
`client.rs::Amqp091Client::{qos, consume_one}`.

**Tests:** `method::tests::{basic_consume_cancel_qos_ids,
basic_deliver_tag_skips_consumer_tag, consume_ok_tag_parsed}`; Live
`tests/rabbitmq_amqp091_e2e.rs::zerodds_amqp091_async_consume`.

**Status:** done

## confirm-Klasse (RabbitMQ-Extension, Klasse 85)

### confirm.select/select-ok + Publisher-Confirms

**Spec:** RabbitMQ-`confirm.select` — Kanal in Confirm-Modus versetzen; jede
publizierte Nachricht wird vom Broker mit server-pushed `basic.ack`
(delivery-tag) bestätigt.

**Repo:** `crates/amqp-0-9-1/src/method.rs::{confirm_select,
basic_ack_delivery_tag}` + `client.rs::Amqp091Client::{confirm_select,
publish_confirmed}`.

**Tests:** `method::tests::confirm_and_tx_ids`; Live
`tests/rabbitmq_amqp091_e2e.rs::zerodds_amqp091_publisher_confirms`.

**Status:** done

## §1.9 tx-Klasse

### §1.9 tx.select/commit/rollback

**Spec:** §1.9 — `tx.select` (Transaktion starten), `tx.commit`, `tx.rollback`.

**Repo:** `crates/amqp-0-9-1/src/method.rs::{tx_select, tx_commit, tx_rollback}`
+ `client.rs::Amqp091Client::{tx_select, tx_commit, tx_rollback}`.

**Tests:** `method::tests::confirm_and_tx_ids`; Live
`tests/rabbitmq_amqp091_e2e.rs::zerodds_amqp091_transactions` (rollback liefert
nicht, commit liefert).

**Status:** done

---

## Audit-Status

11 done / 0 partial / 0 open / 0 n/a (informative) / 0 n/a (rejected).

Test-Lauf: `cargo test -p zerodds-amqp-0-9-1 --lib` — 25 Tests grün, 0 failed.
RabbitMQ-Interop zusätzlich live (Linux bench host, `AMQP_RABBITMQ=1`):
`cargo test -p zerodds-amqp-0-9-1 --test rabbitmq_amqp091_e2e -- --ignored` —
10 Tests grün; Cross-Stack `cargo test -p zerodds-amqp-endpoint --test
rabbitmq_cross_stack_e2e -- --ignored` — 2 Tests grün.
