# `zerodds-ws-bridge` v1.0 — DDS↔WebSocket-Bridge-Daemon

ZeroDDS Vendor-Spec. Spezifiziert das Verhalten eines konfigurierbaren
Daemons, der DDS-Topics mit WebSocket-Clients koppelt.

## Motivation

Browser-Apps und IoT-Geräte koennen nicht nativ am RTPS/UDP-Multicast-
Discovery teilnehmen (kein UDP im Browser, MTU/Routing-Restrictions
hinter NAT). Der `zerodds-ws-bridged`-Daemon ist die Brücke: er ist
ein DDS-DomainParticipant *und* ein WS-Server gleichzeitig — alles was
auf einem Topic published wird, geht raus auf alle subscribten WS-
Clients und umgekehrt.

Komplementär zur Library `crates/websocket-bridge/` (RFC-6455-Codec +
DDS-Bridge-Logik) — die Library ist Building-Block, dieser Daemon ist
das ausführbare Produkt.

## §1 Conformance-Levels

| Level | Anforderung |
|-------|-------------|
| **L1 — Wire** | Daemon spricht RFC 6455 (Base-Framing, Handshake, Close) + RFC 7692 (permessage-deflate optional). |
| **L2 — DDS** | Daemon ist gültiger DDS-DomainParticipant (SPDP/SEDP, Discovery, Liveliness). |
| **L3 — Bridging** | Bidirektional: WS→DDS-Publish + DDS→WS-Push für alle gemappten Topics. |
| **L4 — Config** | Topic-Map per YAML/JSON-Config; Hot-Reload via SIGHUP optional. |
| **L5 — Auth** | TLS (`wss://`) + Token-Auth (Bearer/JWT) + Per-Topic-ACL. |
| **L6 — Multi-Tenant** | Mehrere DomainParticipants pro Daemon; pro WS-Connection eine Tenant-Bindung. |

L1-L4 sind Pflicht. L5-L6 sind optional (Pflicht für Production).

## §2 CLI-Surface

```
zerodds-ws-bridged [OPTIONS]

Options:
  --config <FILE>          Path zur Config-File (YAML/JSON/TOML)
  --listen <ADDR>          Bind-Address (Default 0.0.0.0:8080)
  --domain <ID>            DDS-Domain-ID (Default 0)
  --topic <NAME[:KEY]>     Single-Topic-Override (mehrfach)
  --tls-cert <FILE>        TLS-Cert (PEM); aktiviert wss://
  --tls-key <FILE>         TLS-Key (PEM)
  --auth-token <SECRET>    Bearer-Token-Auth (single-token-mode)
  --log-level <LEVEL>      trace/debug/info/warn/error (Default info)
  --metrics <ADDR>         Prometheus-Scrape-Endpoint (Default off)
  --version                Versions-Info
  --help                   Hilfe

Exit-Codes:
  0   normaler Shutdown (SIGTERM/SIGINT)
  1   Config-Fehler
  2   Bind-Fehler (Port belegt)
  3   DDS-Discovery-Fehler
  4   TLS-Fehler
```

## §3 Config-File-Format

YAML-Schema (auch JSON/TOML akzeptiert):

```yaml
# zerodds-ws-bridged.yaml
listen: "0.0.0.0:8080"
domain: 0
log_level: info

tls:
  enabled: false
  cert_file: "/etc/zerodds/cert.pem"
  key_file: "/etc/zerodds/key.pem"

auth:
  mode: "none"                 # none|bearer|jwt|mtls
  bearer_token: "${TOKEN}"     # ENV-Substitution
  jwt:
    public_key: "/etc/zerodds/jwt-pub.pem"
    audience: "zerodds-bridge"
    required_claim: "scope=dds.read"

topics:
  - name: "Chat::Message"
    type: "Chat::Message"
    direction: "bidir"         # in|out|bidir
    qos:
      reliability: "reliable"
      durability: "volatile"
      history: { kind: "keep_last", depth: 10 }
    ws_path: "/topics/chat"    # Optional, default = /topics/<topic-slug>
    acl:
      read: ["*"]              # Liste der erlaubten auth-subjects
      write: ["alice","bob"]

  - name: "Sensor::Reading"
    type: "Sensor::Reading"
    direction: "out"
    ws_path: "/topics/sensor"

metrics:
  enabled: true
  listen: "127.0.0.1:9090"
  path: "/metrics"
```

ENV-Substitution: `${VAR}` und `${VAR:-default}` werden vor Config-Parse aufgelöst.

## §4 WebSocket-Wire-Protocol

### §4.1 Handshake

Standard RFC 6455 Upgrade-Handshake. Daemon antwortet mit
`Sec-WebSocket-Protocol: zerodds-ws-bridge/1.0` falls Client diesen
Subprotocol-Header sendet (optional).

Auth-Header (Bearer-Mode): `Authorization: Bearer <token>` → 401 bei
fehlerhafter Auth, 403 bei missing-scope.

### §4.2 Pfad-Routing

Pro Topic-Config-Eintrag wird ein WS-Endpoint angelegt:
- `/topics/<slug>` — Default-Pfad (slug = topic-name lowercased + `_`-replaced).
- `ws_path` Override im Config.
- `/topics/__catalog__` — meta-Endpoint, liefert JSON-Liste der verfügbaren Topics + Schemata.
- `/healthz` — HTTP-200 wenn DCPS-Runtime läuft.
- `/metrics` — Prometheus-Format wenn aktiviert.

### §4.3 Frame-Format

Jeder WS-Frame transportiert ein Sample:

**Binary-Frames** (default):
```
+--------+--------+--------------------+--------------------+
| Magic  | Flags  | Encap-Header (4)   | CDR-Payload (...)  |
| "ZDB1" | 1 byte | XCDR2-LE etc.      | Spec §3 conformance |
+--------+--------+--------------------+--------------------+
```
- Magic `0x5A 0x44 0x42 0x31` = "ZDB1" (ZeroDDS Binary v1)
- Flags: bit0 = `dispose`, bit1 = `coherent_set_member`, bit2-7 reserved
- Encap-Header: `[0x00, 0x07, 0x00, 0x00]` (XCDR2-LE) per `zerodds-xcdr2-bindings-conformance-1.0` §3
- CDR-Payload: gemäß Topic-Type, byte-genau wie über RTPS

**Text-Frames** (optional, falls Client `Sec-WebSocket-Protocol: zerodds-ws-bridge/1.0+json` anfordert):
```json
{
  "topic": "Chat::Message",
  "op": "publish",        // publish|dispose|register|unregister
  "key_hash": "a1d0c6e8...",
  "data": { /* JSON-Repräsentation des Samples */ },
  "timestamp_ns": 1730000000000000000
}
```

JSON-Mode reduziert Throughput um ~3-5x — nur für Debug/Browser-Dev.

### §4.4 Control-Messages

Client → Daemon:
```json
{ "op": "subscribe", "topics": ["Chat::Message", "Sensor::Reading"] }
{ "op": "unsubscribe", "topics": ["Chat::Message"] }
{ "op": "ping" }
```

Daemon → Client:
```json
{ "op": "subscribed", "topics": [...] }
{ "op": "error", "code": 403, "message": "scope dds.write required for Chat::Message" }
{ "op": "pong" }
```

Standard RFC 6455 PING/PONG-Frames werden zusätzlich unterstützt.

## §5 Topic-Mapping

### §5.1 Slug-Algorithmus

Topic-Name `Chat::Message` → URL-Slug:
1. Lowercase: `chat::message`
2. `::` → `/`: `chat/message`
3. Andere non-`[a-z0-9/_-]` → `_`
4. Result: `/topics/chat/message`

Override per `ws_path` im Config.

### §5.2 Type-Discovery

Beim Verbindungsaufbau sendet Daemon optional ein Catalog-Frame:
```json
{
  "op": "catalog",
  "topics": [
    {
      "name": "Chat::Message",
      "type": "Chat::Message",
      "ws_path": "/topics/chat/message",
      "qos": { "reliability": "reliable", ... },
      "schema_url": "/schema/chat/message.idl"
    }
  ]
}
```

`/schema/<slug>` liefert die IDL-Definition (oder TypeObject-XML) für
Codegen auf Client-Seite.

## §6 QoS-Translation

WebSocket ist Connection-orientiert (TCP) — die DDS-QoS-Policies
mappen wie folgt:

| DDS-QoS | WS-Verhalten |
|---------|--------------|
| Reliability `RELIABLE` | TCP-Garantien reichen; Daemon dropped keine Frames bei langsamem Client (Backpressure via WS-Mask) |
| Reliability `BEST_EFFORT` | Daemon dropped Frames falls Send-Queue voll (`max_send_queue_per_connection`-Config) |
| Durability `VOLATILE` | nur Live-Samples |
| Durability `TRANSIENT_LOCAL` | Daemon hält letzte N Samples + sendet Burst beim Connect (`max_burst_samples`-Config) |
| History `KEEP_LAST(N)` | analog |
| Deadline | beobachtet, sendet `{ "op": "deadline_missed", "topic": "..." }` |
| Liveliness | Heartbeat per WS-PING / Topic-Liveliness mapped auf Connection-Liveness |
| Partition | partition-Filter im Daemon vor WS-Write |

## §7 Security

### §7.1 TLS

`wss://`-Mode aktiviert über `tls.enabled: true`. Cert-Rotation via
SIGHUP (Daemon liest cert/key neu, Connections halten ihre Session).

### §7.2 Auth-Modes

- `none`: keine Auth (Dev-Mode, bind nur 127.0.0.1 erlaubt)
- `bearer`: HTTP-Header `Authorization: Bearer <token>` gegen
  `bearer_token`-Config-Wert
- `jwt`: JWT-Token-Validierung (RS256/ES256), Claims gegen `acl`-Pro-
  Topic
- `mtls`: Client-Zertifikat-Auth, Subject-DN als Identity

### §7.3 ACL

Per Topic im Config:
```yaml
acl:
  read: ["alice", "bob", "*group:engineers*"]
  write: ["alice"]
```

Rules:
- `*` = alle
- `<name>` = exakter Match
- `*group:<n>*` = JWT-Claim `groups` enthält `<n>`

## §8 Operations + Observability

### §8.1 Logging

Strukturiertes JSON-Log auf stdout (per `--log-level`). Felder:
`timestamp`, `level`, `event`, `connection_id`, `topic`, `bytes`,
`peer`, `latency_us`.

### §8.2 Prometheus-Metrics

Wenn `metrics.enabled: true`:
```
zerodds_ws_bridge_connections_total      counter
zerodds_ws_bridge_connections_active     gauge
zerodds_ws_bridge_frames_in_total        counter{topic, direction}
zerodds_ws_bridge_bytes_in_total         counter{topic, direction}
zerodds_ws_bridge_frames_out_total       counter{topic, direction}
zerodds_ws_bridge_bytes_out_total        counter{topic, direction}
zerodds_ws_bridge_send_queue_drops_total counter{connection_id}
zerodds_ws_bridge_dds_samples_received   counter{topic}
zerodds_ws_bridge_dds_samples_published  counter{topic}
zerodds_ws_bridge_auth_failures_total    counter{reason}
```

### §8.3 OTLP-Spans (optional)

Wenn `OTEL_EXPORTER_OTLP_ENDPOINT`-ENV gesetzt: Daemon emittiert
`zerodds-observability-otlp` Spans pro Connection-Lifecycle und pro
Frame.

## §9 Lifecycle

### §9.1 Startup

1. Config-Parse + Validation (fail-fast bei fehlerhafter YAML).
2. TLS-Cert-Load wenn aktiviert.
3. DCPS-DomainParticipant init auf `domain`.
4. Pro Topic: Reader+Writer registrieren (gemäß `direction`).
5. WS-Server-Bind + Listen.
6. SIGHUP/SIGTERM/SIGINT-Handler.

### §9.2 Shutdown

SIGTERM/SIGINT → graceful drain (max 30s, konfigurierbar):
- Stop accepting new WS-Connections.
- Drain pending DDS-samples (publish queue).
- Send `{ "op": "shutdown" }` an alle WS-Connections.
- Close WS-Connections mit Code 1001 (going away).
- Cleanup DDS-Entities.
- Exit 0.

SIGHUP → Config-Reload (TLS-Cert + ACL hot-update; topic-map-changes
brauchen restart).

## §10 Cross-Vendor

Daemon ist ein normaler RTPS-Peer — Cyclone/RTI/Fast-DDS-Subscriber
empfangen die Samples die ein WS-Client published, und vice versa.
Verifiziert in `crates/websocket-bridge/tests/cross_vendor.rs`.

## §11 Packaging

Per `zerodds-deployment-1.0` Spec:
- Binary: `zerodds-ws-bridged` (statisch gelinkt empfohlen)
- Config-Default: `/etc/zerodds/ws-bridged.yaml` (Linux), `/usr/local/etc/zerodds/` (Mac), `%PROGRAMDATA%\ZeroDDS\` (Win)
- Systemd-Unit: `zerodds-ws-bridged.service`
- launchd-Plist: `org.zerodds.ws-bridged.plist`
- Win-Service: `ZeroDDSWSBridge`
- Docker: `zerodds/ws-bridged:1.0`

Manual: `man 1 zerodds-ws-bridged` + `man 5 zerodds-ws-bridged.yaml`.

## §12 Testing

### §12.1 Unit-Tests

Pro Modul (`config`, `topic_map`, `auth`, `frame_codec`,
`dds_pump`) ≥ 5 Tests in `crates/websocket-bridge/src/bin/`.

### §12.2 Integration-Tests

`crates/websocket-bridge/tests/bridge_e2e.rs`:
- Spawn `zerodds-ws-bridged` als Subprocess.
- WS-Client connect, publish.
- DDS-Subscriber im Test-Process empfängt.
- Verify byte-genauer Roundtrip.

### §12.3 Multi-Vendor

`tests/cross_vendor.rs` (cargo-feature-gated): Cyclone-Subscriber im
Docker-Compose, ZeroDDS-WS-Bridge published, Cyclone-Sub printet.

## §13 Cross-References

- Library: `crates/websocket-bridge/`
- Codec-Spec: `zerodds-ws-bridge`-spezifische Erweiterungen RFC-6455.
- Wire-Format: `zerodds-xcdr2-bindings-conformance-1.0` §3.
- Deployment: `zerodds-deployment-1.0`.
- Verwandte Daemons: `zerodds-mqtt-bridge-1.0`, `zerodds-coap-bridge-1.0`,
  `zerodds-amqp-bridge-daemon-1.0`, `zerodds-grpc-bridge-1.0`.

## §14 Versioning

`1.0` initial. Patch-Updates für Bugfixes, Minor für additive Config-
Felder, Major für Wire-Protocol-Changes.
