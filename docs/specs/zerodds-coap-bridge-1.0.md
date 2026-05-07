# `zerodds-coap-bridge` v1.0 — DDS↔CoAP-Bridge-Daemon

ZeroDDS Vendor-Spec. Spezifiziert das Verhalten eines konfigurierbaren
Daemons, der DDS-Topics mit CoAP-Clients (RFC 7252 + RFC 7641 Observe)
koppelt.

## Motivation

CoAP (RFC 7252) ist der Protokoll-Standard für Constrained-IoT-Devices:
6LoWPAN-Sensoren, Battery-powered-Wearables, LoRa-Gateways. Der
Standard ist UDP-basiert, kompakt (4-Byte-Header), und unterstützt
Observe-Patterns (RFC 7641) für asynchrone Sample-Streams.

Der `zerodds-coap-bridged`-Daemon koppelt DDS-Topics mit CoAP-
Resource-URIs: jedes DDS-Sample wird als CoAP-Response auf eine
Observe-Subscription gepusht; CoAP-POST/PUT-Requests werden als
DDS-Writes interpretiert. Block-Wise-Transfer (RFC 7959) erlaubt
große Samples; DTLS-PSK + Cert-Auth liefert die Edge-Security.

Komplementär zur Library `crates/coap-bridge/` (CoAP-Codec +
DTLS-Stack + DDS-Bridge-Logik) — die Library ist Building-Block,
dieser Daemon ist das ausführbare Produkt.

## §1 Conformance-Levels

| Level | Anforderung |
|-------|-------------|
| **L1 — Wire** | Daemon spricht CoAP (RFC 7252) inkl. Block1/Block2 (RFC 7959), Observe (RFC 7641), No-Response-Option (RFC 7967). |
| **L2 — DDS** | Daemon ist gültiger DDS-DomainParticipant (SPDP/SEDP, Discovery, Liveliness). |
| **L3 — Bridging** | Bidirektional: CoAP→DDS-Publish (POST/PUT) + DDS→CoAP-Push (Observe Notify). |
| **L4 — Config** | URI-Map per YAML-Config; Hot-Reload via SIGHUP optional. |
| **L5 — Auth** | DTLS-PSK + DTLS-Cert (RFC 7252 §9), OSCORE (RFC 8613) optional. |
| **L6 — Multi-Tenant** | Mehrere DomainParticipants pro Daemon; pro DTLS-Session eine Tenant-Bindung. |

L1-L4 sind Pflicht. L5-L6 sind optional (Pflicht für Production).

## §2 CLI-Surface

```
zerodds-coap-bridged [OPTIONS]

Options:
  --config <FILE>           Path zur Config-File (YAML/JSON/TOML)
  --bind <ADDR>             UDP-Bind-Address (Default 0.0.0.0:5683 / DTLS 5684)
  --domain <ID>             DDS-Domain-ID (Default 0)
  --dtls-psk-id <ID>        DTLS-PSK-Identity (single-PSK-Mode)
  --dtls-psk <SECRET>       DTLS-PSK-Secret (Hex oder Base64)
  --dtls-cert <FILE>        DTLS-Server-Cert (PEM); aktiviert coaps://
  --dtls-key <FILE>         DTLS-Server-Key (PEM)
  --topic <DDS:URI>         Single-Topic-Override (mehrfach erlaubt)
  --log-level <LEVEL>       trace/debug/info/warn/error (Default info)
  --metrics <ADDR>          Prometheus-Scrape-Endpoint (Default off)
  --version                 Versions-Info
  --help                    Hilfe

Exit-Codes:
  0   normaler Shutdown (SIGTERM/SIGINT)
  1   Config-Fehler
  2   Bind-Fehler (Port belegt)
  3   DDS-Discovery-Fehler
  4   DTLS-Setup-Fehler
```

## §3 Config-File-Format

YAML-Schema:

```yaml
# zerodds-coap-bridged.yaml
domain: 0
log_level: info

coap:
  bind: "0.0.0.0:5683"
  bind_dtls: "0.0.0.0:5684"
  max_message_size: 1152          # default RFC-7252; raise for jumbo MTU
  ack_timeout_ms: 2000
  max_retransmit: 4
  block_size: 1024                # Block1/Block2 SZX (16/32/64/128/256/512/1024)
  observe_max_age_secs: 60

  dtls:
    enabled: true
    cert_file: "/etc/zerodds/coap-cert.pem"
    key_file:  "/etc/zerodds/coap-key.pem"
    ca_file:   "/etc/zerodds/coap-ca.pem"
    psk:
      - identity: "device-001"
        secret_hex: "deadbeefcafebabe..."
      - identity: "device-002"
        secret_hex: "feedface1234..."

oscore:
  enabled: false
  master_secret_hex: "0102030405060708090a0b0c0d0e0f10"
  master_salt_hex:   "9e7ca92223786340"
  id_context_hex:    ""

topics:
  - dds_name: "Chat::Message"
    dds_type: "Chat::Message"
    coap_uri_path: "chat/message"
    direction: "bidir"
    qos:
      reliability: "reliable"
      durability: "volatile"
      history: { kind: "keep_last", depth: 10 }

  - dds_name: "Sensor::Reading"
    dds_type: "Sensor::Reading"
    coap_uri_path: "sensors/reading"
    direction: "out"
    qos:
      reliability: "best_effort"

  - dds_name: "Actuator::Command"
    dds_type: "Actuator::Command"
    coap_uri_path: "actuator/command"
    direction: "in"
    qos:
      reliability: "reliable"
      durability: "transient_local"

content_format:
  cdr2_le_id: 65000                # Vendor-Range registration (IANA-Registry)
  cdr2_be_id: 65001
  json_id: 50                      # standard application/json

acl:
  default_deny: true
  rules:
    - subject: "device-001"
      allow_post: ["chat/+", "sensors/+"]
      allow_observe: ["chat/+"]

metrics:
  enabled: true
  listen: "127.0.0.1:9092"
  path: "/metrics"
```

ENV-Substitution: `${VAR}` und `${VAR:-default}` werden vor Config-
Parse aufgelöst.

## §4 CoAP-Wire-Protocol

### §4.1 Frame-Format

Standard RFC 7252 Header (4 Bytes) + Token + Options + Payload:

```
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|Ver| T |  TKL  |      Code     |          Message ID           |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|   Token (if any, TKL bytes) ...
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|   Options (if any) ...
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|1 1 1 1 1 1 1 1|    Payload (CDR-Bytes) ...
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

### §4.2 Publish (CoAP→DDS)

`POST coap[s]://<host>:5683/<coap_uri_path>`:
- Options:
  - `Uri-Path`: pro Segment des URI-Paths
  - `Content-Format`: 65000 (XCDR2-LE) oder 65001 (XCDR2-BE) oder 50 (JSON)
  - `If-Match` / `If-None-Match` (optional, für Instance-Lifecycle)
  - `No-Response` (RFC 7967) optional für FIRE-AND-FORGET
- Payload: CDR-Bytes (oder JSON wenn Content-Format=50)

Daemon-Antwort:
- `2.04 Changed` bei akzeptiertem POST → DDS-Write done
- `4.00 Bad Request` bei Decode-Fehler
- `4.13 Request Entity Too Large` wenn ohne Block1
- `5.00 Internal Server Error` bei DDS-Write-Fehler

`PUT` für Idempotente-Sample-Updates (z.B. retain-style mit instance-key).

`DELETE` mappt auf DDS-Dispose.

### §4.3 Subscribe (DDS→CoAP, Observe-Pattern)

`GET coap[s]://<host>:5683/<coap_uri_path>` mit `Observe: 0` Option:
- Daemon registriert den Client in der Notification-List für das Topic.
- Initial-Response: aktueller Sample (oder `2.05 Content` empty bei VOLATILE).
- Bei jedem neuen DDS-Sample: Notify mit `Observe: <seq>`, Token re-used.
- `2.05 Content` mit `Content-Format: 65000` + CDR-Payload.

Cancel: `Observe: 1` Option im GET, oder RST-Reset auf Token, oder
Observe-Timeout `observe_max_age_secs`.

### §4.4 Block-Wise-Transfer

Samples > `block_size` automatisch geblockt:
- Outbound (Notify): `Block2(NUM, M, SZX)` Option
- Inbound (POST): `Block1(NUM, M, SZX)` Option, Daemon assembled bevor DDS-Write

Defragmentation-Cap: konfigurierbar via `coap.max_message_size`
(Default 64KB). Größere Samples werden mit `4.13` gerejected und
müssen per chunked-stream-Topic gesendet werden.

### §4.5 Content-Format-Registry

| ID | Media-Type | Verwendung |
|----|------------|------------|
| 65000 | `application/x-dds-cdr2; bo=le` | XCDR2 little-endian |
| 65001 | `application/x-dds-cdr2; bo=be` | XCDR2 big-endian |
| 65002 | `application/x-dds-cdr1; bo=le` | XCDR1 little-endian (legacy) |
| 50 | `application/json` | JSON-Repräsentation (Debug) |
| 60 | `application/cbor` | CBOR-Repräsentation (optional, future) |

IANA-Registrierung im Vendor-Range 65000-65535.

## §5 Topic-Mapping

### §5.1 Slug-Algorithmus

Topic-Name `Chat::Message` → CoAP-URI-Path-Default:
1. Lowercase: `chat::message`
2. `::` → `/`: `chat/message`
3. Andere non-`[a-z0-9/_-]` → `_`
4. Result: `chat/message`

Override per `coap_uri_path`-Feld im Config.

### §5.2 Type-Discovery

`GET /.well-known/core` (RFC 6690) liefert die Resource-Catalog:
```
</chat/message>;rt="dds.topic";ct=65000;type="Chat::Message",
</sensors/reading>;rt="dds.topic";ct=65000;type="Sensor::Reading",
</actuator/command>;rt="dds.topic";ct=65000;type="Actuator::Command"
```

`GET /schema/<slug>` liefert die IDL-Definition für Codegen auf
Client-Seite.

## §6 QoS-Translation

| DDS-QoS | CoAP-Verhalten |
|---------|----------------|
| Reliability `RELIABLE` | CoAP `CON`-Messages (Confirmable) mit Retransmit |
| Reliability `BEST_EFFORT` | CoAP `NON`-Messages (Non-Confirmable) |
| Durability `VOLATILE` | nur Live-Notify; kein Replay bei Observe-Re-Register |
| Durability `TRANSIENT_LOCAL` | initial-Notify mit letztem Sample |
| History `KEEP_LAST(N)` | analog (Observe-Re-Register sendet die letzten N) |
| Lifespan | `Max-Age` Option im Notify |
| Deadline | beobachtet, Daemon emittiert `5.03 Service Unavailable` mit `Max-Age=0` |
| Liveliness | CoAP-Ping (`0.00 EMPTY` als Confirmable Heartbeat alle `keep_alive_secs`) |
| Partition | Filter im Daemon vor Notify-Push |

## §7 Security

### §7.1 DTLS

`coaps://`-Mode aktiviert über `coap.dtls.enabled: true`. Cipher-
Suites empfohlen:
- `TLS_PSK_WITH_AES_128_CCM_8` (PSK-Mode, RFC 7252 §9.1.3)
- `TLS_ECDHE_ECDSA_WITH_AES_128_CCM_8` (Cert-Mode)
- `TLS_ECDHE_PSK_WITH_AES_128_CBC_SHA256` (Hybrid)

Cert-Rotation via SIGHUP (Daemon liest cert/key neu, Sessions halten).

### §7.2 OSCORE (RFC 8613)

OSCORE-Mode für Ende-zu-Ende-Schutz über CoAP-Proxies:
```yaml
oscore:
  enabled: true
  master_secret_hex: "0102..."
  master_salt_hex:   "9e7c..."
  id_context_hex:    ""
```

Pro Sender wird Sender/Recipient-Context per HKDF abgeleitet. Replay-
Window: 32 (Default).

### §7.3 ACL

Per Topic im Config; Subject = DTLS-PSK-Identity oder Cert-Subject-DN.

## §8 Operations + Observability

### §8.1 Logging

Strukturiertes JSON-Log auf stdout. Felder: `timestamp`, `level`,
`event`, `peer`, `coap_uri`, `dds_topic`, `mid`, `token_hex`,
`block_num`, `bytes`, `latency_us`.

### §8.2 Prometheus-Metrics

Wenn `metrics.enabled: true`:
```
zerodds_coap_bridge_requests_total          counter{method, code}
zerodds_coap_bridge_observers_active        gauge{topic}
zerodds_coap_bridge_notifies_sent_total     counter{topic}
zerodds_coap_bridge_blocks_in_total         counter{topic}
zerodds_coap_bridge_blocks_out_total        counter{topic}
zerodds_coap_bridge_dtls_handshakes_total   counter{result}
zerodds_coap_bridge_oscore_replays_total    counter
zerodds_coap_bridge_dds_samples_received    counter{topic}
zerodds_coap_bridge_dds_samples_published   counter{topic}
zerodds_coap_bridge_acl_denials_total       counter{reason}
```

### §8.3 OTLP-Spans

`OTEL_EXPORTER_OTLP_ENDPOINT` setzt: Daemon emittiert Spans pro
CoAP-Exchange (`coap.request` + `dds.write` als Child).

## §9 Lifecycle

### §9.1 Startup

1. Config-Parse + Validation.
2. DTLS-Cert/PSK-Load wenn aktiviert.
3. DCPS-DomainParticipant init auf `domain`.
4. Pro Topic: Reader+Writer registrieren (gemäß `direction`).
5. UDP-Bind 5683 (CoAP) + 5684 (CoAPs) wenn DTLS.
6. SIGHUP/SIGTERM/SIGINT-Handler installieren.

### §9.2 Shutdown

SIGTERM/SIGINT → graceful drain (max 30s):
- Stop accepting new CoAP-Requests.
- Send `2.05 Content` mit `Observe: 1` (deregister) an alle Observer.
- Drain pending DDS-Writes.
- Cleanup DDS-Entities.
- Exit 0.

SIGHUP → Config-Reload (Cert + ACL hot-update; topic-map-Änderungen
brauchen Restart).

## §10 Cross-Vendor

Daemon ist normaler RTPS-Peer auf der DDS-Seite. CoAP-Seite getestet
gegen libcoap, californium, aiocoap, Eclipse-Wakaama (LwM2M-Stack
nutzt CoAP-Layer).

Verifiziert in `crates/coap-bridge/tests/cross_vendor.rs`.

## §11 Packaging

Per `zerodds-deployment-1.0` Spec:
- Binary: `zerodds-coap-bridged`
- Config-Default: `/etc/zerodds/coap-bridged.yaml`
- Systemd-Unit: `zerodds-coap-bridged.service`
- launchd-Plist: `org.zerodds.coap-bridged.plist`
- Win-Service: `ZeroDDSCoAPBridge`
- Docker: `zerodds/coap-bridged:1.0`

Manual: `man 1 zerodds-coap-bridged` + `man 5 zerodds-coap-bridged.yaml`.

## §12 Testing

### §12.1 Unit-Tests

Pro Modul (`config`, `coap_codec`, `block_assembler`, `observe_table`,
`dtls`, `oscore`, `dds_pump`) ≥ 5 Tests in `crates/coap-bridge/`.

### §12.2 Integration-Tests

`crates/coap-bridge/tests/bridge_e2e.rs`:
- Spawn `zerodds-coap-bridged` als Subprocess.
- libcoap-Test-Client als CoAP-Peer.
- POST → DDS-Subscriber empfängt.
- Observe → DDS-Publisher emit → CoAP-Notify.
- Block1/Block2 mit 4KB-Payload.

### §12.3 Multi-Vendor

`tests/cross_vendor.rs`: Cyclone-DDS-Subscriber + libcoap/californium-
Client gegen ZeroDDS-CoAP-Bridge im Docker-Compose.

## §13 Cross-References

- Library: `crates/coap-bridge/`
- RFC 7252 (CoAP), RFC 7641 (Observe), RFC 7959 (Block), RFC 8613 (OSCORE).
- Wire-Format: `zerodds-xcdr2-bindings-conformance-1.0` §3.
- Deployment: `zerodds-deployment-1.0`.
- Verwandte Daemons: `zerodds-mqtt-bridge-1.0`, `zerodds-ws-bridge-1.0`.

## §14 Versioning

`1.0` initial. Patch-Updates für Bugfixes, Minor für additive Config-
Felder (z.B. neue Content-Format-IDs), Major für Wire-Protocol-
Changes.
