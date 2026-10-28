# `zerodds-corba-bridge` v1.0 — DDS↔CORBA bridge daemon

ZeroDDS vendor spec. Specifies the behavior of a configurable
daemon that couples DDS topics with CORBA IIOP servers (GIOP/IIOP, OMG
formal/2012-11-14).

## Motivation

CORBA is active in the financial sector, in telco switching stacks, in air traffic
control systems, and in industrial legacy software from the early
2000s until today. Migrating such legacy systems to modern
DDS-based real-time messaging is a multi-year program — until
then, bridges are needed. The `zerodds-corba-bridged` daemon is the
coexistence component: CORBA operations are mapped to DDS request/reply
topic pairs; DDS publications are emitted as CORBA notify
events when needed.

Complementary to the libraries `crates/corba-dds-bridge/`,
`crates/corba-iiop/` (TCP transport), `crates/corba-giop/` (frame
codec) — the libraries are building blocks, this daemon is the
executable product.

## §1 Conformance levels

| Level | Requirement |
|-------|-------------|
| **L1 — Wire** | The daemon speaks GIOP 1.0/1.1/1.2 (OMG formal/2012-11-14) including Request/Reply/CancelRequest/LocateRequest/Fragment + IIOP 1.0/1.1/1.2 profile. |
| **L2 — DDS** | The daemon is a valid DDS DomainParticipant (SPDP/SEDP, discovery, liveliness). |
| **L3 — Bridging** | Bidirectional: GIOP request→DDS request topic + DDS reply topic→GIOP reply (request-reply pattern); optionally notification-service-style pub/sub. |
| **L4 — Config** | Operation map via YAML config (RepoId↔DDS topic pairs); hot reload via SIGHUP optional. |
| **L5 — Auth** | TLS over IIOP (SSLIOP) + CSIv2 (`SAS_ContextElement`) + GSSUP username/password. |
| **L6 — Multi-Tenant** | Several DomainParticipants per daemon; one tenant binding per IIOP connection. |

L1-L4 are mandatory. L5-L6 are optional (mandatory for production).

## §2 CLI surface

```
zerodds-corba-bridged [OPTIONS]

Options:
  --config <FILE>              Base file (YAML subset of zerodds-config),
                               plus FILE.d/; without it: corba-bridged.yaml and corba-bridged.d/
                               in the system directory
  --set <PATH=VALUE>           one value above all files, repeatable
  --check-config               check the configuration, print every value with its
                               origin, exit (0 valid, 1 invalid)
  --iiop-bind <ADDR>          IIOP bind (default 0.0.0.0:6833)
  --ssliop-bind <ADDR>        SSLIOP bind (default 0.0.0.0:6834)
  --domain <ID>               DDS domain ID (default 0)
  --orb-id <ID>               ORB identity (default "zerodds-corba-bridge")
  --tls-cert <FILE>           TLS server cert (PEM); enables SSLIOP
  --tls-key <FILE>            TLS server key (PEM)
  --topic <REPO_ID:DDS>       single-mapping override (several times)
  --log-level <LEVEL>         trace/debug/info/warn/error (default info)
  --metrics <ADDR>            Prometheus scrape endpoint (default off)
  --version                   version info
  --help                      help

Exit codes:
  0   normal shutdown (SIGTERM/SIGINT)
  1   config error
  2   bind error (port in use)
  3   DDS discovery error
  4   TLS error
  5   NameService bind error
```

Precedence (lowest first): built-in defaults, environment, files,
flags. `--iiop-bind`, `--ssliop-bind`, `--domain`, `--orb-id`, `--tls-cert`/`--tls-key` (only together), `--topic <REPO_ID=TOPIC>`, `--log-level` and `--metrics` are short forms for `--set` of their file key
and therefore win over the files. The same file may carry the keys of the
DDS runtime (`transport.*`, `discovery.*`, …), see
`zerodds-deployment-1.0.md` §2.1.

## §3 Config file format

YAML subset of `zerodds-config` (block lists, flat inline lists of
scalars, `include:`, drop-ins, `${VAR}`). An unknown key in
a known section and a value of the wrong type prevent the start,
with file and line; an unknown key at the top level produces
a warning, and under `--check-config` an error.

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
    # naming_service: rejected, binding into a NameService is not built
    publish_to_naming: false        # true: rejected, not built
    ior_file: "/var/lib/zerodds/bridge.ior"   # backup file with the stringified IOR
  csiv2:
    enabled: false                  # true: rejected, not built

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
    correlation_id: "request_id"        # field name in the request for reply matching
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
    targets:                          # corbaloc: or IOR strings
      - "corbaloc:iiop:1.2@listener-main:2809/TradingListener"
      - "corbaloc:iiop:1.2@listener-audit:2809/TradingListener"

acl:
  default_deny: true
  rules:
    - subject: "mtls:<SHA-256 of the client certificate, hex>"
      allow_invoke: ["IDL:zerodds/MarketData/*"]

metrics:
  enabled: true
  listen: "127.0.0.1:9096"
  path: "/metrics"
```

ENV substitution: `${VAR}` and `${VAR:-default}`. Reader:
`crates/corba-dds-bridge/src/bridge_daemon.rs` (`DaemonConfig::from_config`).
Rejected with a reason instead of being silently ignored: `csiv2.enabled: true`,
`csiv2.target_supports`, `csiv2.gssup` (CSIv2 not built),
`ior.publish_to_naming: true` and `ior.naming_service` (binding into
a NameService is not built), `correlation_id` other than
`request_id`, `target_object_keys` (an object key names no host;
`targets` takes `corbaloc:` URLs or IORs), `qos.reliability: best_effort`
for request-reply.

## §4 GIOP/IIOP wire protocol

### §4.1 IOR generation

Per mapping the daemon creates an IOR with:
- `type_id`: `repo_id` from the config
- `profile_count`: 1 (IIOP) or 2 (IIOP + SSLIOP)
- `IIOP-Profile` (tag 0):
  - `version`: according to `corba.iiop.giop_version`
  - `host`: bind hostname
  - `port`: 6833
  - `object_key`: stable hash over `(repo_id + dds_topic_pair)` — see §4.5
  - Components (tag 0x20 = `TAG_INTERNET_IOP::TAG_ORB_TYPE`, tag 0x21
    = `TAG_INTERNET_IOP::TAG_CODE_SETS`, tag 0x06 = `TAG_SSL_SEC_TRANS`
    with SSLIOP)
- `SSL-Component` (tag 0x06): `target_supports`, `target_requires`,
  `port` (6834)

The IOR is:
- written to `corba.ior.ior_file`, one line per mapping:
  `<repo_id> <operation> IOR:…`; `--dump-iors` prints the same lines.
  When bound to `0.0.0.0`, the profile carries `corba.iiop.advertise_host`
  (environment `ZERODDS_CORBA_ADVERTISE_HOST`, default `127.0.0.1`).
- bound to the NameService with `corba.ior.publish_to_naming=true` — not
  built, the value is rejected.

Built: `TAG_CODE_SETS` (char native UTF-8, conversion ISO 8859-1; wchar
UTF-16) and with SSLIOP `TAG_SSL_SEC_TRANS` (port, `target_supports`
NoProtection/Integrity/Confidentiality/EstablishTrustInTarget, with a CA also
EstablishTrustInClient; `target_requires` EstablishTrustInClient with
`require_client_cert`). No `TAG_ORB_TYPE`: the value is an ORB identifier
assigned by the OMG, and ZeroDDS has none. omniORB 4.3.3 `catior` reads
both components (Linux test host).

### §4.2 GIOP frame format

Standard GIOP header (12 bytes):
```
+----+----+----+----+--------+--------+--------+------------+
|'G' |'I' |'O' |'P' | major  | minor  | flags  | msg_type   |
+----+----+----+----+--------+--------+--------+------------+
|                  message_size (4 bytes)                   |
+-----------------------------------------------------------+
```
- `flags.bit0` = byte order (0=BE, 1=LE)
- `flags.bit1` = fragment bit (GIOP 1.1+)
- `msg_type`: 0=Request, 1=Reply, 2=CancelRequest, 3=LocateRequest,
  4=LocateReply, 5=CloseConnection, 6=MessageError, 7=Fragment

The body follows with the CDR-encoded RequestHeader/ReplyHeader according to the GIOP
version, then the CDR-encoded operation args/results.

### §4.3 Request-reply mapping (CORBA → DDS)

Incoming GIOP request:
1. Parse RequestHeader → `(object_key, operation, request_id)`.
2. Lookup `(object_key, operation)` → mapping → DDS request topic.
3. Args are CDR-decoded and packed into a `<dds_type>` sample.
4. The daemon publishes on the request topic with source timestamp = now.
5. The daemon creates a pending request entry with `correlation_id`
   = `request_id` (or a custom field from the sample).
6. The DDS subscriber (backend service) processes the request, publishes
   the reply on the reply topic.
7. The daemon correlates the reply via `correlation_id`, sends the GIOP reply
   back with `reply_status=NO_EXCEPTION` + CDR-encoded body.

On timeout (`timeout_ms`): the daemon emits a GIOP reply with
`reply_status=SYSTEM_EXCEPTION`, `TIMEOUT (CORBA::TIMEOUT)`.

The DDS samples carry the GIOP bytes unchanged, in the byte order
of the GIOP message, as XCDR1 (whose alignment from the start of the body corresponds to that of
GIOP 1.2):

```idl
@final struct <Op>Request {
    unsigned long long request_id;   // the bridge's, unique per daemon
    // in and inout arguments, as GIOP encodes them
};
@final struct <Op>Reply {
    unsigned long long request_id;   // of the answered request
    long reply_status;               // 0 NO_EXCEPTION, 1 USER_EXCEPTION,
                                     // 2 SYSTEM_EXCEPTION (ReplyStatusType)
    unsigned long reserved;          // 0; keeps the rest 8-aligned
    // return value and out/inout, or the exception as GIOP encodes it
};
```

`correlation_id` is therefore always `request_id`, the first member. Because the
alignment only fits GIOP 1.2, a mapped operation over GIOP
1.0/1.1 is answered with `NO_IMPLEMENT`. Further answers: unknown
object key `OBJECT_NOT_EXIST`, wrong operation `BAD_OPERATION`, ACL
`NO_PERMISSION`, DDS write error `TRANSIENT`, an XCDR2 reply sample or an
unknown `reply_status` `MARSHAL`. `CancelRequest` removes the open
request.

### §4.4 Notify mapping (DDS → CORBA)

Per `direction=dds_to_corba` mapping:
- A DDS sample on `notify_topic` is sent as a oneway GIOP 1.2 request to every
  target in `targets` (`corbaloc:` or IOR); body = sample payload
  (XCDR1, byte order of the sample; an XCDR2 sample is discarded and
  counted). One connection per target, re-established after an error.

### §4.5 Object key generation

```
object_key = SHA-256( repo_id + "\0" + dds_topic_pair_canonical )[..16]
```
- 16 bytes = stable + collision-resistant for ≤ 2^64 mappings.
- `dds_topic_pair_canonical` = `request_topic + "->" + reply_topic` for RR
  or `notify_topic` for notify.

### §4.6 Fragment handling (GIOP 1.1+)

Messages > `fragment_size` are split into `Fragment` frames
(flag `more_fragments` in the header up to the last frame). Every fragment except
the last ends on an 8-octet boundary of the message, header
included: the first message carries 12 header octets, its data is 4
shorter than a multiple of 8 (omniORB 4.3 fragments the same way, measured;
a reply cut differently made omniORB hang). Inbound,
fragments are reassembled before decoding.

### §4.7 LocateRequest/Reply

The daemon answers `LocateReply` with:
- `OBJECT_HERE` for known mappings.
- `UNKNOWN_OBJECT` otherwise.
- `OBJECT_FORWARD` with an alternative IOR on a `target_supports` mismatch
  (e.g. SSLIOP-only needed, client uses plain IIOP) — not built.

## §5 Topic mapping

Mapping is done **per operation**, not per topic — because CORBA is
RPC-oriented. Per mapping: one request topic + one reply topic
(or one notify topic for one-way).

### §5.1 Slug algorithm for DDS topics

If not set explicitly:
- `repo_id` = `IDL:zerodds/MarketData/Quote:1.0`
- `operation` = `request_quote`
- Default request topic: `MarketData::Quote::request_quote::Request`
- Default reply topic: `MarketData::Quote::request_quote::Reply`

Override via `request_topic.dds_name` / `reply_topic.dds_name`.

### §5.2 Type discovery

At startup time the daemon creates an IDL-to-DDS type mapping via
`crates/idl-rust/` codegen. Per mapping:
- `request_topic.dds_type` = struct with all `in`/`inout` args + `request_id` field
- `reply_topic.dds_type` = struct with all `out`/`inout` args + `request_id` + result field
- Exceptions as a variant in the reply type (tag field `_exception_kind`)

Auto-generated IDL is in `/var/lib/zerodds/corba-bridge/<mapping>.idl`.

What is built is the fixed layout from §4.3 instead of the generation: the bridge
does not know the IDL type and passes the argument bytes through; the
DDS backends declare request and reply type according to this layout.
IDL generation and the exception variant are open.

## §6 QoS translation

CORBA GIOP is request-reply over TCP — TCP guarantees are given.
Topic-to-topic bridging in the zerodds-rpc pattern.

| DDS QoS | CORBA behavior |
|---------|-----------------|
| Reliability `RELIABLE` | standard (only RELIABLE makes sense for RR) |
| Reliability `BEST_EFFORT` | not recommended for request-reply; accepted for `direction=dds_to_corba` notify |
| Durability `VOLATILE` | normal live RPC |
| Durability `TRANSIENT_LOCAL` | open: no replay store per request hash |
| Lifespan | open; `timeout_ms` is a separate setting |
| Deadline | open |
| Liveliness | GIOP `MessageError` on connection loss; ORB reconnect |
| Partition | open |

## §7 Security

### §7.1 SSLIOP

`corba.ssliop.enabled: true` enables TLS over IIOP (SSLIOP per the OMG
SSLIOP spec) on `ssliop.bind`, with `require_client_cert` against `ca_file`;
the IOR carries `TAG_SSL_SEC_TRANS` (§4.1). Measured against omniORB 4.3.3 with
`libomnisslTP4` (`tests/omniorb_e2e.rs`): a client that only allows SSL
finds the port via the IOR and calls with a client certificate.

### §7.2 CSIv2 (Common Secure Interoperability v2)

`corba.csiv2.enabled: true` is rejected: CSIv2 is not built in the daemon.
Described is what it is supposed to enable:
- `SAS_ContextElement` in the ServiceContext list of every request
- `EstablishTrustInClient` (cert) + `EstablishTrustInTarget` (cert)
- GSSUP username/password as fallback (`gssup.target_name`)

Subject resolution: from the TLS cert subject DN or the GSSUP username.

### §7.3 ACL

Subject = subject DN of the client certificate per RFC 4514 (`CN=trader,O=…`);
a rule may instead name the fingerprint `mtls:<SHA-256, hex>`.
With plain IIOP there is no subject. A rule with a matching subject (or
`*`) must list the `repo_id` in `allow_invoke` (prefix with `*`); without
a matching rule, `default_deny` decides. Refused: `NO_PERMISSION`.

## §8 Operations + observability

### §8.1 Logging

JSON log on stderr, one line per event, filtered by `log_level`.
Fields: `timestamp`, `level`, `event`, `peer`, `giop_version`, `request_id`,
`operation`, `repo_id`, `bytes`, `reply_status`, `latency_us`.

Built are `level`, `event` and the fields that the event knows;
`timestamp` is missing (open).

### §8.2 Prometheus metrics

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

Built are the families above except `csiv2_failures_total` (CSIv2 is not
built), plus the generic ones of all bridges (`zerodds_corba_frames_*`,
`_bytes_*`, `_connections_*`, `_dds_samples_*`, `_errors_total`). `status`
is `no_exception`, `user_exception` or `system_exception` (TIMEOUT and
rejections are counted there as well). The label values come from the configuration,
not from the wire; a request to an unknown object is counted under no
operation.

### §8.3 OTLP spans

`OTEL_EXPORTER_OTLP_ENDPOINT` → spans per GIOP exchange (`giop.request`
+ `dds.publish` + `dds.subscribe` + `giop.reply` as children).

Built is the exporter from the environment; the daemon does not create any
spans yet (open).

## §9 Lifecycle

### §9.1 Startup

1. Config parse + validation.
2. TLS cert load if SSLIOP.
3. DCPS DomainParticipant init.
4. Per mapping: topic auto-generation, register reader+writer.
5. IIOP bind 6833 + SSLIOP bind 6834 (optional).
6. NameService resolve + rebind of all IORs (if `publish_to_naming`).
7. Install SIGHUP/SIGTERM/SIGINT handlers.

### §9.2 Shutdown

SIGTERM/SIGINT → graceful drain (max 30s):
- No new connections; a request during the drain is answered with
  `TRANSIENT` (completed NO).
- Open requests get their reply or their `TIMEOUT` (`timeout_ms`);
  whatever is still open after 30 s gets `TRANSIENT` (completed MAYBE).
- Then `CloseConnection` to every connection, in its GIOP version, and
  the connection is closed (GIOP §15.4.7: after `CloseConnection`
  no reply follows, therefore only after the replies).
- NameService unbind of all IORs — does not apply as long as `publish_to_naming`
  is not built.
- Exit 0.

SIGHUP → config reload (TLS cert + ACL hot update; mapping changes
need a restart). Built: the daemon re-reads the same layers (files,
`--set`, flags); ACL and SSLIOP material apply from then on, the material for
connections that are accepted afterwards. Changed mappings, binds,
`ssliop.enabled` or `domain` are reported as `reload_needs_restart`. A
configuration that does not load, or material that does not load, changes nothing
(`reload_refused` with a reason).

## §10 Cross-vendor

The daemon is a normal RTPS peer. CORBA side tested against:
- TAO (ACE+TAO, 8.x)
- JacORB
- omniORB
- Ice-Java (via CORBA compat mode)

Measured against omniORB 4.3.3 (Linux test host) in
`crates/corba-dds-bridge/tests/omniorb_e2e.rs`: an omniORB client calls a
DDS backend via the IOR of the bridge (`add` → 42), `TIMEOUT` without a backend,
`BAD_OPERATION`, an `echo` of 200,000 bytes fragmented in both
directions; three DDS samples reach an omniORB servant as oneway.
`catior` reads the IOR (`tests/cross_vendor.rs`). SSLIOP with client certificate and ACL:
the allowed certificate passes, another one from the same CA gets
`NO_PERMISSION`. TAO, JacORB, Ice: not measured.

## §11 Packaging

Per the `zerodds-deployment-1.0` spec:
- Binary: `zerodds-corba-bridged`
- Config default: `/etc/zerodds/corba-bridged.yaml`
- Systemd unit: `zerodds-corba-bridged.service`
- launchd plist: `org.zerodds.corba-bridged.plist`
- Win service: `ZeroDDSCORBABridge`
- Docker: `zerodds/corba-bridged:1.0`
- IOR backup: `/var/lib/zerodds/corba-bridge/*.ior`

Manual: `man 1 zerodds-corba-bridged` + `man 5 zerodds-corba-bridged.yaml`.

## §12 Testing

### §12.1 Unit tests

Per module (`config`, `giop_codec`, `iiop_transport`, `ior`,
`object_key`, `csiv2`, `dds_pump`, `request_correlator`)
≥ 5 tests in `crates/corba-giop/`, `crates/corba-iiop/`,
`crates/corba-dds-bridge/`.

### §12.2 Integration tests

- `crates/corba-dds-bridge/tests/omniorb_e2e.rs` (omniORB instead of TAO; the
  cases from §10).
- `crates/corba-dds-bridge/tests/bridge_e2e.rs`: the binary, unknown
  object → `OBJECT_NOT_EXIST`, `--dump-iors`.

### §12.3 Multi-vendor

Open: Cyclone DDS backend behind the bridge, TAO/JacORB.

## §13 Cross-references

- Library: `crates/corba-dds-bridge/`, `crates/corba-iiop/`,
  `crates/corba-giop/`, `crates/idl-rust/` (IDL codegen).
- OMG specs: GIOP/IIOP (formal/2012-11-14), CSIv2 (formal/2008-01-01),
  SSLIOP (formal/2008-01-01).
- Wire format: `zerodds-xcdr2-bindings-conformance-1.0` §3 (CDR body).
- Deployment: `zerodds-deployment-1.0`.
- Related daemons: `zerodds-grpc-bridge-1.0` (RPC equivalent),
  `zerodds-amqp-bridge-daemon-1.0`.

## §14 Versioning

`1.0` initial. Patch for bug fixes, minor for additive mapping
configuration, major for wire protocol changes.
