# Track RC2-B — AMQP 0.9.1 / RabbitMQ-native Bridge

**Goal:** ZeroDDS bekommt einen zweiten Bridge-Daemon, der nativ
AMQP 0.9.1 spricht (RabbitMQ-Default-Wire), zusätzlich zum existierenden
AMQP 1.0 Daemon.

**Status:** 📋 todo

**Estimate:** 1-2 Personenwochen.

## Motivation

OASIS AMQP 1.0 ist die ISO-standardisierte Spec, **aber die installed
base bei RabbitMQ läuft auf 0.9.1**. RabbitMQ unterstützt 1.0 nur als
optionales Plugin, default ist 0.9.1. Wer "RabbitMQ-Bridge" sagt, meint
in 95 % der Fälle 0.9.1.

AMQP 0.9.1 ist eine **andere Wire-Spec** — nicht backward-kompatibel zu
1.0:
- Frame-Format anders (method/header/body Frames vs. AMQP 1.0
  performatives)
- Channels statt Sessions/Links
- Exchange/Queue-Modell statt Source/Target
- Class-Method-Hierarchie (basic.publish, queue.declare, etc.)

## In-Scope

### Crates (neu)

- `crates/amqp09-codec` — AMQP 0.9.1 Frame-Codec (decode/encode der
  9 Class-Methods: connection, channel, exchange, queue, basic, tx,
  confirm, access, file)
- `crates/amqp09-bridge` — Bridge-Daemon `zerodds-amqp09-bridged` der
  DDS ↔ AMQP 0.9.1 maps:
  - DDS Topic ↔ AMQP Exchange (default-mapping per type)
  - DDS Sample ↔ AMQP basic.publish content-frame
  - DDS QoS Reliable ↔ AMQP publisher-confirm
  - DDS QoS Persistent ↔ AMQP delivery-mode 2

### Vendor-Spec

`docs/specs/zerodds-amqp09-bridge-1.0.md` mit Conformance-Profilen:
- L1: Wire-codec (frame round-trip)
- L2: AMQP-Connection + Channel-Lifecycle
- L3: Exchange/Queue-Operations
- L4: DDS↔AMQP-Mapping
- L5: Publisher-Confirm + Consumer-Ack
- L6: TLS + SASL-PLAIN (via bridge-security crate)

### CLI / Daemon

`zerodds-amqp09-bridged --config /etc/zerodds/amqp09.yaml`:
- Listener-mode (RabbitMQ-Server-side für legacy Producers)
- Connector-mode (Client zu existing RabbitMQ-Broker)
- Default port: 5672 (Listener), client to broker.

### Tests

- Unit: 9 Class-Methods Wire-Roundtrip
- Cross-Vendor: live gegen RabbitMQ 3.13 + RabbitMQ 4.0 (in
  docker-compose), Pika (Python) als Client, amqp091-go als Client,
  PHP amqplib
- Property-based: Frame-Boundary-Stress mit varying-size payloads
- Performance: ≥ 50k msg/s sustained

### Docs

- User-Guide-Sektion "MQTT vs. AMQP 0.9 vs. AMQP 1.0 — pick the right
  bridge" — Decision-Tree für deployers
- Operator-Guide-Sektion: RabbitMQ-Cluster-Topology + Federation +
  Shovel-Compat

## Out-of-Scope

- **AMQP 0.10** — irrelevant, never widely deployed
- **AMQP-WebSocket** (RabbitMQ Web-STOMP-style 0.9-over-WS) — nicht
  Standard, separate Erweiterung wenn Demand
- **RabbitMQ-Streams** (eigenes Streaming-Protokoll) — nicht AMQP, eigener
  Track post-1.0
- **Federation/Shovel-Configs** als ZeroDDS-managed — User-config-side,
  wir mappen DDS↔AMQP, RabbitMQ topology ist deren Sache

## Acceptance

1. `zerodds-amqp09-bridged` startet, connectet RabbitMQ 3.13 in
   docker-compose
2. Pika-Python-Client als Producer → DDS-Subscriber empfängt sample
3. DDS-Publisher → amqp091-go-Consumer empfängt message
4. Live load 50k msg/s über 5 min ohne reconnect
5. Spec `zerodds-amqp09-bridge-1.0.md` published mit 0/0 partial/open
6. Cross-vendor cross-test: ZeroDDS-AMQP-0.9-bridge ↔ ZeroDDS-AMQP-1.0-
   bridge ↔ DDS roundtrip funktioniert (3-Hop-Demo)

## Dependencies

- bridge-security crate (✅ live)
- amqp-endpoint (✅ AMQP 1.0 Referenz)
- Externe: keine — wir parsen den Wire selbst

## Risks

- **Bridge-Bridge-Routing-Loops** wenn 0.9 und 1.0 gleichzeitig laufen
  und beide auf gleiches DDS-Topic schreiben. Mitigation: source-marker
  in DDS-Sample-Info, dedup auf Bridge-side.
- **RabbitMQ-Cluster-Edge-Cases**: Quorum-Queue-Behavior-bei-Partition
  ist nicht trivial. Mitigation: Test-Matrix mit RabbitMQ-Cluster (3
  Nodes), partition-Inducement.
