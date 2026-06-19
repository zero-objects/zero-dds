# RabbitMQ interop base — AMQP 0.9.1 + 1.0

The foundation for the ZeroDDS ↔ RabbitMQ cross-protocol interop e2e suite.

## Why RabbitMQ 4.0

RabbitMQ 4.0 speaks **both AMQP 0.9.1 and AMQP 1.0 natively on the same port
5672** (the `message_containers` feature flag — AMQP 1.0 is a first-class core
protocol in 4.0, no longer the limited 3.x plugin). A single broker therefore
backs both ZeroDDS interop paths:

| Path | ZeroDDS side | Protocol | Reference client |
|------|--------------|----------|------------------|
| **A** | `zerodds-amqp-bridged` (existing 1.0 client) | AMQP 1.0 (OASIS) | `qpid-proton` |
| **B** | new `amqp-0-9-1` stack | AMQP 0.9.1 (class/method) | `pika` |

**The two protocols share only the name** — 0.9.1 is broker-centric
class/method framing; 1.0 is a symmetric link-based protocol with described
types. They are independent implementations that happen to reach the same broker.

## Setup (codepit, Debian 13 LXC)

```sh
bash setup_rabbitmq.sh      # installs rabbitmq-server 4.0 + pika + qpid-proton
python3 validate_base.py    # proves BOTH protocols roundtrip → "BASE OK"
```

- Broker: `localhost:5672` (AMQP 0.9.1 + 1.0)
- Management: `localhost:15672`
- Test identity: `zerodds` / `zerodds` (administrator, full perms on vhost `/`)

## Addressing

- **0.9.1**: classic model — publish to an `exchange` with a `routing_key`;
  consume from a `queue`; queues/exchanges/bindings declared via `*.declare`.
- **1.0 (RabbitMQ 4.0 v2 format)**: link `target`/`source` addresses are
  `/queues/<name>` and `/exchanges/<name>/<routing-key>`. The queue/exchange
  must exist (declared out-of-band, e.g. via 0.9.1 or the management API);
  AMQP 1.0 in RabbitMQ does not declare topology itself.

## Cross-protocol interop

Because both protocols hit the same broker entities, a message published over
one protocol is consumable over the other (e.g. ZeroDDS-1.0 → RabbitMQ queue →
`pika`-0.9.1 consumer, and vice versa) — the basis for the full e2e matrix.
