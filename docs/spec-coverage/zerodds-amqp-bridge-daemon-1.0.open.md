# zerodds-amqp-bridge-daemon-1.0 — offene Punkte

Aggregat aus `zerodds-amqp-bridge-daemon-1.0.md`, erzeugt von `website/_tools/gen_open_items.py`.
Kein eigener Inhalt: jedes Item steht dort mit seinem Beleg.

---

## §4.5 Disposition-Mapping accepted/rejected/released/modified

**Status:** `partial` — `released`/`modified`/`rejected` vom Broker sind nicht gegen einen Broker gemessen.

## §6 DDS-QoS → AMQP-Behavior Map

**Status:** `partial` — Deadline-Annotation und Partition-Filter fehlen; KEEP_ALL verwirft über 65 536 wartenden Samples das älteste.

## §7.2 SASL PLAIN/SCRAM/EXTERNAL/ANONYMOUS/XOAUTH2

**Status:** `partial` — XOAUTH2 nicht gegen einen Broker gemessen; SCRAM-SHA-512 nicht gebaut.

## §7.3 ACL Daemon-Side

**Status:** `partial` — kein Test prüft, dass eine verweigerte Richtung keine Daten bewegt.

## §8.1 Strukturiertes JSON-Logging

**Status:** `partial` — Zeilen auf stderr, ohne `timestamp`, `container_id`, `delivery_id`, `latency_us`.

## §8.3 OTLP-Spans

**Status:** `partial` — die Konfiguration ist geprüft, der Span-Emit nicht; der fehlende Teil wird als §8.3a `open` geführt.

## §8.3a TRANSFER-Roundtrip-Span

**Status:** `open` — der Emit-Teil von §8.3. Geprüft ist nur, dass die Endpoint-Konfiguration gelesen wird.

## §9.2 Shutdown SIGTERM/SIGINT/SIGHUP

**Status:** `partial` — kein Test für den Drain.

## §10 RTPS-Peer + RabbitMQ/Artemis/Qpid/Solace/ServiceBus

**Status:** `partial` — RabbitMQ 4.0.5 gemessen, die vier weiteren Broker nicht (§10a).

## §10a Broker-Matrix Artemis/Qpid-Dispatch/Solace/Azure ServiceBus

**Status:** `open` — der Teil von §10 und §12.3 jenseits RabbitMQ. ---

## §12.3 Multi-Vendor

**Status:** `open` — Cyclone-Subscriber hinter der Bridge und Broker jenseits RabbitMQ nicht gemessen.
