# `zerodds-corba-bridge` v1.0 — DDS↔CORBA-Bridge-Daemon

ZeroDDS Vendor-Spec. Spezifiziert das Verhalten eines konfigurierbaren
Daemons, der DDS-Topics mit CORBA-IIOP-Servern (GIOP/IIOP, OMG
formal/2012-11-14) koppelt.

## Motivation

CORBA ist im Finanzsektor, in Telco-Switching-Stacks, in Air-Traffic-
Control-Systems, und in industrieller Bestandssoftware der frühen
2000er bis heute aktiv. Die Migration solcher Bestände auf modernes
DDS-basiertes Real-Time-Messaging ist ein Mehrjahres-Programm — bis
dahin braucht es Bridges. Der `zerodds-corba-bridged`-Daemon ist die
Coexistence-Komponente: CORBA-Operations werden auf DDS-Request/Reply-
Topic-Pärchen gemappt; DDS-Pubs werden bei Bedarf als CORBA-Notify-
Events emittiert.

Komplementär zu den Libraries `crates/corba-dds-bridge/`,
`crates/corba-iiop/` (TCP-Transport), `crates/corba-giop/` (Frame-
Codec) — die Libraries sind Building-Blocks, dieser Daemon ist das
ausführbare Produkt.

## §1 Conformance-Levels

| Level | Anforderung |
|-------|-------------|
| **L1 — Wire** | Daemon spricht GIOP 1.0/1.1/1.2 (OMG formal/2012-11-14) inkl. Request/Reply/CancelRequest/LocateRequest/Fragment + IIOP 1.0/1.1/1.2 Profile. |
| **L2 — DDS** | Daemon ist gültiger DDS-DomainParticipant (SPDP/SEDP, Discovery, Liveliness). |
| **L3 — Bridging** | Bidirektional: GIOP-Request→DDS-RequestTopic + DDS-ReplyTopic→GIOP-Reply (Request-Reply-Pattern); optional Notification-Service-Style Pub/Sub. |
| **L4 — Config** | Operation-Map per YAML-Config (RepoId↔DDS-Topic-Pärchen); Hot-Reload via SIGHUP optional. |
| **L5 — Auth** | TLS-over-IIOP (SSLIOP) + CSIv2 (`SAS_ContextElement`) + GSSUP-Username/Password. |
| **L6 — Multi-Tenant** | Mehrere DomainParticipants pro Daemon; pro IIOP-Connection eine Tenant-Bindung. |

L1-L4 sind Pflicht. L5-L6 sind optional (Pflicht für Production).

## §2 CLI-Surface

```
zerodds-corba-bridged [OPTIONS]

Options:
  --config <FILE>             Path zur Config-File (YAML/JSON/TOML)
  --iiop-bind <ADDR>          IIOP-Bind (Default 0.0.0.0:6833)
  --ssliop-bind <ADDR>        SSLIOP-Bind (Default 0.0.0.0:6834)
  --domain <ID>               DDS-Domain-ID (Default 0)
  --naming-service <CORBANAME> z.B. corbaname::nameserver:2809#NameServiceRoot
  --orb-id <ID>               ORB-Identity (Default "zerodds-corba-bridge")
  --tls-cert <FILE>           TLS-Server-Cert (PEM); aktiviert SSLIOP
  --tls-key <FILE>            TLS-Server-Key (PEM)
  --topic <REPO_ID:DDS>       Single-Mapping-Override (mehrfach)
  --log-level <LEVEL>         trace/debug/info/warn/error (Default info)
  --metrics <ADDR>            Prometheus-Scrape-Endpoint (Default off)
  --version                   Versions-Info
  --help                      Hilfe

Exit-Codes:
  0   normaler Shutdown (SIGTERM/SIGINT)
  1   Config-Fehler
  2   Bind-Fehler (Port belegt)
  3   DDS-Discovery-Fehler
  4   TLS-Fehler
  5   NameService-Bind-Fehler
```

## §3 Config-File-Format

YAML-Schema:

```yaml
# zerodds-corba-bridged.yaml
domain: 0
log_level: info

corba:
  orb_id: "zerodds-corba-bridge"
  iiop:
    bind: "0.0.0.0:6833"
    giop_version: "1.2"             # 1.0 | 1.1 | 1.2
    fragment_size: 65536
    max_message_size: 33554432      # 32 MB
  ssliop:
    enabled: true
    bind: "0.0.0.0:6834"
    cert_file: "/etc/zerodds/corba-cert.pem"
    key_file:  "/etc/zerodds/corba-key.pem"
    ca_file:   "/etc/zerodds/corba-ca.pem"
    require_client_cert: false
  ior:
    naming_service: "corbaname::nameserver:2809#NameServiceRoot/Bridge"
    publish_to_naming: true
    ior_file: "/var/lib/zerodds/bridge.ior"   # Backup-Datei mit IOR-Stringified
  csiv2:
    enabled: false
    target_supports: ["Integrity", "Confidentiality", "EstablishTrustInClient"]
    gssup:
      target_name: "zerodds@example.com"

mappings:
  # Each mapping is a CORBA-Interface-Operation ↔ DDS-Topic-Pair
  - repo_id: "IDL:zerodds/MarketData/Quote:1.0"
    operation: "request_quote"
    direction: "corba_to_dds"           # corba_to_dds | dds_to_corba | bidir
    request_topic:
      dds_name: "MarketData::QuoteRequest"
      dds_type: "MarketData::QuoteRequest"
    reply_topic:
      dds_name: "MarketData::QuoteReply"
      dds_type: "MarketData::QuoteReply"
    correlation_id: "request_id"        # Field-Name in Request für Reply-Matching
    timeout_ms: 5000
    qos:
      reliability: "reliable"
      durability: "volatile"

  - repo_id: "IDL:zerodds/Trading/OrderEvents:1.0"
    operation: "on_order_event"
    direction: "dds_to_corba"
    notify_topic:
      dds_name: "Trading::OrderEvent"
      dds_type: "Trading::OrderEvent"
    target_object_keys:
      - "trading_listener_main"
      - "trading_listener_audit"

acl:
  default_deny: true
  rules:
    - subject: "CN=trader-app,OU=trading,O=example.com"
      allow_invoke: ["IDL:zerodds/MarketData/*"]

metrics:
  enabled: true
  listen: "127.0.0.1:9095"
  path: "/metrics"
```

ENV-Substitution: `${VAR}` und `${VAR:-default}`.

## §4 GIOP/IIOP-Wire-Protocol

### §4.1 IOR-Generation

Pro Mapping erzeugt Daemon ein IOR mit:
- `type_id`: `repo_id` aus Config
- `profile_count`: 1 (IIOP) oder 2 (IIOP + SSLIOP)
- `IIOP-Profile` (Tag 0):
  - `version`: gemäß `corba.iiop.giop_version`
  - `host`: Bind-Hostname
  - `port`: 6833
  - `object_key`: stable hash über `(repo_id + dds_topic_pair)` — siehe §4.5
  - Components (Tag 0x20 = `TAG_INTERNET_IOP::TAG_ORB_TYPE`, Tag 0x21
    = `TAG_INTERNET_IOP::TAG_CODE_SETS`, Tag 0x06 = `TAG_SSL_SEC_TRANS`
    bei SSLIOP)
- `SSL-Component` (Tag 0x06): `target_supports`, `target_requires`,
  `port` (6834)

IOR wird:
- in `corba.ior.ior_file` als Stringified-IOR (`IOR:0102...`) geschrieben.
- bei `corba.ior.publish_to_naming=true` gegen NameService gebunden.

### §4.2 GIOP-Frame-Format

Standard GIOP-Header (12 Bytes):
```
+----+----+----+----+--------+--------+--------+------------+
|'G' |'I' |'O' |'P' | major  | minor  | flags  | msg_type   |
+----+----+----+----+--------+--------+--------+------------+
|                  message_size (4 bytes)                   |
+-----------------------------------------------------------+
```
- `flags.bit0` = byte-order (0=BE, 1=LE)
- `flags.bit1` = fragment-bit (GIOP 1.1+)
- `msg_type`: 0=Request, 1=Reply, 2=CancelRequest, 3=LocateRequest,
  4=LocateReply, 5=CloseConnection, 6=MessageError, 7=Fragment

Body folgt mit CDR-Encoded RequestHeader/ReplyHeader gemäß GIOP-
Version, dann CDR-encoded Operation-Args/Results.

### §4.3 Request-Reply-Mapping (CORBA → DDS)

Eingehender GIOP-Request:
1. Parse RequestHeader → `(object_key, operation, request_id)`.
2. Lookup `(object_key, operation)` → Mapping → DDS-Request-Topic.
3. Args werden CDR-decoded und in eine `<dds_type>`-Sample gepackt.
4. Daemon publiziert auf Request-Topic mit Source-Timestamp = jetzt.
5. Daemon erstellt eine Pending-Request-Entry mit `correlation_id`
   = `request_id` (oder Custom-Field aus Sample).
6. DDS-Subscriber (Backend-Service) verarbeitet Request, published
   Reply auf Reply-Topic.
7. Daemon korreliert Reply per `correlation_id`, sendet GIOP-Reply
   zurück mit `reply_status=NO_EXCEPTION` + CDR-encoded Body.

Bei Timeout (`timeout_ms`): Daemon emittiert GIOP-Reply mit
`reply_status=SYSTEM_EXCEPTION`, `TIMEOUT (CORBA::TIMEOUT)`.

### §4.4 Notify-Mapping (DDS → CORBA)

Pro `direction=dds_to_corba`-Mapping:
- DDS-Sample auf `notify_topic` triggert GIOP-Request gegen jeden
  IOR in `target_object_keys` (one-way wenn `oneway=true` in IDL,
  sonst two-way mit Reply ignoriert).

### §4.5 Object-Key-Generation

```
object_key = SHA-256( repo_id + "\0" + dds_topic_pair_canonical )[..16]
```
- 16 Bytes = stabil + collision-resistant für ≤ 2^64 Mappings.
- `dds_topic_pair_canonical` = `request_topic + "->" + reply_topic` für RR
  bzw. `notify_topic` für Notify.

### §4.6 Fragment-Handling (GIOP 1.1+)

Messages > `fragment_size` werden in `Fragment`-Frames aufgeteilt
(MSB-Flag `more_fragments=1` im Header bis letztes Frame). Daemon
re-assembled inbound + fragmented outbound automatisch.

### §4.7 LocateRequest/Reply

Daemon antwortet `LocateReply` mit:
- `OBJECT_HERE` für bekannte Mappings.
- `UNKNOWN_OBJECT` sonst.
- `OBJECT_FORWARD` mit alternative-IOR wenn `target_supports`-Mismatch
  (z.B. SSLIOP-only nötig, Client benutzt plain-IIOP).

## §5 Topic-Mapping

Mapping erfolgt **per Operation**, nicht per Topic — denn CORBA ist
RPC-orientiert. Pro Mapping: ein Request-Topic + ein Reply-Topic
(oder ein Notify-Topic für one-way).

### §5.1 Slug-Algorithmus für DDS-Topics

Wenn nicht explizit gesetzt:
- `repo_id` = `IDL:zerodds/MarketData/Quote:1.0`
- `operation` = `request_quote`
- Default Request-Topic: `MarketData::Quote::request_quote::Request`
- Default Reply-Topic: `MarketData::Quote::request_quote::Reply`

Override per `request_topic.dds_name` / `reply_topic.dds_name`.

### §5.2 Type-Discovery

Daemon erzeugt zur Startup-Time IDL-zu-DDS-Type-Mapping per
`crates/idl-rust/`-Codegen. Pro Mapping:
- `request_topic.dds_type` = struct mit allen `in`/`inout`-Args + `request_id` Field
- `reply_topic.dds_type` = struct mit allen `out`/`inout`-Args + `request_id` + Result-Field
- Exceptions als Variant in Reply-Type (Tag-Field `_exception_kind`)

Auto-generierte IDL liegt in `/var/lib/zerodds/corba-bridge/<mapping>.idl`.

## §6 QoS-Translation

CORBA-GIOP ist Request-Reply über TCP — TCP-Garantien sind gegeben.
Topic-zu-Topic-Bridging im zerodds-rpc-Pattern.

| DDS-QoS | CORBA-Verhalten |
|---------|-----------------|
| Reliability `RELIABLE` | Standard (nur RELIABLE sinnvoll für RR) |
| Reliability `BEST_EFFORT` | nicht empfohlen für Request-Reply; akzeptiert für `direction=dds_to_corba` Notify |
| Durability `VOLATILE` | normaler Live-RPC |
| Durability `TRANSIENT_LOCAL` | Daemon hält letzten Reply pro Request-Hash für Replay |
| Lifespan | mappt auf Mapping-`timeout_ms` |
| Deadline | beobachtet bei `dds_to_corba` Notify |
| Liveliness | GIOP `MessageError` bei Connection-Loss; ORB-Reconnect |
| Partition | Filter im Daemon vor DDS-Publish |

## §7 Security

### §7.1 SSLIOP

`corba.ssliop.enabled: true` aktiviert TLS-over-IIOP (SSLIOP per OMG
SSLIOP-Spec). IOR enthält dann SSL-Component (Tag 0x06) mit
`target_supports`-Bitmap.

### §7.2 CSIv2 (Common Secure Interoperability v2)

`corba.csiv2.enabled: true` aktiviert:
- `SAS_ContextElement` im ServiceContext-List jedes Requests
- `EstablishTrustInClient` (Cert) + `EstablishTrustInTarget` (Cert)
- GSSUP-Username/Password als Fallback (`gssup.target_name`)

Subject-Resolution: aus TLS-Cert-Subject-DN oder GSSUP-Username.

### §7.3 ACL

Per Mapping. Subject = TLS-DN oder GSSUP-User.

## §8 Operations + Observability

### §8.1 Logging

JSON-Log auf stdout. Felder: `timestamp`, `level`, `event`, `peer`,
`giop_version`, `request_id`, `operation`, `repo_id`, `bytes`,
`reply_status`, `latency_us`.

### §8.2 Prometheus-Metrics

```
zerodds_corba_bridge_requests_total            counter{operation, status}
zerodds_corba_bridge_replies_sent_total        counter{operation}
zerodds_corba_bridge_pending_requests          gauge{operation}
zerodds_corba_bridge_request_latency_seconds   histogram{operation}
zerodds_corba_bridge_timeouts_total            counter{operation}
zerodds_corba_bridge_dds_samples_received      counter{dds_topic}
zerodds_corba_bridge_dds_samples_published     counter{dds_topic}
zerodds_corba_bridge_locate_requests_total     counter
zerodds_corba_bridge_csiv2_failures_total      counter{reason}
zerodds_corba_bridge_giop_message_errors_total counter
```

### §8.3 OTLP-Spans

`OTEL_EXPORTER_OTLP_ENDPOINT` → Spans pro GIOP-Exchange (`giop.request`
+ `dds.publish` + `dds.subscribe` + `giop.reply` als Children).

## §9 Lifecycle

### §9.1 Startup

1. Config-Parse + Validation.
2. TLS-Cert-Load wenn SSLIOP.
3. DCPS-DomainParticipant init.
4. Pro Mapping: Topic-Auto-Generation, Reader+Writer registrieren.
5. IIOP-Bind 6833 + SSLIOP-Bind 6834 (optional).
6. NameService-Resolve + Rebind aller IORs (wenn `publish_to_naming`).
7. SIGHUP/SIGTERM/SIGINT-Handler installieren.

### §9.2 Shutdown

SIGTERM/SIGINT → graceful drain (max 30s):
- Send `CloseConnection` an alle Peers.
- Wait für pending Replies (max `timeout_ms`).
- NameService-Unbind aller IORs.
- Cleanup DDS-Entities.
- Exit 0.

SIGHUP → Config-Reload (TLS-Cert + ACL hot-update; Mapping-Änderungen
brauchen Restart).

## §10 Cross-Vendor

Daemon ist normaler RTPS-Peer. CORBA-Seite getestet gegen:
- TAO (ACE+TAO, 8.x)
- JacORB
- omniORB
- Ice-Java (über CORBA-Compat-Mode)

Verifiziert in `crates/corba-dds-bridge/tests/cross_vendor.rs`.

## §11 Packaging

Per `zerodds-deployment-1.0` Spec:
- Binary: `zerodds-corba-bridged`
- Config-Default: `/etc/zerodds/corba-bridged.yaml`
- Systemd-Unit: `zerodds-corba-bridged.service`
- launchd-Plist: `org.zerodds.corba-bridged.plist`
- Win-Service: `ZeroDDSCORBABridge`
- Docker: `zerodds/corba-bridged:1.0`
- IOR-Backup: `/var/lib/zerodds/corba-bridge/*.ior`

Manual: `man 1 zerodds-corba-bridged` + `man 5 zerodds-corba-bridged.yaml`.

## §12 Testing

### §12.1 Unit-Tests

Pro Modul (`config`, `giop_codec`, `iiop_transport`, `ior`,
`object_key`, `csiv2`, `dds_pump`, `request_correlator`)
≥ 5 Tests in `crates/corba-giop/`, `crates/corba-iiop/`,
`crates/corba-dds-bridge/`.

### §12.2 Integration-Tests

`crates/corba-dds-bridge/tests/bridge_e2e.rs`:
- Spawn `zerodds-corba-bridged` als Subprocess.
- TAO-Client (Docker) ruft Bridge-IOR.
- DDS-Service-Process empfängt Request-Topic, sendet Reply-Topic.
- TAO-Client erhält GIOP-Reply.
- Verify byte-genauer CDR-Roundtrip.

### §12.3 Multi-Vendor

`tests/cross_vendor.rs`: Cyclone-DDS-Subscriber + TAO-Client + ZeroDDS-
CORBA-Bridge im Docker-Compose.

## §13 Cross-References

- Library: `crates/corba-dds-bridge/`, `crates/corba-iiop/`,
  `crates/corba-giop/`, `crates/idl-rust/` (IDL-Codegen).
- OMG-Specs: GIOP/IIOP (formal/2012-11-14), CSIv2 (formal/2008-01-01),
  SSLIOP (formal/2008-01-01).
- Wire-Format: `zerodds-xcdr2-bindings-conformance-1.0` §3 (CDR-Body).
- Deployment: `zerodds-deployment-1.0`.
- Verwandte Daemons: `zerodds-grpc-bridge-1.0` (RPC-äquivalent),
  `zerodds-amqp-bridge-daemon-1.0`.

## §14 Versioning

`1.0` initial. Patch für Bugfixes, Minor für additive Mapping-
Konfiguration, Major für Wire-Protocol-Changes.
