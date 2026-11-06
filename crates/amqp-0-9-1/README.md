# `zerodds-amqp-0-9-1`

AMQP **0.9.1** — the classic, broker-centric class/method protocol RabbitMQ
speaks by default and the >80%-deployed AMQP dialect. A completely separate
protocol from AMQP 1.0 (`zerodds-amqp-bridge`): different framing, a field-table
type system, and a broker model in the wire.

- `types` — big-endian wire types + field tables (§4.2)
- `frame` — `type/channel/size/payload/0xCE` frames (§4.2.2)
- `method` — class/method framing (connection/channel/queue/basic)
- `client` — synchronous broker client: handshake, queue.declare,
  basic.publish, basic.get/ack (`std`)

## Tests

`cargo test -p zerodds-amqp-0-9-1` checks the frames, the field tables and
the methods against the octets of the protocol's XML definitions
(`amqp0-9-1.xml`, and RabbitMQ's `amqp0-9-1.extended.xml` for `basic.nack` and
the `confirm` class) and RabbitMQ's errata for the field types.

`tests/rabbitmq_amqp091_e2e.rs` runs the client against RabbitMQ, a node of
each test alone (`tests/support/rabbitmq.rs`), with pika and qpid-proton as the
reference clients. Those tests are `#[ignore]` in the crate tier and run in
the integration pillar's job `integration-bridges-vendor-amqp`:

```bash
cargo nextest run -p zerodds-amqp-0-9-1 --test rabbitmq_amqp091_e2e --run-ignored all
```
