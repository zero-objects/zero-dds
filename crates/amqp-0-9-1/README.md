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

Interop-tested against RabbitMQ 4.0 (see `../amqp-endpoint/competitors/rabbitmq`).
